use crate::{Border, Plane};

/// Burt & Adelson (1983) with `a = 0.375`.
const KERNEL: [f32; 5] = [1.0, 4.0, 6.0, 4.0, 1.0];

const KERNEL_SUM: f32 = KERNEL[0] + KERNEL[1] + KERNEL[2] + KERNEL[3] + KERNEL[4];

const RADIUS: isize = (KERNEL.len() / 2) as isize;

/// Which source sample a kernel tap reads, or `None` where it reads a position
/// that only exists because upsampling invented it.
type Tap = fn(usize, usize, usize, Border) -> Option<usize>;

fn decimating(destination: usize, tap: usize, len: usize, border: Border) -> Option<usize> {
    Some(border.resolve(2 * destination as isize + tap as isize - RADIUS, len))
}

/// Edges are resolved against the upsampled grid rather than the source, which
/// is what OpenCV does and is not the same thing: the mirror axis at the far end
/// falls on an inserted zero, so reflection there lands back on the last real
/// sample instead of the one before it.
fn interpolating(destination: usize, tap: usize, len: usize, border: Border) -> Option<usize> {
    let coordinate = destination as isize + tap as isize - RADIUS;
    (coordinate.rem_euclid(2) == 0).then(|| border.resolve(coordinate, 2 * len) / 2)
}

/// The kernel is an outer product, so a 2D pass factors into one pass per axis:
/// 10 multiplies per sample instead of 25.
fn weighted(
    samples: &[f32],
    stride: usize,
    len: usize,
    destination: usize,
    tap: Tap,
    gain: f32,
    border: Border,
) -> f32 {
    let mut total = 0.0;

    for (index, &weight) in KERNEL.iter().enumerate() {
        if let Some(source) = tap(destination, index, len, border) {
            total += weight * samples[source * stride];
        }
    }

    total * gain / KERNEL_SUM
}

fn across(
    src: &[f32],
    src_width: usize,
    height: usize,
    width: usize,
    tap: Tap,
    gain: f32,
    border: Border,
) -> Vec<f32> {
    let mut resampled = vec![0.0; width * height];

    fill_rows(&mut resampled, width, |y, row| {
        let source = &src[y * src_width..(y + 1) * src_width];
        for (x, sample) in row.iter_mut().enumerate() {
            *sample = weighted(source, 1, src_width, x, tap, gain, border);
        }
    });

    resampled
}

fn down(
    src: &[f32],
    width: usize,
    src_height: usize,
    height: usize,
    tap: Tap,
    gain: f32,
    border: Border,
) -> Vec<f32> {
    let mut resampled = vec![0.0; width * height];

    fill_rows(&mut resampled, width, |y, row| {
        for (x, sample) in row.iter_mut().enumerate() {
            *sample = weighted(&src[x..], width, src_height, y, tap, gain, border);
        }
    });

    resampled
}

/// Every output row is independent of the others, which is the whole reason
/// `rayon` can be dropped in without touching the arithmetic.
fn fill_rows(buffer: &mut [f32], width: usize, fill: impl Fn(usize, &mut [f32]) + Send + Sync) {
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
    const GAIN: f32 = 1.0;
    let width = src.width().div_ceil(2);
    let height = src.height().div_ceil(2);

    let columns = across(
        src.as_slice(),
        src.width(),
        src.height(),
        width,
        decimating,
        GAIN,
        border,
    );
    let samples = down(
        &columns,
        width,
        src.height(),
        height,
        decimating,
        GAIN,
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

    const GAIN: f32 = 2.0;

    let columns = across(
        src.as_slice(),
        src.width(),
        src.height(),
        width,
        interpolating,
        GAIN,
        border,
    );
    let samples = down(
        &columns,
        width,
        src.height(),
        height,
        interpolating,
        GAIN,
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
