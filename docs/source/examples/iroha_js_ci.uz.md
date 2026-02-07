---
lang: uz
direction: ltr
source: docs/source/examples/iroha_js_ci.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: d21afd33da5ee459b1f6ffb6ac7c42adc0852ed7929e69993f81914637b5e6b5
source_last_modified: "2025-12-29T18:16:35.953373+00:00"
translation_last_reviewed: 2026-02-07
---

<!--
  SPDX-License-Identifier: Apache-2.0
  This document provides guidance for running @iroha/iroha-js in CI systems.
-->

# Iroha JS CI Reference

The `@iroha/iroha-js` package bundles native bindings via `iroha_js_host`. Any
CI pipeline that executes tests or builds must provide both a Node.js runtime
and the Rust toolchain so the native bundle can be compiled before the tests
run.

## Recommended Steps

1. Use a Node LTS release (18 or 20) via `actions/setup-node` or your CI
   equivalent.
2. Install the Rust toolchain listed in `rust-toolchain.toml`. We recommend
   `dtolnay/rust-toolchain@v1` in GitHub Actions.
3. Cache the cargo registry/git indexes and the `target/` directory to avoid
   rebuilding the native addon in every job.
4. Run `npm install`, then `npm run lint:test`. The combined script enforces
   ESLint with zero warnings, builds the native addon, and runs the Node test
   suite so CI matches the release gating workflow.
5. Optionally run `node --test` as a fast smoke step once `npm run build:native`
   has produced the addon (for example, presubmit quick-check lanes that reuse
   cached artifacts).
6. Layer any additional linting or formatting checks from your consumer
   project on top of `npm run lint:test` when stricter policies are required.
7. When sharing configuration across services, load `iroha_config` and pass the
   parsed document to `resolveToriiClientConfig({ config })` so Node clients
   reuse the same timeout/retry/token policy as the rest of the deployment (see
   `docs/source/sdk/js/quickstart.md` for a full example).

## GitHub Actions Template

```yaml
name: iroha-js-ci

on:
  push:
    branches: [ main ]
  pull_request:

jobs:
  build:
    runs-on: ubuntu-latest
    strategy:
      fail-fast: false
      matrix:
        node-version: [18, 20]
    steps:
      - uses: actions/checkout@v4

      - name: Set up Node.js
        uses: actions/setup-node@v4
        with:
          node-version: ${{ matrix.node-version }}
          cache: npm

      - name: Set up Rust toolchain
        uses: dtolnay/rust-toolchain@v1
        with:
          toolchain: stable

      - name: Cache cargo artifacts
        uses: actions/cache@v4
        with:
          path: |
            ~/.cargo/registry
            ~/.cargo/git
            target
          key: ${{ runner.os }}-cargo-${{ hashFiles('**/Cargo.lock') }}

      - run: npm install
      - run: npm run lint:test
```

## Fast Smoke Job (Optional)

For pull requests that only touch documentation or TypeScript definitions, a
minimal job can reuse cached artifacts, rebuild the native module, and run the
Node test runner directly:

```yaml
jobs:
  smoke:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: actions/setup-node@v4
        with:
          node-version: 20
      - uses: dtolnay/rust-toolchain@v1
        with:
          toolchain: stable
      - run: npm ci
      - run: npm run build:native
      - run: node --test
```

This job completes quickly while still verifying that the native addon compiles
and that the Node test suite passes.

> **Reference implementation:** the repository includes
> `.github/workflows/javascript-sdk.yml`, which wires the steps above into a
> Node 18/20 matrix with cargo caching.
