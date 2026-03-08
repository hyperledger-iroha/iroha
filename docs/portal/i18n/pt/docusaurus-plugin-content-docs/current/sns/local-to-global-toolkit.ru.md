---
lang: pt
direction: ltr
source: docs/portal/docs/sns/local-to-global-toolkit.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Набор инструментов Local -> Global адресов

Esta página contém `docs/source/sns/local_to_global_toolkit.md` do mono-repo. Para ativar ajudantes CLI e runbooks, digite os seguintes cartões **ADDR-5c**.

##Obzor

- `scripts/address_local_toolkit.sh` оборачивает CLI `iroha`, esta é a configuração:
  - `audit.json` -- estrutura de acordo com `iroha tools address audit --format json`.
  - `normalized.txt` -- преобразованные IH58 (предпочтительно) / compactado (`sora`) (второй выбор) literais para o seletor de domínio local.
- Use o script para acessar os endereços de ingestão do painel (`dashboards/grafana/address_ingest.json`)
  e правилами Alertmanager (`dashboards/alerts/address_ingest_rules.yml`), чтобы доказать безопасность cutover Local-8 /
  Local-12. Следите за панелями коллизий Local-8 и Local-12 и алертами
  `AddressLocal8Resurgence`, `AddressLocal12Collision`, e `AddressInvalidRatioSlo` antes
  продвижением изменений manifesto.
- Сверяйтесь с [Diretrizes de exibição de endereço](address-display-guidelines.md) и
  [Runbook de manifesto de endereço](../../../source/runbooks/address_manifest_ops.md) para UX e contato de resposta a incidentes.

##Implantação

```bash
scripts/address_local_toolkit.sh       --input fixtures/address/local_digest_examples.txt       --output-dir artifacts/address_migration       --network-prefix 753       --format ih58
```

Descrição:

- `--format compressed (`sora`)` para você `sora...` em vez de IH58.
- `domainless output (default)` para usar literais simples.
- `--audit-only` é uma opção de conversão.
- `--allow-errors` fornece uma verificação por meio de uma configuração padrão (suportado pela CLI).

Скрипт выводит пути артефактов в конце выполнения. Приложите оба файла к
tíquete de gerenciamento de alterações com captura de tela do Grafana, подтверждающим ноль
Local-8 é detectado e não Local-12 é coletado no mínimo para >=30 dias.

## Integração CI

1. Crie um script no trabalho anterior e atualize as saídas.
2. Блокируйте mescla, когда `audit.json` сообщает Seletores locais (`domain.kind = local12`).
   então use o `true` (instalado em `false` no dev/test para registro de diagnóstico) e
   Instale `iroha tools address normalize` no CI, este registro é feito
   падали до produção.

Sim. um documento detalhado para detalhes, listas de evidências e trechos de notas de lançamento, que podem ser usados
использовать при анонсе cutover para clientes.