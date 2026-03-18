---
lang: zh-hant
direction: ltr
source: docs/source/sorafs_ci_templates.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 8bf8692c60a466b4d7297c472ca4664bef7ebf94600c6690ffdbc7c94e8b34e3
source_last_modified: "2026-01-03T19:37:17.750649+00:00"
translation_last_reviewed: 2026-02-07
title: SoraFS CI Templates & Release Hooks
summary: Reference pipelines for SF-6a with Sigstore keyless signing and secure token handling.
---

# SoraFS CI Templates & Release Hooks

This guide tracks the canonical continuous-integration snippets promised by
SF-6a. The accompanying cookbook in `docs/examples/sorafs_ci.md` contains
copy‑and‑paste ready templates; the sections below explain the moving parts and
highlight the release automation hooks that downstream projects should adopt.

## GitHub Actions Reference

The `examples/sorafs_ci.md` GitHub Actions job installs the CLI, packages a CAR
archive, builds the manifest, signs it with Sigstore keyless authentication, and
verifies the resulting proof bundle—all without persisting the raw OIDC token.
Key points to keep in mind when adapting the template:

- Grant the workflow `permissions: id-token: write` so GitHub can mint an
  audience-bound OIDC token.
- Install the CLI with `cargo install --path crates/sorafs_car --features cli
  --bin sorafs_cli --force` (until the binary ships on crates.io/Homebrew).
- Call `sorafs_cli manifest sign --identity-token-provider=github-actions`
  (optionally supplying `--identity-token-audience=<value>`) to let the CLI fetch
  the token directly via `ACTIONS_ID_TOKEN_REQUEST_URL` and hash the credential
  in the bundle.
- When no audience override is supplied the CLI defaults to `sigstore`; use a
  more specific audience (for example `sorafs`) when Fulcio policies require it
  and mirror the same value in downstream verification.
- Run `sorafs_cli proof verify` before publishing so CI produces the canonical
  chunk digests alongside the manifest bundle.
- Call `sorafs_cli manifest verify-signature` after signing to ensure the
  bundle (or detached signature) matches the manifest and the freshly computed
  chunk digests before promotion.
- Use `cosign verify-blob --bundle` (installed via `sigstore/cosign-installer`)
  to give downstream consumers a deterministic verification command.
- For release branches, call `scripts/release_sorafs_cli.sh` so signing and
  verification summaries are captured automatically alongside the bundle
  artefacts. The helper accepts `--config` (see `docs/examples/sorafs_cli_release.conf`)
  and defaults to the curated fixtures in `fixtures/sorafs_manifest/ci_sample/` for
  quick dry runs.

```yaml
permissions:
  id-token: write
  contents: read
steps:
  - uses: actions/checkout@v4
  - uses: actions/setup-rust@v1
    with:
      rust-version: 1.92
  - name: Install SoraFS CLI
    run: cargo install --path crates/sorafs_car --features cli --bin sorafs_cli --force
  - name: Package payload
    run: |
      mkdir -p artifacts
      sorafs_cli car pack \
        --input payload.bin \
        --car-out artifacts/payload.car \
        --plan-out artifacts/chunk_plan.json \
        --summary-out artifacts/car_summary.json
      sorafs_cli manifest build \
        --summary artifacts/car_summary.json \
        --manifest-out artifacts/manifest.to \
        --manifest-json-out artifacts/manifest.json
  - name: Sign manifest bundle
    run: |
      sorafs_cli manifest sign \
        --manifest artifacts/manifest.to \
        --chunk-plan artifacts/chunk_plan.json \
        --bundle-out artifacts/manifest.bundle.json \
        --signature-out artifacts/manifest.sig \
        --identity-token-provider=github-actions \
        --identity-token-audience=sorafs | tee artifacts/manifest.sign.summary.json
      sorafs_cli proof verify \
        --manifest artifacts/manifest.to \
        --car artifacts/payload.car \
        --summary-out artifacts/proof.json
      sorafs_cli manifest verify-signature \
        --manifest artifacts/manifest.to \
        --bundle artifacts/manifest.bundle.json \
        --summary artifacts/car_summary.json
  - uses: sigstore/cosign-installer@v3
  - name: Verify bundle
    run: cosign verify-blob --bundle artifacts/manifest.bundle.json artifacts/manifest.to
```

