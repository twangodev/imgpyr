/// A single-channel image: row-major `f32` samples carried with their dimensions.
pub struct Plane {
    data: Vec<f32>,
    width: usize,
    height: usize,
}

impl Plane {
    pub fn zeros(width: usize, height: usize) -> Self {
        Self::from_vec(vec![0.0; width * height], width, height)
    }

    /// Panics unless `data.len() == width * height`.
    pub fn from_vec(data: Vec<f32>, width: usize, height: usize) -> Self {
        assert_eq!(
            data.len(),
            width * height,
            "{width}x{height} needs {} samples, got {}",
            width * height,
            data.len()
        );
        Self {
            data,
            width,
            height,
        }
    }

    pub fn width(&self) -> usize {
        self.width
    }

    pub fn height(&self) -> usize {
        self.height
    }

    pub fn as_slice(&self) -> &[f32] {
        &self.data
    }

    pub fn sample(&self, x: usize, y: usize) -> f32 {
        self.data[y * self.width + x]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_vec_carries_samples_and_dimensions() {
        let plane = Plane::from_vec(vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0], 3, 2);

        assert_eq!(plane.width(), 3);
        assert_eq!(plane.height(), 2);
        assert_eq!(plane.as_slice(), &[1.0, 2.0, 3.0, 4.0, 5.0, 6.0]);
    }

    #[test]
    #[should_panic(expected = "3x2 needs 6 samples, got 5")]
    fn from_vec_rejects_a_length_that_contradicts_the_dimensions() {
        Plane::from_vec(vec![1.0, 2.0, 3.0, 4.0, 5.0], 3, 2);
    }

    #[test]
    fn zeros_is_empty_of_signal() {
        let plane = Plane::zeros(3, 2);

        assert_eq!(plane.width(), 3);
        assert_eq!(plane.height(), 2);
        assert_eq!(plane.as_slice(), &[0.0; 6]);
    }
}
