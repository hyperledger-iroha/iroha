<!-- Auto-generated stub for Arabic (ar) translation. Replace this content with the full translation. -->

---
lang: ar
direction: rtl
source: docs/formal/sumeragi/README.md
status: needs-review
generator: scripts/sync_docs_i18n.py
source_hash: 11eb72b5851bd4763895248c9253df49c337fb2b0921b008672e86ae77caf21a
source_last_modified: "2026-06-21T13:31:16.238431+00:00"
translation_last_reviewed: null
translator: machine-google-reviewed
---

# Sumeragi النموذج الرسمي (TLA+ / Apalache)

يحتوي هذا الدليل على نماذج رسمية محددة للسلامة والحيوية Sumeragi.

## النطاق

يلتقط `Sumeragi.tla` مسار الالتزام:
- تقدم المرحلة (`Propose`، `Prepare`، `CommitVote`، `NewView`، `Committed`)،
- عتبات التصويت والنصاب القانوني (`CommitQuorum`، `ViewQuorum`)،
- نصاب الحصة المرجح (`StakeQuorum`) لحراس الالتزام بأسلوب NPoS،
- السببية لكرات الدم الحمراء (`Init -> Chunk -> Ready -> Deliver`) مع أدلة الرأس/الملخص،
- ضريبة السلع والخدمات وافتراضات العدالة الضعيفة على إجراءات التقدم الصادق.

يلتقط `SumeragiFrontierRecovery.tla` فئة Taira المعلقة المركزة حول واحدة
الكتلة الحدودية المتجاورة المعلقة:
- أدلة الالتزام بالتصويت أدناه أو عند النصاب القانوني،
- تراكم طابور التصويت والاستنزاف المحلي،
- مفقود مقابل حالة الحمولة المحلية،
- ملكية الاسترداد الجديدة مقابل الملكية القديمة،
- علامة إعادة جدولة النصاب القانوني / سرعة النافذة،
- الحدود المستقبلية/أدلة الرؤية الجديدة التي يمكنها إعادة ترسيخ الحدود المحلية،
- الالتزام الحتمي بعد ضريبة السلع والخدمات، وإعادة الإرسال، وتدوير العرض المحدود، و
  نتائج إسقاط الأدلة صفر.

يقوم كلا النموذجين بتجريد تنسيقات الأسلاك عن عمد، ECDSA/التوقيع
التحقق، وتفاصيل الشبكات الكاملة.

## الملفات- `Sumeragi.tla`: نموذج البروتوكول وخصائصه.
- `Sumeragi_fast.cfg`: مجموعة معلمات أصغر حجمًا صديقة لـ CI.
- `Sumeragi_deep.cfg`: مجموعة معلمات ضغط أكبر.
- `SumeragiFrontierRecovery.tla`: نموذج التعافي الحدودي المركز.
- `SumeragiFrontierRecovery_fast.cfg`: مجموعة معلمات حدودية أصغر حجمًا صديقة لـ CI.
- `SumeragiFrontierRecovery_deep.cfg`: مجموعة الحدود المتراكمة/النافذة/العرض الأكبر.
- `SumeragiFrontierRecovery_wide.cfg`: مجموعة الحدود الأوسع اليدوية.
- `SumeragiFrontierRecovery_bug_stale_owner.cfg`: طفرة المالك التي لا معنى لها بالفشل المتوقع.
- `SumeragiFrontierRecovery_bug_vote_queue.cfg`: طفرة قائمة انتظار التصويت بالفشل المتوقع.

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

ثوابت التعافي الحدودية:
-`TypeInvariant`
-`CommitImpliesVoteQuorum`
-`CommitImpliesPayloadAvailability`
-`VoteBackedNotDroppedAsZeroEvidenceZombie`
- `PostGstVoteBackedFrontierHasProgress`، الذي يستبعد المحطة
  حالة ما بعد ضريبة السلع والخدمات حيث لا يوجد استرداد لـ `pending /\ voteBacked /\ ~committed`،
  الالتزام أو إعادة الإرسال أو التدوير أو الانتقال المحدود.الخاصية الزمنية لاسترداد الحدود:
- `PostGstVoteBackedFrontierEventuallyResolves`: بعد ضريبة السلع والخدمات، كل ما لم يتم حله
  تصل الدولة الحدودية المعلقة المدعومة بالتصويت في النهاية إلى الالتزام والحمولة
  الاسترداد، أو إعادة إرسال النصاب القانوني، أو إعادة إرساء الحدود المستقبلية، أو العرض المحدود
  دوران.
- `RecoveredPayloadEventuallyAdvances`: دولة حدودية مدعومة بالتصويت
  لا يمكن أن تظل الحمولة المستردة معلقة إلى الأبد دون التزام،
  إعادة الإرسال أو إعادة الإرسال أو التدوير.
- `QuorumRetransmitEventuallyLeavesPending`: بمجرد بدء إعادة إرسال النصاب القانوني
  بالنسبة لدولة حدودية مدعومة بالتصويت، يجب أن يتم مسح الغلاف المعلق في نهاية المطاف.
-`FutureFrontierEvidenceEventuallyReanchors`: أدلة الحدود/الرؤية الجديدة اللاحقة
  يجب إما مسح الغلاف المعلق أو استهلاكه كمرساة حدودية.

## خريطة الافتراض

