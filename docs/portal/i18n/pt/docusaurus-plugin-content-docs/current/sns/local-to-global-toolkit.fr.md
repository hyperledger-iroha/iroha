---
lang: pt
direction: ltr
source: docs/portal/docs/sns/local-to-global-toolkit.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Kit de endereços Local -> Global

Esta página reflete `docs/source/sns/local_to_global_toolkit.md` do mono-repo. Ele reagrupou os helpers CLI e os runbooks necessários para o item roadmap **ADDR-5c**.

## Apercu

- `scripts/address_local_toolkit.sh` encapsula o CLI `iroha` para produzir:
  - `audit.json` – estrutura de saída de `iroha tools address audit --format json`.
  - `normalized.txt` - literatura IH58 (preferir) / compactada (`sora`) (segunda escolha) convertida para cada seletor de domínio local.
- Associar o script ao painel de administração de endereços (`dashboards/grafana/address_ingest.json`)
  e as regras Alertmanager (`dashboards/alerts/address_ingest_rules.yml`) para provar que o corte Local-8 /
  Local-12 fica ao sul. Monitore os painéis de colisão Local-8 e Local-12 e os alertas
  `AddressLocal8Resurgence`, `AddressLocal12Collision`, e `AddressInvalidRatioSlo` antes de
  promova as alterações do manifesto.
- Consulte as [Diretrizes de exibição de endereço](address-display-guidelines.md) e
  [Address Manifest runbook](../../../source/runbooks/address_manifest_ops.md) para contexto UX e resposta a incidentes.

## Uso

```bash
scripts/address_local_toolkit.sh       --input fixtures/address/local_digest_examples.txt       --output-dir artifacts/address_migration       --network-prefix 753       --format ih58
```

Opções:

- `--format compressed (`sora`)` para a saída `sora...` em vez de IH58.
- `--no-append-domain` para emetre des literaux nus.
- `--audit-only` para ignorar a etapa de conversão.
- `--allow-errors` para continuar a varredura quando as linhas mal formadas forem detectadas (corresponde ao comportamento da CLI).

O script escreve os caminhos dos artefatos no final da execução. Joignez les deux fichiers a
seu ticket de gerenciamento de mudança com captura Grafana que prova zero
detecções Local-8 e zero colisões Local-12 pendente >=30 dias.

## CI de integração

1. Lance o script em um trabalho dedicado e carregue as missões.
2. Bloqueie as mesclagens quando `audit.json` sinalizar os seletores locais (`domain.kind = local12`).
   com o valor padrão `true` (não passe para `false` nos clusters dev/test durante
   diagnóstico de regressões) et ajoutez
   `iroha tools address normalize --fail-on-warning --only-local` um CI para as regressões
   ecoou antes da produção.

Veja a fonte do documento para mais detalhes, listas de verificação de evidências e o trecho de
notas de lançamento que você pode reutilizar para anunciar a transferência para clientes.