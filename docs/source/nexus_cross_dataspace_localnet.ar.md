<!-- Auto-generated stub for Arabic (ar) translation. Replace this content with the full translation. -->

---
lang: ar
direction: rtl
source: docs/source/nexus_cross_dataspace_localnet.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 2324cfc7b086ceb96317eb2260abe41101f17e5c0749d0a1d28ffbf4cb5e8e45
source_last_modified: "2026-02-19T18:33:20.275472+00:00"
translation_last_reviewed: 2026-04-02
translator: machine-google-reviewed
---

# Nexus إثبات الشبكة المحلية عبر البيانات

ينفذ دليل التشغيل هذا دليل التكامل Nexus الذي:

- تشغيل شبكة محلية ذات 4 نظير مع مساحتي بيانات خاصة مقيدة (`ds1`، `ds2`)،
- توجيه حركة مرور الحساب إلى كل مساحة بيانات،
- إنشاء أصل في كل مساحة بيانات،
- تنفيذ تسوية المبادلة الذرية عبر مساحات البيانات في كلا الاتجاهين،
- يثبت دلالات التراجع عن طريق تقديم ساق تعاني من نقص التمويل والتحقق من بقاء الأرصدة دون تغيير.

الاختبار الكنسي هو:
`nexus::cross_dataspace_localnet::cross_dataspace_atomic_swap_is_all_or_nothing`.

## التشغيل السريع

استخدم البرنامج النصي المجمع من جذر المستودع:

```bash
scripts/run_nexus_cross_dataspace_atomic_swap.sh
```

السلوك الافتراضي:

- يعمل فقط على اختبار إثبات مساحة البيانات المشتركة،
- مجموعات `NORITO_SKIP_BINDINGS_SYNC=1`،
- مجموعات `IROHA_TEST_SKIP_BUILD=1`،
- يستخدم `--test-threads=1`،
- يمر `--nocapture`.

## خيارات مفيدة

```bash
scripts/run_nexus_cross_dataspace_atomic_swap.sh --keep-dirs
scripts/run_nexus_cross_dataspace_atomic_swap.sh --no-skip-build
scripts/run_nexus_cross_dataspace_atomic_swap.sh --release
scripts/run_nexus_cross_dataspace_atomic_swap.sh --all-nexus
```

- يحتفظ `--keep-dirs` بأدلة النظراء المؤقتة (`IROHA_TEST_NETWORK_KEEP_DIRS=1`) للطب الشرعي.
- يقوم `--all-nexus` بتشغيل `mod nexus::` (المجموعة الفرعية الكاملة لتكامل Nexus)، وليس فقط اختبار الإثبات.

## بوابة سي آي

مساعد سي آي:

```bash
ci/check_nexus_cross_dataspace_localnet.sh
```

جعل الهدف:

```bash
make check-nexus-cross-dataspace
```

تنفذ هذه البوابة غلاف الإثبات الحتمي وتفشل في المهمة إذا كانت ذرية عبر مساحة البيانات
تراجع سيناريو المبادلة.

## الأوامر المكافئة اليدوية

اختبار الإثبات المستهدف:

```bash
IROHA_TEST_SKIP_BUILD=1 NORITO_SKIP_BINDINGS_SYNC=1 \
  cargo test -p integration_tests --test mod \
  nexus::cross_dataspace_localnet::cross_dataspace_atomic_swap_is_all_or_nothing \
  -- --nocapture --test-threads=1
```

المجموعة الفرعية Nexus الكاملة:

```bash
IROHA_TEST_SKIP_BUILD=1 NORITO_SKIP_BINDINGS_SYNC=1 \
  cargo test -p integration_tests --test mod nexus:: -- --nocapture --test-threads=1
```

## إشارات الإثبات المتوقعة- نجح في الاختبار.
- يظهر تحذير متوقع بشأن عملية التسوية الفاشلة عمدًا والتي تعاني من نقص التمويل:
  `settlement leg requires 10000 but only ... is available`.
- تنجح تأكيدات الرصيد النهائي بعد:
  - مبادلة ناجحة إلى الأمام،
  - مبادلة عكسية ناجحة،
  - فشل المبادلة التي تعاني من نقص التمويل (التراجع عن الأرصدة التي لم تتغير).

## لقطة التحقق الحالية

اعتبارًا من **19 شباط (فبراير) 2026**، تم تنفيذ سير العمل هذا بما يلي:

- الاختبار المستهدف: `1 passed; 0 failed`،
- المجموعة الفرعية Nexus الكاملة: `24 passed; 0 failed`.