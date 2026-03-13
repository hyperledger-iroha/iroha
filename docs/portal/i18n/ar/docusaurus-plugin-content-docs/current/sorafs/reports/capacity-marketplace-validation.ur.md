---
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/reports/capacity-marketplace-validation.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
العنوان: SoraFS التحقق من صحة سوق السعة
ملخص: قائمة التحقق من القبول، تأهيل موفر الخدمة، وسير العمل، وتسوية الخزانة، وبطاقة سوق السعة SoraFS، وبوابة GA.
العلامات: [SF-2c، القبول، قائمة المراجعة]
---

# SoraFS قائمة التحقق من صحة سوق السعة

**فترة المراجعة:** 2026-03-18 -> 2026-03-24  
**أصحاب البرنامج:** فريق التخزين (`@storage-wg`)، مجلس الإدارة (`@council`)، وزارة الخزانة (`@treasury`)  
**النطاق:** تجهيز خطوط الأنابيب للمزود، وتدفقات الفصل في المنازعات، وعمليات تسوية الخزانة من خلال SF-2c GA.

تتيح القائمة المرجعية التالية لمشغلي السوق إمكانية مراجعة المراجعة المطلوبة. هناك أدلة حتمية صفية (الاختبارات، التركيبات، أو الوثائق) لا تتضمن سوى إعادة تشغيل المدققين.

## قائمة التحقق من القبول

### إعداد الموفر| تحقق | التحقق من الصحة | الأدلة |
|-------|-----------|----------|
| إعلانات القدرة القانونية للتسجيل مقبول کرتا ہے | واجهة برمجة تطبيقات اختبار التكامل (API) `/v2/sorafs/capacity/declare` تعمل على معالجة التوقيعات والتقاط البيانات الوصفية وتسجيل العقدة وتسليمها والتحقق من صحة العقد. | `crates/iroha_torii/src/routing.rs:7654` |
| العقد الذكي الحمولات غير المتطابقة کو رفض کرتا ہے | اختبار الوحدة يتضمن معرفات المزود وحقول GiB المخصصة للإعلان الموقع والذي يتوافق مع الثبات. | `crates/iroha_core/src/smartcontracts/isi/sorafs.rs:3445` |
| تنبعث المصنوعات اليدوية الأساسية لـ CLI من کرتا ہے | تسخير CLI مخرجات Norito/JSON/Base64 الحتمية لـ کھتا ہے ورحلات الذهاب والإياب التحقق من صحة کرتا ہے تاکہ إعلانات المشغلين دون اتصال تیار کر سکیں. | `crates/sorafs_car/tests/capacity_cli.rs:17` |
| دليل المشغل لسير عمل القبول وحواجز الحماية الحكومية التي تغطي كرتا ہے | مخطط إعلان التوثيق، وافتراضيات السياسة، وخطوات مراجعة المجلس التي تعداد القرار. | `../storage-capacity-marketplace.md` |

### حل النزاعات| تحقق | التحقق من الصحة | الأدلة |
|-------|-----------|----------|
| سجلات النزاع ملخص الحمولة الأساسية التي لا تزال مستمرة | سجل نزاع اختبار الوحدة كرتا، وفك تشفير الحمولة النافعة المخزنة، وتأكيد الحالة المعلقة كرتا، وحتمية دفتر الأستاذ. | `crates/iroha_core/src/smartcontracts/isi/sorafs.rs:1835` |
| منشئ نزاعات CLI المخطط الأساسي سے يتطابق مع کرتا ہے | اختبار CLI `CapacityDisputeV1` هو لمخرجات Base64/Norito وملخصات JSON التي تغطي الكرتا، وهي عبارة عن حزم أدلة تجزئة حتمية. | `crates/sorafs_car/tests/capacity_cli.rs:455` |
| إعادة تشغيل نزاع الاختبار/حتمية العقوبة بشكل ثابت | القياس عن بعد لإثبات الفشل من خلال إعادة تشغيل دفتر الأستاذ واللقطات الائتمانية والنزاع مرتين، بالإضافة إلى القطع المائلة للأقران والقطع الحتمية. | `crates/iroha_core/src/smartcontracts/isi/sorafs.rs:3430` |
| تصعيد دليل التشغيل وتدفق الإلغاء في مستند كرتا | دليل العمليات سير عمل المجلس ومتطلبات الأدلة وإجراءات التراجع والتقاط البيانات. | `../dispute-revocation-runbook.md` |

