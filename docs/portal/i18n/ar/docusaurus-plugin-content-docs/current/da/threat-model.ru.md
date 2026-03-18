---
lang: ar
direction: rtl
source: docs/portal/docs/da/threat-model.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::note Канонический источник
Эта страница отражает `docs/source/da/threat_model.md`. آخر الإصدارات
:::

# Модель угроз Data Availability Sora Nexus

_Последняя проверка: 2026-01-19 — Следующая проверка: 2026-04-19_

Частота обслуживания: Data Availability Working Group (<=90 дней). Каждая
редакция должна появиться в `status.md` со ссылками на активные тикеты
смягчений и артефакты симуляций.

## Цель и область

Программа Data Availability (DA) обеспечивает доступность Taikai broadcast,
Nexus lane blobs и governance artefacts при Byzantine, сетевых и операционных
сбоях. Эта модель угроз закрепляет инженерную работу DA-1 (архитектура и модель
угроз) и служит базовым ориентиром для дальнейших задач DA (DA-2 .. DA-10).

Компоненты в рамках:
- Расширение Torii DA ingest и writers Norito metadata.
- Деревья хранения blobs на SoraFS (hot/cold tiers) и политики репликации.
- Nexus block commitments (wire formats, proofs, light-client APIs).
- Hooks принудительного PDP/PoTR для DA payloads.
- Операторские процессы (pinning, eviction, slashing) и observability pipelines.
- Governance approvals для допуска/исключения DA операторов и контента.

Вне рамок документа:
- Полная экономическая модель (зафиксирована в workstream DA-7).
- Базовые протоколы SoraFS, уже покрытые SoraFS threat model.
- Client SDK ergonomics вне угрозной поверхности.

## Архитектурный обзор

1. **Submission:** Клиенты отправляют blobs через Torii DA ingest API. أوزيل
   разбивает blobs, кодирует Norito manifests (тип blob, lane, epoch, flags codec),
   и хранит chunks в hot tier SoraFS.
2. **Advertisement:** Pin intents и replication hints распространяются к
   storage providers через registry (SoraFS marketplace) с policy tags,
   определяющими цели hot/cold retention.
3. **Commitment:** Nexus sequencers включают blob commitments (CID + optional
   KZG roots) в канонический блок. يستخدم العملاء الخفيفون تجزئة الالتزام و
   объявленную metadata для проверки availability.
4. **Replication:** Storage nodes подтягивают назначенные shares/chunks, проходят
   PDP/PoTR challenges и перемещают данные между hot/cold tiers по политике.
5. **Fetch:** Потребители получают данные через SoraFS или DA-aware gateways,
   التحقق من البراهين وإنشاء طلبات الإصلاح عند تحديث النسخ المتماثلة.
6. **Governance:** Парламент и комитет DA утверждают операторов, rent schedules
   и escalations enforcement. Governance artefacts проходят тем же DA путем для
   прозрачности процесса.

## Активы и владельцы

Шкала влияния: **Critical** ломает безопасность/живучесть ledger; **عالية**
блокирует DA backfill или клиентов; **معتدل** اكتب جملة، لا
восстановимо; **Low** ограниченное влияние.

| الأصول | الوصف | النزاهة | التوفر | السرية | المالك |
| --- | --- | --- | --- | --- | --- |
| DA blobs (chunks + manifests) | Taikai, lane, governance blobs в SoraFS | حرجة | حرجة | معتدل | DA WG / فريق التخزين |
| Norito DA manifests | البيانات الوصفية المميزة للنقط | حرجة | عالية | معتدل | مجموعة عمل البروتوكول الأساسي |
| كتلة الالتزامات | CIDs + KZG roots внутри Nexus blocks | حرجة | عالية | منخفض | مجموعة عمل البروتوكول الأساسي |
| جداول PDP/PoTR | Каденс enforcement для DA replicas | عالية | عالية | منخفض | فريق التخزين |
| تسجيل المشغل | Одобренные storage providers и политики | عالية | عالية | منخفض | مجلس الحكم |
| Rent and incentive records | Записи ledger для DA rent и штрафов | عالية | معتدل | منخفض | مجموعة عمل الخزانة |
| لوحات التحكم في إمكانية الملاحظة | DA SLOs, глубина репликации, алерты | معتدل | عالية | منخفض | SRE / إمكانية الملاحظة |
| Repair intents | مستلزمات إعادة تدوير القطع المرغوبة | معتدل | معتدل | منخفض | فريق التخزين |

