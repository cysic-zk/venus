use crate::shmem_sys;
use crate::AsmShmemHeader;
use libc::{close, mmap, munmap, MAP_FAILED, MAP_SHARED, PROT_READ, PROT_WRITE};

use std::{
    io,
    os::raw::c_void,
    ptr,
    sync::atomic::{fence, Ordering},
};
use tracing::debug;

use anyhow::anyhow;
use anyhow::Result;

struct MappedFile {
    fd: i32,
}

/// A shared memory manager that supports multiple contiguous shared memory files.
///
/// This struct reserves a large virtual address range upfront and maps multiple
/// shared memory files (`_0`, `_1`, etc.) into contiguous portions of that range.
///
/// File layout:
/// - `{base_name}_0`: Initial file with size `initial_size`, contains the header
/// - `{base_name}_1`, `_2`, ...: Incremental files with size `incremental_size`
pub(crate) struct AsmMultiShmem<H: AsmShmemHeader> {
    base_name: String,
    reserved_ptr: *mut c_void,
    reserved_size: usize,
    initial_size: usize,
    incremental_size: usize,
    mapped_files: Vec<MappedFile>,
    total_mapped_size: usize,
    unlock_mapped_memory: bool,
    read_write: bool,
    _phantom: std::marker::PhantomData<H>,
}

// SAFETY: the non-auto fields are `reserved_ptr` (a raw pointer into a reserved
// mmap region) and the mapped-file descriptors. The reserved address range is
// fixed for the handle's lifetime and growth only maps into it without moving
// existing mappings, so sending/sharing the handle across threads is sound.
unsafe impl<H: AsmShmemHeader> Send for AsmMultiShmem<H> {}
unsafe impl<H: AsmShmemHeader> Sync for AsmMultiShmem<H> {}

impl<H: AsmShmemHeader> Drop for AsmMultiShmem<H> {
    /// Releases this handle's mappings and descriptors — and deliberately does
    /// *not* `shm_unlink` the segments.
    ///
    /// The segments are named per `(pid, local_rank, hints)`, not per program,
    /// so every program on this worker maps the same ones. Unlinking here would
    /// destroy a segment another program's live handle is still using, and the
    /// name would then be silently re-created by the next setup — leaving the
    /// old inode resident and pinned with nothing able to reach it. Unlinking the
    /// names belongs to the last owner of their `SharedSegments`, and to
    /// `cleanup_stale_shmem()` for a process that died holding one.
    fn drop(&mut self) {
        for file in &self.mapped_files {
            unsafe { close(file.fd) };
        }

        // Unmap the entire reserved region (this handles all the MAP_FIXED mappings too)
        if !self.reserved_ptr.is_null() && self.reserved_size > 0 {
            unsafe {
                if munmap(self.reserved_ptr, self.reserved_size) != 0 {
                    tracing::error!(
                        "munmap failed for multi-shmem '{}': {:?}",
                        self.base_name,
                        io::Error::last_os_error()
                    );
                }
            }
        }
    }
}

impl<H: AsmShmemHeader> AsmMultiShmem<H> {
    /// Opens and maps the initial shared memory file, reserving address space for growth.
    ///
    /// # Arguments
    /// * `base_name` - Base name for shared memory files (files will be `{base_name}_0`, `_1`, etc.)
    /// * `initial_size` - Size of the first file (`_0`)
    /// * `incremental_size` - Size of subsequent files (`_1`, `_2`, ...)
    /// * `max_size` - Total virtual address space to reserve
    /// * `unlock_mapped_memory` - If true, don't use MAP_LOCKED
    /// * `read_write` - If true, open `O_RDWR` and map `PROT_READ|PROT_WRITE`
    ///   (needed only so the region can be registered as default-pinned for GPU
    ///   H2D; the consumer itself never writes).
    pub fn open_and_map(
        base_name: &str,
        initial_size: usize,
        incremental_size: usize,
        max_size: usize,
        unlock_mapped_memory: bool,
        read_write: bool,
    ) -> Result<Self> {
        if base_name.is_empty() {
            return Err(anyhow!("Shared memory base name cannot be empty"));
        }

        if max_size < initial_size {
            return Err(anyhow!(
                "max_size ({}) must be >= initial_size ({})",
                max_size,
                initial_size
            ));
        }

        if incremental_size == 0 {
            return Err(anyhow!("incremental_size must be > 0"));
        }

        // Reserve the entire address range with an anonymous mapping
        // MAP_NORESERVE prevents reserving swap space for the entire range
        let reserved_ptr = unsafe {
            mmap(
                ptr::null_mut(),
                max_size,
                libc::PROT_NONE,
                libc::MAP_PRIVATE | libc::MAP_ANONYMOUS | libc::MAP_NORESERVE,
                -1,
                0,
            )
        };

        if reserved_ptr == MAP_FAILED {
            let err = io::Error::last_os_error();
            return Err(anyhow!(
                "Failed to reserve {} bytes of address space for '{}': {}",
                max_size,
                base_name,
                err
            ));
        }

        debug!("Reserved {} bytes at {:p} for multi-shmem '{}'", max_size, reserved_ptr, base_name);

        let mut this = Self {
            base_name: base_name.to_string(),
            reserved_ptr,
            reserved_size: max_size,
            initial_size,
            incremental_size,
            mapped_files: Vec::with_capacity(8),
            total_mapped_size: 0,
            unlock_mapped_memory,
            read_write,
            _phantom: std::marker::PhantomData,
        };

        // Map the initial file (_0)
        if let Err(e) = this.map_file(0) {
            unsafe { munmap(reserved_ptr, max_size) };
            return Err(e);
        }

        this.total_mapped_size = initial_size;

        Ok(this)
    }

