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

let sharpened = pyramid.collapse(Border::Mirror);
```

Each band holds only the detail at its scale; `collapse` sums them back. Left
alone it returns the input to within one float ULP.

Planes are single channel, so call three times for RGB.

```sh
cargo run --example pyramid     # writes every level to target/
```

The `rayon` feature parallelises the row passes, about 4x.

## Acknowledgements

Burt, P. J. and Adelson, E. H. (1983). "The Laplacian Pyramid as a Compact
Image Code." *IEEE Transactions on Communications*, 31(4), 532-540.
[doi:10.1109/TCOM.1983.1095851](https://doi.org/10.1109/TCOM.1983.1095851)

OpenCV's `pyrDown` and `pyrUp`, whose kernel and border conventions this
follows and whose output the test fixtures are recorded from.
