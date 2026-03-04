---
lang: ru
direction: ltr
source: docs/examples/sorafs_ci.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 61536f7f6dbecb2658cb5e07a15ed99888957bf87b473e0089e38f8431156c18
source_last_modified: "2025-11-08T09:36:10.010765+00:00"
translation_last_reviewed: 2026-01-01
---

---
title: CI справочник SoraFS
summary: Пример workflow GitHub Actions с подписью и проверкой в одном job и заметками для ревью.
---

# CI справочник SoraFS

Этот фрагмент повторяет руководство из `docs/source/sorafs_ci_templates.md` и показывает, как
интегрировать подпись, проверку и proof checks в один job GitHub Actions.

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

## Примечания

- `sorafs_cli` должен быть доступен на runner (например, `cargo install --path crates/sorafs_car --features cli` перед этими шагами).
- Workflow должен указать явный OIDC audience (здесь `sorafs`); скорректируйте `--identity-token-audience` под вашу политику Fulcio.
- Release pipeline должен архивировать `artifacts/manifest.bundle.json`, `artifacts/manifest.sig` и `artifacts/proof.json` для ревью governance.
- Детерминированные примерные артефакты находятся в `fixtures/sorafs_manifest/ci_sample`; копируйте их в тесты, когда нужны golden manifests, chunk plans или bundle JSON без пересчета pipeline.

## Проверка fixtures

Детерминированные артефакты для этого workflow лежат в
`fixtures/sorafs_manifest/ci_sample`. Пайплайны могут повторить шаги выше и
сравнить результаты с каноническими файлами, например:

```bash
diff -u fixtures/sorafs_manifest/ci_sample/car_summary.json artifacts/car_summary.json
diff -u fixtures/sorafs_manifest/ci_sample/chunk_plan.json artifacts/chunk_plan.json
diff -u fixtures/sorafs_manifest/ci_sample/manifest.sign.summary.json artifacts/manifest.sign.summary.json
diff -u fixtures/sorafs_manifest/ci_sample/manifest.bundle.json artifacts/manifest.bundle.json
diff -u fixtures/sorafs_manifest/ci_sample/manifest.verify.summary.json artifacts/manifest.verify.summary.json
diff -u fixtures/sorafs_manifest/ci_sample/proof.json artifacts/proof.json
```

Пустые diff подтверждают, что билд создал byte-identical manifests, планы и bundles подписи.
См. `fixtures/sorafs_manifest/ci_sample/README.md` для полного списка и советов по
шаблонам release notes из captured summaries.
