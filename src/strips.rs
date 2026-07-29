#[cfg(feature = "rayon")]
use crate::kernel::KERNEL;

/// Splitting each thread's share four ways trades a little halo refiltering
/// for resilience to uneven scheduling.
#[cfg(feature = "rayon")]
pub(crate) fn rows_per_strip(height: usize) -> usize {
    height
        .div_ceil(4 * rayon::current_num_threads())
        .max(KERNEL.len())
}

#[cfg(not(feature = "rayon"))]
pub(crate) fn rows_per_strip(height: usize) -> usize {
    height.max(1)
}

pub(crate) fn fill_row_strips(
    buffer: &mut [f32],
    width: usize,
    strip_height: usize,
    fill: impl Fn(usize, &mut [f32]) + Send + Sync,
) {
    #[cfg(feature = "rayon")]
    {
        use rayon::prelude::*;
        buffer
            .par_chunks_mut(strip_height * width)
            .enumerate()
            .for_each(|(strip, rows)| fill(strip * strip_height, rows));
    }

    #[cfg(not(feature = "rayon"))]
    buffer
        .chunks_mut(strip_height * width)
        .enumerate()
        .for_each(|(strip, rows)| fill(strip * strip_height, rows));
}

pub(crate) fn fill_rows(
    buffer: &mut [f32],
    width: usize,
    fill: impl Fn(usize, &mut [f32]) + Send + Sync,
) {
    #[cfg(feature = "rayon")]
    {
        use rayon::prelude::*;
        buffer
            .par_chunks_mut(width)
            .enumerate()
            .for_each(|(y, row)| fill(y, row));
    }

    #[cfg(not(feature = "rayon"))]
    buffer
        .chunks_mut(width)
        .enumerate()
        .for_each(|(y, row)| fill(y, row));
}
