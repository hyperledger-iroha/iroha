---
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/reports/capacity-marketplace-validation.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
العنوان: Validacion del mercado de capacidad SoraFS
العلامات: [SF-2c، القبول، قائمة المراجعة]
ملخص: قائمة التحقق من القبول التي تتضمن تأهيل مقدمي الخدمة وتدفقات النزاعات والتسوية التي تأهيل التوفر العام لسوق القدرة SoraFS.
---

# قائمة التحقق من صحة سوق القدرة SoraFS

**نافذة المراجعة:** 2026-03-18 -> 2026-03-24  
**مسؤولو البرنامج:** فريق التخزين (`@storage-wg`)، ومجلس الإدارة (`@council`)، وTreasury Guild (`@treasury`)  
**الفرصة:** مسارات تأهيل مقدمي الخدمة، وتدفقات التحكيم في النزاعات، وعمليات التوفيق المطلوبة لـ GA SF-2c.

يجب مراجعة قائمة المراجعة التالية قبل تأهيل السوق للمشغلين الخارجيين. كل ملف يحتوي على أدلة حتمية (اختبارات أو تركيبات أو وثائق) يمكن للمراجعين إعادة إنتاجها.

## قائمة التحقق من القبول

### تأهيل مقدمي الخدمة| تشيكيو | التحقق من الصحة | ايفيدنسيا |
|-------|-----------|----------|
| السجل يقبل الإعلانات القانونية المتعلقة بالقدرة | يتم إجراء اختبار التكامل `/v2/sorafs/capacity/declare` عبر واجهة برمجة تطبيقات التطبيق، والتحقق من طريقة الشركة، والتقاط البيانات التعريفية، وتسليم العقدة إلى السجل. | `crates/iroha_torii/src/routing.rs:7654` |
| العقد الذكي rechaza حمولات desalineados | يضمن الاختبار الموحد أن معرفات الموفر ومجالات GiB المخترقة تتزامن مع الإعلان الثابت قبل الاستمرار. | `crates/iroha_core/src/smartcontracts/isi/sorafs.rs:3445` |
| يصدر El CLI عناصر أساسية من الإعداد | يسرد تسخير CLI البيانات المحددة Norito/JSON/Base64 ويتحقق من الرحلات ذهابًا وإيابًا حتى يتمكن المشغلون من إعداد الإعلانات دون اتصال بالإنترنت. | `crates/sorafs_car/tests/capacity_cli.rs:17` |
| يلتقط دليل المشغلين تدفق الدخول وحواجز الحماية الحكومية | تسرد الوثائق مبدأ الإعلان وافتراضيات السياسة وإجراءات المراجعة للمجلس. | `../storage-capacity-marketplace.md` |

### حل النزاعات| تشيكيو | التحقق من الصحة | ايفيدنسيا |
|-------|-----------|----------|
| تستمر سجلات النزاع مع ملخص الحمولة الكنسي | يقوم الاختبار الموحد بتسجيل نزاع، وفك تشفير الحمولة المخزنة، وتأكيد الحالة المعلقة لضمان تحديد دفتر الأستاذ. | `crates/iroha_core/src/smartcontracts/isi/sorafs.rs:1835` |
| يتزامن مولد نزاعات CLI مع الاسم الكنسي | اختبار CLI يخرج Base64/Norito ويستأنف JSON لـ `CapacityDisputeV1`، مع التأكد من أن حزم الأدلة لها شكل حتمي. | `crates/sorafs_car/tests/capacity_cli.rs:455` |
| اختبار الإعادة يختبر حسم النزاع/العقوبة | يُنتج قياس إثبات الفشل عن بُعد عدة مرات لقطات متطابقة من دفتر الأستاذ، والاعتمادات، والمناقشات حتى يتم تحديد الخطوط المائلة بين أقرانهم. | `crates/iroha_core/src/smartcontracts/isi/sorafs.rs:3430` |
| كتاب التشغيل يستند إلى تدفق التصعيد والإلغاء | يلتقط دليل العمليات تدفق المجلس ومتطلبات الأدلة وإجراءات التراجع. | `../dispute-revocation-runbook.md` |

