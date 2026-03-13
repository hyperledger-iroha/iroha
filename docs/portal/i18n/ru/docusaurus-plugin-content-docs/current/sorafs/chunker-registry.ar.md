---
lang: ru
direction: ltr
source: docs/portal/docs/sorafs/chunker-registry.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
идентификатор: chunker-реестр
title: سجل ملفات chunker في SoraFS
Sidebar_label: Добавить чанкёр
описание: Создан новый блок управления чанкером на SoraFS.
---

:::примечание
Был установлен `docs/source/sorafs/chunker_registry.md`. Он был убит в фильме "Сфинкс" القديمة.
:::

## Загрузка чанкера для SoraFS (SF-2a)

Он создал SoraFS, а затем разбил фрагменты, чтобы получить возможность использовать его.
Получение данных от CDC в течение всей недели в режиме дайджеста/мультикодека в режиме реального времени. проявляет وأرشيفات CAR.

على مؤلفي الملفات الرجوع إلى
[`docs/source/sorafs/chunker_profile_authoring.md`](./chunker-profile-authoring.md)
للاطلاع على البيانات المطلوبة وقائمة التحقق وقالب المقترح قبل إرسال أي دخال جديد.
Он был создан в 2007 году.
[قائمة تحقق إطلاق السجل](./chunker-registry-rollout-checklist.md) и
[Манифест в промежуточной версии](./staging-manifest-playbook)
Все запланированные матчи и постановка «Нью-Йорка».

### الملفات

| Пространство имен | الاسم | СемВер | معرف الملف | الحد الأدنى (بايت) | الهدف (بايت) | الحد الأقصى (بايت) | قناع القطع | Мультихэш | بدائل | ملاحظات |
|-----------|-------|--------|------------|--------------------|---------------|----|-----------|-----------|--------|---------|
| `sorafs` | `sf1` | `1.0.0` | `1` | 65536 | 262144 | 524288 | `0x0000ffff` | `0x1f` (BLAKE3-256) | `["sorafs.sf1@1.0.0", "sorafs.sf1@1.0.0"]` | Подробнее о светильниках SF-1 |

Создан для `sorafs_manifest::chunker_registry` (например, [`chunker_registry_charter.md`](./chunker-registry-charter.md)). Код для `ChunkerProfileDescriptor`:

* `namespace` – используется для проверки работоспособности (например, `sorafs`).
* `name` – تسمية مقروءة للبشر (`sf1`, `sf1-fast`, …).
* `semver` – Вы можете получить дополнительную информацию о программе.
* `profile` – `ChunkProfile` Параметры (мин/цель/макс/маска).
* `multihash_code` – мультихэш-запрос обрабатывает фрагмент фрагмента (`0x1f`
  Установите флажок SoraFS).

В манифесте بتسلسل الملفات عبر `ChunkingProfileV1`. تسجل البنية بيانات السجل (пространство имен, имя, семвер) إلى جانب
Центр по контролю и профилактике заболеваний США (CDC) объявил о создании Центра по контролю и профилактике заболеваний США (CDC). Он сказал, что Сэнсэйл Уинстон в 1993 году `profile_id`
В 2007 году он сказал: Вы можете получить доступ к HTTP-запросу
Пожалуйста, проверьте `Accept-Chunker`. Он был убит в 2007 году в Нью-Йорке.
(`namespace.name@semver`) Зарегистрируйтесь для `profile_aliases`, затем нажмите кнопку .

Доступ к интерфейсу CLI:

```
$ cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- --list-profiles
[
  {
    "namespace": "sorafs",
    "name": "sf1",
    "semver": "1.0.0",
    "handle": "sorafs.sf1@1.0.0",
    "profile_id": 1,
    "min_size": 65536,
    "target_size": 262144,
    "max_size": 524288,
    "break_mask": "0x0000ffff",
    "multihash_code": 31
  }
]
```

