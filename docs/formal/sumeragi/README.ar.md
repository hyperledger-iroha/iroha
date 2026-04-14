<!-- Auto-generated stub for Arabic (ar) translation. Replace this content with the full translation. -->

---
lang: ar
direction: rtl
source: docs/formal/sumeragi/README.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 56f1412b2db729ba69057ce15ac8bae707310fd5a6d01be2da816fdee18218f7
source_last_modified: "2026-02-23T14:48:46.580877+00:00"
translation_last_reviewed: 2026-04-02
translator: machine-google-reviewed
---

# Sumeragi النموذج الرسمي (TLA+ / Apalache)

يحتوي هذا الدليل على نموذج رسمي محدد لسلامة مسار الالتزام وحيويته Sumeragi.

## النطاق

يلتقط النموذج:
- تقدم المرحلة (`Propose`، `Prepare`، `CommitVote`، `NewView`، `Committed`)،
- عتبات التصويت والنصاب القانوني (`CommitQuorum`، `ViewQuorum`)،
- نصاب الحصة المرجح (`StakeQuorum`) لحراس الالتزام بأسلوب NPoS،
- السببية لكرات الدم الحمراء (`Init -> Chunk -> Ready -> Deliver`) مع أدلة الرأس/الملخص،
- ضريبة السلع والخدمات وافتراضات العدالة الضعيفة على إجراءات التقدم الصادق.

يقوم عمدا بتجريد تنسيقات الأسلاك والتوقيعات وتفاصيل الشبكات الكاملة.

## الملفات

- `Sumeragi.tla`: نموذج البروتوكول وخصائصه.
- `Sumeragi_fast.cfg`: مجموعة معلمات أصغر حجمًا صديقة لـ CI.
- `Sumeragi_deep.cfg`: مجموعة معلمات ضغط أكبر.

## خصائص

الثوابت:
-`TypeInvariant`
-`CommitImpliesQuorum`
-`CommitImpliesStakeQuorum`
-`CommitImpliesDelivered`
-`DeliverImpliesEvidence`

خاصية زمنية:
- `EventuallyCommit` (`[] (gst => <> committed)`)، مع تشفير عدالة ما بعد ضريبة السلع والخدمات
  تشغيلياً في `Next` (تم تمكين حراس المهلة/الوقاية الوقائية من الأخطاء
  إجراءات التقدم). يؤدي هذا إلى إبقاء النموذج قابلاً للتحقق باستخدام Apalache 0.52.x، والذي
  لا يدعم عوامل تشغيل `WF_` داخل الخصائص الزمنية المحددة.

## الجري

من جذر المستودع:

```bash
bash scripts/formal/sumeragi_apalache.sh fast
bash scripts/formal/sumeragi_apalache.sh deep
```

### الإعداد المحلي القابل للتكرار (لا يلزم Docker)قم بتثبيت سلسلة أدوات Apalache المحلية المثبتة والتي يستخدمها هذا المستودع:

```bash
bash scripts/formal/install_apalache.sh 0.52.2
```

يكتشف العداء هذا التثبيت تلقائيًا في:
`target/apalache/toolchains/v0.52.2/bin/apalache-mc`.
بعد التثبيت، يجب أن يعمل `ci/check_sumeragi_formal.sh` بدون vars env الإضافية:

```bash
bash ci/check_sumeragi_formal.sh
```

إذا لم يكن Apalache موجودًا في `PATH`، فيمكنك:

- اضبط `APALACHE_BIN` على المسار القابل للتنفيذ، أو
- استخدم الخيار الاحتياطي Docker (يتم تمكينه افتراضيًا عندما يكون `docker` متاحًا):
  - الصورة: `APALACHE_DOCKER_IMAGE` (`ghcr.io/apalache-mc/apalache:latest` الافتراضي)
  - يتطلب البرنامج الخفي Docker قيد التشغيل
  - تعطيل الإجراء الاحتياطي باستخدام `APALACHE_ALLOW_DOCKER=0`.

أمثلة:

```bash
APALACHE_BIN=/opt/apalache/bin/apalache-mc bash scripts/formal/sumeragi_apalache.sh fast
APALACHE_DOCKER_IMAGE=ghcr.io/apalache-mc/apalache:latest bash scripts/formal/sumeragi_apalache.sh deep
```

## ملاحظات

- يكمل هذا النموذج (لا يحل محل) اختبارات نموذج الصدأ القابلة للتنفيذ في
  `crates/iroha_core/src/sumeragi/main_loop/tests/state_machine_model_tests.rs`
  و
  `crates/iroha_core/src/sumeragi/main_loop/tests/state_machine_fairness_model_tests.rs`.
- الاختبارات محددة بقيم ثابتة في ملفات `.cfg`.
- يقوم PR CI بتشغيل هذه الفحوصات في `.github/workflows/pr.yml` عبر
  `ci/check_sumeragi_formal.sh`.