## الحماية والقوة

| Актор | Возможности | Мотивации | مساعدة |
| --- | --- | --- | --- |
| Malicious client | Отправка النقط المشوهة، إعادة تشغيل البيانات التي لا معنى لها، استيعاب DoS. | بث مباشر Taikai, حقنة غير صالحة. | Нет привилегированных ключей. |
| Byzantine storage node | إسقاط النسخ المتماثلة، وتزوير أدلة PDP/PoTR، والتواطؤ. | قم بالاحتفاظ بـ DA، وتخلص من الإيجار، وتمتع بالبيانات. | احصل على بيانات اعتماد المشغل الصالحة. |
| Compromised sequencer | حذف الالتزامات، والمراوغة في الكتل، وإعادة ترتيب البيانات الوصفية. | Склыть DA Submits, создать несогласованность. | التوافق المطلق كبير. |
| Insider operator | حوكمة الشركات موجودة, بعد سياسات الاحتفاظ, الحصول على بيانات الاعتماد. | أسلوب اقتصادي وتخريب. | توفير البنية التحتية الساخنة/الباردة. |
| Network adversary | استخدام القسم، النسخ المتماثل، حركة مرور MITM. | توافر الصورة، وتدهور SLOs. | Не ломает TLS, но может замедлять/дропать. |
| Observability attacker | إضافة لوحات المعلومات/التنبيهات، لدعم الأحداث. | Скрыть DA outages. | تحتاج إلى توصيل خط أنابيب القياس عن بعد. |

## Границы доверия

- **حدود الدخول:** العميل -> امتداد Torii DA. مصادقة جديدة على
  запрос, rate limiting и валидация payload.
- **حدود النسخ المتماثل:** تقوم عقد التخزين باستبدال القطع والبراهين. Узлы
  взаимно аутентифицированы, но могут вести себя Byzantine.
- **حدود دفتر الأستاذ:** بيانات الكتلة المُلتزم بها مقابل التخزين خارج السلسلة. Консенсус
  الحفاظ على الجودة، ولكن التوافر يتطلب إنفاذًا خارج السلسلة.
- **حدود الحوكمة:** قرار المجلس/البرلمان بشأن المشغلين والموازنة والميزانية
  slashing. Нарушения здесь напрямую влияют на DA deployment.
- **Observability boundary:** Сбор metrics/logs и экспорт в dashboards/alert
  الأدوات. العبث بإنهاء الانقطاعات أو الهجمات.

## سيناريوهات اللعب والمعارضين

### نصائح لاستيعاب الطعام

** السيناريو:** يقوم العميل الضار بتصحيح حمولات Norito أو
النقط كبيرة الحجم لمصادر الموارد أو البيانات الوصفية الإضافية.

**Контрмеры**
- التحقق من صحة مخطط Norito مع الإصدارات المعدلة حديثًا؛ reject unknown flags.
- تحديد المعدل والمصادقة على Torii لاستيعاب نقطة النهاية.
- Ограничения chunk size и детерминированный encoding в SoraFS chunker.
- يظهر خط أنابيب القبول сограняет المجموع الاختباري فقط بعد совпадения.
- ذاكرة التخزين المؤقت لإعادة التشغيل الحتمية (`ReplayCache`) تتجاهل `(lane, epoch, sequence)`,
  сохраняет high-water marks на диске и отвергает duplicates/stale replays; الملكية
  وتُظهر أحزمة الزغب بصمات الأصابع المتباينة وعمليات الإرسال خارج الترتيب.
  [الصناديق/iroha_core/src/da/replay_cache.rs:1]
  [fuzz/da_replay_cache.rs:1] [صناديق/iroha_torii/src/da/ingest.rs:1]

**Оставшиеся пробелы**
- Torii ingest должен встроить replay cache в admission и сохранять sequence
  cursors между рестартами.
- Norito DA schemas имеют выделенный fuzz harness (`fuzz/da_ingest_schema.rs`) для
  проверки encode/decode инвариантов; coverage dashboards должны сигнализировать
  при регрессии.

### Удержание репликации

