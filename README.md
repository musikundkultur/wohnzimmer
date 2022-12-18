# wohnzimmer

[![Build Status](https://github.com/musikundkultur/wohnzimmer/workflows/ci/badge.svg)](https://github.com/musikundkultur/wohnzimmer/actions?query=workflow%3Aci)
[![License: Apache 2.0](https://img.shields.io/badge/License-Apache_2.0-blue.svg)](https://opensource.org/licenses/Apache-2.0)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

This repository contains the source code for the website of the Musik- und
Kulturförderverein e.V. at [musikundkultur.de](https://musikundkultur.de) /
[alhambra-luckenwalde.de](https://alhambra-luckenwalde.de).

## Configuration

Configuration is loaded from multiple places in the following order:

1. The file `config/default.toml` is always loaded.

2. If present, the environment specific file `config/{environment}.toml` is
   loaded based on the value of the `APP_ENV` environment variable, which
   defaults to `development`.

3. If present, the file `config/local.toml` can be created to override certain
   configuration values locally (it's on `.gitignore`).

4. Finally, environment variables (with optional `WZ_` prefix) can be set to
   override any configuration value. E.g. the config `server.listen_addr` can
   be set via either of these environment variables: `SERVER__LISTEN_ADDR` /
   `WZ_SERVER__LISTEN_ADDR`.

   **Note**: A double-underscore (`__`) is used as path separator for nested
   configuration attributes.

### Logging configuration

The log level can be configured via the `RUST_LOG` environment variable.
Examples can be found in then [`env_logger`
documentation](https://docs.rs/env_logger/latest/env_logger/).

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

Alternatively, the application can be built and deployed from a local machine
via [`flyctl`](https://github.com/superfly/flyctl):

```sh
flyctl deploy
```

## License

The source code of wohnzimmer is licensed under either of [Apache License,
Version 2.0](LICENSE-APACHE.md) or [MIT license](LICENSE-MIT) at your option.

The Lato font is licensed under the [Open Font License](static/fonts/lato/OFL.txt).