### تسوية الخزينة| تحقق | التحقق من الصحة | الأدلة |
|-------|-----------|----------|
| استحقاق دفتر الأستاذ إسقاط نقع لمدة 30 يومًا يتطابق مع كرتا ہے | يحتوي موفري اختبارات Soak على 30 نافذة تسوية، بالإضافة إلى إدخالات دفتر الأستاذ ومرجع الدفع المتوقع الذي يختلف اختلافًا كبيرًا. | `crates/iroha_core/src/smartcontracts/isi/sorafs.rs:3000` |
| تسوية تصدير دفتر الأستاذ ہر رات سجل ہوتا ہے | `capacity_reconcile.py` توقعات دفتر الأستاذ للرسوم التي نفذت عمليات تصدير نقل XOR وقارنت البطاقة، وتصدر مقاييس Prometheus بطاقة، وAlertmanager وهي بوابة موافقة الخزانة. | `scripts/telemetry/capacity_reconcile.py:1`،`docs/source/sorafs/runbooks/capacity_reconciliation.md:1`،`dashboards/alerts/sorafs_capacity_rules.yml:100` |
| عقوبات لوحات معلومات الفواتير وسطح القياس عن بعد الاستحقاق | Grafana استيراد استحقاق GiB-hour وعدادات الضربات ومؤامرة الضمانات المستعبدة وإمكانية الرؤية عند الطلب متعددة. | `dashboards/grafana/sorafs_capacity_penalties.json:1` |
| تقرير منشور عن منهجية نقع وأرشيف أوامر إعادة التشغيل کرتا ہے | قم بالإبلاغ عن نطاق الامتصاص وأوامر التنفيذ وخطافات إمكانية المراقبة من قبل المراجعين بالتفصيل. | `./sf2c-capacity-soak.md` |

## ملاحظات التنفيذ

تسجيل الخروج هو مجموعة التحقق من الصحة التالية:

```bash
cargo test -p iroha_torii --features app_api -- capacity_declaration_handler_accepts_request
cargo test -p iroha_core -- register_capacity_declaration_rejects_provider_mismatch
cargo test -p iroha_core -- register_capacity_dispute_inserts_record
cargo test -p iroha_core -- capacity_dispute_replay_is_deterministic
cargo test -p iroha_core -- capacity_fee_ledger_30_day_soak_deterministic
cargo test -p sorafs_car --features cli --test capacity_cli
python3 scripts/telemetry/capacity_reconcile.py --snapshot <state.json> --ledger <ledger.ndjson> --warn-only
```

يقوم المشغلون الذين `sorafs_manifest_stub capacity {declaration,dispute}` بتسجيل الحمولات النافعة لطلبات الإعداد/الاعتراض بشكل متكرر وإنشاء ملفات وبيانات JSON/Norito بايت في تذكرة الإدارة بالإضافة إلى أرشيف ثابت.

## قطع أثرية لتسجيل الخروج| قطعة أثرية | المسار | بليك2ب-256 |
|----------|------|-------------|
| حزمة موافقة تأهيل الموفر | `docs/examples/sorafs_capacity_marketplace_validation/2026-03-24_onboarding_signoff.md` | `8f41a745d8d94710fe81c07839651520429d4abea5729bc00f8f45bbb11daa4c` |
| حزمة الموافقة على حل النزاعات | `docs/examples/sorafs_capacity_marketplace_validation/2026-03-24_dispute_signoff.md` | `c3ac3999ef52857170fedb83cddbff7733ef5699f8b38aea2e65ae507a6229f7` |
| حزمة الموافقة على تسوية الخزينة | `docs/examples/sorafs_capacity_marketplace_validation/2026-03-24_treasury_signoff.md` | `0511aeed1f5607c329428cd49c94d1af51292c85134c10c3330c172b0140e8c6` |

إن المصنوعات اليدوية عبارة عن نسخ موقعة وإصدار حزمة وهو ما يمثل سجلًا محفوظًا وتغييرات في الإدارة لنك كري.

##الموافقات

- قائد فريق التخزين — @storage-tl (24/03/2026)  
- أمين مجلس الحكم — @council-sec (24-03-2026)  
- قائد عمليات الخزانة — @treasury-ops (2026-03-24)