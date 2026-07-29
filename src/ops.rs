use std::ops::Range;

use crate::{Border, Plane};

/// Burt & Adelson (1983) with `a = 0.375`.
const KERNEL: [f32; 5] = [1.0, 4.0, 6.0, 4.0, 1.0];

const KERNEL_SUM: f32 = KERNEL[0] + KERNEL[1] + KERNEL[2] + KERNEL[3] + KERNEL[4];

const RADIUS: isize = (KERNEL.len() / 2) as isize;

trait Taps {
    const GAIN: f32;

    fn coordinate(destination: usize, tap: usize) -> isize;

    fn extent(len: usize) -> usize;

    fn contributes(coordinate: isize) -> bool;

    fn index(coordinate: usize) -> usize;
}

struct Decimate;

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
}

struct Interpolate;

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
}

/// Border resolution costs an integer division per tap, so the hot path avoids it.
fn interior<T: Taps>(count: usize, len: usize) -> Range<usize> {
    let extent = T::extent(len) as isize;
    let inside = |destination: usize| {
        T::coordinate(destination, 0) >= 0 && T::coordinate(destination, KERNEL.len() - 1) < extent
    };

    let start = (0..count).find(|&d| inside(d)).unwrap_or(count);
    let end = (start..count).find(|&d| !inside(d)).unwrap_or(count);

    start..end
}

fn interior_sample<T: Taps>(src: &[f32], destination: usize) -> f32 {
    let mut total = 0.0;

    for (tap, &weight) in KERNEL.iter().enumerate() {
        let coordinate = T::coordinate(destination, tap);
        if T::contributes(coordinate) {
            total += weight * src[T::index(coordinate as usize)];
        }
    }

    total * T::GAIN / KERNEL_SUM
}

fn bordered_sample<T: Taps>(src: &[f32], len: usize, destination: usize, border: Border) -> f32 {
    let mut total = 0.0;

    for (tap, &weight) in KERNEL.iter().enumerate() {
        let coordinate = T::coordinate(destination, tap);
        if T::contributes(coordinate) {
            let resolved = border.resolve(coordinate, T::extent(len));
            total += weight * src[T::index(resolved)];
        }
    }

    total * T::GAIN / KERNEL_SUM
}

fn filter_row<T: Taps>(
    source: &[f32],
    src_width: usize,
    span: &Range<usize>,
    border: Border,
    row: &mut [f32],
) {
    for (x, sample) in row[..span.start].iter_mut().enumerate() {
        *sample = bordered_sample::<T>(source, src_width, x, border);
    }
    for (offset, sample) in row[span.clone()].iter_mut().enumerate() {
        *sample = interior_sample::<T>(source, span.start + offset);
    }
    for (offset, sample) in row[span.end..].iter_mut().enumerate() {
        *sample = bordered_sample::<T>(source, src_width, span.end + offset, border);
    }
}

/// A rolling window of horizontally filtered rows, keyed by source row so the
/// vertical pass never touches a full-plane intermediate.
struct FilteredRows {
    samples: Vec<f32>,
    width: usize,
    holds: [Option<usize>; KERNEL.len()],
}

impl FilteredRows {
    fn new(width: usize) -> Self {
        Self {
            samples: vec![0.0; KERNEL.len() * width],
            width,
            holds: [None; KERNEL.len()],
        }
    }