**السيناريو:** يقوم مشغلو التخزين البيزنطيون بتعيينات الدبوس، ثم يسقطونها
قطع وإنتاج PDP/PoTR من خلال الاستجابات المزورة أو التواطؤ.

**Контрмеры**
- تم زيادة جدول تحدي PDP/PoTR لحمولات DA مع ظهورها على مر الزمن.
- Multi-source replication с quorum thresholds; fetch orchestrator выявляет
  missing shards и инициирует repair.
- Governance slashing привязан к failed proofs и missing replicas.
- Automated reconciliation job (`cargo xtask da-commitment-reconcile`) сравнивает
  استيعاب الإيصالات مع التزامات DA (SignedBlockWire/`.norito`/JSON)، النموذج
  JSON evidence bundle для governance и падает при missing/mismatched tickets,
  чтобы Alertmanager мог пейджить по omission/tampering.

**Оставшиеся пробелы**
- Simulation harness в `integration_tests/src/da/pdp_potr.rs` (tests:
  `integration_tests/tests/da/pdp_potr_simulation.rs`) كشف التواطؤ
  и partition, проверяя детерминированное выявление Byzantine. Продолжать
  التوسيع مع DA-5.
- Политика cold-tier eviction требует signed audit trail, чтобы исключить covert drops.

### متابعة الالتزامات

**السيناريو:** جهاز التسلسل المخترق ينشر الكتل ذات الترشيح/التحسين DA
commitments, вызывая fetch failures и light-client inconsistencies.

**Контрмеры**
- Консенсус проверяет block proposals против DA submission queues; أقرانهم
  отвергают предложения без обязательных commitments.
- Light clients проверяют inclusion proofs перед выдачей fetch handles.
- Audit trail сравнивает submission receipts и block commitments.
- Automated reconciliation job (`cargo xtask da-commitment-reconcile`) сравнивает
  استيعاب الإيصالات مع الالتزامات DA (SignedBlockWire/`.norito`/JSON)، النموذج
  JSON evidence bundle и падает при missing/mismatched tickets для Alertmanager.

**Оставшиеся пробелы**
- Закрыто reconciliation job + Alertmanager hook; حزم الحكم
  по умолчанию ingest-ят JSON evidence bundle.

### قسم الشبكة والمراقبة

**Сценарий:** Adversary разделяет replication network, мешая узлам получать
назначенные chunks или отвечать на PDP/PoTR challenges.**الضوابط**
- يوفر مزود الخدمة متعدد المناطق مسارات شبكة مختلفة.
- تتضمن نوافذ التحدي الارتعاش والإصلاح الاحتياطي خارج النطاق.
- لوحات معلومات إمكانية المراقبة تراقب عمق النسخ المتماثل وتحدي النجاح وما إلى ذلك
  جلب الكمون مع عتبات التنبيه.

**المشكلة الأساسية**
- محاكاة التقسيم لأحداث Taikai المباشرة بعد الخروج؛ مطلوب اختبارات نقع.
- الحجز السياسي لإصلاح عرض النطاق الترددي غير رسمي.

### إخلاء الشبكة

**السيناريو:** يقوم المشغل بالتلاعب بسياسات الاحتفاظ بالسجل،
القائمة البيضاء لمقدمي الخدمات الضارة أو إرسال التنبيهات.

**الضوابط**
- تتطلب إجراءات الحوكمة التوقيعات متعددة الأطراف والسجلات الموثقة Norito.
- تغييرات السياسة تنشر الاشتراكات في سجلات المراقبة والأرشفة.
- فرض خط أنابيب المراقبة - وهو عبارة عن سجلات Norito للإلحاق فقط مع تسلسل التجزئة.
- أتمتة مراجعة الوصول ربع السنوية (`cargo xtask da-privilege-audit`)
  أدلة البيان/إعادة التشغيل (بالإضافة إلى منافذ المشغلين)، حذف المفقودين/غير الدليل/
  الإدخالات القابلة للكتابة عالميًا، واحصل على حزمة JSON الموقعة للوحات معلومات الإدارة.

**المشكلة الأساسية**
- أدلة العبث بلوحة القيادة توفر لقطات موقعة.

## Reеester остаточный riskков

