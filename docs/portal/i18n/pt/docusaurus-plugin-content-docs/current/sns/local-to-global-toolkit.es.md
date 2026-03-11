---
lang: pt
direction: ltr
source: docs/portal/docs/sns/local-to-global-toolkit.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Kit de direções Local -> Global

Esta página reflete `docs/source/sns/local_to_global_toolkit.md` do mono-repo. Empaque os auxiliares de CLI e runbooks necessários para o item de roteiro **ADDR-5c**.

## Resumo

- `scripts/address_local_toolkit.sh` usa CLI `iroha` para produzir:
  - `audit.json` -- saída estruturada de `iroha tools address audit --format json`.
  - `normalized.txt` -- literais I105 (preferido) / compactado (`sora`) (segunda melhor opção) convertidos para cada seletor de domínio Local.
- Combine o script com o painel de ingestão de direções (`dashboards/grafana/address_ingest.json`)
  e as regras do Alertmanager (`dashboards/alerts/address_ingest_rules.yml`) para testar a transferência Local-8 /
  Local-12 é seguro. Observe os painéis de colisão Local-8 e Local-12 e os alertas
  `AddressLocal8Resurgence`, `AddressLocal12Collision`, e `AddressInvalidRatioSlo` antes de
  promover mudanças de manifesto.
- Referência a [Diretrizes de exibição de endereço](address-display-guidelines.md) e el
  [Address Manifest runbook](../../../source/runbooks/address_manifest_ops.md) para contexto de UX e resposta a incidentes.

## Uso

```bash
scripts/address_local_toolkit.sh       --input fixtures/address/local_digest_examples.txt       --output-dir artifacts/address_migration       --network-prefix 753       --format i105
```

Opções:

- `--format I105` para saída `sora...` em vez de I105.
- `domainless output (default)` para emitir literalmente sem domínio.
- `--audit-only` para omitir a etapa de conversão.
- `--allow-errors` para seguir escaneando quando aparecerem filas malformadas (coincide com o comportamento da CLI).

O script descreve as rotas de artefatos até o final da execução. Juntando ambos os arquivos a
seu ticket de gerenciamento de mudanças junto com a captura de tela de Grafana que provavelmente será zero
detecções Local-8 e zero colisões Local-12 por >=30 dias.

## Integração CI

1. Execute o script em um trabalho dedicado e suba suas saídas.
2. Bloco mescla quando `audit.json` relata seletores locais (`domain.kind = local12`).
   em seu valor por defeito `true` (solo override a `false` em clusters dev/test al
   diagnosticar regressões) e agregar
   `iroha tools address normalize` para CI para que intenções de
   regressão caída antes de começar a produção.

Consulte o documento fonte para mais detalhes, listas de verificação de evidências e o trecho de
notas de lançamento que você pode reutilizar para anunciar a transferência aos clientes.