    /// Checks if the producer has allocated more space and maps any new files.
    ///
    /// This reads `allocated_size` from the header (always in file `_0`) and maps
    /// any new files that have been created by the producer.
    ///
    /// This does NOT move existing mappings, so pointers and slices to already-mapped data remain valid.
    pub fn check_size_changed(&mut self) -> Result<bool> {
        let allocated_size = self.map_header().allocated_size() as usize;

        if allocated_size <= self.total_mapped_size {
            return Ok(false);
        }

        // Calculate how many files should exist
        let files_needed = if allocated_size <= self.initial_size {
            1
        } else {
            1 + (allocated_size - self.initial_size).div_ceil(self.incremental_size)
        };

        let current_files = self.mapped_files.len();

        if files_needed <= current_files {
            // Size increased but within current file - just update total
            self.total_mapped_size = allocated_size;
            return Ok(true);
        }

        debug!(
            "Multi-shmem '{}': allocated_size={}, need {} files, have {}",
            self.base_name, allocated_size, files_needed, current_files
        );

        // Map all new files
        for file_idx in current_files..files_needed {
            self.map_file(file_idx)?;
        }

        self.total_mapped_size = allocated_size;

        fence(Ordering::Acquire);

        Ok(true)
    }

    /// Maps a specific file index into the reserved address space.
    fn map_file(&mut self, file_idx: usize) -> Result<()> {
        let file_name = format!("{}_{}", self.base_name, file_idx);

        let open_flags = if self.read_write { libc::O_RDWR } else { libc::O_RDONLY };
        let fd = shmem_sys::open(&file_name, open_flags)?;

        unsafe {
            // For _0, validate that the header has a non-zero allocated size
            if file_idx == 0 {
                let temp_map = mmap(ptr::null_mut(), size_of::<H>(), PROT_READ, MAP_SHARED, fd, 0);
                if temp_map == MAP_FAILED {
                    let err = io::Error::last_os_error();
                    close(fd);
                    return Err(anyhow!("mmap failed for header of '{}': {}", file_name, err));
                }

                let header = (temp_map as *const H).read();
                let allocated_size = header.allocated_size();
                munmap(temp_map, size_of::<H>());

                if allocated_size == 0 {
                    close(fd);
                    return Err(anyhow!("Shared memory '{}' has zero allocated size", file_name));
                }
            }

            // Calculate the offset where this file should be mapped
            let offset = if file_idx == 0 {
                0
            } else {
                self.initial_size + (file_idx - 1) * self.incremental_size
            };

            let file_size = if file_idx == 0 { self.initial_size } else { self.incremental_size };

            // The producer decides how many files get mapped: a MAP_FIXED mapping past the
            // reservation would replace unrelated mappings of this process, and pages past the
            // end of the backing object raise SIGBUS when touched.
            if offset.checked_add(file_size).map_or(true, |end| end > self.reserved_size) {
                close(fd);
                return Err(anyhow!(
                    "Shared memory '{}' does not fit in the {} bytes reserved for '{}'",
                    file_name,
                    self.reserved_size,
                    self.base_name
                ));
            }
            match shmem_sys::object_size(fd) {
                Ok(object_size) if object_size >= file_size => {}
                Ok(object_size) => {
                    close(fd);
                    return Err(anyhow!(
                        "Shared memory '{}' is smaller than its mapping ({} < {} bytes)",
                        file_name,
                        object_size,
                        file_size
                    ));
                }
                Err(err) => {
                    close(fd);
                    return Err(anyhow!("fstat('{}') failed: {}", file_name, err));
                }
            }

            let target_addr = self.reserved_ptr.add(offset);

            let mut flags = MAP_SHARED | libc::MAP_FIXED;
            if !self.unlock_mapped_memory {
                flags |= libc::MAP_LOCKED;
            }

            let prot = if self.read_write { PROT_READ | PROT_WRITE } else { PROT_READ };
            let mapped_ptr = mmap(target_addr, file_size, prot, flags, fd, 0);
            if mapped_ptr == MAP_FAILED {
                let err = io::Error::last_os_error();
                close(fd);
                return Err(anyhow!(
                    "mmap(MAP_FIXED) failed for '{}': {} ({} bytes at {:p})",
                    file_name,
                    err,
                    file_size,
                    target_addr
                ));
            }

            debug!(
                "Mapped '{}' ({} bytes) at {:p} (offset {})",
                file_name, file_size, mapped_ptr, offset
            );

            self.mapped_files.push(MappedFile { fd });
        }

        Ok(())
    }