## GitLab CI Reference

GitLab exposes the pipeline job token as `CI_JOB_JWT`; the template mirrors the
GitHub workflow by letting `sorafs_cli` consume the token via the default
environment variable. The stages in the cookbook separate packaging from proof
verification so projects can fan out additional checks (e.g., gateway
replay tests) without duplicating installation steps.

```yaml
stages:
  - build
  - verify

variables:
  RUST_VERSION: "1.92"

build_manifest:
  stage: build
  image: rust:${RUST_VERSION}
  before_script:
    - rustup default ${RUST_VERSION}
    - cargo install --path crates/sorafs_car --features cli --bin sorafs_cli --force
  script:
    - mkdir -p artifacts
    - sorafs_cli car pack --input payload.bin --car-out artifacts/payload.car --plan-out artifacts/chunk_plan.json --summary-out artifacts/car_summary.json
    - sorafs_cli manifest build --summary artifacts/car_summary.json --manifest-out artifacts/manifest.to --manifest-json-out artifacts/manifest.json
    - SIGSTORE_ID_TOKEN="$CI_JOB_JWT" sorafs_cli manifest sign --manifest artifacts/manifest.to --chunk-plan artifacts/chunk_plan.json --bundle-out artifacts/manifest.bundle.json --signature-out artifacts/manifest.sig | tee artifacts/manifest.sign.summary.json
    - sorafs_cli proof verify --manifest artifacts/manifest.to --car artifacts/payload.car --summary-out artifacts/proof.json
    - sorafs_cli manifest verify-signature --manifest artifacts/manifest.to --bundle artifacts/manifest.bundle.json --summary artifacts/car_summary.json
  artifacts:
    paths:
      - artifacts/

verify_bundle:
  stage: verify
  image: ghcr.io/sigstore/cosign/cosign:v2.2.4
  needs: [build_manifest]
  script:
    - cosign verify-blob --bundle artifacts/manifest.bundle.json artifacts/manifest.to
```

## Secure Credential Handling

- **Rely on hashed bundles:** `manifest bundle` JSON stores the BLAKE3 hash of
  the OIDC token and its source label. Only enable `--include-token=true` for
  air-gapped verification, and scrub the resulting artefact immediately after
  use.
- **Leave tokens in environment variables:** pass credentials via
  `SIGSTORE_ID_TOKEN` (or explicit `--identity-token-env`). Never echo the token
  to logs and prefer masked secrets when CI exposes audit views.
- **Scope the OIDC audience:** configure the OIDC step to request the narrowest
  audience accepted by Fulcio so bundles remain tied to a specific workflow.
- **Rotate bundles on retry:** if a job retries after the token expires, let the
  CLI derive a new key pair; bundles embed the issuance timestamp so downstream
  attestations remain auditable.

## Release Hooks

Run these hooks as soon as the release gate succeeds so every promotion leaves
the same audit trail.

1. **Create and push the tag.** After the checklist passes, cut a signed tag for
   the CLI (and matching SDK crates when they ship together) and push it
   upstream so auditors can anchor the commit:
   ```bash
   git tag -s sorafs-cli-vX.Y.Z -m "SoraFS CLI & SDK vX.Y.Z"
   git push origin sorafs-cli-vX.Y.Z
   ```
   Repeat the tagging step (`sorafs-sdk-vX.Y.Z`, etc.) for any sibling
   repositories that release in lock-step.
