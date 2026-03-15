---
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/reports/capacity-marketplace-validation.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
العنوان: Validacao do mercado de capacidade SoraFS
العلامات: [SF-2c، القبول، قائمة المراجعة]
ملخص: قائمة التحقق من الاستفادة من تأهيل موفري الخدمة وتدفقات النزاع والتسوية التي تحرر التوفر العام لسوق القدرة SoraFS.
---

# قائمة التحقق من صحة سوق القدرة SoraFS

** جانيلا دي ريفيساو: ** 2026-03-18 -> 2026-03-24  
**الردود على البرنامج:** فريق التخزين (`@storage-wg`)، ومجلس الإدارة (`@council`)، وTreasury Guild (`@treasury`)  
**الخبر:** مسارات تأهيل مقدمي الخدمة وتدفقات الفصل في النزاعات وعمليات التسوية المطلوبة لـ GA SF-2c.

يجب مراجعة قائمة التحقق أيضًا قبل تأهيلها أو تسويقها للمشغلين الخارجيين. كل ما يربط بين الأدلة الحتمية (الاختبارات أو التركيبات أو المستندات) التي يمكن للمراجعين تقديمها.

## قائمة التحقق من aceitacao

### تأهيل مقدمي الخدمة| تشيكاجيم | فاليداكاو | ايفيدنسيا |
|-------|-----------|----------|
| سجل هذا الإعلان القانوني للقدرة | يتم إجراء اختبار التكامل `/v2/sorafs/capacity/declare` عبر واجهة برمجة تطبيقات التطبيق، والتحقق من معالجة الاغتيالات، والتقاط البيانات الوصفية، وتسليمها لعقدة التسجيل. | `crates/iroha_torii/src/routing.rs:7654` |
| يا عقد ذكي rejeita حمولات متباينة | تضمن وحدة الاختبار أن معرفات الموفر ومجالات GiB المخترقة تتوافق مع الإعلان عنها قبل الاستمرار. | `crates/iroha_core/src/smartcontracts/isi/sorafs.rs:3445` |
| O CLI يصدر Artefatos Canonicos de onboarding | يكشف تسخير CLI عن Norito/JSON/Base64 المحددات ويتحقق من الرحلات ذهابًا وإيابًا حتى يتمكن المشغلون من إعداد الإعلانات دون اتصال بالإنترنت. | `crates/sorafs_car/tests/capacity_cli.rs:17` |
| دليل المشغل يشمل سير عمل القبول وحواجز الحماية | تسرد الوثيقة مخطط الإعلان وافتراضيات السياسة وتصاريح المراجعة للمجلس. | `../storage-capacity-marketplace.md` |

### حل النزاعات| تشيكاجيم | فاليداكاو | ايفيدنسيا |
|-------|-----------|----------|
| سجلات النزاع مستمرة مع ملخص الحمولة الكنسي | يقوم هذا الموحد بتسجيل نزاع أو فك تشفيره أو تخزين الحمولة وتأكيد الحالة المعلقة لضمان تحديد دفتر الأستاذ. | `crates/iroha_core/src/smartcontracts/isi/sorafs.rs:1835` |
| مدير المناقشات الذي تقوم به واجهة سطر الأوامر (CLI) يتوافق مع المخطط الكنسي | اختبار CLI يشمل Base64/Norito ويستأنف JSON لـ `CapacityDisputeV1`، مما يضمن أدلة الحزم التي تتضمن حتمية التجزئة. | `crates/sorafs_car/tests/capacity_cli.rs:455` |
| اختبار الإعادة يبرهن على حسم النزاع/العقوبة | يُنتج قياس إثبات الفشل عن بُعد مرتين في بعض الأحيان، وهو عبارة عن لقطات متطابقة من دفتر الأستاذ، والائتمانات، والمجادلات من أجل خفض عدد المحددات بين أقرانه. | `crates/iroha_core/src/smartcontracts/isi/sorafs.rs:3430` |
| وثيقة دليل التشغيل أو تدفق التصعيد والتحديث | يلتقط دليل العمليات أو سير العمل أو المجلس أو متطلبات الأدلة أو إجراءات التراجع. | `../dispute-revocation-runbook.md` |

