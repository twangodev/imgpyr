use crate::ops::fill_rows;
use crate::{Border, Plane, expand, reduce};

/// A plane and its successive halvings.
pub struct GaussianPyramid {
    levels: Vec<Plane>,
}

impl GaussianPyramid {
    /// `levels` counts reductions, so the result holds `levels + 1` planes.
    pub fn build(src: &Plane, levels: usize, border: Border) -> Self {
        let mut planes = Vec::with_capacity(levels + 1);
        planes.push(src.clone());
        planes.extend(reductions(src, levels, border));

        Self { levels: planes }
    }

    pub fn level(&self, index: usize) -> &Plane {
        &self.levels[index]
    }

    pub fn len(&self) -> usize {
        self.levels.len()
    }

    pub fn is_empty(&self) -> bool {
        self.levels.is_empty()
    }
}

/// Detail separated by scale, finest first, plus what is left once every scale
/// has been taken out.
pub struct LaplacianPyramid {
    bands: Vec<Plane>,
    residual: Plane,
    border: Border,
}

impl LaplacianPyramid {
    pub fn build(src: &Plane, levels: usize, border: Border) -> Self {
        let mut reduced = reductions(src, levels, border);

        let bands = (0..levels)
            .map(|index| {
                let finer = if index == 0 { src } else { &reduced[index - 1] };
                let mut blurred = expand(&reduced[index], finer.width(), finer.height(), border);
                combine_into(finer, &mut blurred, |detailed, blurred| detailed - blurred);
                blurred
            })
            .collect();

        Self {
            bands,
            residual: reduced.pop().unwrap_or_else(|| src.clone()),
            border,
        }
    }

    pub fn band(&self, index: usize) -> &Plane {
        &self.bands[index]
    }

    pub fn band_mut(&mut self, index: usize) -> &mut Plane {
        &mut self.bands[index]
    }

    pub fn residual(&self) -> &Plane {
        &self.residual
    }

    pub fn len(&self) -> usize {
        self.bands.len()
    }

    pub fn is_empty(&self) -> bool {
        self.bands.is_empty()
    }

    /// Reconstruction is only exact when analysis and synthesis use the same
    /// expand, so the border is taken from `build` rather than the caller.
    pub fn collapse(&self) -> Plane {
        let border = self.border;
        self.bands
            .iter()
            .rev()
            .fold(self.residual.clone(), |coarser, band| {
                let mut blurred = expand(&coarser, band.width(), band.height(), border);
                combine_into(band, &mut blurred, |detail, blurred| detail + blurred);
                blurred
            })
    }
}

fn reductions(src: &Plane, levels: usize, border: Border) -> Vec<Plane> {
    let mut planes: Vec<Plane> = Vec::with_capacity(levels);

    for _ in 0..levels {
        let coarser = reduce(planes.last().unwrap_or(src), border);
        planes.push(coarser);
    }

    planes
}

