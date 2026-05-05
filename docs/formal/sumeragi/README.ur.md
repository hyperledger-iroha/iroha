<!-- Auto-generated stub for Urdu (ur) translation. Replace this content with the full translation. -->

---
lang: ur
direction: rtl
source: docs/formal/sumeragi/README.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: e89f83a4ce35b7cab8d3bfcee27eafb761f6a281c445a7cae13ae9d228760fe7
source_last_modified: "2026-04-30T20:10:10.884040+00:00"
translation_last_reviewed: 2026-05-01
translator: machine-google-reviewed
---

# Sumeragi رسمی ماڈل (TLA+ / Apalache)

اس ڈائریکٹری میں Sumeragi حفاظت اور زندہ دلی کے لیے پابند رسمی ماڈل شامل ہیں۔

## دائرہ کار

`Sumeragi.tla` عزم کا راستہ پکڑتا ہے:
- مرحلے کی ترقی (`Propose`, `Prepare`, `CommitVote`, `NewView`, `Committed`)
- ووٹ اور کورم کی حد (`CommitQuorum`, `ViewQuorum`)،
- وزنی اسٹیک کورم (`StakeQuorum`) NPoS طرز کے کمٹ گارڈز کے لیے،
- RBC causality (`Init -> Chunk -> Ready -> Deliver`) ہیڈر/ڈائجسٹ ثبوت کے ساتھ،
- جی ایس ٹی اور ایماندارانہ پیشرفت کے اقدامات پر کمزور منصفانہ مفروضے۔

`SumeragiFrontierRecovery.tla` ایک کے ارد گرد مرکوز Taira hang کلاس کو پکڑتا ہے۔
زیر التوا متصل سرحدی بلاک:
- کمٹ ووٹ کا ثبوت نیچے یا کورم پر،
- ووٹ کی قطار کا بیک لاگ اور مقامی ڈرین،
- لاپتہ بمقابلہ مقامی پے لوڈ ریاست،
- تازہ بمقابلہ باسی سرحدی بحالی کی ملکیت،
- کورم ری شیڈول مارکر/ونڈو پیسنگ،
- مستقبل کے سرحدی/نئے منظر کے ثبوت جو مقامی سرحد کو دوبارہ ترتیب دے سکتے ہیں،
- GST کے بعد کا تعین، دوبارہ ترسیل، باؤنڈڈ ویو روٹیشن، اور
  صفر ثبوت ڈراپ نتائج.

دونوں ماڈلز جان بوجھ کر تار کی شکلوں، ECDSA/دستخط کو خلاصہ کرتے ہیں۔
تصدیق، اور نیٹ ورکنگ کی مکمل تفصیلات۔

## فائلیں۔- `Sumeragi.tla`: پروٹوکول ماڈل اور خصوصیات۔
- `Sumeragi_fast.cfg`: چھوٹا CI-دوستانہ پیرامیٹر سیٹ۔
- `Sumeragi_deep.cfg`: بڑا تناؤ پیرامیٹر سیٹ۔
- `SumeragiFrontierRecovery.tla`: فوکسڈ فرنٹیئر ریکوری ماڈل۔
- `SumeragiFrontierRecovery_fast.cfg`: چھوٹا CI-دوستانہ فرنٹیئر پیرامیٹر سیٹ۔
- `SumeragiFrontierRecovery_deep.cfg`: بڑا فرنٹیئر بیک لاگ/ونڈو/ویو باؤنڈ سیٹ۔
- `SumeragiFrontierRecovery_wide.cfg`: دستی وسیع فرنٹیئر باؤنڈ سیٹ۔
- `SumeragiFrontierRecovery_bug_stale_owner.cfg`: متوقع ناکامی باسی مالک کی تبدیلی۔
- `SumeragiFrontierRecovery_bug_vote_queue.cfg`: متوقع ناکامی ووٹ کی قطار میں تبدیلی۔

## خواص

متغیرات:
- `TypeInvariant`
- `CommitImpliesQuorum`
- `CommitImpliesStakeQuorum`
- `CommitImpliesDelivered`
- `DeliverImpliesEvidence`

عارضی جائیداد:
- `EventuallyCommit` (`[] (gst => <> committed)`)، جی ایس ٹی کے بعد کی منصفانہ انکوڈ کے ساتھ
  `Next` میں عملی طور پر (ٹائم آؤٹ/فالٹ پریمپشن گارڈز فعال
  ترقی کے اقدامات)۔ یہ Apalache 0.52.x کے ساتھ ماڈل کو چیک کرنے کے قابل رکھتا ہے، جو
  چیک شدہ وقتی خصوصیات کے اندر `WF_` فیئرنس آپریٹرز کو سپورٹ نہیں کرتا ہے۔