| خطر | احتمال | التأثير | المالك | خطة التخفيف |
| --- | --- | --- | --- | --- |
| إعادة تشغيل بيانات DA حتى يتم الحصول على ذاكرة التخزين المؤقت لتسلسل DA-2 | ممكن | معتدل | مجموعة عمل البروتوكول الأساسي | تحقيق تسلسل ذاكرة التخزين المؤقت + التحقق من صحة nonce في DA-2؛ إضافة اختبارات الانحدار. |
| تواطؤ PDP/PoTR عند استخدام الكمبيوتر | من غير المرجح | عالية | فريق التخزين | احصل على جدول التحدي الجديد مع أخذ العينات من مختلف مقدمي الخدمة؛ التحقق من خلال تسخير المحاكاة. |
| فجوة تدقيق الإخلاء من الدرجة الباردة | ممكن | عالية | SRE / فريق التخزين | نسخ سجلات التدقيق الموقعة والإيصالات على السلسلة لعمليات الإخلاء؛ مراقبة لوحات المعلومات. |
| الكمون الإغفال التسلسل | ممكن | عالية | مجموعة عمل البروتوكول الأساسي | يقارن `cargo xtask da-commitment-reconcile` الإيصالات مقابل الالتزامات (SignedBlockWire/`.norito`/JSON) ويتحكم في الحوكمة في حالة التذاكر المفقودة/غير المتطابقة. |
| مرونة التقسيم для Taikai البث المباشر | ممكن | حرجة | الشبكات TL | تدريبات التقسيم; استعادة عرض النطاق الترددي إصلاح. توثيق تجاوز فشل SOP. |
| الحكم الانجراف امتياز | من غير المرجح | عالية | مجلس الحكم | ربع سنوي `cargo xtask da-privilege-audit` (البيان/إعادة التشغيل + مسارات إضافية) مع بوابة JSON + لوحة القيادة؛ تثبيت عناصر التدقيق على السلسلة. |

## المتابعات المطلوبة

1. نشر مخططات Norito لاستيعاب DA ومثال المتجهات (внести в DA-2).
2. قم بتغيير ذاكرة التخزين المؤقت لإعادة التشغيل من خلال Torii DA لاستيعاب المؤشرات التسلسلية وحفظها
   قبل إعادة تشغيل المطعم.
3. ** اكتمل (2026/02/05):** مجموعة أدوات محاكاة PDP/PoTR
   التواطؤ + التقسيم مع نمذجة جودة الخدمة المتراكمة ؛ سم. `integration_tests/src/da/pdp_potr.rs`
   (الاختبارات: `integration_tests/tests/da/pdp_potr_simulation.rs`) مع تحديد الاختبارات.
4. **اكتمل (29/05/2026):** `cargo xtask da-commitment-reconcile` сравнивает
   استيعاب الإيصالات مع التزامات DA (SignedBlockWire/`.norito`/JSON)، وحذفها
   `artifacts/da/commitment_reconciliation.json` وملحق بـ Alertmanager/Governance
   الحزم لتنبيهات الإغفال/التلاعب (`xtask/src/da.rs`).
5. ** اكتمل (29 مايو 2026): ** `cargo xtask da-privilege-audit` عرض البيان/إعادة التشغيل
   التخزين المؤقت (و المسارات من المشغلين)، حذف المفقود/غير الدليل/القابل للكتابة في العالم و
   قم بإنشاء حزمة JSON موقعة لمراجعات لوحة المعلومات/الإدارة
   (`artifacts/da/privilege_audit.json`)، سد الفجوة في أتمتة مراجعة الوصول.

**اذهب إلى آخر:**

- إعادة تشغيل ذاكرة التخزين المؤقت وهبوط المؤشرات في DA-2. تحقيق في
  `crates/iroha_core/src/da/replay_cache.rs` (منطق ذاكرة التخزين المؤقت) والتكامل Torii в
  `crates/iroha_torii/src/da/ingest.rs`، حيث يتم فحص بصمات الأصابع من خلال `/v1/da/ingest`.
- يتم تنفيذ عمليات محاكاة تدفق PDP/PoTR من خلال أداة إثبات التدفق في
  `crates/sorafs_car/tests/sorafs_cli.rs`، فتح تدفقات طلب PoR/PDP/PoTR،
  سيناريوهات الفشل من نماذج الألعاب.
