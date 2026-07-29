/// How a filter reads past the edge of a plane.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Border {
    /// `aaa|abcde|eee`
    Replicate,
    /// `dcb|abcde|dcb`, reflect-101. The only mode OpenCV's `pyrUp` accepts.
    Mirror,
}

impl Border {
    /// The real sample standing in for `coord`, which may fall outside `0..len`.
    pub fn resolve(self, coord: isize, len: usize) -> usize {
        let last = len as isize - 1;
        if last <= 0 {
            return 0;
        }
        match self {
            Border::Replicate => coord.clamp(0, last) as usize,
            Border::Mirror => {
                let folded = coord.rem_euclid(2 * last);
                (if folded > last {
                    2 * last - folded
                } else {
                    folded
                }) as usize
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn coordinates_inside_the_plane_are_untouched() {
        for border in [Border::Replicate, Border::Mirror] {
            assert_eq!(border.resolve(0, 5), 0);
            assert_eq!(border.resolve(2, 5), 2);
            assert_eq!(border.resolve(4, 5), 4);
        }
    }

    #[test]
    fn replicate_holds_the_edge_sample() {
        assert_eq!(Border::Replicate.resolve(-1, 5), 0);
        assert_eq!(Border::Replicate.resolve(-4, 5), 0);
        assert_eq!(Border::Replicate.resolve(5, 5), 4);
        assert_eq!(Border::Replicate.resolve(9, 5), 4);
    }

    #[test]
    fn mirror_folds_without_repeating_the_edge_sample() {
        assert_eq!(Border::Mirror.resolve(-1, 5), 1);
        assert_eq!(Border::Mirror.resolve(-2, 5), 2);
        assert_eq!(Border::Mirror.resolve(5, 5), 3);
        assert_eq!(Border::Mirror.resolve(6, 5), 2);
    }

    #[test]
    fn mirror_folds_repeatedly_for_coordinates_far_outside() {
        assert_eq!(Border::Mirror.resolve(8, 5), 0);
        assert_eq!(Border::Mirror.resolve(-8, 5), 0);
        assert_eq!(Border::Mirror.resolve(11, 5), 3);
    }

    #[test]
    fn a_single_sample_absorbs_every_coordinate() {
        for border in [Border::Replicate, Border::Mirror] {
            assert_eq!(border.resolve(-7, 1), 0);
            assert_eq!(border.resolve(0, 1), 0);
            assert_eq!(border.resolve(7, 1), 0);
        }
    }
}
