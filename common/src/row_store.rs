//! Streaming stores for trace rows written once into a recycled trace buffer.

use rayon::prelude::*;

/// Rows one padding task writes: enough to amortize the task and its fence.
const PAD_ROWS_PER_TASK: usize = 1 << 12;

/// Stores a finished row into a (cache-cold, recycled) trace buffer with non-temporal stores.
///
/// A plain copy first reads every destination line for ownership and waits for it; a row that is
/// written once and not read back by this thread does not need that. Rows are written in order, so
/// the write-combining buffers fill whole lines across row boundaries; a run of rows ends with
/// [`store_rows_fence`]. Falls back to a plain copy off x86-64 or for a row that is not a whole
/// number of 8-byte words at an 8-byte aligned address.
#[inline(always)]
pub fn store_row_streaming<R: Copy>(dst: &mut R, src: &R) {
    #[cfg(target_arch = "x86_64")]
    {
        use std::arch::x86_64::_mm_stream_si64;
        let size = std::mem::size_of::<R>();
        let d = dst as *mut R as *mut i64;
        if size % 8 == 0 && (d as usize) % 8 == 0 {
            let s = src as *const R as *const i64;
            // SAFETY: `d` and `s` cover `size` bytes of two distinct `R`s (an exclusive and a shared
            // borrow), `d` is 8-byte aligned (checked), `s` is read unaligned, and SSE2 is part of
            // the x86-64 baseline.
            unsafe {
                for i in 0..size / 8 {
                    _mm_stream_si64(d.add(i), s.add(i).read_unaligned());
                }
            }
            return;
        }
    }
    *dst = *src;
}

/// Orders the streaming row stores before anything this thread publishes after them.
#[inline(always)]
pub fn store_rows_fence() {
    #[cfg(target_arch = "x86_64")]
    // SAFETY: a store fence has no memory-safety preconditions.
    unsafe {
        std::arch::x86_64::_mm_sfence()
    };
}

/// Writes `row` into every row of `rows`, in parallel and with streaming stores: the padding rows
/// of a recycled trace buffer are written once and not read back.
pub fn pad_rows_streaming<R: Copy + Send + Sync>(rows: &mut [R], row: &R) {
    rows.par_chunks_mut(PAD_ROWS_PER_TASK).for_each(|chunk| {
        for dst in chunk {
            store_row_streaming(dst, row);
        }
        store_rows_fence();
    });
}
