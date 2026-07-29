//! Writes every level of both pyramids as a PNG.
//!
//! ```text
//! cargo run --example pyramid              # synthetic zone plate
//! cargo run --example pyramid -- photo.png
//! ```

use std::path::{Path, PathBuf};

use imgpyr::{Border, GaussianPyramid, LaplacianPyramid, Plane};

const LEVELS: usize = 5;
const BORDER: Border = Border::Mirror;

fn main() {
    let plane = match std::env::args().nth(1) {
        Some(path) => load(&path),
        None => zone_plate(512, 512),
    };

    let output = PathBuf::from("target/pyramid");
    std::fs::create_dir_all(&output).expect("create output directory");

    let gaussian = GaussianPyramid::build(&plane, LEVELS, BORDER);
    for index in 0..gaussian.len() {
        write(
            gaussian.level(index),
            &output.join(format!("gaussian-{index}.png")),
        );
    }

    let laplacian = LaplacianPyramid::build(&plane, LEVELS, BORDER);
    for index in 0..laplacian.len() {
        write_signed(
            laplacian.band(index),
            &output.join(format!("laplacian-{index}.png")),
        );
    }
    write(laplacian.residual(), &output.join("laplacian-residual.png"));
    write(&laplacian.collapse(BORDER), &output.join("collapsed.png"));

    println!("{} levels written to {}", gaussian.len(), output.display());
}

/// Spatial frequency rises with the square of the radius, so one image spans
/// everything from flat to past the sampling limit. Reduction that skipped the
/// blur would ring with moire here.
fn zone_plate(width: usize, height: usize) -> Plane {
    let centre = (width as f32 / 2.0, height as f32 / 2.0);
    let sharpness = 12.0 / (width.min(height) as f32);

    let samples = (0..width * height)
        .map(|i| {
            let dx = (i % width) as f32 - centre.0;
            let dy = (i / width) as f32 - centre.1;
            0.5 + 0.5 * ((dx * dx + dy * dy) * sharpness * sharpness).cos()
        })
        .collect();

    Plane::from_vec(samples, width, height)
}

fn load(path: &str) -> Plane {
    let image = image::open(path).expect("decode image").to_luma32f();
    let (width, height) = image.dimensions();
    Plane::from_vec(image.into_raw(), width as usize, height as usize)
}

fn write(plane: &Plane, path: &Path) {
    save(plane, path, |sample| sample);
}

/// Bands straddle zero, so they need lifting into range before an unsigned
/// format can hold them. The gain is presentational only.
fn write_signed(plane: &Plane, path: &Path) {
    save(plane, path, |sample| 0.5 + sample * 4.0);
}

fn save(plane: &Plane, path: &Path, map: impl Fn(f32) -> f32) {
    let samples = plane
        .as_slice()
        .iter()
        .map(|&sample| (map(sample).clamp(0.0, 1.0) * 255.0) as u8)
        .collect();

    image::GrayImage::from_raw(plane.width() as u32, plane.height() as u32, samples)
        .expect("dimensions match sample count")
        .save(path)
        .expect("write png");
}
