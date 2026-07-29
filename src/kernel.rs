use std::ops::Range;

/// Burt & Adelson (1983) with `a = 0.375`.
pub(crate) const KERNEL: [f32; 5] = [1.0, 4.0, 6.0, 4.0, 1.0];

pub(crate) const KERNEL_SUM: f32 = KERNEL[0] + KERNEL[1] + KERNEL[2] + KERNEL[3] + KERNEL[4];

pub(crate) const RADIUS: isize = (KERNEL.len() / 2) as isize;

pub(crate) trait Taps {
    const GAIN: f32;

    /// Both passes leave their sums unnormalised, so this folds the kernel
    /// normalisation for each of them into one multiply.
    const SCALE: f32 = Self::GAIN * Self::GAIN / (KERNEL_SUM * KERNEL_SUM);

    fn coordinate(destination: usize, tap: usize) -> isize;

    fn extent(len: usize) -> usize;

    fn contributes(coordinate: isize) -> bool;

    fn index(coordinate: usize) -> usize;

    /// Unnormalised, and branch-free so the compiler can vectorise it. The
    /// per-tap `contributes` test is what otherwise blocks that.
    fn fill_interior(src: &[f32], span: Range<usize>, row: &mut [f32]);
}

pub(crate) struct Decimate;

impl Taps for Decimate {
    const GAIN: f32 = 1.0;

    fn coordinate(destination: usize, tap: usize) -> isize {
        2 * destination as isize + tap as isize - RADIUS
    }

    fn extent(len: usize) -> usize {
        len
    }

    fn contributes(_: isize) -> bool {
        true
    }

    fn index(coordinate: usize) -> usize {
        coordinate
    }

    fn fill_interior(src: &[f32], span: Range<usize>, row: &mut [f32]) {
        for (offset, sample) in row[span.clone()].iter_mut().enumerate() {
            let base = 2 * (span.start + offset) - RADIUS as usize;
            *sample = KERNEL[0] * src[base]
                + KERNEL[1] * src[base + 1]
                + KERNEL[2] * src[base + 2]
                + KERNEL[3] * src[base + 3]
                + KERNEL[4] * src[base + 4];
        }
    }
}

pub(crate) struct Interpolate;

impl Taps for Interpolate {
    const GAIN: f32 = 2.0;

    fn coordinate(destination: usize, tap: usize) -> isize {
        destination as isize + tap as isize - RADIUS
    }

    /// The far mirror axis sits on an inserted zero, so reflection there returns
    /// the last real sample rather than the one before it. OpenCV does the same.
    fn extent(len: usize) -> usize {
        2 * len
    }

    fn contributes(coordinate: isize) -> bool {
        coordinate.rem_euclid(2) == 0
    }

    fn index(coordinate: usize) -> usize {
        coordinate / 2
    }

    fn fill_interior(src: &[f32], span: Range<usize>, row: &mut [f32]) {
        for (offset, sample) in row[span.clone()].iter_mut().enumerate() {
            let destination = span.start + offset;
            let source = destination / 2;
            *sample = if destination.is_multiple_of(2) {
                KERNEL[0] * src[source - 1] + KERNEL[2] * src[source] + KERNEL[4] * src[source + 1]
            } else {
                KERNEL[1] * src[source] + KERNEL[3] * src[source + 1]
            };
        }
    }
}
