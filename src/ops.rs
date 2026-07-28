use crate::{Border, Plane};

/// Burt & Adelson (1983) with `a = 0.375`.
const KERNEL: [f32; 5] = [1.0, 4.0, 6.0, 4.0, 1.0];

const KERNEL_SUM: f32 = KERNEL[0] + KERNEL[1] + KERNEL[2] + KERNEL[3] + KERNEL[4];

/// Blurs and halves a plane, rounding each dimension up.
pub fn reduce(src: &Plane, border: Border) -> Plane {
    let width = src.width().div_ceil(2);
    let height = src.height().div_ceil(2);
    let mut samples = Vec::with_capacity(width * height);

    for y in 0..height {
        for x in 0..width {
            samples.push(weighted_sum_around(src, 2 * x, 2 * y, border));
        }
    }

    Plane::from_vec(samples, width, height)
}

fn weighted_sum_around(src: &Plane, cx: usize, cy: usize, border: Border) -> f32 {
    const RADIUS: isize = 2;
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

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    const BORDERS: [Border; 2] = [Border::Replicate, Border::Mirror];

    fn plane_from(width: usize, height: usize, value: impl Fn(f32, f32) -> f32) -> Plane {
        let samples = (0..width * height)
            .map(|i| value((i % width) as f32, (i / width) as f32))
            .collect();
        Plane::from_vec(samples, width, height)
    }

    /// A constant is shift-invariant, so this cannot see a decimation-phase
    /// error — that is what the ramp is for.
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

    proptest! {
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
