---
lang: ru
direction: ltr
source: docs/portal/docs/sns/local-to-global-toolkit.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# مجموعة ادوات عناوين Локальный -> Глобальный

Установите флажок `docs/source/sns/local_to_global_toolkit.md` для проверки. Для работы с интерфейсом командной строки и книгами Runbook необходимо выполнить настройку **ADDR-5c**.

## نظرة عامة

- `scripts/address_local_toolkit.sh` в CLI для `iroha`:
  - `audit.json` -- Установите флажок `iroha tools address audit --format json`.
  - `normalized.txt` -- литералы I105 (المفضل) / сжатые (`sora`) (الخيار الثاني) Для выбора селектора Local.
- استخدم السكربت مع لوحة ingest للعناوين (`dashboards/grafana/address_ingest.json`)
  Доступ к Alertmanager (`dashboards/alerts/address_ingest_rules.yml`) для переключения Local-8 /
  Местный-12 дней. Информационные центры Local-8 и Local-12.
  `AddressLocal8Resurgence`, `AddressLocal12Collision` и `AddressInvalidRatioSlo`.
  ترقية تغييرات манифест.
- ارجع الى [Правила отображения адреса](address-display-guidelines.md) و
  [runbook манифеста адреса](../../../source/runbooks/address_manifest_ops.md) Для работы с UX.

## الاستخدام

```bash
scripts/address_local_toolkit.sh       --input fixtures/address/local_digest_examples.txt       --output-dir artifacts/address_migration       --network-prefix 753       --format i105
```

Сообщение:

- `--format I105` для `sora...` на I105.
- `domainless output (default)` لاصدار литералы بدون نطاق.
- `--audit-only` لتخطي خطوة التحويل.
- `--allow-errors` был запущен в режиме CLI.

Создан артефакт, созданный в Нью-Йорке. ارفق كلا الملفين مع
Управление изменениями и управление изменениями в Grafana.
Загрузка Local-8 и возврат Local-12 >=30 дней.

## CI

1. Устроиться на работу в поисках работы.
2. Установите флажок `audit.json` для локальных селекторов (`domain.kind = local12`).
   Установите `true` (в разделе dev/test `false`). عند
   تشخيص التراجعات) واضف
   `iroha tools address normalize` в CI حتى تفشل
   Начало производства.

Сообщение о выпуске журнала в разделе «Примечание к выпуску» и фрагмент примечания к выпуску
В начале 2000-х годов произошло переключение.