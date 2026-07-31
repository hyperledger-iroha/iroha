<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Python SDK Release Automation (PY6-P3)

Roadmap item **PY6-P3 — CI/type-check automation & support policy** also calls
for deterministic release automation so the Python SDK can ship wheels with
reproducible evidence. This guide documents the release checklist, the
`release_smoke.sh` harness, and the artefact bundle reviewers expect before any
PyPI upload. Production authenticity comes only from the externally signed
aggregate release manifest described below.

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
inside a clean virtual environment, executes the Norito RPC smoke test, checks
the package with `twine`, and performs a dry-run upload. It accepts no signing,
provenance, key, or manifest-output options and produces no signature or
release-authentication record.

Example invocation:

```bash
PYTHON_RELEASE_SMOKE_KEEP_DIST=1 \
  python/iroha_python/scripts/release_smoke.sh
```

`PYTHON_RELEASE_SMOKE_KEEP_DIST=1` is the only harness-specific environment
knob; it preserves `dist/` for the protected release job. Without it, the
harness removes `dist/` on exit.

## 3. Authenticate the reviewed candidate externally

1. Preserve the smoke transcript and stage the exact wheel/source-distribution
   bytes into the protected release candidate inventory.
2. Generate and independently review the package checksums in that workflow.
   Do not treat the smoke harness as an evidence or manifest generator.
3. Bind the reviewed package inventory into the canonical aggregate release
   manifest. The protected signer produces the raw 64-byte Ed25519 signature
   outside the repository through PKCS#11/HSM policy.
4. Verify the public tuple before upload:

   ```bash
   python3 scripts/release_manifest_signing.py verify \
     --manifest release_manifest.json \
     --signature release_manifest.json.sig \
     --public-key release_manifest.json.pub \
     --trusted-signing-fingerprint "$TRUSTED_SIGNING_FINGERPRINT" \
     --release-manifest-verifier /opt/iroha/bin/sorafs-validate \
     --trusted-release-manifest-verifier-sha256 \
       "$TRUSTED_RELEASE_MANIFEST_VERIFIER_SHA256"
   ```

   OIDC/cosign provenance may be attached later by the protected release
   workflow, but cannot substitute for this check and is never produced by the
   smoke harness.

## 4. Final upload

After the smoke run passes:

1. Complete the protected external-Ed25519 authentication step above.
2. Upload the exact authenticated wheel and source distribution to PyPI:

   ```bash
   cd python/iroha_python
   python -m twine upload dist/*
   ```

3. Attach the smoke transcript, reviewed package checksum inventory, and
   authenticated aggregate-manifest tuple to the release ticket.

The release smoke script can be wired into CI (nightly or per-release branches)
to prove build/install/import/package-metadata automation stays healthy. It must
not receive release signing authority.
