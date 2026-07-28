//! Times a pyramid build at sensor resolution.
//!
//! ```text
//! cargo run --release --example bench                    # GFX 50, 51 MP
//! cargo run --release --example bench -- 16384 12288     # 200 MP
//! ```

use std::time::Instant;

use imgpyr::{Border, LaplacianPyramid, Plane};

const LEVELS: usize = 6;

fn main() {
    let mut args = std::env::args().skip(1).map(|arg| {
        arg.parse::<usize>()
            .expect("usage: bench [<width> <height>]")
    });
    let (width, height) = match (args.next(), args.next()) {
        (Some(width), Some(height)) => (width, height),
        _ => (8256, 6192),
    };

    let megapixels = (width * height) as f64 / 1e6;
    let source = Plane::from_vec(
        (0..width * height).map(|i| (i % 251) as f32 / 251.0).collect(),
        width,
        height,
    );

    let start = Instant::now();
    let pyramid = LaplacianPyramid::build(&source, LEVELS, Border::Mirror);
    let build = start.elapsed();

    let start = Instant::now();
    let restored = pyramid.collapse(Border::Mirror);
    let collapse = start.elapsed();

    let drift = restored
        .as_slice()
        .iter()
        .zip(source.as_slice())
        .map(|(a, b)| (a - b).abs())
        .fold(0.0f32, f32::max);

    println!("{width}x{height} ({megapixels:.1} MP), {LEVELS} levels, single channel");
    println!(
        "  build     {:>7.0} ms   {:>6.1} MP/s",
        build.as_secs_f64() * 1e3,
        megapixels / build.as_secs_f64()
    );
    println!(
        "  collapse  {:>7.0} ms   {:>6.1} MP/s",
        collapse.as_secs_f64() * 1e3,
        megapixels / collapse.as_secs_f64()
    );
    println!("  round trip drifts at most {drift:.2e}");
}