Вызовите CLI для обработки JSON (`--json-out`, `--por-json-out`, `--por-proof-out`,
`--por-sample-out`) `-`, который находится в стандартном интерфейсе пользователя. Кейл Дэйл Сейр
Он сказал, что это будет означать, что он сделал это для того, чтобы сделать это.

### مصفوفة التوافق وخطة الإطلاق


Установите флажок для `sorafs.sf1@1.0.0` и установите флажок `sorafs.sf1@1.0.0`. фильм "Мост"
Используется для CARv1 + SHA-256 в зависимости от конфигурации (`Accept-Chunker` + `Accept-Digest`).| المكون | حالة | ملاحظات |
|--------|--------|---------|
| `sorafs_manifest_chunk_store` | ✅ مدعوم | يتحقق من المقبض المعتمد + البدائل, ويبث التقارير عبر `--json-out=-`, ويفرض Код: `ensure_charter_compliance()`. |
| `sorafs_manifest_stub` | ⚠️ قديم | مُنشئ قديم؛ استخدم `iroha app sorafs toolkit pack` для CAR/манифеста и `--plan=-` для проверки работоспособности. |
| `sorafs_provider_advert_stub` | ⚠️ قديم | مساعد تحقق оффлайн فقط؛ Реклама провайдера в Интернете. |
| `sorafs_fetch` (оркестратор разработчика) | ✅ مدعوم | Загрузите `chunk_fetch_specs` и установите `range` для CARv2. |
| Исправления в SDK (Rust/Go/TS) | ✅ مدعوم | يُعاد توليدها عبر `export_vectors`; Он сказал, что О'Лэнн в фильме "Пансионат" в Нью-Йорке сказал: المجلس. |
| Открыт шлюз Torii | ✅ مدعوم | Создан `Accept-Chunker`, установлен в `Content-Chunker`, установлен Bridge CARv1, установленный на сервере. понизить версию الصريحة. |

Ответ на вопрос:

- **Обработка фрагментов** — вызов CLI для Iroha `sorafs toolkit pack` обрабатывает фрагменты, CAR, Он был выбран PoR в Лос-Анджелесе.
- **Реклама поставщика** Используется для установки `/v2/sorafs/providers` (только для `range`).
- **Шлюз для шлюза** — Проверьте настройки `Content-Chunker`/`Content-Digest` в разделе `Content-Chunker`/`Content-Digest`. Д-р Миссисипи Он был построен в 2007 году на мосту в Атлантическом океане.

Сюжет фильма: Он сказал, что в фильме "Нет" (Миссис Миссисипи) المقترح) قبل وسم
`sorafs.sf1@1.0.0` используется для установки моста CARv1 в Сан-Франциско.

Для создания PoR необходимо создать фрагмент/сегмент/лист, который можно использовать для проверки:

```
$ cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- ./docs.tar \
    --por-proof=0:0:0 --por-proof-out=leaf.proof.json
```

Его имя было отправлено в رقمي (`--profile-id=1`).
(`--profile=sorafs.sf1@1.0.0`); Для этого необходимо создать пространство имен/имя/семвер.
Создан в Нью-Йорке.

Создайте `--promote-profile=<handle>` для создания файла JSON в режиме онлайн-обработки (Берег в формате JSON).
(например) для `chunker_registry_data.rs` в случае необходимости:

```
$ cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- \
    --promote-profile=sorafs.sf1@1.0.0
```

يتضمن التقرير الرئيسي (وملف البرهان الاختياري) дайджест الجذري, وbytes الأوراق المُعاينة
(на английском языке) и дайджесты الأشقاء для сегмента/части
64 КиБ/4 КиБ в формате `por_root_hex`.

В фильме рассказывается о Джоне Миссисипи в фильме "Старый мир".
`--por-proof-verify` (доступен интерфейс командной строки `"por_proof_verified": true`, который можно использовать при настройке):

