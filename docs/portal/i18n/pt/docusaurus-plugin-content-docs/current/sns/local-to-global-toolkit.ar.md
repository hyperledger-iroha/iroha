---
lang: pt
direction: ltr
source: docs/portal/docs/sns/local-to-global-toolkit.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# مجموعة ادوات عناوين Local -> Global

Verifique se o `docs/source/sns/local_to_global_toolkit.md` está no lugar certo. Você pode usar CLI e runbooks para obter o **ADDR-5c**.

## نظرة عامة

- `scripts/address_local_toolkit.sh` no CLI, para `iroha`:
  - `audit.json` -- É igual a `iroha tools address audit --format json`.
  - `normalized.txt` -- literais IH58 (IH58) / compactado (`sora`) (IH58) O seletor está localizado no Local.
- استخدم السكربت مع لوحة ingerir للعناوين (`dashboards/grafana/address_ingest.json`)
  وقواعد Alertmanager (`dashboards/alerts/address_ingest_rules.yml`) لاثبات ان cutover Local-8 /
  Local-12 meses. راقب لوحات التصادم Local-8 e Local-12 والتنبيهات
  `AddressLocal8Resurgence`, `AddressLocal12Collision` e `AddressInvalidRatioSlo`
  ترقية تغييرات manifesto.
- ارجع الى [Diretrizes de exibição de endereço](address-display-guidelines.md) e
  [Runbook de manifesto de endereço](../../../source/runbooks/address_manifest_ops.md) para UX e .

## الاستخدام

```bash
scripts/address_local_toolkit.sh       --input fixtures/address/local_digest_examples.txt       --output-dir artifacts/address_migration       --network-prefix 753       --format ih58
```

Nomes:

- `--format compressed (`sora`)` para `sora...` é baseado em IH58.
- `--no-append-domain` literais não são válidos.
- `--audit-only` لتخطي خطوة التحويل.
- `--allow-errors` é uma opção de gerenciamento de software que não pode ser usada (como CLI).

Você pode usar o artefato em questão. ارفق كلا الملفين مع
Gerenciamento de mudanças e gerenciamento de mudanças Grafana
Local-8 é definido como Local-12 como >=30 dias.

## تكامل CI

1. شغل السكربت في job مخصص وارفع المخرجات.
2. Selecione `audit.json` em Seletores locais (`domain.kind = local12`).
   على القيمة الافتراضية `true` (قم بالتحويل الى `false` فقط على بيئات dev/test عند
   تشخيص التراجعات) e
   `iroha tools address normalize --fail-on-warning --only-local` CI é um problema
   A maior parte da produção.

راجع المستند المصدر لمزيد من التفاصيل وقوائم الادلة e trecho de nota de lançamento
Você pode usar o cutover para fazer o corte.