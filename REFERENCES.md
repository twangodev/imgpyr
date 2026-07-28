# References

- Burt & Adelson (1983). "The Laplacian Pyramid as a Compact Image Code."
  *IEEE Trans. Communications*, 31(4), 532–540.
  [doi:10.1109/TCOM.1983.1095851](https://doi.org/10.1109/TCOM.1983.1095851)
  — the kernel, `[1, 4, 6, 4, 1] / 16`.

- Burt & Adelson (1983). "A Multiresolution Spline with Application to Image
  Mosaics." *ACM Transactions on Graphics*, 2(4), 217–236.
  [doi:10.1145/245.247](https://doi.org/10.1145/245.247)
  — blending by scale band.

OpenCV `pyrDown`/`pyrUp` (BSD-3 header, Apache-2.0 project) is the numerical
oracle. scikit-image is not — it uses a Gaussian resize, not the 5-tap. The
`image-pyramid` crate is GPL-3.0 and unconsulted.
