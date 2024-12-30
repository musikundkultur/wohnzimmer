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

### Google Calendar Integration

Upcoming events can be pulled from a Google Calendar. To enable the
integration, enable the `google-calendar` event source in the configuration:

```toml
[calendar]
event_source = "google-calendar"
```

Furthermore, the following two environment variables need to be set to
appropriate values:

- `GOOGLE_CALENDAR_ID`: The ID of the calendar. You can find it in the
  **Integrate calendar** section of your calendar's settings.
- `GOOGLE_APPLICATION_CREDENTIALS_JSON`: Populate it with the base64 encoded
  JSON Service Account Credentials obtained from the Google Cloud Console (see
  below).

#### Calendar Setup

1. Create a new project in the [Google Cloud
   Console](https://console.cloud.google.com/) or use an existing one.
2. Enable the Google Calendar API via the [API
   Library](https://console.cloud.google.com/apis/library).
3. Create a new Service Account via the [IAM
   Console](https://console.cloud.google.com/iam-admin/serviceaccounts).
4. Create new credentials for this Service Account by navigating to the
   **Keys** tab and then select **Add key** => **Create new key** => Choose
   **JSON** as key type. Save the downloaded JSON credentials file.
5. Store the credentials in the `GOOGLE_APPLICATION_CREDENTIALS_JSON`
   environment variable:

   ```sh
   export GOOGLE_APPLICATION_CREDENTIALS_JSON="$(base64 --wrap=0 credentials.json)"
   ```
6. Navigate to your Google Calendar's settings and share it with the Service
   Account's email address. Read permissions are sufficient for the integration
   to work.

## Release process

> [!NOTE]
> Until version
> [v0.7.1](https://github.com/musikundkultur/wohnzimmer/releases/tag/v0.7.1) we
> used [Release Please](https://github.com/googleapis/release-please) to create
> releases based on [Conventional Commit
> messages](https://www.conventionalcommits.org/en/v1.0.0/) to automate the
> release process. The release process was simplified and as a consequence
> there are no automated updates to [`CHANGELOG.md`](CHANGELOG.md) anymore.

The [`container`](.github/workflows/container.yml) workflow automatically
deploys container images build off of Git tags and the `main` branch directly
to production.

### Manual releases

In rare cases it might be necessary to trigger a manual deployment. Given the
necessary repository permissions, the
[`container`](.github/workflows/container.yml) workflow can be triggered manually
for arbitrary branches and tags via workflow dispatch.

This will build the image and start a deployment for the commit referenced by
the tag or the branches' `HEAD`.

Alternatively, the application can be built and deployed from a local machine
via [`flyctl`](https://github.com/superfly/flyctl):

```sh
flyctl deploy
```

## License

The source code of wohnzimmer is licensed under either of [Apache License,
Version 2.0](LICENSE-APACHE) or [MIT license](LICENSE-MIT) at your option.

The Lato font is licensed under the [Open Font License](static/fonts/lato/OFL.txt).
