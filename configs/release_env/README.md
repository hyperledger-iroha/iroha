<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Release Environment Files

Store release automation secrets in this directory so `scripts/release/load_env.sh`
can hydrate the required environment variables before staging or GA publishes.

Each profile is a simple shell-style `.env` file with `KEY=VALUE` lines. Example:

```env
ANDROID_MAVEN_STAGING_USERNAME=example-user
ANDROID_MAVEN_STAGING_TOKEN=replace-me
```

Create `configs/release_env/<profile>.env` (git-ignored) or drop the file under
`$XDG_CONFIG_HOME/iroha-release/<profile>.env` and run:

```bash
eval "$(scripts/release/load_env.sh <profile>)"
```

Profiles are referenced in `docs/source/sdk/android/maven_staging_plan.md`.
