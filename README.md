# wohnzimmer

[![Build Status](https://github.com/musikundkultur/wohnzimmer/workflows/ci/badge.svg)](https://github.com/musikundkultur/wohnzimmer/actions?query=workflow%3Aci)
[![License: Apache 2.0](https://img.shields.io/badge/License-Apache_2.0-blue.svg)](https://opensource.org/licenses/Apache-2.0)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

This repository contains the source code for the website of the Musik- und
Kulturförderverein e.V. at [musikundkultur.de](https://musikundkultur.de) /
[alhambra-luckenwalde.de](https://alhambra-luckenwalde.de).

## Release process

This project uses
[Release Please](https://github.com/googleapis/release-please) which works with
[Conventional Commit messages](https://www.conventionalcommits.org/en/v1.0.0/)
to automate the release process.

Whenever a Conventional Commit lands in the `main` branch, Release Please will
update the current Release PR with the next version bump and a new entry in the
Changelog ([here is an
example](https://github.com/musikundkultur/wohnzimmer/pull/6)).

**Please note**: Commits that do not follow the Conventional Commit
specification will not cause problems. However, they will not show up in the
[`CHANGELOG.md`](CHANGELOG.md) and will also not cause any version bumps so it
is advised to avoid these.

### Merging a Release PR

Merging a Release PR into `main` will trigger the
[`release`](.github/workflows/release.yml) workflow which performs the
following steps:

- It creates a tag and a [GitHub
  release](https://github.com/musikundkultur/wohnzimmer/releases) for the
  target version bump.
- It builds a new docker image and pushes it to
  [ghcr.io](https://github.com/musikundkultur/wohnzimmer/pkgs/container/wohnzimmer).
- The application is automatically deployed to [fly.io](https://fly.io/) using
  the new docker image.

### Manual releases

In rare cases it might be necessary to trigger a manual deployment. Given the
necessary repository permissions, the
[`release`](.github/workflows/release.yml) workflow can be triggered manually
for arbitrary branches and tags via workflow dispatch.

This will start a deployment for the commit referenced by the tag or the
branches' `HEAD` without creating a new GitHub release.

## License

The source code of wohnzimmer is licensed under either of [Apache License,
Version 2.0](LICENSE-APACHE.md) or [MIT license](LICENSE-MIT) at your option.

The Lato font is licensed under the [Open Font License](static/fonts/lato/OFL.txt).