### التوفيق دي tesoreria| تشيكيو | التحقق من الصحة | ايفيدنسيا |
|-------|-----------|----------|
| يتزامن تراكم دفتر الأستاذ مع عملية نقع لمدة 30 يومًا | يختبر النقع خمسة مقدمي خدمات في 30 نافذة تسوية، ويقارنون إدخالات دفتر الأستاذ بمرجع الدفع المتوقع. | `crates/iroha_core/src/smartcontracts/isi/sorafs.rs:3000` |
| يتم تسجيل تسوية صادرات دفتر الأستاذ كل ليلة | يقوم `capacity_reconcile.py` بمقارنة الرسوم المتوقعة لدفتر الأستاذ مع صادرات XOR التي تم تنفيذها، وإصدار مقاييس Prometheus، وطلب موافقة التخزين عبر Alertmanager. | `scripts/telemetry/capacity_reconcile.py:1`،`docs/source/sorafs/runbooks/capacity_reconciliation.md:1`،`dashboards/alerts/sorafs_capacity_rules.yml:100` |
| توضح لوحات معلومات الفواتير العقوبات وقياس التراكم عن بعد | استيراد Grafana يجمع الرسوم البيانية GiB-hour وعدادات الضربات والضمانات المرتبطة بالرؤية عند الطلب. | `dashboards/grafana/sorafs_capacity_penalties.json:1` |
| تم نشر التقرير أرشفة منهجية النقع وأوامر إعادة التشغيل | يفصل التقرير نسبة النقع وأوامر الإخراج وخطافات المراقبة للمراجعين. | `./sf2c-capacity-soak.md` |

## ملاحظات التنفيذ

إعادة تشغيل مجموعة التحقق من الصحة قبل تسجيل الخروج:

```bash
cargo test -p iroha_torii --features app_api -- capacity_declaration_handler_accepts_request
cargo test -p iroha_core -- register_capacity_declaration_rejects_provider_mismatch
cargo test -p iroha_core -- register_capacity_dispute_inserts_record
cargo test -p iroha_core -- capacity_dispute_replay_is_deterministic
cargo test -p iroha_core -- capacity_fee_ledger_30_day_soak_deterministic
cargo test -p sorafs_car --features cli --test capacity_cli
python3 scripts/telemetry/capacity_reconcile.py --snapshot <state.json> --ledger <ledger.ndjson> --warn-only
```

يتعين على المشغلين إعادة إنشاء حمولات طلب الإعداد/الخلاف باستخدام `sorafs_manifest_stub capacity {declaration,dispute}` وحفظ وحدات البايت JSON/Norito الناتجة جنبًا إلى جنب مع بطاقة الإدارة.

## تحف فنية| قطعة أثرية | روتا | بليك2ب-256 |
|----------|------|-------------|
| حزمة الموافقة على تأهيل مقدمي الخدمة | `docs/examples/sorafs_capacity_marketplace_validation/2026-03-24_onboarding_signoff.md` | `8f41a745d8d94710fe81c07839651520429d4abea5729bc00f8f45bbb11daa4c` |
| حزمة الموافقة على حل النزاعات | `docs/examples/sorafs_capacity_marketplace_validation/2026-03-24_dispute_signoff.md` | `c3ac3999ef52857170fedb83cddbff7733ef5699f8b38aea2e65ae507a6229f7` |
| حزمة موافقة على التوفيق في التخزين | `docs/examples/sorafs_capacity_marketplace_validation/2026-03-24_treasury_signoff.md` | `0511aeed1f5607c329428cd49c94d1af51292c85134c10c3330c172b0140e8c6` |

احفظ النسخ الثابتة من هذه المصنوعات من خلال حزمة الإصدار وقم بتخزينها في سجل تغييرات الإدارة.

## أروباسيونيس

- قائد معدات التخزين — @storage-tl (24/03/2026)  
- أمانة مجلس الإدارة — @council-sec (24-03-2026)  
- قائد عمليات التخزين — @treasury-ops (24/03/2026)