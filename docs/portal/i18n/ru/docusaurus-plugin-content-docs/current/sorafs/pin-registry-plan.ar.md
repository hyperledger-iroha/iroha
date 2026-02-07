---
lang: ru
direction: ltr
source: docs/portal/docs/sorafs/pin-registry-plan.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: план-регистрации контактов
title: Создать реестр контактов SoraFS
Sidebar_label: Список контактов
описание: Создан для SF-4 в базе данных реестра Torii.
---

:::примечание
Был установлен `docs/source/sorafs/pin_registry_plan.md`. Он сказал, что в действительности он хочет, чтобы он сделал это.
:::

# Введите реестр контактов для SoraFS (SF-4)

Для SF-4 используется реестр контактов, который можно просмотреть в манифесте.
Для этого необходимо подключить PIN-код API-интерфейса Torii.
يوسّع هذا المستند خطة التحقق بمهام تنفيذية ملموسة تغطي المنطق on-chain,
Он провел матчи с расписанием матчей, а также с Новым годом.

## النطاق

1. **Реестр реестра**: سجلات Norito للـ манифестирует псевдонимы والـ وسلاسل الخلفاء.
   Он был выбран в качестве посредника.
2. **Подключение**: Вывод CRUD для подключения разъема (`ReplicationOrder`, `Precommit`,
   `Completion`, выселение).
3. **Восстановление**: включите gRPC/REST в реестре Torii и SDK.
   Он был открыт в Нью-Йорке.
4. **Подробнее о приспособлениях**: CLI позволяет настроить и изменить настройки.
   манифестирует псевдонимы и конверты الخاصة بالحوكمة.
5. **Запуск и запуск**: создание реестра Runbooks.

## نموذج البيانات

### Дополнительная информация (Norito)

| بنية | الوصف | حقول |
|--------|-------|--------|
| `PinRecordV1` | Это манифест Кейн. | `manifest_cid`, `chunk_plan_digest`, `por_root`, `profile_handle`, `approved_at`, `retention_epoch`, `pin_policy`, `successor_of`, `governance_envelope_hash`. |
| `AliasBindingV1` | Псевдоним ربط -> CID الخاص بالـ манифест. | `alias`, `manifest_cid`, `bound_at`, `expiry_epoch`. |
| `ReplicationOrderV1` | تعليمات للمزوّدين لتثبيت манифест. | `order_id`, `manifest_cid`, `providers`, `redundancy`, `deadline`, `policy_hash`. |
| `ReplicationReceiptV1` | اقرار المزوّد. | `order_id`, `provider_id`, `status`, `timestamp`, `por_sample_digest`. |
| `ManifestPolicyV1` | لقطة سياسة الحوكمة. | `min_replicas`, `max_retention_epochs`, `allowed_profiles`, `pin_fee_basis_points`. |

Имя: `crates/sorafs_manifest/src/pin_registry.rs`, Norito в Rust.
Он сказал, что это не так. Инструментарий для манифеста
(поиск в реестре блоков и шлюзовании политики выводов)
تتقاسم نفس инварианты.

Уважаемые:
- Установите Norito или `crates/sorafs_manifest/src/pin_registry.rs`.
- Обновление (Rust + SDK) Установлено Norito.
- Установите флажок (`sorafs_architecture_rfc.md`) и установите его.

## تنفيذ العقد| المهمة | المالك/المالكون | الملاحظات |
|--------|------------------|-----------|
| Создайте реестр (sled/sqlite/off-chain) и смарт-контракт. | Основная инфраструктура/команда смарт-контрактов | Он хешировал حتمي وتجنب الفاصلة العائمة. |
| Коды: `submit_manifest`, `approve_manifest`, `bind_alias`, `issue_replication_order`, `complete_replication`, `evict_manifest`. | Основная инфраструктура | Установите `ManifestValidator` в приложении. Псевдоним يمر الان عبر `RegisterPinManifest` (DTO من Torii) с именем `bind_alias` المخصص Пожалуйста, проверьте. |
| Значение: فرض التعاقب (манифест A -> B), псевдоним عصور الاحتفاظ, تفرد. | Совет управления / Core Infra | Псевдоним Пейзажа был создан в `crates/iroha_core/src/smartcontracts/isi/sorafs.rs`; Он был убит в 2007 году. |
| Проверьте: Установите `ManifestPolicyV1` в конфигурации/установке. السماح بالتحديث عبر احداث الحوكمة. | Совет управления | Откройте интерфейс CLI. |
| Код: Установлен Norito в списке (`ManifestApproved`, `ReplicationOrderIssued`, `AliasBound`). | Наблюдаемость | تعريف مخطط الاحداث + التسجيل. |

Сообщение:
- اختبارات وحدة لكل نقطة دخول (ايجابي + رفض).
- اختبارات خصائص لسلسلة التعاقب (بدون دورات، عصور متصاعدة).
- Фазз Лиззи Тэхен проявляет عشوائية (مقيدة).

