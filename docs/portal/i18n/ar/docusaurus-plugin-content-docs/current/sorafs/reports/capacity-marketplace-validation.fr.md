---
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/reports/capacity-marketplace-validation.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
العنوان: التحقق من صحة مسيرة السعة SoraFS
العلامات: [SF-2c، القبول، قائمة المراجعة]
ملخص: قائمة التحقق من القبول تغطي تأهيل مقدمي الخدمة وتدفق الدعاوى وتسوية الخزينة المشروطة بالتوفر العام لسوق السعة SoraFS.
---

# قائمة التحقق من صحة سوق السعة SoraFS

** نافذة العرض: ** 2026-03-18 -> 2026-03-24  
**مسؤولو البرنامج:** فريق التخزين (`@storage-wg`)، ومجلس الإدارة (`@council`)، وTreasury Guild (`@treasury`)  
**المنفذ:** خطوط الأنابيب لضم مقدمي الخدمة، وتدفق الفصل في الدعاوى القضائية، وعملية تسوية الخزينة المطلوبة لـ GA SF-2c.

يجب مراجعة قائمة التحقق هذه قبل تنشيط السوق للمشغلين الخارجيين. كل خط يراجع أدلة محددة (اختبارات أو تركيبات أو وثائق) يمكن للمراجعين أن يستمتعوا بها.

## قائمة التحقق من القبول

### تأهيل مقدمي الخدمة| تحقق | التحقق من الصحة | الأدلة |
|-------|-----------|----------|
| يقبل التسجيل الإقرارات القانونية المتعلقة بالسعة | يتم إجراء اختبار التكامل `/v2/sorafs/capacity/declare` عبر واجهة برمجة تطبيقات التطبيق، والتحقق من إدارة التوقيعات، والتقاط البيانات الوصفية، والتسليم من خلال سجل جديد. | `crates/iroha_torii/src/routing.rs:7654` |
| يعيد العقد الذكي الحمولات غير المتماسكة | يضمن الاختبار الموحد أن معرفات الموفر والأبطال GiB تتفاعل مع الإعلان الموقع مسبقًا. | `crates/iroha_core/src/smartcontracts/isi/sorafs.rs:3445` |
| Le CLI emet des artifacts d'onboarding canoniques | يقوم تسخير CLI بكتابة الطلعات Norito/JSON/Base64 لتحديد الرحلات ذهابًا وإيابًا وصلاحيتها حتى يتمكن المشغلون من إعداد الإعلانات دون اتصال بالإنترنت. | `crates/sorafs_car/tests/capacity_cli.rs:17` |
| يغطي دليل المشغل سير عمل القبول والمراقبة الحكومية | تسرد الوثائق مخطط الإعلان وافتراضيات السياسة وخطوات المراجعة للمجلس. | `../storage-capacity-marketplace.md` |

### قرار الخصومة| تحقق | التحقق من الصحة | الأدلة |
|-------|-----------|----------|
| تسجيلات الدعاوى الدائمة مع ملخص قانوني للحمولة | تقوم وحدة الاختبار بتسجيل دعوى، وفك تشفير مخزون الحمولة، وتأكيد الوضع المعلق لضمان تحديد دفتر الأستاذ. | `crates/iroha_core/src/smartcontracts/isi/sorafs.rs:1835` |
| يتوافق مُنشئ الدعاوى في CLI مع المخطط الأساسي | يغطي اختبار CLI عمليات الطلع Base64/Norito واستئنافات JSON لـ `CapacityDisputeV1`، مع التأكد من أن حزم الأدلة قد تم تحديدها بطريقة محددة. | `crates/sorafs_car/tests/capacity_cli.rs:455` |
| اختبار الإعادة يثبت حسم الدعوى/العقوبة | يستمتع القياس عن بعد لإثبات الفشل مرتين بإنتاج لقطات متطابقة من دفتر الأستاذ والائتمان والدعوى بحيث يتم تحديد الخطوط المائلة بين النظراء. | `crates/iroha_core/src/smartcontracts/isi/sorafs.rs:3430` |
| يوثق دليل التشغيل تدفق التصعيد والإلغاء | يلتقط دليل العمليات سير عمل المجلس ومتطلبات الإجراءات وإجراءات التراجع. | `../dispute-revocation-runbook.md` |

