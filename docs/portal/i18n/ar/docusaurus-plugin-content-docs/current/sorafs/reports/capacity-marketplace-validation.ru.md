---
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/reports/capacity-marketplace-validation.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
العنوان: التحقق من صحة ساحة السوق SoraFS
العلامات: [SF-2c، القبول، قائمة المراجعة]
ملخص: اختيار الخيارات، ومقدمي خدمات التوصيل المتميزين، والإمدادات والإمدادات العالمية التي تساعد في تحقيق النجاح في ساحة السوق الخروج SoraFS إلى GA.
---

# قائمة التحقق من صحة ساحة السوق SoraFS

**الاختبارات الصحيحة:** 2026-03-18 -> 2026-03-24  
**البرامج المفضلة:** فريق التخزين (`@storage-wg`)، مجلس الإدارة (`@council`)، نقابة الخزانة (`@treasury`)  
**الإخلاء:** مقدمو خدمة التوصيل العاديون، وقنوات التوزيع والعمليات العالمية، اللازمة لـ GA SF-2c.

يجب أن يتم التحقق من قائمة الاختيار بما في ذلك شاشة السوق للمشغلين الآخرين. ترتبط كل خطوة بتحديد المؤهلات (الاختبارات أو التركيبات أو الوثائق)، التي يمكن لمدققي الحسابات التحدث عنها.

## اختيار الخيارات

### موفري الخدمة| بروفيركا | التحقق | التصريح |
|-------|-----------|----------|
| مبدأ التسجيل الإقرارات القانونية | يتم استخدام اختبار التكامل `/v2/sorafs/capacity/declare` من خلال واجهة برمجة تطبيقات التطبيق، والتحقق من صحة الوصف، وحفظ البيانات التعريفية، وإرسالها إلى وحدات التسجيل. | `crates/iroha_torii/src/routing.rs:7654` |
| العقد الذكي يزيل الحمولات غير الضرورية | يضمن اختبار الوحدة أن موفري المعرفات وأنهم يلتزمون بدمج GiB مع الإقرارات المقدمة قبل التخزين. | `crates/iroha_core/src/smartcontracts/isi/sorafs.rs:3445` |
| CLI تقوم بشراء القطع الأثرية الأساسية | يستخدم تسخير CLI Norito/JSON/Base64 ذهابًا وإيابًا والتحقق من الرحلات ذهابًا وإيابًا، حيث يمكن للمشغلين أن يعلنوا دون اتصال بالإنترنت. | `crates/sorafs_car/tests/capacity_cli.rs:17` |
| مشغلو الأخشاب يراقبون تنفيذ سير العمل وإدارة الدرابزين | تقوم الوثائق بإعادة صياغة إعلانات النظام وسياسة الإعدادات الافتراضية وما يجب مراجعة المجلس. | `../storage-capacity-marketplace.md` |

### تحليل الجراثيم| بروفيركا | التحقق | التصريح |
|-------|-----------|----------|
| تسجيل المواد المصاحبة للحمولة الكنسي | يقوم اختبار الوحدة بتسجيل النشاط وفك تشفير الحمولة المخزنة وتأكيد الحالة المعلقة لضمان تحديد دفتر الأستاذ. | `crates/iroha_core/src/smartcontracts/isi/sorafs.rs:1835` |
| المولدات الداعمة لـ CLI مخطط قانوني | يكشف اختبار CLI عن Base64/Norito وملفات JSON لـ `CapacityDisputeV1`، ويضمن تحديد حزم أدلة التجزئة. | `crates/sorafs_car/tests/capacity_cli.rs:455` |
| إعادة اختبار إعادة تحديد السبب/العقوبة | إثبات فشل القياس عن بعد، الأشخاص المتصلين، دفتر الأستاذ الخاص باللقطات المتطابقة، والائتمان والنزاع، حيث تم تحديد الخطوط المائلة بين الأقران. | `crates/iroha_core/src/smartcontracts/isi/sorafs.rs:3430` |
| يوثق Runbook خطوات التصعيد والإلغاء | تعمل العملية على إصلاح مجلس سير العمل، والحاجة إلى الإحالة، وإجراءات التراجع. | `../dispute-revocation-runbook.md` |

