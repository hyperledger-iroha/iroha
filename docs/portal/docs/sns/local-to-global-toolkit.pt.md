---
lang: pt
direction: ltr
source: docs/portal/docs/sns/local-to-global-toolkit.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 20dd2db9bb7af3c40b35154b927df8000cb8c0d6c8e6190e71fb3e6491403149
source_last_modified: "2025-12-19T22:33:53.968499+00:00"
translation_last_reviewed: 2026-01-01
---

# Kit de enderecos Local -> Global

Esta pagina espelha `docs/source/sns/local_to_global_toolkit.md` do mono-repo. Ela agrupa os helpers de CLI e runbooks exigidos pelo item de roadmap **ADDR-5c**.

## Visao geral

- `scripts/address_local_toolkit.sh` encapsula a CLI `iroha` para produzir:
  - `audit.json` -- saida estruturada de `iroha tools address audit --format json`.
  - `normalized.txt` -- literais i105 (preferido) / i105 (segunda melhor opcao) convertidos para cada selector de dominio Local.
- Combine o script com o dashboard de ingest de enderecos (`dashboards/grafana/address_ingest.json`)
  e as regras do Alertmanager (`dashboards/alerts/address_ingest_rules.yml`) para provar que o cutover Local-8 /
  Local-12 e seguro. Observe os paineis de colisao Local-8 e Local-12 e os alertas
  `AddressLocal8Resurgence`, `AddressLocal12Collision`, e `AddressInvalidRatioSlo` antes de
  promover mudancas de manifest.
- Consulte as [Address Display Guidelines](address-display-guidelines.md) e o
  [Address Manifest runbook](../../../source/runbooks/address_manifest_ops.md) para contexto de UX e resposta a incidentes.

## Uso

```bash
scripts/address_local_toolkit.sh       --input fixtures/address/local_digest_examples.txt       --output-dir artifacts/address_migration       --network-prefix 753       --format i105
```

Opcoes:

- `--format i105` para saida `sora...` em vez de i105.
- `domainless output (default)` para emitir literais sem dominio.
- `--audit-only` para pular a etapa de conversao.
- `--allow-errors` para continuar a varredura quando linhas malformadas aparecerem (igual ao comportamento da CLI).

O script escreve os caminhos dos artefatos ao final da execucao. Anexe os dois arquivos ao
seu ticket de gestao de mudancas junto com o screenshot do Grafana que comprove zero
deteccoes Local-8 e zero colisoes Local-12 por >=30 dias.

## Integracao CI

1. Rode o script em um job dedicado e envie as saidas.
2. Bloqueie merges quando `audit.json` reportar selectores Local (`domain.kind = local12`).
   no valor padrao `true` (so altere para `false` em clusters dev/test ao diagnosticar
   regressoes) e adicione
   `iroha tools address normalize` ao CI para que regressos
   falhem antes de chegar a producao.

Veja o documento fonte para mais detalhes, checklists de evidencia e o snippet de
release notes que voce pode reutilizar ao anunciar o cutover para clientes.
