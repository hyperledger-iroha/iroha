---
lang: hy
direction: ltr
source: docs/source/sdk/python/release_automation.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 82a9f5b1e728031fa2f017c4ccf03ddf7a5a8b2b80a89d934562169414b20c1e
source_last_modified: "2025-12-29T18:16:36.064895+00:00"
translation_last_reviewed: 2026-02-07
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Python SDK Release Automation (PY6-P3)

Roadmap item **PY6-P3 — CI/type-check automation & support policy** also calls
for deterministic release automation so the Python SDK can ship signed wheels
with reproducible evidence. This guide documents the release checklist, the
`release_smoke.sh` harness, and the artefact bundle reviewers expect before any
PyPI upload.

## 1. Pre-flight checks

1. Ensure local sources are clean and rebased on the release branch.
2. Run the repository-wide checks so the SDK stays aligned with the other
   crates and fixtures:

   ```bash
   make python-checks
   ```

3. Capture the `git status` and `git rev-parse HEAD` output for the release
   evidence bundle.

## 2. Run the release smoke harness

`python/iroha_python/scripts/release_smoke.sh` builds the wheel, installs it
inside a clean virtual environment, executes the Norito RPC smoke test, and
performs a dry-run upload via `twine`. The script also emits deterministic
artefacts:

- `dist/release_artifacts.json` — manifest describing the wheel, changelog,
  and checksum bundle (path configurable via `--manifest-out`).
- `dist/SHA256SUMS` — hashes covering the wheel + changelog preview.
- `{wheel}.sigstore` / `{wheel}.sigstore.sig` — Sigstore bundle and detached
  signature produced via `cosign sign-blob` when the `cosign` binary and an
  identity token are available. Pass `--require-sigstore` (or set
  `PYTHON_RELEASE_SIGSTORE_MODE=require`) to fail the run if Sigstore signing
  is unavailable, or `--skip-sigstore` / `PYTHON_RELEASE_SIGSTORE_MODE=skip`
  for local smoke runs without OIDC access. Override the token source with
  `--sigstore-token-env` (defaults to `SIGSTORE_ID_TOKEN` and can be exported
  via CI’s workload identity helper).
- `{wheel}.sig` / `{wheel}.pub` — detached signature and public key when
  OpenSSL is available (override the key with `--signing-key` or export one via
  `PYTHON_RELEASE_SIGNING_KEY`).
- `dist/CHANGELOG_PREVIEW.md` — commits since the latest tag.

Example invocation:

```bash
python/iroha_python/scripts/release_smoke.sh \
  --signing-key /secure/path/release-key.pem \
  --manifest-out artifacts/python_release_manifest.json \
  --require-sigstore
```

Environment knobs:

- `PYTHON_RELEASE_SMOKE_KEEP_DIST=1` keeps the `dist/` directory for inspection.
- `PYTHON_RELEASE_SIGNING_KEY` points to an existing private key. When omitted,
  the script generates a temporary RSA key (recorded in the manifest).
- `PYTHON_RELEASE_SIGSTORE_MODE` controls Sigstore behaviour (`auto` by
  default, `require` or `skip` mirror the CLI flags).
- `PYTHON_RELEASE_SIGSTORE_TOKEN_ENV` names the environment variable containing
  the Sigstore OIDC token (defaults to `SIGSTORE_ID_TOKEN`).
- `PYTHON_RELEASE_SIGSTORE_COSIGN_BIN` (or `--cosign-bin`) selects the `cosign`
  binary when the default `cosign` on `PATH` is unsuitable.

## 3. Review the manifest + artefacts

1. Inspect `release_artifacts.json` to confirm:
   - The reported version matches the target tag.
   - `wheel.sha256` is populated and the `ephemeral_signature` flag is `false`
     when using a production key.
   - The `sigstore.status` field is `signed` (or `disabled` when you purposely
     skipped signing) and the bundle/signature paths match the exported files.
2. Verify `SHA256SUMS` against the wheel and changelog files:

   ```bash
   (cd python/iroha_python/dist && sha256sum -c SHA256SUMS)
   ```

3. When Sigstore signing ran, validate the bundle:

   ```bash
   WHEEL_NAME=$(basename python/iroha_python/dist/*.whl)
   COSIGN_EXPERIMENTAL=1 cosign verify-blob \
     --bundle "python/iroha_python/dist/${WHEEL_NAME}.sigstore" \
     "python/iroha_python/dist/${WHEEL_NAME}"
   ```

4. Store the manifest, checksum file, changelog preview, Sigstore bundle, and
   detached signatures alongside the release request (governance and DevRel
   expect these artefacts when reviewing PY6 deliverables).

## 4. Final upload

After the smoke run passes:

1. Push the release tag (e.g., `git tag -s python-vX.Y.Z && git push origin python-vX.Y.Z`).
2. Upload the wheel and sdist to PyPI:

   ```bash
   cd python/iroha_python
   python -m twine upload dist/*
   ```

3. Attach `release_artifacts.json`, `SHA256SUMS`, and `CHANGELOG_PREVIEW.md` to
   the release ticket so auditors can cross-check the evidence without running
   the tooling locally.

The release smoke script can be wired into CI (nightly or per-release branches)
to prove the automation stays healthy, satisfying the PY6-P3 “release automation”
gate while keeping reproducibility evidence close to the artefacts.
