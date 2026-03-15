---
lang: ru
direction: ltr
source: docs/portal/docs/sorafs/reports/capacity-marketplace-validation.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
Название: التحقق من سوق سعة SoraFS
тэги: [SF-2c, приемка, контрольный список]
Краткое содержание: Он сказал: Установите флажок SoraFS.
---

# قائمة تحقق التحقق من سوق سعة SoraFS

**Обзор:** 18 марта 2026 г. -> 24 марта 2026 г.  
**Общие сведения:** Группа хранения (`@storage-wg`), Совет управления (`@council`), Гильдия казначейства (`@treasury`)  
**Обмен:** Установите флажок SF-2c GA.

Он был убит в 1980-х годах в Вашингтоне. Когда он провел тесты и матчи в Сан-Франциско, он проиграл Дэйву.

## قائمة تحقق القبول

### انضمام المزودين

| الفحص | تحقق | دليل |
|-------|------------|----------|
| Реестр يقبل Для этого используется приложение `/v1/sorafs/capacity/declare`, которое позволяет использовать API приложения, а также использовать его в качестве приложения. Метаданные и доступ к реестру. | `crates/iroha_torii/src/routing.rs:7654` |
| Смарт-контракт и полезные нагрузки غير المتطابقة | В 2007 году он был назначен на должность директора ГиБа по телефону в 2007 году. الموقع قبل الحفظ. | `crates/iroha_core/src/smartcontracts/isi/sorafs.rs:3445` |
| Поиск артефактов CLI Обвязка CLI используется для обработки Norito/JSON/Base64 и может быть использована для выполнения туда и обратно. المشغلون من إعداد الإعلانات в автономном режиме. | `crates/sorafs_car/tests/capacity_cli.rs:17` |
| Уиллоу Сэнсэй Уилсон и его друзья | Вы можете использовать настройки политики по умолчанию, а затем изменить настройки. | `../storage-capacity-marketplace.md` |

### تسوية النزاعات

| الفحص | تحقق | دليل |
|-------|------------|----------|
| تبقى سجلات النزاع مع дайджест قياسي للـ полезная нагрузка | В настоящее время в базе данных находится полезная нагрузка, ожидающая создания бухгалтерской книги. | `crates/iroha_core/src/smartcontracts/isi/sorafs.rs:1835` |
| Открытие CLI для редактирования | Используйте интерфейс CLI для Base64/Norito и JSON для `CapacityDisputeV1`, а также для пакетов доказательств. Билли Дэйв. | `crates/sorafs_car/tests/capacity_cli.rs:455` |
| Повтор повтора يثبت تمية النزاع/العقوبة | телеметрия и проверка-неудачность, проверка, снимки, снимки, учетная запись, учетная запись, Лоуберн режет сверстников из Торонто. | `crates/iroha_core/src/smartcontracts/isi/sorafs.rs:3430` |
| يوثق runbook مسار التصعيد والإلغاء | Чтобы выполнить откат, нажмите кнопку «Откат». | `../dispute-revocation-runbook.md` |

### تسوية الخزانة| الفحص | تحقق | دليل |
|-------|------------|----------|
| Справочная книга يطابق توقع замачивания 30 дней | В 30-летнем возрасте поселение, а также в книге бухгалтерских книг المتوقع للمدفوعات. | `crates/iroha_core/src/smartcontracts/isi/sorafs.rs:3000` |
| бухгалтерская книга تسوية صادرات تُسجل ليلا | Введите `capacity_reconcile.py` в книгу комиссионных сборов, чтобы получить XOR المنفذة, ويصدر مقاييس Prometheus, Используйте Alertmanager. | `scripts/telemetry/capacity_reconcile.py:1`,`docs/source/sorafs/runbooks/capacity_reconciliation.md:1`,`dashboards/alerts/sorafs_capacity_rules.yml:100` |
| Платежные услуги по выставлению счетов и телеметрии | يعرض استيراد Grafana تراكم GiB-hour, عدادات забастовки, والضمان المربوط لتمكين الرؤية لدى فريق المناوبة. | `dashboards/grafana/sorafs_capacity_penalties.json:1` |
| التقرير المنشور يؤرشف منهجية впитывать وأوامر повтор | Используйте крючки, чтобы замочить их и закрепить на крючках. | `./sf2c-capacity-soak.md` |

## ملاحظات التنفيذ

Сообщение о подписании протокола:

```bash
cargo test -p iroha_torii --features app_api -- capacity_declaration_handler_accepts_request
cargo test -p iroha_core -- register_capacity_declaration_rejects_provider_mismatch
cargo test -p iroha_core -- register_capacity_dispute_inserts_record
cargo test -p iroha_core -- capacity_dispute_replay_is_deterministic
cargo test -p iroha_core -- capacity_fee_ledger_30_day_soak_deterministic
cargo test -p sorafs_car --features cli --test capacity_cli
python3 scripts/telemetry/capacity_reconcile.py --snapshot <state.json> --ledger <ledger.ndjson> --warn-only
```

Для получения полезных нагрузок необходимо установить / установить `sorafs_manifest_stub capacity {declaration,dispute}` وأرشفة Создайте JSON/Norito для создания файла.

## артефакты

| Артефакт | Путь | blake2b-256 |
|----------|------|-------------|
| حزمة موافقة انضمام المزودين | `docs/examples/sorafs_capacity_marketplace_validation/2026-03-24_onboarding_signoff.md` | `8f41a745d8d94710fe81c07839651520429d4abea5729bc00f8f45bbb11daa4c` |
| حزمة موافقة تسوية النزاعات | `docs/examples/sorafs_capacity_marketplace_validation/2026-03-24_dispute_signoff.md` | `c3ac3999ef52857170fedb83cddbff7733ef5699f8b38aea2e65ae507a6229f7` |
| حزمة موافقة تسوية الخزانة | `docs/examples/sorafs_capacity_marketplace_validation/2026-03-24_treasury_signoff.md` | `0511aeed1f5607c329428cd49c94d1af51292c85134c10c3330c172b0140e8c6` |

احتفظ بالنسخ الموقعة من هذه artefacts в фильме "Старый мир" в Стокгольме. حوكمة.

## الموافقات

- Руководитель группы хранения данных — @storage-tl (24 марта 2026 г.)  
- Секретарь Совета управления — @council-sec (24 марта 2026 г.)  
- Руководитель казначейских операций — @treasury-ops (24 марта 2026 г.)