فرنٹیئر ریکوری انویرینٹس:
- `TypeInvariant`
- `CommitImpliesVoteQuorum`
- `CommitImpliesPayloadAvailability`
- `VoteBackedNotDroppedAsZeroEvidenceZombie`
- `PostGstVoteBackedFrontierHasProgress`، جو ٹرمینل کو مسترد کرتا ہے۔
  GST کے بعد کی ریاست جہاں `pending /\ voteBacked /\ ~committed` کی کوئی وصولی نہیں ہے،
  کمٹ، دوبارہ ترسیل، گردش، یا باؤنڈڈ ڈراپ ٹرانزیشن۔فرنٹیئر ریکوری عارضی جائیداد:
- `PostGstVoteBackedFrontierEventuallyResolves`: GST کے بعد، ہر حل طلب
  ووٹ کی حمایت سے زیر التواء سرحدی ریاست بالآخر کمٹ، پے لوڈ تک پہنچ جاتی ہے۔
  ریکوری، کورم ری ٹرانسمٹ، مستقبل کے سرحدی رینکور، یا باؤنڈڈ ویو
  گردش
- `RecoveredPayloadEventuallyAdvances`: ووٹ کی حمایت یافتہ سرحدی ریاست جس میں ہے۔
  بازیاب شدہ پے لوڈ بغیر عہد کے ہمیشہ کے لیے زیر التواء نہیں رہ سکتا،
  دوبارہ ترسیل، رینکور، یا گردش۔
- `QuorumRetransmitEventuallyLeavesPending`: ایک بار کورم ری ٹرانسمٹ ختم ہونے کے بعد
  ووٹ کی حمایت یافتہ سرحدی ریاست کے لیے، زیر التواء ریپر کو بالآخر صاف کرنا چاہیے۔
- `FutureFrontierEvidenceEventuallyReanchors`: بعد میں فرنٹیئر/نئے منظر کا ثبوت
  یا تو زیر التواء ریپر کو صاف کرنا چاہیے یا فرنٹیئر رینکر کے طور پر استعمال کیا جانا چاہیے۔

## مفروضہ نقشہ

فرنٹیئر ماڈل جان بوجھ کر محدود ہے۔ یہ عمل درآمد ہیں۔
سطحوں کو خلاصہ کرتا ہے:| ماڈل کا تصور | نفاذ کی سطح |
| --- | --- |
| `pending`, `contiguous`, `payloadState` | `PendingBlock` ہینڈلنگ اور `crates/iroha_core/src/sumeragi/main_loop/reschedule.rs` میں مقامی پے لوڈ چیکس کے علاوہ `proposal_handlers.rs` میں BlockCreated/فرنٹیئر اونرشپ کا مواد بنانا۔ |
| `commitVotes`, `queuedVotes` | `reschedule_defers_vote_backed_quorum_timeout_while_vote_queue_backlogged` اور `crates/iroha_core/src/sumeragi/main_loop/tests.rs` میں `crates/iroha_core/src/sumeragi/main_loop/tests.rs` کے ذریعے کمٹ ووٹ کی گنتی اور ووٹ داخل کرنے کا عمل۔ |
| `recoveryOwner` | `frontier_slot_has_active_owner_state_for_view(...)` میں فعال/باسی فرنٹیئر مالک کی ریاست، `maybe_yield_stale_frontier_owner_for_fresh_proposal(...)` میں باسی مالک کی پیداوار، اور `drop_superseded_contiguous_frontier_owner_state(...)` میں صفائی کو ختم کرنا۔ |
| `quorumRescheduleArmed`, `quorumWindowAge` | `reschedule_stale_pending_blocks_with_now(...)` میں ووٹ کی حمایت یافتہ کورم ری شیڈول پیسنگ؛ ریگریشن کوریج میں `reschedule_skips_vote_backed_retransmit_while_frontier_quorum_timeout_window_owned` شامل ہے۔ |
| `payloadRecovered` | `request_frontier_owner_body_repair(...)`، `handle_frontier_body_gap_with_topology(...)`، اور `stale_frontier_rbc_repair_is_actionable(...)` میں عین فرنٹیئر باڈی کی مرمت اور باسی RBC مرمت کا داخلہ۔ |
| `quorumRetransmitted`, `rotated` | کورم ری ٹرانسمٹ ٹارگٹ سلیکشن، `rebroadcast_pending_block_updates(...)`، اور `reschedule_stale_pending_blocks_with_now(...)` میں ڈیٹرمنسٹک ویو چینج کالز۔ |
| `futureFrontierEvidence` | `on_pacemaker_propose_ready(...)` میں مستقبل کے نئے منظر / اعلیٰ سرحدی کورم کا ثبوت، جس کا احاطہ `pacemaker_reanchors_frontier_when_future_new_view_quorum_exists` کے ذریعے کیا گیا ہے۔ |