### مصالحة الكنز| تحقق | التحقق من الصحة | الأدلة |
|-------|-----------|----------|
| يتوافق تراكم دفتر الأستاذ مع توقع النقع لمدة 30 يومًا | يغطي الاختبار خمسة مقدمي خدمات على 30 نافذة تسوية، ومقارنة إدخالات دفتر الأستاذ بمرجع الدفع الحاضر. | `crates/iroha_core/src/smartcontracts/isi/sorafs.rs:3000` |
| يتم تسجيل تسوية صادرات دفتر الأستاذ كل ليلة | `capacity_reconcile.py` قارن بين تنبيهات رسوم دفتر الأستاذ من خلال عمليات تصدير XOR، واستخدام المقاييس Prometheus، واستقبال موافقة الكنز عبر Alertmanager. | `scripts/telemetry/capacity_reconcile.py:1`،`docs/source/sorafs/runbooks/capacity_reconciliation.md:1`،`dashboards/alerts/sorafs_capacity_rules.yml:100` |
| تعرض لوحات معلومات الفواتير العقوبات والقياس عن بعد للتراكم | يقوم L'import Grafana بتتبع تراكم GiB-hour وحسابات الضربات والضمانات التي تعمل على الرؤية عند الطلب. | `dashboards/grafana/sorafs_capacity_penalties.json:1` |
| تنشر العلاقة أرشفة منهجية النقع وأوامر إعادة التشغيل | يفصل التقرير باب النقع وأوامر التنفيذ والخطافات التي يمكن ملاحظتها من قبل المراجعين. | `./sf2c-capacity-soak.md` |

## ملاحظات التنفيذ

إعادة ربط مجموعة التحقق من الصحة قبل تسجيل الخروج:

```bash
cargo test -p iroha_torii --features app_api -- capacity_declaration_handler_accepts_request
cargo test -p iroha_core -- register_capacity_declaration_rejects_provider_mismatch
cargo test -p iroha_core -- register_capacity_dispute_inserts_record
cargo test -p iroha_core -- capacity_dispute_replay_is_deterministic
cargo test -p iroha_core -- capacity_fee_ledger_30_day_soak_deterministic
cargo test -p sorafs_car --features cli --test capacity_cli
python3 scripts/telemetry/capacity_reconcile.py --snapshot <state.json> --ledger <ledger.ndjson> --warn-only
```

يتعين على المشغلين إعادة إنشاء حمولات الطلب على متن الطائرة/الدعوى باستخدام `sorafs_manifest_stub capacity {declaration,dispute}` وأرشفة وحدات البايت JSON/Norito الناتجة عن بطاقات الإدارة.

## قطع أثرية للتوقيع| قطعة أثرية | المسار | بليك2ب-256 |
|----------|------|-------------|
| حزمة الموافقة على تأهيل مقدمي الخدمة | `docs/examples/sorafs_capacity_marketplace_validation/2026-03-24_onboarding_signoff.md` | `8f41a745d8d94710fe81c07839651520429d4abea5729bc00f8f45bbb11daa4c` |
| حزمة الموافقة على حل الدعاوى | `docs/examples/sorafs_capacity_marketplace_validation/2026-03-24_dispute_signoff.md` | `c3ac3999ef52857170fedb83cddbff7733ef5699f8b38aea2e65ae507a6229f7` |
| حزمة الموافقة على تسوية الكنز | `docs/examples/sorafs_capacity_marketplace_validation/2026-03-24_treasury_signoff.md` | `0511aeed1f5607c329428cd49c94d1af51292c85134c10c3330c172b0140e8c6` |

احتفظ بالنسخ الموقعة على هذه العناصر مع حزمة الإصدار وقم بإيداعها في سجل تغيير الإدارة.

##الموافقات

- قائد فريق التخزين — @storage-tl (24/03/2026)  
- أمين مجلس الحكم — @council-sec (24-03-2026)  
- قائد عمليات الخزانة — @treasury-ops (2026-03-24)