### المصالحة دو تيسورو| تشيكاجيم | فاليداكاو | ايفيدنسيا |
|-------|-----------|----------|
| يتوافق دفتر الأستاذ مع مشروع نقع لمدة 30 يومًا | يمكنك الاختيار من بين خمسة مقدمي خدمات في 30 شهرًا من التسوية، ومقارنة مدخلات دفتر الأستاذ بمرجع الدفع المتوقع. | `crates/iroha_core/src/smartcontracts/isi/sorafs.rs:3000` |
| تسوية صادرات دفتر الأستاذ والتسجيل كل ليلة | يقوم `capacity_reconcile.py` بمقارنة التوقعات الخاصة برسوم دفتر الأستاذ مع عمليات تصدير XOR المنفذة، وإصدار المقاييس Prometheus، وإرسال طلبات الحصول على الموافقة عبر Alertmanager. | `scripts/telemetry/capacity_reconcile.py:1`،`docs/source/sorafs/runbooks/capacity_reconciliation.md:1`،`dashboards/alerts/sorafs_capacity_rules.yml:100` |
| تعرض لوحات التحكم في الفواتير العقوبات وقياس الاستحقاق عن بعد | استيراد مخطط Grafana أو تراكم GiB-hour ومقابلات الضربات والضمانات المرتبطة بالرؤية عند الطلب. | `dashboards/grafana/sorafs_capacity_penalties.json:1` |
| تحتوي العلاقة المنشورة على منهجية النقع وأوامر الإعادة | العلاقة التفصيلية أو نطاق النقع وأوامر التنفيذ وخطافات المراقبة للمدققين. | `./sf2c-capacity-soak.md` |

## ملاحظات التنفيذ

أعد تنفيذ مجموعة التحقق من صحة ما قبل تسجيل الخروج:

```bash
cargo test -p iroha_torii --features app_api -- capacity_declaration_handler_accepts_request
cargo test -p iroha_core -- register_capacity_declaration_rejects_provider_mismatch
cargo test -p iroha_core -- register_capacity_dispute_inserts_record
cargo test -p iroha_core -- capacity_dispute_replay_is_deterministic
cargo test -p iroha_core -- capacity_fee_ledger_30_day_soak_deterministic
cargo test -p sorafs_car --features cli --test capacity_cli
python3 scripts/telemetry/capacity_reconcile.py --snapshot <state.json> --ledger <ledger.ndjson> --warn-only
```

يجب على المشغلين إعادة إنشاء حمولات طلب الإعداد/الخلاف مع `sorafs_manifest_stub capacity {declaration,dispute}` واسترجاع البايتات JSON/Norito الناتجة إلى جانب تذكرة الإدارة.

## Artefatos de aprovacao| ارتيفاتو | كامينيو | بليك2ب-256 |
|----------|------|-------------|
| حزمة الموافقة على تأهيل مقدمي الخدمة | `docs/examples/sorafs_capacity_marketplace_validation/2026-03-24_onboarding_signoff.md` | `8f41a745d8d94710fe81c07839651520429d4abea5729bc00f8f45bbb11daa4c` |
| حزمة الموافقة على حل النزاعات | `docs/examples/sorafs_capacity_marketplace_validation/2026-03-24_dispute_signoff.md` | `c3ac3999ef52857170fedb83cddbff7733ef5699f8b38aea2e65ae507a6229f7` |
| Pacote de aprovacao de reconciliacao do tesouro | `docs/examples/sorafs_capacity_marketplace_validation/2026-03-24_treasury_signoff.md` | `0511aeed1f5607c329428cd49c94d1af51292c85134c10c3330c172b0140e8c6` |

احفظ النسخ المحذوفة من هذه القطع الأثرية كحزمة الإصدار والقفل، كسجل لتعديلات الإدارة.

## أبروفاكويس

- قائد فريق التخزين - @storage-tl (24/03/2026)  
- أمين سر مجلس الحكم - @council-sec (2026-03-24)  
- قائد عمليات الخزانة - @treasury-ops (24-03-2026)