---
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/reports/capacity-marketplace-validation.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
العنوان: التحقق من سوق صلاح SoraFS
العلامات: [SF-2c، القبول، قائمة المراجعة]
ملخص: قائمة الاختيار للقبول يغطي مشغلي الكرومين، تدفقات الاشتراك، وتسوية التأمين التي تضبط جاهزية التسجيل العام لسوق SoraFS.
---

# قم بالتحقق من التحقق من سوق سليم SoraFS

**نافذة التعديل:** 2026-03-18 -> 2026-03-24  
**مالكو البرنامج:** فريق التخزين (`@storage-wg`)، مجلس الإدارة (`@council`)، نقابة الخزانة (`@treasury`)  
**النطاق:** مسارات الشركاء المصدرين، تدفقات تحكيم، الاشتراك، اتفاقية تعاون خاصة لـ SF-2c GA.

يجب مراجعة قائمة التفتيش أدناه قبل السوق للمشغلين الخارجيين. كل صف مرتبط بدليل حتمي (الاختبارات أو التركيبات أو الأدلة) يمكن للمشرفين إعادة تشغيله.

## قائمة قبول القبول

### التصديرين| الطاب | التحقق | الدليل |
|-------|-----------|----------|
| يقبل التسجيل إعلانات السعة النوعية | يقوم باختبار متكامل ويستطيع `/v2/sorafs/capacity/declare` عبر واجهة برمجة تطبيقات التطبيق، مع التحقق من التشفير التواقيع، والتقاط البيانات الوصفية، وتسليمها إلى التسجيل في العقدة. | `crates/iroha_torii/src/routing.rs:7654` |
| رفض العقد الذكي الـ الحمولات غير المتطابقة | تضمن وحدات اختبار أن معرفات المزود وحقول GiB الملتزم بها تطابق الإعلان عن الموقع قبل الحفظ. | `crates/iroha_core/src/smartcontracts/isi/sorafs.rs:3445` |
| قادم CLI المصنوعات اليدوية الوافدين | يقوم بتسخير CLI بكتابة مخرجات Norito/JSON/Base64 حتمية ويتحقق من الرحلات ذهابًا وإيابًا حتى يتمكن من البدء في إعداد الإعلانات دون اتصال بالإنترنت. | `crates/sorafs_car/tests/capacity_cli.rs:17` |
| يحرر دليلكم للانضمام وحظر التعدد | أعد إعداد مخطط الإعلان، السياسات الافتراضية، وخطوات المراجعة للمجلس. | `../storage-capacity-marketplace.md` |

### الاتفاق| الطاب | التحقق | الدليل |
|-------|-----------|----------|
| لا توجد كائنات تتوافق مع ملخص قياسي للـ payload | تسجل نسخ تجريبية، ويفك الحمولة المخزنة، ويؤكد حالة معلقة وحتمية دفتر الأستاذ. | `crates/iroha_core/src/smartcontracts/isi/sorafs.rs:1835` |
| مولد نزاعات CLI يطابق الأسطوري | غطاء اختبار CLI مخرجات Base64/Norito وملخصات JSON لـ `CapacityDisputeV1`، بما في ذلك بما يضمن أن حزم الأدلة تُهش بشكل حتمي. | `crates/sorafs_car/tests/capacity_cli.rs:455` |
| اختبار الإعادة ثابت حتمية تختلف/العقوبة | القياس عن بعد الخاصة بـ إثبات الفشل التي تُعاد مرتين إنتاج لقطات متطابقة للـ دفتر الأستاذ والائتمان والنزاع، ليبقى مائلة حطمياً بين أقرانهم. | `crates/iroha_core/src/smartcontracts/isi/sorafs.rs:3430` |
| يركض مسار كتاب العمل التصعيد والإلغاء | يحدّد دليل عمليات سير المجلس ومتطلبات الأدلة الأساسية. | `../dispute-revocation-runbook.md` |

###اتفاقية التجارة| الطاب | التحقق | الدليل |
|-------|-----------|----------|
| جيف ليدجر يطابق توقعًا نقع لمدة 30 يومًا | تم اختبارها عبر خمسة قرون على مدار 30 تسوية، مع مقارنة بيانات دفتر الأستاذ بالمرجع المتميز للمدفوعات. | `crates/iroha_core/src/smartcontracts/isi/sorafs.rs:3000` |
| اتفاقية صادرات دفتر الأستاذ تُسجل ليلا | يقارن `capacity_reconcile.py` دفتر حسابات رسوم التوقعات بصادرات تحويل XOR المنفذة، ويصدر مقاييس Prometheus، وضبط الموافقة المسبقة عن علم عبر Alertmanager. | `scripts/telemetry/capacity_reconcile.py:1`،`docs/source/sorafs/runbooks/capacity_reconciliation.md:1`،`dashboards/alerts/sorafs_capacity_rules.yml:100` |
| لوحات الفواتير تعرضت للأضرار والقياس عن بعد التراكم | استيراد العرض Grafana Nokia GiB-hour، عدادات الضربات، والضمان المربوط الرؤية لدى فريق المناوبة. | `dashboards/grafana/sorafs_capacity_penalties.json:1` |
| تقرير المقال يؤرشف نقع وأوامر الإعادة | يشرح شرح نطاق نقع وأوامر التنفيذ والخطافات للرصد من قبل المقررين. | `./sf2c-capacity-soak.md` |

##التنفيذ

أعد تشغيل حزمة التحقق قبل تسجيل الخروج:

```bash
cargo test -p iroha_torii --features app_api -- capacity_declaration_handler_accepts_request
cargo test -p iroha_core -- register_capacity_declaration_rejects_provider_mismatch
cargo test -p iroha_core -- register_capacity_dispute_inserts_record
cargo test -p iroha_core -- capacity_dispute_replay_is_deterministic
cargo test -p iroha_core -- capacity_fee_ledger_30_day_soak_deterministic
cargo test -p sorafs_car --features cli --test capacity_cli
python3 scripts/telemetry/capacity_reconcile.py --snapshot <state.json> --ledger <ledger.ndjson> --warn-only
```

يجب على تشغيلها إعادة توليد طلبات الحمولة النافعة للانضمام/النزاع عبر `sorafs_manifest_stub capacity {declaration,dispute}` وتسعى إلى اكتشاف أقراص JSON/Norito وتتزايد عادة تذكرة التزايد.

##مصنوعات الموافقة

| قطعة أثرية | المسار | بليك2ب-256 |
|----------|------|-------------|
| حزمة الموافقة المتزايدة | `docs/examples/sorafs_capacity_marketplace_validation/2026-03-24_onboarding_signoff.md` | `8f41a745d8d94710fe81c07839651520429d4abea5729bc00f8f45bbb11daa4c` |
| الاتفاقية الاتفاقية | `docs/examples/sorafs_capacity_marketplace_validation/2026-03-24_dispute_signoff.md` | `c3ac3999ef52857170fedb83cddbff7733ef5699f8b38aea2e65ae507a6229f7` |
| حزمة اتفاقية الاتفاقية التجارية | `docs/examples/sorafs_capacity_marketplace_validation/2026-03-24_treasury_signoff.md` | `0511aeed1f5607c329428cd49c94d1af51292c85134c10c3330c172b0140e8c6` |

احتفظ بنسخة الموقع من هذه القطع الأثرية مع الإصدار المتوفر وربطها في سجل سجلات الدخول.

##شكرات- قائد فريق التخزين — @storage-tl (24/03/2026)  
- أمين مجلس الحكم — @council-sec (24-03-2026)  
- قائد عمليات الخزانة — @treasury-ops (2026-03-24)