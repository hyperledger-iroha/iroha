---
lang: he
direction: rtl
source: docs/portal/docs/sns/local-to-global-toolkit.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

# Kit de enderecos Local -> Global

Esta pagina espelha `docs/source/sns/local_to_global_toolkit.md` do mono-repo. Ela agrupa os helpers de CLI e runbooks exigidos pelo item de roadmap **ADDR-5c**.

## Visao geral

- `scripts/address_local_toolkit.sh` encapsula a CLI `iroha` para produzir:
  - `audit.json` -- saida estruturada de `iroha tools address audit --format json`.
  - `normalized.txt` -- literais IH58 (preferido) / compressed (`sora`) (segunda melhor opcao) convertidos para cada selector de dominio Local.
- Combine o script com o dashboard de ingest de enderecos (`dashboards/grafana/address_ingest.json`)
  e as regras do Alertmanager (`dashboards/alerts/address_ingest_rules.yml`) para provar que o cutover Local-8 /
  Local-12 e seguro. Observe os paineis de colisao Local-8 e Local-12 e os alertas
  `AddressLocal8Resurgence`, `AddressLocal12Collision`, e `AddressInvalidRatioSlo` antes de
  promover mudancas de manifest.
- Consulte as [Address Display Guidelines](address-display-guidelines.md) e o
  [Address Manifest runbook](../../../source/runbooks/address_manifest_ops.md) para contexto de UX e resposta a incidentes.

## Uso

```bash
scripts/address_local_toolkit.sh       --input fixtures/address/local_digest_examples.txt       --output-dir artifacts/address_migration       --network-prefix 753       --format ih58
```

Opcoes:

- `--format compressed (`sora`)` para saida `sora...` em vez de IH58.
- `--no-append-domain` para emitir literais sem dominio.
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
   `iroha tools address normalize --fail-on-warning --only-local` ao CI para que regressos
   falhem antes de chegar a producao.

Veja o documento fonte para mais detalhes, checklists de evidencia e o snippet de
release notes que voce pode reutilizar ao anunciar o cutover para clientes.
