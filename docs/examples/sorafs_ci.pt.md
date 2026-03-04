---
lang: pt
direction: ltr
source: docs/examples/sorafs_ci.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 61536f7f6dbecb2658cb5e07a15ed99888957bf87b473e0089e38f8431156c18
source_last_modified: "2025-11-08T09:36:10.010765+00:00"
translation_last_reviewed: 2026-01-01
---

---
title: Guia de CI SoraFS
summary: Workflow do GitHub Actions reunindo etapas de assinatura e verificacao com notas de revisao.
---

# Guia de CI SoraFS

Este snippet espelha a orientacao em `docs/source/sorafs_ci_templates.md` e demonstra como integrar
assinatura, verificacao e checks de proof em um unico job do GitHub Actions.

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

## Notas

- `sorafs_cli` deve estar disponivel no runner (ex., `cargo install --path crates/sorafs_car --features cli` antes destes passos).
- O workflow deve fornecer um audience OIDC explicito (aqui `sorafs`); ajuste `--identity-token-audience` para combinar com sua politica Fulcio.
- O pipeline de release deve arquivar `artifacts/manifest.bundle.json`, `artifacts/manifest.sig` e `artifacts/proof.json` para revisao de governance.
- Artefatos deterministas de exemplo ficam em `fixtures/sorafs_manifest/ci_sample`; copie-os para testes quando precisar de manifests, chunk plans ou bundle JSON golden sem recomputar o pipeline.

## Verificacao de fixtures

Artefatos deterministas para este workflow ficam sob
`fixtures/sorafs_manifest/ci_sample`. Pipelines podem reproduzir os passos acima e
comparar suas saidas contra os arquivos canonicos, por exemplo:

```bash
diff -u fixtures/sorafs_manifest/ci_sample/car_summary.json artifacts/car_summary.json
diff -u fixtures/sorafs_manifest/ci_sample/chunk_plan.json artifacts/chunk_plan.json
diff -u fixtures/sorafs_manifest/ci_sample/manifest.sign.summary.json artifacts/manifest.sign.summary.json
diff -u fixtures/sorafs_manifest/ci_sample/manifest.bundle.json artifacts/manifest.bundle.json
diff -u fixtures/sorafs_manifest/ci_sample/manifest.verify.summary.json artifacts/manifest.verify.summary.json
diff -u fixtures/sorafs_manifest/ci_sample/proof.json artifacts/proof.json
```

Diffs vazios confirmam que o build produziu manifests, planos e bundles de assinatura com bytes
identicos. Veja `fixtures/sorafs_manifest/ci_sample/README.md` para um listing completo e dicas
sobre templates de notas de release a partir dos summaries capturados.
