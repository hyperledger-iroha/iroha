---
lang: he
direction: rtl
source: docs/portal/docs/sns/local-to-global-toolkit.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Kit de enderecos מקומי -> גלובלי

Esta pagina espelha `docs/source/sns/local_to_global_toolkit.md` לעשות מונו-ריפו. Ela agrupa os helpers de CLI e runbooks exigidos pelo item de roadmap **ADDR-5c**.

## Visao Geral

- Encapsula `scripts/address_local_toolkit.sh` ו-CLI `iroha` למוצר:
  - `audit.json` -- saida estruturada de `iroha tools address audit --format json`.
  - `normalized.txt` -- literais IH58 (מועדף) / דחוס (`sora`) (segunda melhor opcao) convertidos para cada selector de dominio Local.
- שלב או סקריפט com o לוח מחוונים de ingest de enderecos (`dashboards/grafana/address_ingest.json`)
  e as regras do Alertmanager (`dashboards/alerts/address_ingest_rules.yml`) para provar que o cutover Local-8 /
  Local-12 e seguro. צפה ב-os paineis de colisao Local-8 e Local-12 e os alertas
  `AddressLocal8Resurgence`, `AddressLocal12Collision`, ו-`AddressInvalidRatioSlo` antes de
  מקדם mudancas de manifest.
- התייעצו בתור [הנחיות להצגת כתובת](address-display-guidelines.md) e o
  [פנקס מניפסט כתובות](../../../source/runbooks/address_manifest_ops.md) להקשר של UX ותשובה לתקריות.

## שימוש

```bash
scripts/address_local_toolkit.sh       --input fixtures/address/local_digest_examples.txt       --output-dir artifacts/address_migration       --network-prefix 753       --format ih58
```

אופקים:

- `--format compressed (`sora`)` para saida `sora...` em vez de IH58.
- `domainless output (default)` para emitir literais sem dominio.
- `--audit-only` למטרת שיחה.
- `--allow-errors` para continuar a varredura quando linhas malformadas aparecerem (igual ao comportamento da CLI).

O script escreve os caminhos dos artefatos ao final da execucao. Anexe os dois arquivos ao
seu ticket de gestao de mudancas junto com o צילום מסך לעשות Grafana que comprove zero
deteccoes Local-8 e zero colisoes Local-12 por >=30 dias.

## Integracao CI

1. Rode o script em um job dedicado e envie as saidas.
2. Bloqueie ממזג את quando `audit.json` מבחר דיווח מקומי (`domain.kind = local12`).
   no valor padrao `true` (לכן יש לשנות את `false` עם clusters dev/test ao diagnosticar
   regressoes) e adicione
   `iroha tools address normalize` ao CI עבור רגרסוס
   falhem antes de chegar a producao.

Veja o documento fonte para mais detalhes, checklists de evidencia e o snippet de
הערות שחרור que voce pode reusaler ao ununciar o cutover para clientes.