## چل رہا ہے۔

ذخیرہ کی جڑ سے:

```bash
bash scripts/formal/sumeragi_apalache.sh fast
bash scripts/formal/sumeragi_apalache.sh deep
bash scripts/formal/sumeragi_apalache.sh frontier-fast
bash scripts/formal/sumeragi_apalache.sh frontier-deep
bash scripts/formal/sumeragi_apalache.sh frontier-wide
```

رنر ہر موڈ کے لیے ایک واضح Apalache `--length` سیٹ کرتا ہے:| موڈ | لمبائی | مطلوبہ استعمال |
| --- | ---: | --- |
| `fast` | 10 | CI کمٹ پاتھ چیک |
| `deep` | 10 | بڑا کمٹ پاتھ چیک |
| `frontier-fast` | 10 | CI فرنٹیئر چیک |
| `frontier-deep` | 12 | بڑا فرنٹیئر چیک |
| `frontier-wide` | 14 | دستی / رات کے وقت سرحدی تناؤ کی جانچ |

`APALACHE_LENGTH=<n>` مقامی طور پر تلاش کرتے وقت فی موڈ ڈیفالٹ کو اوور رائیڈ کرتا ہے۔
جوابی مثال یا ایک پابند ثبوت کو وسیع کرنا۔

### دوبارہ پیدا کرنے کے قابل مقامی سیٹ اپ (Docker کی ضرورت نہیں)

اس ذخیرے کے ذریعہ استعمال کردہ پن شدہ مقامی اپالاچی ٹول چین کو انسٹال کریں:

```bash
bash scripts/formal/install_apalache.sh 0.52.2
```

رنر اس انسٹال کا خود بخود پتہ لگاتا ہے:
`target/apalache/toolchains/v0.52.2/bin/apalache-mc`۔
تنصیب کے بعد، `ci/check_sumeragi_formal.sh` کو اضافی env vars کے بغیر کام کرنا چاہیے:

```bash
bash ci/check_sumeragi_formal.sh
```

متوقع ناکامی کی تغیرات جان بوجھ کر عام CI سے باہر ہیں۔ انہیں چاہیے
Apalache کے تحت ناکام ہو جاتے ہیں اور ماڈل کو تبدیل کرتے وقت کارآمد ہوتے ہیں:

```bash
bash ci/check_sumeragi_formal_expected_failures.sh
```

اگر Apalache `PATH` میں نہیں ہے، تو آپ یہ کر سکتے ہیں:

- `APALACHE_BIN` کو قابل عمل راستے پر سیٹ کریں، یا
- Docker فال بیک استعمال کریں (`docker` دستیاب ہونے پر بطور ڈیفالٹ فعال):
  - تصویر: `APALACHE_DOCKER_IMAGE` (پہلے سے طے شدہ `ghcr.io/apalache-mc/apalache:0.52.2`)
  - چلانے والے Docker ڈیمون کی ضرورت ہے۔
  - `APALACHE_ALLOW_DOCKER=0` کے ساتھ فال بیک کو غیر فعال کریں۔

مثالیں:

```bash
APALACHE_BIN=/opt/apalache/bin/apalache-mc bash scripts/formal/sumeragi_apalache.sh fast
APALACHE_DOCKER_IMAGE=ghcr.io/apalache-mc/apalache:0.52.2 bash scripts/formal/sumeragi_apalache.sh frontier-deep
```

## نوٹس- یہ ماڈل قابل عمل زنگ ماڈل ٹیسٹوں کی تکمیل کرتا ہے (تبدیل نہیں کرتا)
  `crates/iroha_core/src/sumeragi/main_loop/tests/state_machine_model_tests.rs`
  اور
  `crates/iroha_core/src/sumeragi/main_loop/tests/state_machine_fairness_model_tests.rs`۔
- چیک `.cfg` فائلوں میں مستقل اقدار کے پابند ہیں۔
- PR CI ان چیکوں کو `.github/workflows/pr.yml` کے ذریعے چلاتا ہے۔
  `ci/check_sumeragi_formal.sh`۔