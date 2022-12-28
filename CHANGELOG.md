# Changelog

## [0.2.2](https://github.com/musikundkultur/wohnzimmer/compare/v0.2.1...v0.2.2) (2022-12-28)


### Miscellaneous

* add events for Q1 2023 ([#16](https://github.com/musikundkultur/wohnzimmer/issues/16)) ([2c36bda](https://github.com/musikundkultur/wohnzimmer/commit/2c36bdaa4cba563ca763d7fc9ff3b53b238e2d56))

## [0.2.1](https://github.com/musikundkultur/wohnzimmer/compare/v0.2.0...v0.2.1) (2022-12-22)


### Bug Fixes

* **security:** remove dependency on vulnerable version of time package ([3e083f3](https://github.com/musikundkultur/wohnzimmer/commit/3e083f35e78138e48b41ef297e93cd8189d3aad2))


### Miscellaneous

* **docs:** add paragraph about deployment from local machine ([f3662fc](https://github.com/musikundkultur/wohnzimmer/commit/f3662fcf12a7fae2741cb25cfb40ae8a1caac098))

## [0.2.0](https://github.com/musikundkultur/wohnzimmer/compare/v0.1.3...v0.2.0) (2022-12-18)


### Features

* add hierarchical application config ([#10](https://github.com/musikundkultur/wohnzimmer/issues/10)) ([5fa8652](https://github.com/musikundkultur/wohnzimmer/commit/5fa865217a5caff89e2514eba41c839edd68b42d))
* **calendar:** add `EventSource` abstraction ([#12](https://github.com/musikundkultur/wohnzimmer/issues/12)) ([6b4540e](https://github.com/musikundkultur/wohnzimmer/commit/6b4540ee1e69def10dd1261040ed9be407507e2e))


### Bug Fixes

* replace incorrect `.map_err()` with `match` ([dcb71ea](https://github.com/musikundkultur/wohnzimmer/commit/dcb71ea8f32ef4e9d095e1c4c175abe1758044e4))


### Miscellaneous

* add support for canonical `link` tag ([#13](https://github.com/musikundkultur/wohnzimmer/issues/13)) ([61f261c](https://github.com/musikundkultur/wohnzimmer/commit/61f261c081c7c9d757428fc47a64243176df1601))
* add support for canonical `link` tag ([#13](https://github.com/musikundkultur/wohnzimmer/issues/13)) ([2823048](https://github.com/musikundkultur/wohnzimmer/commit/28230485009b0266cb9f5c9d0497400b141e4de6))

## [0.1.3](https://github.com/musikundkultur/wohnzimmer/compare/v0.1.2...v0.1.3) (2022-12-16)


### Bug Fixes

* **meta:** correct team reference in `CODEOWNERS` ([#8](https://github.com/musikundkultur/wohnzimmer/issues/8)) ([db3e8ac](https://github.com/musikundkultur/wohnzimmer/commit/db3e8ac80293e839d3fd24f639cf18fb0e2f4615))


### Miscellaneous

* **docs:** document release process ([471cc4e](https://github.com/musikundkultur/wohnzimmer/commit/471cc4edab464babfb1cd25acc95fa8ff09e86d7))

## [0.1.2](https://github.com/musikundkultur/wohnzimmer/compare/v0.1.1...v0.1.2) (2022-12-15)


### Miscellaneous

* **build:** use `cargo-chef` to enable caching in docker builds ([#4](https://github.com/musikundkultur/wohnzimmer/issues/4)) ([ba60754](https://github.com/musikundkultur/wohnzimmer/commit/ba607541c6a70e62595a654bb4a5609559b95953))
* **release:** build and push docker image ([#7](https://github.com/musikundkultur/wohnzimmer/issues/7)) ([96ebb30](https://github.com/musikundkultur/wohnzimmer/commit/96ebb30ec2e973c4343eebc64c69f43317042645))

## [0.1.1](https://github.com/musikundkultur/wohnzimmer/compare/v0.1.0...v0.1.1) (2022-12-07)


### Bug Fixes

* **seo:** add meta description ([79ac003](https://github.com/musikundkultur/wohnzimmer/commit/79ac003871c4fac553048532876dcf5f0f277af1))


### Miscellaneous

* **deps:** bump actions/cache from 3.0.8 to 3.0.11 ([#1](https://github.com/musikundkultur/wohnzimmer/issues/1)) ([26778e9](https://github.com/musikundkultur/wohnzimmer/commit/26778e9b48f975439cba69a91ef6d8088479c60c))

## 0.1.0 (2022-12-07)


### Features

* add `Dockerfile` ([b277a53](https://github.com/musikundkultur/wohnzimmer/commit/b277a53bf87976de8173d5fb51da283a38aaf99d))
* use `clap` to parse command-line flags ([0465bd6](https://github.com/musikundkultur/wohnzimmer/commit/0465bd6e9e9362ca7d085dcccd5bb1cc5132139c))


### Bug Fixes

* correct build badge url ([ee73f8d](https://github.com/musikundkultur/wohnzimmer/commit/ee73f8d063408d0cc224492ef1acbaf64e53f90f))
* remove misplaced `config.toml` ([666e81f](https://github.com/musikundkultur/wohnzimmer/commit/666e81ffc7bd39fdcafdd23708c9c96f752c6e4e))


### Miscellaneous

* add `fly.toml` and release workflow ([71a3c16](https://github.com/musikundkultur/wohnzimmer/commit/71a3c16b3f2bb9ae931cafe8382ab4a0ae6e2b8d))
* add `release-please` to `release` workflow ([dcb5454](https://github.com/musikundkultur/wohnzimmer/commit/dcb5454acbd5683fc210416d8e1e5c9767054b2a))
* add favicons for all platforms ([258cc22](https://github.com/musikundkultur/wohnzimmer/commit/258cc22d2847443eb5b2a231505184d0be062ca7))
* add images and handler for static files ([4381d8e](https://github.com/musikundkultur/wohnzimmer/commit/4381d8eab2b6fcfd31d4d1639e3ceecc6102740f))
* add webserver boilerplate ([a764f9c](https://github.com/musikundkultur/wohnzimmer/commit/a764f9cf45b7a9f00d1a3c803134fcb350112393))
* import current page content and style ([0978788](https://github.com/musikundkultur/wohnzimmer/commit/0978788b47c0b9605236700dc8eaa01440991025))
* initial commit ([9e30e8e](https://github.com/musikundkultur/wohnzimmer/commit/9e30e8e11fac7bd2da321cd926f085a9dfd955f2))
