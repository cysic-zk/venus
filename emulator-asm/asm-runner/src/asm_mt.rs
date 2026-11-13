use std::borrow::Cow;
use std::fmt::Debug;
use zisk_common::EmuTrace;
use zisk_common::EmuTraceStart;
use zisk_core::{REGS_IN_MAIN_FROM, REGS_IN_MAIN_TO, REGS_IN_MAIN_TOTAL_NUMBER};

use crate::AsmShmemHeader;

#[repr(C)]
#[derive(Debug)]
pub(crate) struct AsmMTHeader {
    pub version: u64,
    pub exit_code: u64,
    pub shmem_allocated_size: u64,
    pub shmem_used_size: u64,
    pub num_chunks: u64,
}

impl AsmShmemHeader for AsmMTHeader {
    fn allocated_size(&self) -> u64 {
        self.shmem_allocated_size
    }
}

/// Maximum size in bytes of a chunk's metadata: the `AsmMTChunk` header, 3 words of slack and 32
/// bytes. Must match `MAX_TRACE_CHUNK_INFO` in emulator-asm/src/constants.hpp.
#[cfg_attr(not(all(target_os = "linux", target_arch = "x86_64")), allow(dead_code))]
pub(crate) const MAX_TRACE_CHUNK_INFO: usize = std::mem::size_of::<AsmMTChunk>() + 3 * 8 + 32;

/// Chunk header written by the generated asm (zisk_rom_2_asm.rs) and by emulator-asm; the layout
/// must match `MT_CHUNK_*` in emulator-asm/src/constants.hpp.
#[repr(C)]
#[derive(Debug)]
pub(crate) struct AsmMTChunk {
    pub pc: u64,
    pub sp: u64,
    pub c: u64,
    pub step: u64,
    /// r1..=REGS_IN_MAIN_TO
    pub registers: [u64; REGS_IN_MAIN_TO],
    pub last_c: u64,
    pub end: u64,
    pub steps: u64,
    pub mem_reads_size: u64,
}

// SAFETY: `AsmMTChunk` is a `repr(C)` struct of `u64`s.
unsafe impl crate::AsmShmemChunk for AsmMTChunk {
    fn payload_words(&self) -> u64 {
        self.mem_reads_size
    }
}

impl AsmMTChunk {
    /// The `EmuTrace` of a chunk header read by [`AsmMultiShmem::read_chunk`] and its memory
    /// reads, which it borrows from the shared memory without copying.
    ///
    /// # Safety
    /// `mem_reads` must stay mapped for as long as the returned trace is used: its lifetime is
    /// extended to `'static`.
    ///
    /// [`AsmMultiShmem::read_chunk`]: crate::AsmMultiShmem::read_chunk
    pub unsafe fn to_emu_trace(chunk: AsmMTChunk, mem_reads: &[u64]) -> EmuTrace {
        // Zero-copy: borrow mem_reads directly from shared memory.
        // SAFETY: the caller keeps the shared memory mapped while the trace is in use.
        let mem_reads: Cow<'static, [u64]> = unsafe {
            Cow::Borrowed(&*std::ptr::slice_from_raw_parts(mem_reads.as_ptr(), mem_reads.len()))
        };

        let mut registers = [0u64; REGS_IN_MAIN_TOTAL_NUMBER];
        registers[REGS_IN_MAIN_FROM..].copy_from_slice(&chunk.registers[..REGS_IN_MAIN_TO]);

        // Return the parsed OutputChunk
        EmuTrace {
            start_state: EmuTraceStart {
                pc: chunk.pc,
                sp: chunk.sp,
                c: chunk.c,
                step: chunk.step,
                regs: registers,
            },
            last_c: chunk.last_c,
            end: chunk.end == 1,
            steps: chunk.steps,
            mem_reads,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The value of `#define NAME <integer>` in emulator-asm/src/constants.hpp.
    fn c_define(name: &str) -> usize {
        let header = include_str!("../../src/constants.hpp");
        let prefix = format!("#define {name} ");
        let line = header
            .lines()
            .find(|l| l.starts_with(&prefix))
            .unwrap_or_else(|| panic!("constants.hpp has no `{prefix}...`"));
        line[prefix.len()..]
            .trim()
            .parse()
            .unwrap_or_else(|_| panic!("constants.hpp: `{line}` is not an integer define"))
    }

    /// The value of `.equ NAME, <value>` in emulator-asm/src/dma/dma_constants.inc.
    fn asm_equ(name: &str) -> String {
        let inc = include_str!("../../src/dma/dma_constants.inc");
        let prefix = format!(".equ {name}, ");
        let line = inc
            .lines()
            .find(|l| l.starts_with(&prefix))
            .unwrap_or_else(|| panic!("dma_constants.inc has no `{prefix}...`"));
        line[prefix.len()..].trim().to_string()
    }

    /// The C runtime and the DMA assembly each keep their own copy of the register count in
    /// the chunk header; both must match the main-trace registers, which the generated asm
    /// and `AsmMTChunk` follow.
    #[test]
    fn chunk_layout_matches_the_c_runtime() {
        assert_eq!(
            c_define("MT_CHUNK_REGS"),
            REGS_IN_MAIN_TO,
            "set MT_CHUNK_REGS in emulator-asm/src/constants.hpp to REGS_IN_MAIN_TO"
        );
        assert_eq!(
            asm_equ("MT_CHUNK_REGS").parse::<usize>().ok(),
            Some(REGS_IN_MAIN_TO),
            "set MT_CHUNK_REGS in emulator-asm/src/dma/dma_constants.inc to REGS_IN_MAIN_TO"
        );
        // The assembly derives the chunk allowance from MT_CHUNK_REGS with the same formula.
        assert_eq!(
            asm_equ("MAX_TRACE_CHUNK_INFO"),
            "(((4 + MT_CHUNK_REGS + 4 + 3) * 8) + 32)",
            "dma_constants.inc must derive MAX_TRACE_CHUNK_INFO from MT_CHUNK_REGS"
        );
        // pc, sp, c, step, registers, last_c, end, steps, mem_reads_size
        assert_eq!(std::mem::size_of::<AsmMTChunk>(), (4 + REGS_IN_MAIN_TO + 4) * 8);
        assert_eq!(MAX_TRACE_CHUNK_INFO, (4 + REGS_IN_MAIN_TO + 4 + 3) * 8 + 32);
    }
}
