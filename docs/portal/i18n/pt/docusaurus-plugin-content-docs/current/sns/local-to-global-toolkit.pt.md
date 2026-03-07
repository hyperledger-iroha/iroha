---
lang: pt
direction: ltr
source: docs/portal/docs/sns/local-to-global-toolkit.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Kit de enderecos Local -> Global

Esta página espelha `docs/source/sns/local_to_global_toolkit.md` do mono-repo. Ela agrupa os helpers de CLI e runbooks exigidos pelo item de roadmap **ADDR-5c**.

## Visão geral

- `scripts/address_local_toolkit.sh` encapsula um CLI `iroha` para produzir:
  - `audit.json` -- disse estruturada de `iroha tools address audit --format json`.
  - `normalized.txt` -- literais IH58 (preferido) / compactado (`sora`) (segunda melhor opção) convertidos para cada seletor de domínio Local.
- Combine o script com o dashboard de ingestão de endereços (`dashboards/grafana/address_ingest.json`)
  e as regras do Alertmanager (`dashboards/alerts/address_ingest_rules.yml`) para provar que o cutover Local-8 /
  Local-12 e seguro. Observe os paineis de colisão Local-8 e Local-12 e os alertas
  `AddressLocal8Resurgence`, `AddressLocal12Collision`, e `AddressInvalidRatioSlo` antes de
  promover mudanças de manifesto.
- Consulte como [Diretrizes de exibição de endereço](address-display-guidelines.md) e o
  [Address Manifest runbook](../../../source/runbooks/address_manifest_ops.md) para contexto de UX e resposta a incidentes.

## Uso

```bash
scripts/address_local_toolkit.sh       --input fixtures/address/local_digest_examples.txt       --output-dir artifacts/address_migration       --network-prefix 753       --format ih58
```

Opções:

- `--format compressed (`sora`)` para disse `sora...` em vez de IH58.
- `domainless output (default)` para emitir literais sem domínio.
- `--audit-only` para pular a etapa de conversação.
- `--allow-errors` para continuar a varredura quando linhas malformadas aparecerem (igual ao comportamento da CLI).

O roteiro escreve os caminhos dos artistas ao final da execução. Anexo os dois arquivos ao
seu ticket de gerenciamento de mudanças junto com o screenshot do Grafana que comprove zero
detecta Local-8 e zero colisões Local-12 por >=30 dias.

## Integração CI

1. Rode o script em um trabalho dedicado e envie as ditas.
2. Bloqueie mescla quando `audit.json` reportar seletores locais (`domain.kind = local12`).
   no valor padrão `true` (então altere para `false` em clusters dev/test ao diagnosticar
   regressos) e acréscimos
   `iroha tools address normalize` ao CI para que regressos
   falhem antes de chegar a produção.

Veja o documento fonte para mais detalhes, checklists de evidências e o trecho de
notas de lançamento que você pode reutilizar ao anunciar a transferência para clientes.