- نتائج نقع القدرة والإصلاح في
  `docs/source/sorafs/reports/sf2c_capacity_soak.md`، Sumeragi مصفوفة النقع в
  `docs/source/sumeragi_soak_matrix.md` (هذه هي المتغيرات المحلية). هذا
  المصنوعات اليدوية تثبت التدريبات الطويلة من ريسترا ريسكوف.
- المصالحة + أتمتة امتياز التدقيق нанодится в
  `docs/automation/da/README.md` والأوامر الجديدة
  `cargo xtask da-commitment-reconcile` / `cargo xtask da-privilege-audit`; استخدم
  المخرجات الموضحة في `artifacts/da/` عند تقديم الأدلة إلى حزم الإدارة.

## أدلة المحاكاة ونمذجة جودة الخدمة (2026-02)

من أجل إنهاء متابعة DA-1 رقم 3، تمكنا من تحديد PDP/PoTR
تسخير المحاكاة в `integration_tests/src/da/pdp_potr.rs` (الاختبارات:
`integration_tests/tests/da/pdp_potr_simulation.rs`). تسخير распеделяет
يتم الاستخدام في 3 مناطق، لتقسيم الأقسام/التواطؤ إلى خارطة طريق حقيقية،
التخلص من تأخير PoTR ونموذج إصلاح تراكم، خارج الطبقة الساخنة
ميزانية الإصلاح. أكمل السيناريو الافتراضي (12 حقبة، 18 تحدي PDP + 2 PoTR
النوافذ لكل عصر) إلى المقاييس التالية:

<!-- BEGIN_DA_SIM_TABLE -->
<!-- AUTO-GENERATED by scripts/docs/render_da_threat_model_tables.py; do not edit manually. -->
| متري | القيمة | ملاحظات |
| --- | --- | --- |
| تم اكتشاف حالات فشل PDP | 48 / 49 (98.0%) | يتم اكتشاف جميع الأقسام; قطعة واحدة غير قابلة للاكتشاف مرتبطة بغضب واضح. |
| PDP يعني زمن انتقال الكشف | 0.0 العصور | يتم تثبيته في عصر آخر. |
| تم اكتشاف حالات فشل PoTR | 28 / 77 (36.4%) | يتم إجراء الكشف من خلال النشرة >=2 نوافذ PoTR، مما يؤدي إلى ترك معظم الأشخاص في سجل المخاطر المتبقية. |
| PoTR يعني زمن الكشف | 2.0 العصور | Сооответствует عتبة تأخر 2-عصر في التصعيد الأرشيفي. |
| ذروة قائمة انتظار الإصلاح | 38 بيان | تراكمت الأعمال المتراكمة عندما يتم تشغيل الأقسام لمدة 4 إصلاحات / عصر. |
| زمن الاستجابة ص95 | 30,068 مللي ثانية | قم بإزالة نافذة التحدي لمدة 30 ثانية مع +/- 75 مللي ثانية من الارتعاش لأخذ عينات جودة الخدمة. |
<!-- END_DA_SIM_TABLE -->

هذه النتائج تتجه نحو النماذج الأولية للوحات معلومات DA ومعايير السعادة
أمثلة على "تسخير المحاكاة + نمذجة جودة الخدمة" من خارطة الطريق.

الأتمتة تحدث ل
`cargo xtask da-threat-model-report [--out <path|->] [--seed <u64|0xhex>] [--config <path>]`,
التي تطلق العنان للحزام القديم وتكتب Norito JSON в
`artifacts/da/threat_model_report.json` للشفاء. يتم استخدام الأموال الجيدة
هذا الملف لتحديث المواد في المستندات والتنبيهات المتعلقة بمعدلات الكشف،
قوائم الانتظار الإصلاح أو عينات جودة الخدمة.

لتحديث الأجهزة اللوحية، استخدم `make docs-da-threat-model` لتتعرف عليه
`cargo xtask da-threat-model-report`, إعادة الاتصال
`docs/source/da/_generated/threat_model_report.json`، وقم بإعادة كتابة القسم من خلاله
`scripts/docs/render_da_threat_model_tables.py`. زيركالو `docs/portal`
(`docs/portal/docs/da/threat-model.md`) يتم الاتصال به كوسيلة للمزامنة.