    fn fetch<T: Taps>(
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

    fn row(&self, source_row: usize) -> &[f32] {
        let slot = source_row % KERNEL.len();
        &self.samples[slot * self.width..(slot + 1) * self.width]
    }
}

fn blend_rows(taps: &[(f32, &[f32])], gain: f32, destination: &mut [f32]) {
    let width = destination.len();

    match *taps {
        [(w0, r0), (w1, r1)] => {
            let (r0, r1) = (&r0[..width], &r1[..width]);
            for (x, sample) in destination.iter_mut().enumerate() {
                let mut total = 0.0;
                total += w0 * r0[x];
                total += w1 * r1[x];
                *sample = total * gain / KERNEL_SUM;
            }
        }
        [(w0, r0), (w1, r1), (w2, r2)] => {
            let (r0, r1, r2) = (&r0[..width], &r1[..width], &r2[..width]);
            for (x, sample) in destination.iter_mut().enumerate() {
                let mut total = 0.0;
                total += w0 * r0[x];
                total += w1 * r1[x];
                total += w2 * r2[x];
                *sample = total * gain / KERNEL_SUM;
            }
        }
        [(w0, r0), (w1, r1), (w2, r2), (w3, r3), (w4, r4)] => {
            let (r0, r1, r2, r3, r4) = (
                &r0[..width],
                &r1[..width],
                &r2[..width],
                &r3[..width],
                &r4[..width],
            );
            for (x, sample) in destination.iter_mut().enumerate() {
                let mut total = 0.0;
                total += w0 * r0[x];
                total += w1 * r1[x];
                total += w2 * r2[x];
                total += w3 * r3[x];
                total += w4 * r4[x];
                *sample = total * gain / KERNEL_SUM;
            }
        }
        _ => {
            for (x, sample) in destination.iter_mut().enumerate() {
                let mut total = 0.0;
                for &(weight, row) in taps {
                    total += weight * row[x];
                }
                *sample = total * gain / KERNEL_SUM;
            }
        }
    }
}

fn resample<T: Taps>(
    src: &[f32],
    src_width: usize,
    src_height: usize,
    width: usize,
    height: usize,
    border: Border,
) -> Vec<f32> {
    let mut resampled = vec![0.0; width * height];
    let span = interior::<T>(width, src_width);
    let extent = T::extent(src_height);

    fill_row_strips(
        &mut resampled,
        width,
        rows_per_strip(height),
        |top, strip| {
            let mut window = FilteredRows::new(width);

            for (offset, destination) in strip.chunks_mut(width).enumerate() {
                let y = top + offset;

                let mut contributing = [(0.0, 0usize); KERNEL.len()];
                let mut count = 0;
                for (tap, &weight) in KERNEL.iter().enumerate() {
                    let coordinate = T::coordinate(y, tap);
                    if T::contributes(coordinate) {
                        let source_row = T::index(border.resolve(coordinate, extent));
                        window.fetch::<T>(source_row, src, src_width, &span, border);
                        contributing[count] = (weight, source_row);
                        count += 1;
                    }
                }

                let mut taps = [(0.0, [].as_slice()); KERNEL.len()];
                for (tap, &(weight, source_row)) in contributing[..count].iter().enumerate() {
                    taps[tap] = (weight, window.row(source_row));
                }

                blend_rows(&taps[..count], T::GAIN, destination);
            }
        },
    );

    resampled
}

/// Splitting each thread's share four ways trades a little halo refiltering
/// for resilience to uneven scheduling.
#[cfg(feature = "rayon")]
fn rows_per_strip(height: usize) -> usize {
    height
        .div_ceil(4 * rayon::current_num_threads())
        .max(KERNEL.len())
}

#[cfg(not(feature = "rayon"))]
fn rows_per_strip(height: usize) -> usize {
    height.max(1)
}

fn fill_row_strips(
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

/// Blurs and halves a plane, rounding each dimension up.
pub fn reduce(src: &Plane, border: Border) -> Plane {
    let width = src.width().div_ceil(2);
    let height = src.height().div_ceil(2);

    let samples = resample::<Decimate>(
        src.as_slice(),
        src.width(),
        src.height(),
        width,
        height,
        border,
    );

    Plane::from_vec(samples, width, height)
}

/// Doubles a plane onto an explicitly sized destination.
///
/// The size is given rather than derived because doubling is ambiguous: a
/// 51-wide plane expands to either 101 or 102, and only the caller knows which
/// one it was reduced from.
///
/// Panics unless `width` and `height` reduce back to the source dimensions.
pub fn expand(src: &Plane, width: usize, height: usize, border: Border) -> Plane {
    assert!(
        width.div_ceil(2) == src.width() && height.div_ceil(2) == src.height(),
        "{width}x{height} does not reduce to {}x{}",
        src.width(),
        src.height()
    );

    let samples = resample::<Interpolate>(
        src.as_slice(),
        src.width(),
        src.height(),
        width,
        height,
        border,
    );

    Plane::from_vec(samples, width, height)
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    // The unfactored 5x5 form, kept as an independent reference for the
    // separable implementation to be checked against.
    /// Half the taps land on the zeros that upsampling inserts and contribute
    /// nothing; `GAIN` restores the energy they would have carried.
    fn reference_expand_at(src: &Plane, x: usize, y: usize, border: Border) -> f32 {
        const GAIN: f32 = 2.0;
        let mut total = 0.0;

        for (row, &row_weight) in KERNEL.iter().enumerate() {
            let Some(sy) = reference_upsampled_source(y, row, src.height(), border) else {
                continue;
            };

            for (column, &column_weight) in KERNEL.iter().enumerate() {
                let Some(sx) = reference_upsampled_source(x, column, src.width(), border) else {
                    continue;
                };
                total += row_weight * column_weight * src.sample(sx, sy);
            }
        }

        total * (GAIN * GAIN) / (KERNEL_SUM * KERNEL_SUM)
    }

    /// `None` where the tap falls on an inserted zero.
    fn reference_upsampled_source(
        destination: usize,
        tap: usize,
        len: usize,
        border: Border,
    ) -> Option<usize> {
        let coordinate = destination as isize + tap as isize - RADIUS;
        (coordinate.rem_euclid(2) == 0).then(|| border.resolve(coordinate, 2 * len) / 2)
    }

    fn reference_reduce_at(src: &Plane, cx: usize, cy: usize, border: Border) -> f32 {
        let mut total = 0.0;

        for (row, &row_weight) in KERNEL.iter().enumerate() {
            let sy = border.resolve(cy as isize + row as isize - RADIUS, src.height());

            for (column, &column_weight) in KERNEL.iter().enumerate() {
                let sx = border.resolve(cx as isize + column as isize - RADIUS, src.width());
                total += row_weight * column_weight * src.sample(sx, sy);
            }
        }

        total / (KERNEL_SUM * KERNEL_SUM)
    }

    const BORDERS: [Border; 2] = [Border::Replicate, Border::Mirror];

    fn plane_from(width: usize, height: usize, value: impl Fn(f32, f32) -> f32) -> Plane {
        let samples = (0..width * height)
            .map(|i| value((i % width) as f32, (i / width) as f32))
            .collect();
        Plane::from_vec(samples, width, height)
    }

    /// A constant is shift-invariant, so this cannot see a decimation-phase
    /// error. That is what the ramp is for.
    #[test]
    fn a_constant_plane_survives_reduction() {
        for border in BORDERS {
            let reduced = reduce(&plane_from(9, 7, |_, _| 0.5), border);

            for (i, &sample) in reduced.as_slice().iter().enumerate() {
                assert!(
                    (sample - 0.5).abs() < 1e-6,
                    "{border:?} drifted to {sample} at index {i}"
                );
            }
        }
    }

    /// Reduction must land on even source coordinates. Sampling half a step off
    /// still yields a ramp, so only its offset gives the error away.
    ///
    /// Affine extension survives neither border mode, so the edges are excluded.
    #[test]
    fn reduction_samples_a_ramp_at_even_coordinates() {
        let ramp = |x: f32, y: f32| 0.1 + 0.03 * x + 0.07 * y;

        for border in BORDERS {
            let reduced = reduce(&plane_from(9, 7, ramp), border);

            for y in 1..reduced.height() - 1 {
                for x in 1..reduced.width() - 1 {
                    let expected = ramp((2 * x) as f32, (2 * y) as f32);
                    let actual = reduced.sample(x, y);
                    assert!(
                        (actual - expected).abs() < 1e-5,
                        "{border:?} at ({x}, {y}): {actual} != {expected}"
                    );
                }
            }
        }
    }

    #[test]
    fn expansion_fills_the_requested_size() {
        let source = Plane::zeros(5, 4);

        let even = expand(&source, 10, 8, Border::Mirror);
        assert_eq!((even.width(), even.height()), (10, 8));

        let odd = expand(&source, 9, 7, Border::Mirror);
        assert_eq!((odd.width(), odd.height()), (9, 7));
    }

    #[test]
    #[should_panic(expected = "9x7 does not reduce to 4x4")]
    fn expansion_rejects_a_size_that_does_not_reduce_to_the_source() {
        expand(&Plane::zeros(4, 4), 9, 7, Border::Mirror);
    }

    /// Catches the zero-insertion gain: without it a flat region loses three
    /// quarters of its value.
    #[test]
    fn a_constant_plane_survives_expansion() {
        for border in BORDERS {
            for (width, height) in [(10, 8), (9, 7)] {
                let expanded = expand(&plane_from(5, 4, |_, _| 0.5), width, height, border);

                for (i, &sample) in expanded.as_slice().iter().enumerate() {
                    assert!(
                        (sample - 0.5).abs() < 1e-6,
                        "{border:?} {width}x{height} drifted to {sample} at index {i}"
                    );
                }
            }
        }
    }

    /// Source sample `i` must land on destination `2i`, with odd destinations
    /// interpolated halfway. A ramp is the only input that distinguishes the
    /// even and odd tap sets, which carry different weights.
    #[test]
    fn expansion_interpolates_a_ramp() {
        let ramp = |x: f32, y: f32| 0.1 + 0.03 * x + 0.07 * y;

        for border in BORDERS {
            let expanded = expand(&plane_from(5, 4, ramp), 10, 8, border);

            for y in 2..expanded.height() - 2 {
                for x in 2..expanded.width() - 2 {
                    let expected = ramp(x as f32 / 2.0, y as f32 / 2.0);
                    let actual = expanded.sample(x, y);
                    assert!(
                        (actual - expected).abs() < 1e-5,
                        "{border:?} at ({x}, {y}): {actual} != {expected}"
                    );
                }
            }
        }
    }

    fn textured(width: usize, height: usize, seed: f32) -> Plane {
        plane_from(width, height, |x, y| {
            (x * 1.7 + seed).sin() * (y * 0.9 - seed).cos()
        })
    }

    proptest! {
        /// Factoring the kernel into two passes must not change the answer.
        #[test]
        fn reduction_matches_the_unfactored_form(
            width in 1usize..40,
            height in 1usize..40,
            seed in 0.0f32..10.0,
        ) {
            let source = textured(width, height, seed);

            for border in BORDERS {
                let separable = reduce(&source, border);

                for y in 0..separable.height() {
                    for x in 0..separable.width() {
                        let expected = reference_reduce_at(&source, 2 * x, 2 * y, border);
                        prop_assert!((separable.sample(x, y) - expected).abs() < 1e-5);
                    }
                }
            }
        }

        #[test]
        fn expansion_matches_the_unfactored_form(
            width in 1usize..60,
            height in 1usize..60,
            seed in 0.0f32..10.0,
        ) {
            let source = textured(width.div_ceil(2), height.div_ceil(2), seed);

            for border in BORDERS {
                let separable = expand(&source, width, height, border);

                for y in 0..height {
                    for x in 0..width {
                        let expected = reference_expand_at(&source, x, y, border);
                        prop_assert!((separable.sample(x, y) - expected).abs() < 1e-5);
                    }
                }
            }
        }

        #[test]
        fn any_constant_survives_expansion_at_any_size(
            width in 1usize..60,
            height in 1usize..60,
            value in -10.0f32..10.0,
        ) {
            let source = plane_from(width.div_ceil(2), height.div_ceil(2), |_, _| value);

            for border in BORDERS {
                for &sample in expand(&source, width, height, border).as_slice() {
                    prop_assert!((sample - value).abs() < 1e-5);
                }
            }
        }

        #[test]
        fn reduction_halves_each_dimension_rounding_up(
            width in 1usize..300,
            height in 1usize..300,
        ) {
            let reduced = reduce(&Plane::zeros(width, height), Border::Mirror);

            prop_assert_eq!(reduced.width(), width.div_ceil(2));
            prop_assert_eq!(reduced.height(), height.div_ceil(2));
        }

        #[test]
        fn any_constant_survives_reduction_at_any_size(
            width in 1usize..40,
            height in 1usize..40,
            value in -10.0f32..10.0,
        ) {
            for border in BORDERS {
                for &sample in reduce(&plane_from(width, height, |_, _| value), border).as_slice() {
                    prop_assert!((sample - value).abs() < 1e-5);
                }
            }
        }
    }
}
