---
lang: pt
direction: ltr
source: docs/examples/sorafs_ci_sample/README.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 94d85ce53120b453bf81ac03a09b41ba64470194917dc913b7fb55f4da2f8b09
source_last_modified: "2025-11-02T18:54:59.610441+00:00"
translation_last_reviewed: 2026-01-01
---

# Fixtures de exemplo de CI SoraFS

Este diretorio empacota artefatos deterministas gerados a partir do payload de exemplo em `fixtures/sorafs_manifest/ci_sample/`. O bundle demonstra o pipeline end-to-end de empacotamento e assinatura SoraFS que os workflows de CI executam.

## Inventario de artefatos

| Arquivo | Descricao |
|------|-------------|
| `payload.txt` | Payload fonte usado pelos scripts de fixture (amostra de texto plano). |
| `payload.car` | Arquivo CAR emitido por `sorafs_cli car pack`. |
| `car_summary.json` | Resumo gerado por `car pack` capturando digests de chunks e metadados. |
| `chunk_plan.json` | JSON de plano de fetch descrevendo faixas de chunks e expectativas de providers. |
| `manifest.to` | Manifest Norito produzido por `sorafs_cli manifest build`. |
| `manifest.json` | Renderizacao legivel do manifest para debug. |
| `proof.json` | Resumo PoR emitido por `sorafs_cli proof verify`. |
| `manifest.bundle.json` | Bundle de assinatura keyless gerado por `sorafs_cli manifest sign`. |
| `manifest.sig` | Assinatura Ed25519 separada correspondente ao manifest. |
| `manifest.sign.summary.json` | Resumo CLI emitido durante o signing (hashes, metadados do bundle). |
| `manifest.verify.summary.json` | Resumo CLI de `manifest verify-signature`. |

Todos os digests referenciados em release notes e documentacao saem destes arquivos. O workflow `ci/check_sorafs_cli_release.sh` regenera os mesmos artefatos e compara com as versoes commitadas.

## Regeneracao de fixtures

Execute os comandos abaixo a partir da raiz do repo para regenerar o set de fixtures. Eles espelham as etapas do workflow `sorafs-cli-fixture`:

```bash
sorafs_cli car pack       --input fixtures/sorafs_manifest/ci_sample/payload.txt       --car-out fixtures/sorafs_manifest/ci_sample/payload.car       --plan-out fixtures/sorafs_manifest/ci_sample/chunk_plan.json       --summary-out fixtures/sorafs_manifest/ci_sample/car_summary.json

sorafs_cli manifest build       --summary fixtures/sorafs_manifest/ci_sample/car_summary.json       --manifest-out fixtures/sorafs_manifest/ci_sample/manifest.to       --manifest-json-out fixtures/sorafs_manifest/ci_sample/manifest.json

sorafs_cli proof verify       --manifest fixtures/sorafs_manifest/ci_sample/manifest.to       --car fixtures/sorafs_manifest/ci_sample/payload.car       --summary-out fixtures/sorafs_manifest/ci_sample/proof.json

sorafs_cli manifest sign       --manifest fixtures/sorafs_manifest/ci_sample/manifest.to       --summary fixtures/sorafs_manifest/ci_sample/car_summary.json       --chunk-plan fixtures/sorafs_manifest/ci_sample/chunk_plan.json       --bundle-out fixtures/sorafs_manifest/ci_sample/manifest.bundle.json       --signature-out fixtures/sorafs_manifest/ci_sample/manifest.sig       --identity-token "$(cat fixtures/sorafs_manifest/ci_sample/fixture_identity_token.jwt)"       --issued-at 1700000000       > fixtures/sorafs_manifest/ci_sample/manifest.sign.summary.json

sorafs_cli manifest verify-signature       --manifest fixtures/sorafs_manifest/ci_sample/manifest.to       --bundle fixtures/sorafs_manifest/ci_sample/manifest.bundle.json       --summary fixtures/sorafs_manifest/ci_sample/car_summary.json       --chunk-plan fixtures/sorafs_manifest/ci_sample/chunk_plan.json       --expect-token-hash 7b56598bca4584a5f5631ce4e510b8c55bd9379799f231db2a3476774f45722b       > fixtures/sorafs_manifest/ci_sample/manifest.verify.summary.json
```

Se algum passo produzir hashes diferentes, investigue antes de atualizar os fixtures. Os workflows de CI dependem de output determinista para detectar regressoes.

## Cobertura futura

Conforme perfis adicionais de chunker e formatos de proof avancarem na roadmap, seus fixtures canonicos serao adicionados neste diretorio (por exemplo,
`sorafs.sf2@1.0.0` (ver `fixtures/sorafs_manifest/ci_sample_sf2/`) ou proofs de streaming PDP). Cada novo perfil seguira a mesma estrutura - payload, CAR,
plan, manifest, proofs e artefatos de assinatura - para que a automacao downstream possa comparar releases sem scripting customizado.