    /// Reads the header from the shared memory (always from file `_0`).
    ///
    /// # Panics
    /// Panics if no files are mapped yet. This is an invariant guard, not an I/O
    /// path: with no mapped files there is no readable header and the read would
    /// be UB. `open_and_map` always maps `_0` and nothing ever unmaps it for the
    /// handle's lifetime, so on a live handle this never fires.
    pub fn map_header(&self) -> H {
        if self.mapped_files.is_empty() {
            panic!("Multi-shmem '{}' has no mapped files, cannot read header", self.base_name);
        }

        unsafe { (self.reserved_ptr as *const H).read() }
    }

    /// Returns the base pointer of the mapped region. Only the GPU memory planner registers it.
    #[cfg(any(gpu, test))]
    pub fn mapped_ptr(&self) -> *mut c_void {
        self.reserved_ptr
    }

    /// Returns a pointer to the data area (after the header).
    pub fn data_ptr(&self) -> *mut c_void {
        unsafe { self.reserved_ptr.add(size_of::<H>()) }
    }

    /// Returns the total currently mapped size. Only the GPU memory planner registers it.
    #[cfg(any(gpu, test))]
    pub fn total_mapped_size(&self) -> usize {
        self.total_mapped_size
    }

    /// Address from which a chunk may run past the mapped extent, `margin` bytes before its end:
    /// a reader at or past it checks for new files before reading the next chunk.
    pub fn growth_threshold(&self, margin: usize) -> usize {
        self.reserved_ptr as usize + self.total_mapped_size.saturating_sub(margin)
    }

    /// Reads the chunk header at `*cursor` and the `u64` words it says follow it, then advances
    /// `*cursor` past both.
    ///
    /// The sizes come from the producer: this fails, instead of reading past it, unless header
    /// and payload lie within the extent the producer has advertised and this reader has mapped.
    pub fn read_chunk<T: AsmShmemChunk>(&self, cursor: &mut *const T) -> Result<(T, &[u64])> {
        let base = self.reserved_ptr as usize;
        let end = base + self.total_mapped_size;
        let header_start = *cursor as usize;
        if header_start % align_of::<T>() != 0 {
            return Err(anyhow!("Misaligned chunk header in multi-shmem '{}'", self.base_name));
        }
        let payload_start = header_start
            .checked_add(size_of::<T>())
            .filter(|&header_end| header_start >= base && header_end <= end)
            .ok_or_else(|| anyhow!("Chunk header exceeds multi-shmem '{}'", self.base_name))?;
        // SAFETY: the header lies inside the mapped region, aligned (both checked above), and
        // `AsmShmemChunk` types are valid for any bytes.
        let chunk = unsafe { (header_start as *const T).read() };

        let words = chunk.payload_words();
        let len = usize::try_from(words)
            .ok()
            .filter(|&len| len <= (end - payload_start) / size_of::<u64>())
            .ok_or_else(|| {
                anyhow!("Chunk payload of {} words exceeds multi-shmem '{}'", words, self.base_name)
            })?;
        // SAFETY: the payload lies inside the mapped region checked above (8-aligned, as the
        // header is a whole number of words), which stays mapped while `self` is borrowed.
        let payload = unsafe { std::slice::from_raw_parts(payload_start as *const u64, len) };

        *cursor = payload.as_ptr_range().end as *const T;
        Ok((chunk, payload))
    }
}

