---
lang: ru
direction: ltr
source: docs/portal/docs/sns/local-to-global-toolkit.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Локальный -> Глобальный

یہ صفحہ `docs/source/sns/local_to_global_toolkit.md` کا عکاس ہے۔ Дорожная карта **ADDR-5c** Использование помощников CLI и модулей runbook

## جائزہ

- `scripts/address_local_toolkit.sh` `iroha` Обертка CLI в случае необходимости:
  - `audit.json` -- `iroha tools address audit --format json` для структурированного вывода.
  - `normalized.txt` -- ہر Селектор локального домена کے لیے IH58 (ترجیحی) / сжатые (`sora`, второй по качеству) литералы.
- На панели управления приемом адресов (`dashboards/grafana/address_ingest.json`)
  Правила Alertmanager (`dashboards/alerts/address_ingest_rules.yml`)
  Переключение Local-8/Local-12 Local-8 и панели столкновений Local-12
  `AddressLocal8Resurgence`, `AddressLocal12Collision`, `AddressInvalidRatioSlo` оповещения
  Если вы проявите تبدیلیاں, продвинете
- UX-интерфейс реагирования на инциденты کے لیے [Рекомендации по отображению адреса] (address-display-guidelines.md) اور
  [Ранбук манифеста адреса](../../../source/runbooks/address_manifest_ops.md)

## استعمال

```bash
scripts/address_local_toolkit.sh       --input fixtures/address/local_digest_examples.txt       --output-dir artifacts/address_migration       --network-prefix 753       --format ih58
```

Сообщение:

- `--format compressed` IH58 — выход `sora...` — выходной сигнал
- `--no-append-domain` تاکہ голые литералы نکلیں۔
- Шаг преобразования `--audit-only` چھوڑنے کے لیے۔
- `--allow-errors` — неверные строки при сканировании (поведение CLI)

Пути артефактов لکھتا ہے۔ دونوں فائلیں
Заявка на управление изменениями позволяет просмотреть скриншот Grafana.
>=30 دن تک صفر Local-8 обнаружений اور صفر Local-12 коллизий دکھائے۔

## CI انضمام

1. Выделите специальную работу и получите результаты, которые вам нужны.
2. Локальные селекторы `audit.json` رپورٹ کرے (`domain.kind = local12`) для объединения данных
   Значение по умолчанию `true` (для разработчиков/тестов используются регрессии, необходимые для `false`).
   `iroha tools address normalize --fail-on-warning --only-local` کو کو میں شامل کریں تاکہ
   производство регрессий

Ознакомьтесь с контрольными списками доказательств, а также фрагмент примечания к выпуску, который можно использовать для проверки.
Если вам нужен переходный вариант, вы можете сделать это прямо сейчас.