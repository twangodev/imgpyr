# Changelog

## 0.1.0 (2026-07-29)


### Features

* add example writing pyramid levels as PNGs ([19a4285](https://github.com/twangodev/imgpyr/commit/19a4285312150994a22d9726f8f943a2cef2f425))
* add expand ([5a17fa0](https://github.com/twangodev/imgpyr/commit/5a17fa081cbb6132b5730b386d18ad4b320213e6))
* add Gaussian and Laplacian pyramids ([bd56182](https://github.com/twangodev/imgpyr/commit/bd56182f55cbda9141f981464dc5351bfa6c2cb9))
* add Plane and Border ([77dda2f](https://github.com/twangodev/imgpyr/commit/77dda2fd234d777922744cb57434524b77717bd7))
* add reduce ([64c5407](https://github.com/twangodev/imgpyr/commit/64c5407902395f4738b386df7ebbcd3d95036875))


### Bug Fixes

* resolve expand borders against the upsampled grid ([5ee4abd](https://github.com/twangodev/imgpyr/commit/5ee4abdffde3b656fe3b6413a2a358cfb375a94b))


### Performance Improvements

* add optional rayon feature for row-parallel passes ([46b19c1](https://github.com/twangodev/imgpyr/commit/46b19c12acb8a0d520c83c56c1be29ca0ec19c40))
* factor the kernel into separable passes ([0505845](https://github.com/twangodev/imgpyr/commit/05058451ede18936abbf0594cedc7d984df68627))