fn combine_into(band: &Plane, blurred: &mut Plane, op: impl Fn(f32, f32) -> f32 + Send + Sync) {
    let width = blurred.width();
    let band_samples = band.as_slice();

    fill_rows(blurred.as_mut_slice(), width, |y, row| {
        let source = &band_samples[y * width..(y + 1) * width];
        for (sample, &value) in row.iter_mut().zip(source) {
            *sample = op(value, *sample);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    /// Reconstruction is exact in real arithmetic. In `f32` the error comes from
    /// cancellation in `G - E`, so it scales with the plane's peak magnitude
    /// rather than with each sample: near-zero samples drift by the same tiny
    /// absolute amount as large ones, which is many ulps but little signal.
    const RECONSTRUCTION_EPSILONS: f32 = 8.0;

    fn plane_from(width: usize, height: usize, value: impl Fn(f32, f32) -> f32) -> Plane {
        let samples = (0..width * height)
            .map(|i| value((i % width) as f32, (i / width) as f32))
            .collect();
        Plane::from_vec(samples, width, height)
    }

    #[test]
    fn a_gaussian_pyramid_keeps_the_source_and_one_plane_per_reduction() {
        let pyramid = GaussianPyramid::build(&Plane::zeros(65, 49), 3, Border::Mirror);

        assert_eq!(pyramid.len(), 4);
        assert_eq!(
            (pyramid.level(0).width(), pyramid.level(0).height()),
            (65, 49)
        );
        assert_eq!(
            (pyramid.level(1).width(), pyramid.level(1).height()),
            (33, 25)
        );
        assert_eq!(
            (pyramid.level(2).width(), pyramid.level(2).height()),
            (17, 13)
        );
        assert_eq!(
            (pyramid.level(3).width(), pyramid.level(3).height()),
            (9, 7)
        );
    }

    #[test]
    fn a_laplacian_band_matches_the_level_it_came_from() {
        let pyramid = LaplacianPyramid::build(&Plane::zeros(65, 49), 2, Border::Mirror);

        assert_eq!(pyramid.len(), 2);
        assert_eq!(
            (pyramid.band(0).width(), pyramid.band(0).height()),
            (65, 49)
        );
        assert_eq!(
            (pyramid.band(1).width(), pyramid.band(1).height()),
            (33, 25)
        );
        assert_eq!(
            (pyramid.residual().width(), pyramid.residual().height()),
            (17, 13)
        );
    }

    /// The expand terms cancel algebraically, so this proves little beyond the
    /// size bookkeeping, which is exactly where odd dimensions break.
    #[test]
    fn collapse_restores_the_source() {
        let source = plane_from(65, 49, |x, y| (x * 0.7).sin() + (y * 0.3).cos());

        let peak = source.as_slice().iter().fold(0.0f32, |m, v| m.max(v.abs()));

        for border in [Border::Replicate, Border::Mirror] {
            let restored = LaplacianPyramid::build(&source, 3, border).collapse();

            for (i, (&actual, &expected)) in restored
                .as_slice()
                .iter()
                .zip(source.as_slice())
                .enumerate()
            {
                assert!(
                    (actual - expected).abs() <= RECONSTRUCTION_EPSILONS * f32::EPSILON * peak,
                    "{border:?} at {i}: {actual} != {expected}"
                );
            }
        }
    }

    #[test]
    fn a_constant_plane_leaves_every_band_empty() {
        for border in [Border::Replicate, Border::Mirror] {
            let pyramid = LaplacianPyramid::build(&plane_from(65, 49, |_, _| 0.5), 3, border);

            for index in 0..pyramid.len() {
                for (i, &sample) in pyramid.band(index).as_slice().iter().enumerate() {
                    assert!(
                        sample.abs() < 1e-6,
                        "{border:?} band {index} holds {sample} at {i}"
                    );
                }
            }
        }
    }

    /// The 5-tap kernel reproduces affine signals exactly, so a ramp is also
    /// pure residual, but only away from the edges, where neither border mode
    /// extends a ramp as a ramp.
    #[test]
    fn a_ramp_leaves_every_band_empty_away_from_the_edges() {
        const MARGIN: usize = 6;
        let ramp = |x: f32, y: f32| 0.1 + 0.03 * x + 0.07 * y;

        for border in [Border::Replicate, Border::Mirror] {
            let pyramid = LaplacianPyramid::build(&plane_from(65, 49, ramp), 2, border);

            for index in 0..pyramid.len() {
                let band = pyramid.band(index);

                for y in MARGIN..band.height() - MARGIN {
                    for x in MARGIN..band.width() - MARGIN {
                        let sample = band.sample(x, y);
                        assert!(
                            sample.abs() < 1e-4,
                            "{border:?} band {index} holds {sample} at ({x}, {y})"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn editing_a_band_changes_what_collapse_produces() {
        let source = plane_from(33, 25, |x, y| 0.01 * x + 0.02 * y);
        let mut pyramid = LaplacianPyramid::build(&source, 2, Border::Mirror);

        pyramid.band_mut(1).as_mut_slice()[0] += 1.0;
        let edited = pyramid.collapse();

        let drift: f32 = edited
            .as_slice()
            .iter()
            .zip(source.as_slice())
            .map(|(a, b)| (a - b).abs())
            .sum();
        assert!(drift > 0.5, "editing a band did not reach the output");
    }

    proptest! {
        /// Odd dimensions are where reduce and expand stop agreeing on size.
        #[test]
        fn collapse_restores_the_source_at_any_size(
            width in 1usize..80,
            height in 1usize..80,
            levels in 0usize..4,
        ) {
            let source = plane_from(width, height, |x, y| 0.01 * x - 0.02 * y);

            let restored = LaplacianPyramid::build(&source, levels, Border::Mirror).collapse();

            prop_assert_eq!(restored.width(), width);
            prop_assert_eq!(restored.height(), height);
            for (&actual, &expected) in restored.as_slice().iter().zip(source.as_slice()) {
                prop_assert!((actual - expected).abs() < 1e-4);
            }
        }
    }
}
