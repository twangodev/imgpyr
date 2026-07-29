use std::ops::Range;

use crate::Border;
use crate::kernel::{KERNEL, Taps};

/// Border resolution costs an integer division per tap, so the hot path avoids it.
pub(crate) fn interior<T: Taps>(count: usize, len: usize) -> Range<usize> {
    let extent = T::extent(len) as isize;
    let inside = |destination: usize| {
        T::coordinate(destination, 0) >= 0 && T::coordinate(destination, KERNEL.len() - 1) < extent
    };

    let start = (0..count).find(|&d| inside(d)).unwrap_or(count);
    let end = (start..count).find(|&d| !inside(d)).unwrap_or(count);

    start..end
}

pub(crate) fn bordered_sample<T: Taps>(
    src: &[f32],
    len: usize,
    destination: usize,
    border: Border,
) -> f32 {
    let mut total = 0.0;

    for (tap, &weight) in KERNEL.iter().enumerate() {
        let coordinate = T::coordinate(destination, tap);
        if T::contributes(coordinate) {
            let resolved = border.resolve(coordinate, T::extent(len));
            total += weight * src[T::index(resolved)];
        }
    }

    total
}

pub(crate) fn filter_row<T: Taps>(
    source: &[f32],
    src_width: usize,
    span: &Range<usize>,
    border: Border,
    row: &mut [f32],
) {
    for (x, sample) in row[..span.start].iter_mut().enumerate() {
        *sample = bordered_sample::<T>(source, src_width, x, border);
    }
    T::fill_interior(source, span.clone(), row);
    for (offset, sample) in row[span.end..].iter_mut().enumerate() {
        *sample = bordered_sample::<T>(source, src_width, span.end + offset, border);
    }
}

/// A rolling window of horizontally filtered rows, keyed by source row so the
/// vertical pass never touches a full-plane intermediate.
pub(crate) struct FilteredRows {
    samples: Vec<f32>,
    width: usize,
    holds: [Option<usize>; KERNEL.len()],
}

impl FilteredRows {
    pub(crate) fn new(width: usize) -> Self {
        Self {
            samples: vec![0.0; KERNEL.len() * width],
            width,
            holds: [None; KERNEL.len()],
        }
    }

    pub(crate) fn fetch<T: Taps>(
        &mut self,
        source_row: usize,
        src: &[f32],
        src_width: usize,
        span: &Range<usize>,
        border: Border,
    ) {
        let slot = source_row % KERNEL.len();
        if self.holds[slot] != Some(source_row) {
            let source = &src[source_row * src_width..(source_row + 1) * src_width];
            let row = &mut self.samples[slot * self.width..(slot + 1) * self.width];
            filter_row::<T>(source, src_width, span, border, row);
            self.holds[slot] = Some(source_row);
        }
    }

    pub(crate) fn row(&self, source_row: usize) -> &[f32] {
        let slot = source_row % KERNEL.len();
        &self.samples[slot * self.width..(slot + 1) * self.width]
    }
}