/// A chunk header the producer writes into an [`AsmMultiShmem`], followed by `u64` words.
///
/// # Safety
/// Implementors are `repr(C)` structs of `u64`s (a whole number of words, 8-aligned): any bytes are
/// a valid value, so a header can be read from memory another process writes.
pub(crate) unsafe trait AsmShmemChunk {
    /// Number of `u64` words that follow the header.
    fn payload_words(&self) -> u64;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ShmemWriter;
    use std::ffi::CString;

    #[repr(C)]
    #[derive(Debug)]
    struct TestHeader {
        allocated_size: u64,
        _pad: u64,
    }
    impl AsmShmemHeader for TestHeader {
        fn allocated_size(&self) -> u64 {
            self.allocated_size
        }
    }

    fn create_segment(name: &str, size: usize) {
        let c = CString::new(name).unwrap();
        unsafe {
            libc::shm_unlink(c.as_ptr());
            let fd = libc::shm_open(c.as_ptr(), libc::O_CREAT | libc::O_RDWR, 0o600);
            assert!(fd >= 0);
            assert_eq!(libc::ftruncate(fd, size as libc::off_t), 0);
            libc::close(fd);
        }
    }

    fn unlink_segment(name: &str) {
        let c = CString::new(name).unwrap();
        unsafe { libc::shm_unlink(c.as_ptr()) };
    }

    #[test]
    fn open_and_map_maps_file_zero_and_reads_header() {
        let base = format!("ZISK_unittest_multi_{}", std::process::id());
        let initial = 4096usize;
        let file0 = format!("{base}_0");
        create_segment(&file0, initial);
        // Header's allocated_size at offset 0 must be non-zero.
        {
            let w = ShmemWriter::new(&file0, initial, true).unwrap();
            w.write_u64_at(0, initial as u64).unwrap();
        }

        let m = AsmMultiShmem::<TestHeader>::open_and_map(
            &base,
            initial,
            initial,
            initial * 4,
            true,
            false,
        )
        .unwrap();
        assert_eq!(m.map_header().allocated_size(), initial as u64);
        assert_eq!(m.total_mapped_size(), initial);
        assert!(!m.mapped_ptr().is_null());
        assert!(!m.data_ptr().is_null());
        drop(m); // Frees the reserved range but leaves the segment linked: segments are shared.
        unlink_segment(&file0);
    }

    #[repr(C)]
    struct TestChunk {
        payload: u64,
    }
    // SAFETY: a `repr(C)` struct of one `u64`.
    unsafe impl AsmShmemChunk for TestChunk {
        fn payload_words(&self) -> u64 {
            self.payload
        }
    }

    /// The producer's sizes are not trusted: a chunk whose payload runs past the mapped extent is
    /// an error, not a read past the mapping.
    #[test]
    fn read_chunk_stays_inside_the_mapping() {
        let base = format!("ZISK_unittest_multichunk_{}", std::process::id());
        let initial = 4096usize;
        let file0 = format!("{base}_0");
        create_segment(&file0, initial);
        let header = size_of::<TestHeader>();
        {
            let w = ShmemWriter::new(&file0, initial, true).unwrap();
            w.write_u64_at(0, initial as u64).unwrap();
            // A chunk with 2 words, then one claiming the rest of the mapping plus one word.
            w.write_u64_at(header, 2).unwrap();
            let next = header + 3 * 8;
            w.write_u64_at(next, ((initial - next) / 8) as u64).unwrap();
        }
        let m = AsmMultiShmem::<TestHeader>::open_and_map(
            &base, initial, initial, initial, true, false,
        )
        .unwrap();
        let mut cursor = m.data_ptr() as *const TestChunk;
        let (chunk, payload) = m.read_chunk(&mut cursor).unwrap();
        assert_eq!((chunk.payload, payload.len()), (2, 2));
        assert!(m.read_chunk(&mut cursor).is_err());
        drop(m);
        unlink_segment(&file0);
    }

    #[test]
    fn open_and_map_rejects_empty_base_name() {
        assert!(
            AsmMultiShmem::<TestHeader>::open_and_map("", 4096, 4096, 8192, true, false).is_err()
        );
    }

    #[test]
    fn open_and_map_rejects_max_smaller_than_initial() {
        let base = format!("ZISK_unittest_multibad_{}", std::process::id());
        assert!(AsmMultiShmem::<TestHeader>::open_and_map(&base, 8192, 4096, 4096, true, false)
            .is_err());
    }
}