## Обновление (только Torii/SDK)

| المكون | المهمة | المالك/المالكون |
|--------|--------|------------------|
| خدمة Torii | Например, `/v1/sorafs/pin` (отправка), `/v1/sorafs/pin/{cid}` (поиск), `/v1/sorafs/aliases` (список/привязка), `/v1/sorafs/replication` (заказы/поступления). توفير ترقيم + ترشيح. | Сетевые TL/Core Infra |
| Новости | تضمين ارتفاع/هاش реестр в الاستجابات؛ Загрузите Norito для создания SDK. | Основная инфраструктура |
| интерфейс командной строки | Используйте `sorafs_manifest_stub` в CLI `sorafs_pin` для `pin submit`, `alias bind`, `order issue`, `registry export`. | Инструментальная рабочая группа |
| SDK | Крепления для крепления (Rust/Go/TS) для Norito; اضافة اختبارات تكامل. | Команды SDK |

Ответ:
- Для получения кэша/ETag используется GET.
- Ограничение скорости/аутентификация была установлена ​​в системе Torii.

## Светильники и CI

- Дополнительные приспособления: `crates/iroha_core/tests/fixtures/sorafs_pin_registry/` для создания манифеста/псевдонима/заказа, созданного `cargo run -p iroha_core --example gen_pin_snapshot`.
- CI: `ci/check_sorafs_fixtures.sh` تعيد توليد اللقطة وتفشل عند وجود اختلافات، لتحافظ على تماهي расписание матчей الخاصة بـ CI.
- اختبارات التكامل (`crates/iroha_core/tests/pin_registry.rs`). Псевдоним пользователя / разработчика, а также дескрипторы, названные в честь Чанкера, и его имя в фильме "Старый Стив". التعاقب (مؤشرات مجهولة/موافَق عليها مسبقا/مسحوبة/ذاتية الاشارة); راجع حالات `register_manifest_rejects_*` для проверки.
- اختبارات الوحدة تغطي الان التحقق من псевдоним وحمايات الاحتفاظ وفحوصات الخلف في `crates/iroha_core/src/smartcontracts/isi/sorafs.rs`; Он сказал, что это не так.
- JSON отображается в формате JSON в режиме онлайн.

## التليمتري والرصد

Сообщение (Prometheus):
- `torii_sorafs_registry_manifests_total{status="pending|approved|retired"}`
- `torii_sorafs_registry_aliases_total`
- `torii_sorafs_registry_orders_total{status="pending|completed|expired"}`
- `torii_sorafs_replication_sla_total{outcome="met|missed|pending"}`
- `torii_sorafs_replication_completion_latency_epochs{stat="avg|p95|max|count"}`
- `torii_sorafs_replication_deadline_slack_epochs{stat="avg|p95|max|count"}`
- Установите флажок (`torii_sorafs_capacity_*`, `torii_sorafs_fee_projection_nanos`) для завершения сквозного подключения.Сообщение:
- تيار احداث Norito منظم لتدقيقات الحوكمة (موقع؟).

Ответ:
- اوامر تكرار معلقة تتجاوز SLA.
- Он получил псевдоним اقل من العتبة.
- مخالفات الاحتفاظ (проявление لم يجدد قبل الانتهاء).

Ответ на вопрос:
- Grafana JSON `docs/source/grafana_sorafs_pin_registry.json` позволяет просматривать манифесты, создавать псевдонимы, отставание в работе и т. д. SLA, задержка и провисание, а также возможность подключения к сети.

## Runbooks

- Создайте `docs/source/sorafs/migration_ledger.md` для создания реестра.
- Код: `docs/source/sorafs/runbooks/pin_registry_ops.md` (Мистер Дэйв) Сделайте это.
- В фильме: Он был убит в 2007 году в 1990 году.
- Доступ к API для загрузки (например, Docusaurus).

## الاعتماديات والتسلسل

1. Установите флажок ManifestValidator.
2. Установите Norito + установите флажок .
3. Покупка + раздача еды.
4. Календарь матчей «Спорт-Сити» и «Спорт-Сити».
5. Используйте приложения/runbooks, чтобы получить информацию о том, как это сделать.

Он был убит в 1980-х годах в SF-4 в 1980-х годах в Нью-Йорке.
Включите REST в следующий раз:

- `GET /v1/sorafs/pin` и `GET /v1/sorafs/pin/{digest}` манифестирует
  Назовите псевдонимы واوامر التكرار وكائن اتستاشن مشتق من هاش اخر كتلة.
- псевдоним `GET /v1/sorafs/aliases` и `GET /v1/sorafs/replication`
  Он был убит в 2007 году.

Запуск CLI (`iroha app sorafs pin list`, `pin show`, `alias list`,
`replication list`) Зарегистрируйтесь в реестре реестра
Получите доступ к API.