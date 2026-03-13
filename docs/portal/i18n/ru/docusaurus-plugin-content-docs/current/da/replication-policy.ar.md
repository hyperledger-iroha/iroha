---
lang: ru
direction: ltr
source: docs/portal/docs/da/replication-policy.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::примечание
Код `docs/source/da/replication_policy.md`. ابق النسختين متزامنتين حتى يتم
سحب الوثائق القديمة.
:::

# Сценарий Колумбийского университета (DA-4)

_الحالة: Источник: Рабочая группа по базовому протоколу / группа хранения данных / SRE_

Вы можете принять участие в приеме الخاص بـ DA اهداف احتفاظ حتمية لكل فئة blob مذكورة في
`roadmap.md` (формат DA-4). يرفض Torii Дополнительная информация
Он сказал, что хочет, чтобы это произошло, когда он был в Стокгольме / Колумбии. بعدد
Он был убит Джоном Уилсоном в Нью-Йорке.

## Справочные материалы

| فئة капля | احتفاظ горячая | احتفاظ холод | النسخ المطلوبة | فئة التخزين | وسم الحوكمة |
|----------|------------|--------------|----------------|-------------|-------------|
| `taikai_segment` | 24 месяца | 14 дней | 5 | `hot` | `da.taikai.live` |
| `nexus_lane_sidecar` | 6 дней | 7 дней | 4 | `warm` | `da.sidecar` |
| `governance_artifact` | 12 дней | 180 дней | 3 | `cold` | `da.governance` |
| _По умолчанию (в режиме реального времени)_ | 6 дней | 30 дней | 3 | `warm` | `da.default` |

Зарегистрируйтесь для `torii.da_ingest.replication_policy` и нажмите кнопку .
طلبات `/v2/da/ingest`. يعيد Torii манифестирует مع ملف الاحتفاظ المفروض ويصدر
Создатель Сэнсэй Уилсон Уинстон Миссисипи в фильме "Страна" Использование SDK
المتقادمة.

### فئات توفر Тайкай

تعلن манифестирует توجيه Taikai (`taikai.trm`) и `availability_class`
(`hot`, `warm`, или `cold`). يفرض Torii السياسة المطابقة قبل التقسيم بحيث يمكن
Он был показан в прямом эфире в честь Дня Рождения. Ответ:

| فئة التوفر | احتفاظ горячая | احتفاظ холод | النسخ المطلوبة | فئة التخزين | وسم الحوكمة |
|------------|------------|-------------|----------------|-------------|-------------|
| `hot` | 24 месяца | 14 дней | 5 | `hot` | `da.taikai.live` |
| `warm` | 6 дней | 30 дней | 4 | `warm` | `da.taikai.warm` |
| `cold` | 1 месяц | 180 дней | 3 | `cold` | `da.taikai.archive` |

Установите `hot`, чтобы установить его в режиме онлайн. قم
بتجاوز الافتراضيات عبر
`torii.da_ingest.replication_policy.taikai_availability` اذا كانت شبكتك تستخدم
اهدافا مختلفة.

## الاعداد

تعيش السياسة تحت `torii.da_ingest.replication_policy` и выберите *default* مع
مصفوفة переопределяет لكل فئة. معرفات الفئة غير حساسة لحالة الاحرف وتقبل
`taikai_segment`, `nexus_lane_sidecar`, `governance_artifact`, и `custom:<u16>`
للامتدادات المعتمدة حوكما. Используйте `hot`, `warm` и `cold`.

```toml
[torii.da_ingest.replication_policy.default_retention]
hot_retention_secs = 21600          # 6 h
cold_retention_secs = 2592000       # 30 d
required_replicas = 3
storage_class = "warm"
governance_tag = "da.default"

[[torii.da_ingest.replication_policy.overrides]]
class = "taikai_segment"
[torii.da_ingest.replication_policy.overrides.retention]
hot_retention_secs = 86400          # 24 h
cold_retention_secs = 1209600       # 14 d
required_replicas = 5
storage_class = "hot"
governance_tag = "da.taikai.live"
```

Он сказал Хейну и Лилле Бэнглу в 2007 году. لتشديد فئة, حدّث override
المطابق؛ Для этого необходимо установить `default_retention`.

Он Тэхен и Тайкай Тайкай
`torii.da_ingest.replication_policy.taikai_availability`:

```toml
[[torii.da_ingest.replication_policy.taikai_availability]]
availability_class = "cold"
[torii.da_ingest.replication_policy.taikai_availability.retention]
hot_retention_secs = 3600          # 1 h
cold_retention_secs = 15552000     # 180 d
required_replicas = 3
storage_class = "cold"
governance_tag = "da.taikai.archive"
```

## دلالات الانفاذ

- يستبدل Torii `RetentionPolicy` для получения дополнительной информации о том, как это сделать.
  В манифесте.
- ترفض демонстрирует المبنية مسبقا التي تعلن ملف احتفاظ غير مطابق بـ
  `400 schema mismatch` был установлен в режиме онлайн.
- Переопределение переопределения переопределения (`blob_class`, встроенный режим проверки)
  لاظهار المتصلين غير الملتزمين اثناء развертывание.راجع [خطة ingest لتوفر البيانات](ingest-plan.md) (قائمة التحقق) للبوابة المحدثة
التي تغطي انفاذ الاحتفاظ.

## عمل اعادة التكرار (картина DA-4)

انفاذ الاحتفاظ هو الخطوة الاولى فقط. يجب على المشغلين ايضا اثبات ان манифестирует
الحية واوامر التكرار تبقى متسقة مع السياسة المكونة حتى يتمكن SoraFS во время
Кляксы в сказке.

1. **راقب الانحراف.** يصدر Torii
   `overriding DA retention policy to match configured network baseline` عندما
   Он сказал, что хочет сделать это. قرن هذا السجل مع قياسات
   `torii_sorafs_replication_*` запускается в режиме ожидания.
2. **В случае необходимости использования **.

   ```bash
   cargo xtask da-replication-audit \
     --config configs/iroha/torii.toml \
     --manifest spool/da/manifests/*.json \
     --replication-order artifacts/da/orders/*.norito \
     --json-out artifacts/da/replication_audit.json
   ```

   Код `torii.da_ingest.replication_policy` в разделе "Программы"
   Создание манифеста (JSON и Norito) и создание полезных данных
   `ReplicationOrderV1` — дайджест манифеста. Ответ на вопрос:

   - `policy_mismatch` - Отображается в манифесте, указанном в документе.
     (Lan يجب ان يحدث ذلك الا اذا كان Torii مكونا بشكل خاطئ).
   - `replica_shortfall` - امر التكرار الحي يطلب نسخا اقل من
     `RetentionPolicy.required_replicas` в يقدم تعيينات اقل من الهدف.

   Служба поддержки CI/дежурства по вызову в режиме реального времени
   فورا. Создайте файл JSON `docs/examples/da_manifest_review_template.md`.
   لتصويت البرلمان.
3. **Зарегистрируйтесь.** Выполните настройку `ReplicationOrderV1`.
   Он сказал, что хочет сделать это.
   [SoraFS рынок емкости хранения](../sorafs/storage-capacity-marketplace.md)
   Он выступил с речью в газете "Турнир". للتجاوزات الطارئة, اربط مخرجات
   CLI для `iroha app da prove-availability` для SREs в дайджесте
   Это PDP.

Создан файл `integration_tests/tests/da/replication_policy.rs`; تقوم
Для получения дополнительной информации обратитесь к `/v2/da/ingest`.
манифест المسترجع يعرض الملف المفروض بدلا من نية المتصل.