النموذج الحدودي محدود عمدا. هذه هي التنفيذ
السطوح تلخص:| مفهوم النموذج | سطح التنفيذ |
| --- | --- |
| `pending`، `contiguous`، `payloadState` | معالجة `PendingBlock` وفحص الحمولة المحلية في `crates/iroha_core/src/sumeragi/main_loop/reschedule.rs`، بالإضافة إلى تجسيد ملكية BlockCreated/الحدود في `proposal_handlers.rs`. |
| `commitVotes`، `queuedVotes` | يتم إجراء عد الأصوات الالتزامية وبوابة دخول الأصوات بواسطة `reschedule_defers_vote_backed_quorum_timeout_while_vote_queue_backlogged` و`reschedule_ignores_quorum_timeout_vote_queue_backlog` في `crates/iroha_core/src/sumeragi/main_loop/tests.rs`. |
| `recoveryOwner` | حالة مالك الحدود النشطة/التي لا معنى لها في `frontier_slot_has_active_owner_state_for_view(...)`، وعائد المالك الذي لا معنى له في `maybe_yield_stale_frontier_owner_for_fresh_proposal(...)`، وتحل محل التنظيف في `drop_superseded_contiguous_frontier_owner_state(...)`. |
| `quorumRescheduleArmed`، `quorumWindowAge` | إعادة جدولة النصاب القانوني المدعوم بالتصويت في `reschedule_stale_pending_blocks_with_now(...)`؛ تتضمن تغطية الانحدار `reschedule_skips_vote_backed_retransmit_while_frontier_quorum_timeout_window_owned`. |
| `payloadRecovered` | إصلاح الجسم الحدودي الدقيق وقبول إصلاح كرات الدم الحمراء التي لا معنى لها في `request_frontier_owner_body_repair(...)` و`handle_frontier_body_gap_with_topology(...)` و`stale_frontier_rbc_repair_is_actionable(...)`. |
| `quorumRetransmitted`، `rotated` | تحديد هدف إعادة إرسال النصاب القانوني، `rebroadcast_pending_block_updates(...)`، واستدعاءات تغيير العرض الحتمية في `reschedule_stale_pending_blocks_with_now(...)`. |
| `futureFrontierEvidence` | دليل النصاب المستقبلي الجديد/الحدود الأعلى في `on_pacemaker_propose_ready(...)`، الذي يغطيه `pacemaker_reanchors_frontier_when_future_new_view_quorum_exists`. |

## الجري

من جذر المستودع:

```bash
bash scripts/formal/sumeragi_apalache.sh fast
bash scripts/formal/sumeragi_apalache.sh deep
bash scripts/formal/sumeragi_apalache.sh frontier-fast
bash scripts/formal/sumeragi_apalache.sh frontier-deep
bash scripts/formal/sumeragi_apalache.sh frontier-wide
```

يقوم العداء بتعيين Apalache `--length` صريحًا لكل وضع:| الوضع | الطول | الاستخدام المقصود |
| --- | ---: | --- |
| `fast` | 10 | التحقق من مسار التزام CI |
| `deep` | 10 | فحص أكبر لمسار الالتزام |
| `frontier-fast` | 10 | فحص الحدود CI |
| `frontier-deep` | 12 | فحص الحدود الأكبر |
| `frontier-wide` | 14 | فحص الضغط الحدودي يدويًا/ليليًا |

يتجاوز `APALACHE_LENGTH=<n>` الإعداد الافتراضي لكل وضع عند استكشاف ملف
مثال مضاد أو توسيع دليل محدود.

### الإعداد المحلي القابل للتكرار (لا يلزم Docker)

قم بتثبيت سلسلة أدوات Apalache المحلية المثبتة والتي يستخدمها هذا المستودع:

```bash
bash scripts/formal/install_apalache.sh 0.52.2
```

يكتشف العداء هذا التثبيت تلقائيًا في:
`target/apalache/toolchains/v0.52.2/bin/apalache-mc`.
بعد التثبيت، يجب أن يعمل `ci/check_sumeragi_formal.sh` بدون vars env الإضافية:

```bash
bash ci/check_sumeragi_formal.sh
```

طفرات الفشل المتوقع تكون خارج نطاق CI الطبيعي عن عمد. ينبغي عليهم ذلك
تفشل تحت Apalache وتكون مفيدة عند تغيير النموذج:

```bash
bash ci/check_sumeragi_formal_expected_failures.sh
```

إذا لم يكن Apalache موجودًا في `PATH`، فيمكنك:

- اضبط `APALACHE_BIN` على المسار القابل للتنفيذ، أو
- استخدم الخيار الاحتياطي Docker (يتم تمكينه افتراضيًا عندما يكون `docker` متاحًا):
  - الصورة: `APALACHE_DOCKER_IMAGE` (الافتراضي `ghcr.io/apalache-mc/apalache:0.52.2`)
  - يتطلب البرنامج الخفي Docker قيد التشغيل
  - تعطيل الإجراء الاحتياطي باستخدام `APALACHE_ALLOW_DOCKER=0`.

أمثلة:

```bash
APALACHE_BIN=/opt/apalache/bin/apalache-mc bash scripts/formal/sumeragi_apalache.sh fast
APALACHE_DOCKER_IMAGE=ghcr.io/apalache-mc/apalache:0.52.2 bash scripts/formal/sumeragi_apalache.sh frontier-deep
```

## ملاحظات- يكمل هذا النموذج (لا يحل محل) اختبارات نموذج الصدأ القابلة للتنفيذ في
  `crates/iroha_core/src/sumeragi/main_loop/tests/state_machine_model_tests.rs`
  و
  `crates/iroha_core/src/sumeragi/main_loop/tests/state_machine_fairness_model_tests.rs`.
- الاختبارات محددة بقيم ثابتة في ملفات `.cfg`.
- يقوم PR CI بتشغيل عمليات التحقق هذه في `.github/workflows/pr.yml` عبر
  `ci/check_sumeragi_formal.sh`.
