# imgpyr

Gaussian and Laplacian image pyramids.

```rust
use imgpyr::{Border, LaplacianPyramid, Plane};

let mut pyramid = LaplacianPyramid::build(&plane, 5, Border::Mirror);

for index in 0..pyramid.len() {
    for sample in pyramid.band_mut(index).as_mut_slice() {
        *sample *= 1.5;
    }
}

let sharpened = pyramid.collapse();
```

Each band holds only the detail at its scale; `collapse` sums them back. Left
alone it reconstructs the input to within a few times `f32::EPSILON` of the
plane's peak magnitude.

Planes are single channel, so call three times for RGB.

```sh
cargo run --example pyramid     # writes every level to target/
```

The `rayon` feature parallelises the row passes: 4.4x on 24 cores at 51 MP.
The work is memory bound, so more cores buy less than you would expect.

## Acknowledgements

Burt, P. J. and Adelson, E. H. (1983). "The Laplacian Pyramid as a Compact
Image Code." *IEEE Transactions on Communications*, 31(4), 532-540.
[doi:10.1109/TCOM.1983.1095851](https://doi.org/10.1109/TCOM.1983.1095851)

Burt, P. J. and Adelson, E. H. (1983). "A Multiresolution Spline with
Application to Image Mosaics." *ACM Transactions on Graphics*, 2(4), 217-236.
[doi:10.1145/245.247](https://doi.org/10.1145/245.247)

OpenCV's `pyrDown` and `pyrUp`, whose kernel and border conventions this
follows and whose output the test fixtures are recorded from.