```
$ cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- ./docs.tar \
    --por-proof-verify=leaf.proof.json
```

لأخذ عينات على دفعات، استخدم `--por-sample=<count>` ويمكنك تمريرseed/مسار إخراج اختياري.
Пользователь CLI, созданный пользователем (`splitmix64`, посеянный)
Ниже приведены сведения:```
$ cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- ./docs.tar \
    --por-sample=8 --por-sample-seed=0xfeedface --por-sample-out=por.samples.json
```
```

يعكس manifest stub البيانات نفسها، وهو مناسب عند برمجة اختيار `--chunker-profile-id` في
الـ pipelines. كما تقبل CLIs الخاصة بـ chunk store صيغة المقبض المعتمد
(`--profile=sorafs.sf1@1.0.0`) لتجنب ترميز معرفات رقمية صلبة في سكريبتات البناء:

```
$ Cargo Run -p sorafs_manifest --bin sorafs_manifest_stub -- --list-chunker-profiles
[
  {
    «идентификатор_профиля»: 1,
    "пространство имен": "сорафа",
    "имя": "SF1",
    "семвер": "1.0.0",
    "handle": "sorafs.sf1@1.0.0",
    "min_size": 65536,
    "target_size": 262144,
    "max_size": 524288,
    "break_mask": "0x0000ffff",
    «мультихэш_код»: 31
  }
]
```

يتطابق الحقل `handle` (`namespace.name@semver`) مع ما تقبله CLIs عبر
`--profile=…`، مما يجعل نسخه مباشرة إلى الأتمتة آمنًا.

### التفاوض على chunkers

تعلن البوابات والعملاء الملفات المدعومة عبر provider adverts:

```
ProviderAdvertBodyV1 {
    ...
    chunk_profile: Profile_id (отображение профиля)
    возможности: [...]
}
```

يُعلن جدولة chunk متعددة المصادر عبر القدرة `range`. يقبلها CLI عبر
`--capability=range[:streams]` حيث يشفّر اللاحق الرقمي الاختياري التوازي المفضل لدى المزود
لجلب النطاق (على سبيل المثال، `--capability=range:64` تعلن ميزانية 64 stream).
وعند حذفه، يعود المستهلكون إلى تلميح `max_streams` العام المنشور في مكان آخر من الإعلان.

عند طلب بيانات CAR، يجب أن يرسل العملاء ترويسة `Accept-Chunker` بقائمة tuples
`(namespace, name, semver)` بحسب ترتيب التفضيل:

```

تختار البوابات ملفًا مدعومًا من الطرفين (الافتراضي `sorafs.sf1@1.0.0`) عبر ترويسة
Приложение `Content-Chunker`. تضمن проявляет الملف المختار и تتمكن العقد اللاحقة من التحقق
Для создания фрагментов можно использовать HTTP.

### ДАВАЙТЕ АВТОМОБИЛЬ

Откройте манифест CIDv1 для `dag-cbor` (`0x71`). وللتوافق القديم نحتفظ
Получение CARv1+SHA-2:

* ** المسار الأساسي** – CARv2, дайджест DBAKE3 (многохэш `0x1f`),
  `MultihashIndexSorted`, его имя - Хан Сэн.
  Установите флажок `Accept-Chunker` или `Accept-Digest: sha2-256`.

Он написал дайджест المعتمد.

### المطابقة

* Обратите внимание на `sorafs.sf1@1.0.0` светильники.
  `fixtures/sorafs_chunker` Дополнительная информация
  `fuzz/sorafs_chunker`. Он был создан для работы с Rust и Go и Node.
* Установите `chunker_registry::lookup_by_profile` для получения дополнительной информации о `ChunkProfile::DEFAULT` в режиме онлайн. عرضية.
* В этом случае отображаются изменения для `iroha app sorafs toolkit pack` и `sorafs_manifest_stub` для `sorafs_manifest_stub`.