### كل التفاصيل| بروفيركا | التحقق | التصريح |
|-------|-----------|----------|
| يتم تضمين دفتر الأستاذ الاستحقاق مع مشروع مدته 30 يومًا | اختبار Soak يؤكد على مقدمي الخدمات لمدة 30 دقيقة من التسوية، ويسجل دفتر الأستاذ الخاص بهم باستخدام المرجع المرجعي. | `crates/iroha_core/src/smartcontracts/isi/sorafs.rs:3000` |
| يتم تسجيل دفتر الأستاذ العالمي للتصدير كل ليلة | `capacity_reconcile.py` هو دفتر أستاذ رسوم الصيانة الدقيق من خلال تصدير XOR المعزز، والمقاييس العامة Prometheus، وبوابة المعرفة عن طريق Alertmanager. | `scripts/telemetry/capacity_reconcile.py:1`،`docs/source/sorafs/runbooks/capacity_reconciliation.md:1`،`dashboards/alerts/sorafs_capacity_rules.yml:100` |
| تعرض لوحات معلومات الفوترة الغرامات واستحقاق القياس عن بعد | استيراد Grafana لديه تراكم GiB-hour وضربات الشيك والضمانات المستعبدة للعروض عند الطلب. | `dashboards/grafana/sorafs_capacity_penalties.json:1` |
| طرق الأرشفة العامة للنقع والأوامر الإعادة | قم بوصف النقع والأوامر وخطافات المراقبة للمدققين. | `./sf2c-capacity-soak.md` |

## Пимечания по выполнению

التحقق من نوع التحقق قبل تسجيل الخروج:

```bash
cargo test -p iroha_torii --features app_api -- capacity_declaration_handler_accepts_request
cargo test -p iroha_core -- register_capacity_declaration_rejects_provider_mismatch
cargo test -p iroha_core -- register_capacity_dispute_inserts_record
cargo test -p iroha_core -- capacity_dispute_replay_is_deterministic
cargo test -p iroha_core -- capacity_fee_ledger_30_day_soak_deterministic
cargo test -p sorafs_car --features cli --test capacity_cli
python3 scripts/telemetry/capacity_reconcile.py --snapshot <state.json> --ledger <ledger.ndjson> --warn-only
```

يقوم المشغلون المسؤولون عن إنشاء الحمولات الصافية بالنقل/النقل عبر `sorafs_manifest_stub capacity {declaration,dispute}` وأرشفة الوصول إليها JSON/Norito بايت موجود في تذكرة الإدارة.

## تسجيل الخروج للقطع الأثرية| قطعة أثرية | المسار | بليك2ب-256 |
|----------|------|-------------|
| مقدمو خدمات إلحاق الحزم | `docs/examples/sorafs_capacity_marketplace_validation/2026-03-24_onboarding_signoff.md` | `8f41a745d8d94710fe81c07839651520429d4abea5729bc00f8f45bbb11daa4c` |
| أعشاب مميّزة للموافقة على الحزم | `docs/examples/sorafs_capacity_marketplace_validation/2026-03-24_dispute_signoff.md` | `c3ac3999ef52857170fedb83cddbff7733ef5699f8b38aea2e65ae507a6229f7` |
| موافقة الحزمة العالمية | `docs/examples/sorafs_capacity_marketplace_validation/2026-03-24_treasury_signoff.md` | `0511aeed1f5607c329428cd49c94d1af51292c85134c10c3330c172b0140e8c6` |

تتوفر نسخ هذه القطع الأثرية من خلال حزمة الإصدار وتتيح لك الوصول إلى إدارة التحسين.

## مشاركة

- قائد فريق التخزين — @storage-tl (24/03/2026)  
- أمين مجلس الحكم — @council-sec (24-03-2026)  
- قائد عمليات الخزانة — @treasury-ops (2026-03-24)