---
lang: fr
direction: ltr
source: docs/examples/sorafs_ci.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 61536f7f6dbecb2658cb5e07a15ed99888957bf87b473e0089e38f8431156c18
source_last_modified: "2025-11-08T09:36:10.010765+00:00"
translation_last_reviewed: 2026-01-01
---

---
title: Guide CI SoraFS
summary: Workflow GitHub Actions regroupant les etapes de signature et verification avec notes de revue.
---

# Guide CI SoraFS

Cet extrait reprend les indications de `docs/source/sorafs_ci_templates.md` et montre comment integrer
la signature, la verification et les checks de proof dans un seul job GitHub Actions.

```yaml
name: sorafs-cli-release

on:
  push:
    branches: [main]

permissions:
  contents: read
  id-token: write

jobs:
  release:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: actions/setup-rust@v1
        with:
          rust-version: 1.92

      - name: Package payload
        run: |
          mkdir -p artifacts
          sorafs_cli car pack             --input payload.bin             --car-out artifacts/payload.car             --plan-out artifacts/chunk_plan.json             --summary-out artifacts/car_summary.json
          sorafs_cli manifest build             --summary artifacts/car_summary.json             --manifest-out artifacts/manifest.to

      - name: Sign manifest bundle
        run: |
          sorafs_cli manifest sign             --manifest artifacts/manifest.to             --chunk-plan artifacts/chunk_plan.json             --bundle-out artifacts/manifest.bundle.json             --signature-out artifacts/manifest.sig             --identity-token-provider=github-actions             --identity-token-audience=sorafs | tee artifacts/manifest.sign.summary.json

      - name: Verify manifest bundle
        run: |
          sorafs_cli manifest verify-signature             --manifest artifacts/manifest.to             --bundle artifacts/manifest.bundle.json             --summary artifacts/car_summary.json

      - name: Proof verification
        run: |
          sorafs_cli proof verify             --manifest artifacts/manifest.to             --car artifacts/payload.car             --summary-out artifacts/proof.json

      - uses: sigstore/cosign-installer@v3
      - name: Verify bundle with cosign
        run: cosign verify-blob --bundle artifacts/manifest.bundle.json artifacts/manifest.to
```

## Notes

- `sorafs_cli` doit etre disponible sur le runner (p. ex., `cargo install --path crates/sorafs_car --features cli` avant ces etapes).
- Le workflow doit fournir un audience OIDC explicite (ici `sorafs`); ajustez `--identity-token-audience` pour correspondre a votre politique Fulcio.
- Le pipeline de release doit archiver `artifacts/manifest.bundle.json`, `artifacts/manifest.sig` et `artifacts/proof.json` pour la revue de governance.
- Des artefacts deterministes d'exemple vivent dans `fixtures/sorafs_manifest/ci_sample`; copiez-les dans les tests quand vous avez besoin de manifests, chunk plans ou bundle JSON golden sans recalculer le pipeline.

## Verification des fixtures

Les artefacts deterministes pour ce workflow vivent sous
`fixtures/sorafs_manifest/ci_sample`. Les pipelines peuvent rejouer les etapes ci-dessus et
comparer leurs sorties aux fichiers canoniques, par exemple:

```bash
diff -u fixtures/sorafs_manifest/ci_sample/car_summary.json artifacts/car_summary.json
diff -u fixtures/sorafs_manifest/ci_sample/chunk_plan.json artifacts/chunk_plan.json
diff -u fixtures/sorafs_manifest/ci_sample/manifest.sign.summary.json artifacts/manifest.sign.summary.json
diff -u fixtures/sorafs_manifest/ci_sample/manifest.bundle.json artifacts/manifest.bundle.json
diff -u fixtures/sorafs_manifest/ci_sample/manifest.verify.summary.json artifacts/manifest.verify.summary.json
diff -u fixtures/sorafs_manifest/ci_sample/proof.json artifacts/proof.json
```

Des diffs vides confirment que le build a produit des manifests, des plans et des bundles de
signature avec des octets identiques. Voir `fixtures/sorafs_manifest/ci_sample/README.md` pour une
liste complete et des conseils sur la creation de notes de release a partir des summaries captures.