2. **Re-sign the manifest with the helper script.** From the workspace root run:
   ```bash
   scripts/release_sorafs_cli.sh \
     --manifest artifacts/manifest.to \
     --chunk-plan artifacts/chunk_plan.json \
     --chunk-summary artifacts/car_summary.json \
     --identity-token-provider=github-actions \
     --identity-token-audience=sorafs
   ```
  The helper rebuilds `sorafs_cli` if required, re-signs the manifest, and
  writes the evidence bundle to `artifacts/sorafs_cli_release/`
  (`manifest.bundle.json`, `manifest.sig`, `manifest.sign.summary.json`, and
  `manifest.verify.summary.json`). Review the summaries before continuing; the
  `sorafs-cli-fixture` workflow runs the same script against the canonical
  fixtures on every push/PR. Kick the workflow after you update the fixtures to
  double-check the diff gate:
  ```bash
  gh workflow run sorafs-cli-fixture --ref <branch>
  gh run watch -w sorafs-cli-fixture
  ```
3. **Refresh deterministic fixtures.** When hashes change, copy the refreshed
   artefacts into the canonical fixture directory so downstream CI jobs remain
   byte-for-byte reproducible:
   ```bash
   cp artifacts/sorafs_cli_release/manifest.bundle.json fixtures/sorafs_manifest/ci_sample/manifest.bundle.json
   cp artifacts/sorafs_cli_release/manifest.sig fixtures/sorafs_manifest/ci_sample/manifest.sig
  cp artifacts/sorafs_cli_release/manifest.sign.summary.json fixtures/sorafs_manifest/ci_sample/manifest.sign.summary.json
  cp artifacts/sorafs_cli_release/manifest.verify.summary.json fixtures/sorafs_manifest/ci_sample/manifest.verify.summary.json
   ```
   Re-run `cargo test -p sorafs_car --features cli` to ensure the fixture check
   stays green. Update `docs/examples/sorafs_ci_sample/manifest.template.json`
   if any digests move.
4. **Commit fixture and doc updates.**
   ```bash
   git add fixtures/sorafs_manifest/ci_sample docs/examples/sorafs_ci_sample
   git commit -m "Refresh SoraFS CI fixtures for vX.Y.Z"
   git push origin HEAD
   ```
5. **Archive the governance evidence bundle.** Package the artefacts the council
   expects for verification and store the tarball in your release bucket:
   ```bash
  tar -czf artifacts/sorafs_cli_release/sorafs-cli-vX.Y.Z-governance.tgz \
    -C artifacts/sorafs_cli_release manifest.bundle.json manifest.sig \
    manifest.sign.summary.json
   ```
   Record the tarball’s SHA256 digest; you will include it in the governance
   ticket.
6. **Notify governance.** Open the governance release ticket (see
   `docs/source/sorafs/developer/deployment.md`) with:
   - the tag name and commit hash,
   - a link to the published release notes,
   - SHA256 digests for `manifest.bundle.json`, `manifest.sig`, and the
     governance tarball, and
   - pointers to the refreshed fixtures so peers can diff their builds.
   Attach the tarball or upload it to the evidence bucket before closing the
   ticket.
7. **Update downstream docs when flags change.** If new CLI flags, outputs, or
   environment variables were introduced, refresh `docs/examples/sorafs_ci.md`
   and cross-link any new options from the quickstart material.

## Sample Manifests & Fixtures

Roadmap item SF-6a ships a deterministic fixture bundle under
`fixtures/sorafs_manifest/ci_sample`. The directory runs the sample payload
(`payload.txt`) through the CLI so every artefact referenced by this guide is
versioned: CAR archive, chunk plan, CAR summary, manifest (`.to` + JSON), PoR
summary, keyless signature bundle, detached signature, and the manifest-sign
summary captured from STDOUT. Paths remain relative to the repository root so
byte-for-byte comparisons survive across runners.

Regenerate the fixtures with:

```sh
cargo run -p sorafs_car --features cli --bin sorafs_cli -- \
  car pack \
  --input=fixtures/sorafs_manifest/ci_sample/payload.txt \
  --car-out=fixtures/sorafs_manifest/ci_sample/payload.car \
  --plan-out=fixtures/sorafs_manifest/ci_sample/chunk_plan.json \
  --summary-out=fixtures/sorafs_manifest/ci_sample/car_summary.json

cargo run -p sorafs_car --features cli --bin sorafs_cli -- \
  manifest build \
  --summary=fixtures/sorafs_manifest/ci_sample/car_summary.json \
  --manifest-out=fixtures/sorafs_manifest/ci_sample/manifest.to \
  --manifest-json-out=fixtures/sorafs_manifest/ci_sample/manifest.json

cargo run -p sorafs_car --features cli --bin sorafs_cli -- \
  proof verify \
  --manifest=fixtures/sorafs_manifest/ci_sample/manifest.to \
  --car=fixtures/sorafs_manifest/ci_sample/payload.car \
  --summary-out=fixtures/sorafs_manifest/ci_sample/proof.json

cargo run -p sorafs_car --features cli --bin sorafs_cli -- \
  manifest sign \
  --manifest=fixtures/sorafs_manifest/ci_sample/manifest.to \
  --summary=fixtures/sorafs_manifest/ci_sample/car_summary.json \
  --chunk-plan=fixtures/sorafs_manifest/ci_sample/chunk_plan.json \
  --bundle-out=fixtures/sorafs_manifest/ci_sample/manifest.bundle.json \
  --signature-out=fixtures/sorafs_manifest/ci_sample/manifest.sig \
  --identity-token='eyJhbGciOiJFZERTQSIsInR5cCI6IkpXVCJ9.eyJpc3MiOiJodHRwczovL2ZpeHR1cmUuZXhhbXBsZS9pc3N1ZXIiLCJzdWIiOiJjaS1maXh0dXJlIiwiYXVkIjoic29yYWZzIiwiZXhwIjoxNzAwMDAzNjAwLCJpYXQiOjE3MDAwMDAwMDAsImp0aSI6ImNpLXNhbXBsZS10b2tlbi0wMDEifQ.c2FtcGxlc2lnbmF0dXJl' \
  --issued-at=1700000000 \
> fixtures/sorafs_manifest/ci_sample/manifest.sign.summary.json

cargo run -p sorafs_car --features cli --bin sorafs_cli -- \
  manifest verify-signature \
  --manifest=fixtures/sorafs_manifest/ci_sample/manifest.to \
  --bundle=fixtures/sorafs_manifest/ci_sample/manifest.bundle.json \
  --summary=fixtures/sorafs_manifest/ci_sample/car_summary.json \
  --chunk-plan=fixtures/sorafs_manifest/ci_sample/chunk_plan.json \
  --expect-token-hash=7b56598bca4584a5f5631ce4e510b8c55bd9379799f231db2a3476774f45722b \
> fixtures/sorafs_manifest/ci_sample/manifest.verify.summary.json
```

Downstream CI can diff their freshly generated artefacts against the canonical
bundle whenever they need golden data:

```sh
diff -u fixtures/sorafs_manifest/ci_sample/car_summary.json artifacts/car_summary.json
diff -u fixtures/sorafs_manifest/ci_sample/chunk_plan.json artifacts/chunk_plan.json
diff -u fixtures/sorafs_manifest/ci_sample/manifest.sign.summary.json artifacts/manifest.sign.summary.json
diff -u fixtures/sorafs_manifest/ci_sample/manifest.verify.summary.json artifacts/manifest.verify.summary.json
diff -u fixtures/sorafs_manifest/ci_sample/proof.json artifacts/proof.json
```

The integration test `ci_sample_fixtures_are_consistent` in
`crates/sorafs_car/tests/sorafs_cli.rs` keeps these artefacts aligned with the
manifest, signature bundle, and proof summary.

See `fixtures/sorafs_manifest/ci_sample/README.md` for regeneration commands and
`docs/examples/sorafs_ci_sample/README.md` for templating tips and a miniature
repository layout that mirrors the CI cookbook.

## Optional Add-ons

- **Gateway smoke tests** – once SF-5d fixtures land, add a `gateway-smoke`
  job that replays capability/refusal bundles against staging gateways.
- **Governance announcements** – add a job that uploads the evidence tarball in
  `artifacts/sorafs_cli_release/` to the governance release tracker (or evidence
  bucket) after the manual checklist completes.
