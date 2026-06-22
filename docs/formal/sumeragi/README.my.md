<!-- Auto-generated stub for Burmese (my) translation. Replace this content with the full translation. -->

---
lang: my
direction: ltr
source: docs/formal/sumeragi/README.md
status: needs-review
generator: scripts/sync_docs_i18n.py
source_hash: 11eb72b5851bd4763895248c9253df49c337fb2b0921b008672e86ae77caf21a
source_last_modified: "2026-06-21T13:31:16.238431+00:00"
translation_last_reviewed: null
translator: machine-google-reviewed
---

#Sumeragi တရားဝင်မော်ဒယ် (TLA+ / Apalache)

ဤလမ်းညွှန်တွင် Sumeragi ဘေးကင်းမှုနှင့် အသက်ရှင်သန်မှုအတွက် ကန့်သတ်ထားသော တရားဝင်မော်ဒယ်များ ပါရှိသည်။

## နယ်ပယ်

`Sumeragi.tla` သည် commit လမ်းကြောင်းကို ဖမ်းယူသည်-
- အဆင့်တိုးတက်မှု (`Propose`, `Prepare`, `CommitVote`, `NewView`, `Committed`)၊
- ဆန္ဒမဲနှင့် အထမြောက်မှု သတ်မှတ်ချက်များ (`CommitQuorum`, `ViewQuorum`)၊
- NPoS စတိုင် ကျူးလွန်အစောင့်များအတွက်၊
- ခေါင်းစီး/အချေအတင် အထောက်အထားများဖြင့် RBC အကြောင်းရင်း (`Init -> Chunk -> Ready -> Deliver`)၊
- GST နှင့် ရိုးသားသော တိုးတက်မှုလုပ်ဆောင်ချက်များအပေါ် အားနည်းသော တရားမျှတမှုဆိုင်ရာ ယူဆချက်များ။

`SumeragiFrontierRecovery.tla` သည် တစ်ခုအနီးတစ်ဝိုက်တွင် focused Taira hang class ကိုဖမ်းယူသည်။
ဆိုင်းငံ့နေသော နယ်ခြားမျဉ်းပိတ်ဆို့-
- အောက်ဖော်ပြပါ သို့မဟုတ် အထမြောက်သော ဆန္ဒမဲပေးသည့် အထောက်အထား၊
- မဲတန်းစီဇယားနှင့် ရပ်ကွက်တွင်း ရေဆင်း၊
- ပျောက်ဆုံးနေမှုနှင့် ဒေသတွင်း ဝန်ထုပ်ဝန်ပိုးအခြေအနေ၊
- fresh vs. stale frontier recovery ကျတယ်၊
- အထမြောက်ခြင်း- အချိန်ဇယား အမှတ်အသား/ ပြတင်းပေါက် အစီအမံ၊
- ဒေသန္တရနယ်နိမိတ်ကို မှီဝဲနိုင်သည့် အနာဂတ်နယ်ခြား/အမြင်သစ် အထောက်အထားများ၊
- အဆုံးအဖြတ်ပေးသော GST ကျူးလွန်မှု၊ ပြန်လည်ပေးပို့မှု၊ ကန့်သတ်ထားသော မြင်ကွင်း-လည်ပတ်မှုနှင့်
  သုညအထောက်အထား ကျဆင်းမှု ရလဒ်များ။

မော်ဒယ်နှစ်ခုစလုံးသည် ရည်ရွယ်ချက်ရှိရှိ လွဲမှားနေသော ဝါယာကြိုးဖော်မတ်များ၊ ECDSA/လက်မှတ်
အတည်ပြုခြင်းနှင့် ကွန်ရက်ချိတ်ဆက်မှုဆိုင်ရာ အသေးစိတ်အချက်အလက်များ အပြည့်အစုံ။

## ဖိုင်များ- `Sumeragi.tla`- ပရိုတိုကော မော်ဒယ်နှင့် ဂုဏ်သတ္တိများ။
- `Sumeragi_fast.cfg`- သေးငယ်သော CI-ဖော်ရွေသော ကန့်သတ်ဘောင်။
- `Sumeragi_deep.cfg`- ပိုကြီးသော ဖိစီးမှု ကန့်သတ်ဘောင်။
- `SumeragiFrontierRecovery.tla`- အာရုံစိုက်ထားသော နယ်ခြားပြန်လည်ရယူရေးမော်ဒယ်။
- `SumeragiFrontierRecovery_fast.cfg`- သေးငယ်သော CI-ဖော်ရွေသော ရှေ့တန်း ကန့်သတ်ဘောင်။
- `SumeragiFrontierRecovery_deep.cfg`- ပိုကြီးသော frontier backlog/window/view bound set။
- `SumeragiFrontierRecovery_wide.cfg`- လက်စွဲပိုကျယ်သော နယ်နိမိတ်ဘောင်းကျင်အစုံ။
- `SumeragiFrontierRecovery_bug_stale_owner.cfg`- မျှော်လင့်ထားသော-ပျက်ကွက်မှု အဟောင်း-ပိုင်ရှင် ဗီဇပြောင်းခြင်း။
- `SumeragiFrontierRecovery_bug_vote_queue.cfg`- မျှော်မှန်း-ကျရှုံးမဲ-စာရင်း ပြောင်းလဲမှု။

## သတ္တိ

ပုံစံကွဲများ-
- `TypeInvariant`
- `CommitImpliesQuorum`
- `CommitImpliesStakeQuorum`
- `CommitImpliesDelivered`
- `DeliverImpliesEvidence`

ယာယီပိုင်ဆိုင်မှု-
- GST လွန်မျှတမှု ကုဒ်ဖြင့် ပြုလုပ်ထားသော `EventuallyCommit` (`[] (gst => <> committed)`)
  `Next` တွင် လည်ပတ်လုပ်ဆောင်နိုင်သည် (အချိန်လွန်/အမှားပြင်ဆင်မှု အစောင့်များကို ဖွင့်ထားသည်
  တိုးတက်မှုလုပ်ဆောင်ချက်များ)။ ၎င်းသည် မော်ဒယ်ကို Apalache 0.52.x ဖြင့် စစ်ဆေးနိုင်စေပါသည်။
  စစ်ဆေးထားသော ယာယီဂုဏ်သတ္တိများအတွင်းရှိ `WF_` တရားမျှတမှု အော်ပရေတာများကို မပံ့ပိုးပါ။

နယ်ခြားပြန်လည်ရယူရေးပုံစံများ-
- `TypeInvariant`
- `CommitImpliesVoteQuorum`
- `CommitImpliesPayloadAvailability`
- `VoteBackedNotDroppedAsZeroEvidenceZombie`
- `PostGstVoteBackedFrontierHasProgress`၊
  `pending /\ voteBacked /\ ~committed` တွင် ပြန်လည်ရယူခြင်းမရှိသော GST လွန်အခြေအနေ၊
  ကတိပြုခြင်း၊ ပြန်ပို့ခြင်း၊ လှည့်ခြင်း၊ သို့မဟုတ် ကန့်သတ်ထားသော လွှတ်တင်ခြင်းအကူးအပြောင်း။နယ်ခြားပြန်လည်ထူထောင်ရေး ယာယီပိုင်ဆိုင်မှု-
- `PostGstVoteBackedFrontierEventuallyResolves`- GST ပြီးနောက်၊ အားလုံးကို မဖြေရှင်းနိုင်ပါ။
  မဲကျောထောက်နောက်ခံပြုထားသော ဆိုင်းငံ့ထားသော နယ်နိမိတ်ပြည်နယ်သည် နောက်ဆုံးတွင် commit, payload သို့ရောက်ရှိသွားပါသည်။
  ပြန်လည်ရယူခြင်း၊ အထမြောက်ခြင်း ပြန်လည်ပေးပို့ခြင်း၊ အနာဂတ်နယ်နိမိတ်ဆွဲခြင်း သို့မဟုတ် ကန့်သတ်ထားသော မြင်ကွင်း
  လည်ပတ်မှု။
- `RecoveredPayloadEventuallyAdvances`- ဆန္ဒမဲဖြင့် ကျောထောက်နောက်ခံပြုထားသော နယ်နိမိတ်ပြည်နယ်တစ်ခု
  ဝန်ထုပ်ဝန်ပိုးကို ပြန်လည်ရယူပြီး ကတိမတည်ဘဲ ထာဝစဉ် ဆိုင်းငံ့မနေနိုင်ပါ။
  retransmit၊ renchor သို့မဟုတ် rotation။
- `QuorumRetransmitEventuallyLeavesPending`- quotum retransmit ပြီးသည်နှင့်တစ်ပြိုင်နက်
  မဲကျောထောက်နောက်ခံပြုထားသော နယ်နိမိတ်ပြည်နယ်တစ်ခုအတွက်၊ ဆိုင်းငံ့ထားသော ထုပ်ပိုးမှုကို နောက်ဆုံးတွင် ရှင်းလင်းရပါမည်။
- `FutureFrontierEvidenceEventuallyReanchors`- နောက်ပိုင်း နယ်နိမိတ်/အမြင်သစ် အထောက်အထား
  ဆိုင်းငံ့ထားသော ထုပ်ပိုးမှုကို ရှင်းပစ်ရမည် သို့မဟုတ် နယ်နိမိတ် ဖြတ်တောက်ခြင်းအဖြစ် စားသုံးရမည်။

## ယူဆချက်မြေပုံ

Frontier model သည် ရည်ရွယ်ချက်ရှိရှိ ကန့်သတ်ချက်ဖြစ်သည်။ ဒါတွေက အကောင်အထည်ဖော်မှုပါ။
၎င်းကို abstract ပေါ်အောင်ဖော်ပြသည်| စံပြအယူအဆ | အကောင်အထည်ဖော်ခြင်း |
| ---| ---|
| `pending`, `contiguous`, `payloadState` | `PendingBlock` ကိုင်တွယ်ခြင်းနှင့် `crates/iroha_core/src/sumeragi/main_loop/reschedule.rs` တွင် ဒေသတွင်း ပေးချေမှုစစ်ဆေးမှုများ၊ နှင့် `proposal_handlers.rs` တွင် BlockCreated/ Frontier ပိုင်ဆိုင်ခွင့် အကောင်အထည်ပေါ်လာခြင်း |
| `commitVotes`, `queuedVotes` | `reschedule_defers_vote_backed_quorum_timeout_while_vote_queue_backlogged` နှင့် `reschedule_ignores_quorum_timeout_vote_queue_backlog` တွင် `crates/iroha_core/src/sumeragi/main_loop/tests.rs` ဖြင့်ကျင့်သုံးသော ဆန္ဒမဲရေတွက်ခြင်းနှင့် မဲအဝင်ဂိတ်ကို ကတိပြုခြင်း။ |
| `recoveryOwner` | `frontier_slot_has_active_owner_state_for_view(...)` တွင် အသက်ဝင်သော/ဟောင်းနွမ်းနေသော နယ်နိမိတ်ပိုင်ရှင်ပြည်နယ်၊ `maybe_yield_stale_frontier_owner_for_fresh_proposal(...)` တွင် ပိုင်ရှင်မဲ့အထွက်နှုန်းနှင့် `drop_superseded_contiguous_frontier_owner_state(...)` တွင် သန့်ရှင်းရေးကို အစားထိုးထားသည်။ |
| `quorumRescheduleArmed`, `quorumWindowAge` | `reschedule_stale_pending_blocks_with_now(...)` တွင် မဲဆန္ဒပေးထားသော ကျောထောက်နောက်ခံပြုထားသော အစီရမ်အစီအစဥ်အား ပြန်လည်သတ်မှတ်ခြင်း ဆုတ်ယုတ်မှုလွှမ်းခြုံမှုတွင် `reschedule_skips_vote_backed_retransmit_while_frontier_quorum_timeout_window_owned` ပါဝင်သည်။ |
| `payloadRecovered` | `request_frontier_owner_body_repair(...)`၊ `handle_frontier_body_gap_with_topology(...)` နှင့် `stale_frontier_rbc_repair_is_actionable(...)` တွင် နယ်ခြားကိုယ်ထည်ပြုပြင်ခြင်းနှင့် ဟောင်းနွမ်းနေသော RBC ပြုပြင်ခြင်းဝင်ခွင့် အတိအကျ။ |
| `quorumRetransmitted`, `rotated` | Quorum သည် ပစ်မှတ်ရွေးချယ်မှု၊ `rebroadcast_pending_block_updates(...)` နှင့် `reschedule_stale_pending_blocks_with_now(...)` တွင် အဆုံးအဖြတ်ပေးသော အမြင်ပြောင်းလဲမှုခေါ်ဆိုမှုများကို ပြန်လည်ပေးပို့သည်။ |
| `futureFrontierEvidence` | `pacemaker_reanchors_frontier_when_future_new_view_quorum_exists` ဖြင့် အကျုံးဝင်သော `on_pacemaker_propose_ready(...)` ရှိ အနာဂတ်အမြင်သစ်/ပိုမိုမြင့်မားသော ရှေ့တန်းအထမြောက်သော အထမြောက် အထောက်အထား။ |

## ပြေးသည်။

repository root မှ

```bash
bash scripts/formal/sumeragi_apalache.sh fast
bash scripts/formal/sumeragi_apalache.sh deep
bash scripts/formal/sumeragi_apalache.sh frontier-fast
bash scripts/formal/sumeragi_apalache.sh frontier-deep
bash scripts/formal/sumeragi_apalache.sh frontier-wide
```

အပြေးသမားသည် မုဒ်တစ်ခုစီအတွက် အတိအလင်း Apalache `--length` ကို သတ်မှတ်သည်-| မုဒ် | အရှည် | အသုံးပြုရန် ရည်ရွယ်သည် |
| ---| ---: | ---|
| `fast` | 10 | CI commit-path check |
| `deep` | 10 | ပိုကြီးတဲ့ commit-path check |
| `frontier-fast` | 10 | CI နယ်ခြားစစ်ဆေးခြင်း |
| `frontier-deep` | 12 | ပိုကြီးတဲ့ နယ်ခြားစစ်ဆေးမှု |
| `frontier-wide` | 14 | လူကိုယ်တိုင်/ ညစဉ် ဖိစီးမှု စစ်ဆေးခြင်း |

`APALACHE_LENGTH=<n>` သည် စက်တွင်းတစ်ခုအား ရှာဖွေသည့်အခါ per-mode ပုံသေကို လွှမ်းမိုးသည်
တန်ပြန်ဥပမာ သို့မဟုတ် ကန့်သတ်ထားသောသက်သေကို ချဲ့ထွင်ခြင်း။

### ပြန်လည်ထုတ်လုပ်နိုင်သော စက်တွင်းထည့်သွင်းမှု (Docker မလိုအပ်ပါ)

ဤသိုလှောင်ခန်းမှအသုံးပြုသော ပင်ထိုးထားသော ဒေသတွင်း Apalache toolchain ကို ထည့်သွင်းပါ-

```bash
bash scripts/formal/install_apalache.sh 0.52.2
```

အပြေးသမားသည် ဤထည့်သွင်းမှုကို အလိုအလျောက် သိရှိသည်-
`target/apalache/toolchains/v0.52.2/bin/apalache-mc`။
ထည့်သွင်းပြီးနောက်၊ `ci/check_sumeragi_formal.sh` သည် အပို env vars မပါဘဲ အလုပ်လုပ်သင့်သည်-

```bash
bash ci/check_sumeragi_formal.sh
```

မျှော်မှန်းထားသည့် ပျက်ကွက်သော ဗီဇပြောင်းလဲမှုများသည် သာမန် CI ပြင်ပတွင် ရည်ရွယ်ချက်ရှိရှိ ဆောင်ရွက်ခြင်း ဖြစ်သည်။ လုပ်သင့်တယ်။
Apalache အောက်တွင် ပျက်ကွက်ပြီး မော်ဒယ်ကို ပြောင်းလဲသည့်အခါ အသုံးဝင်သည်-

```bash
bash ci/check_sumeragi_formal_expected_failures.sh
```

Apalache သည် `PATH` တွင်မဟုတ်ပါက၊ သင်သည်-

- `APALACHE_BIN` ကို executable path သို့ သတ်မှတ်ပါ။
- Docker ကို အသုံးပြုပါ (`docker` ကို ရရှိသောအခါ မူရင်းအတိုင်း ဖွင့်ထားသည်)
  ပုံ- `APALACHE_DOCKER_IMAGE` (မူရင်း `ghcr.io/apalache-mc/apalache:0.52.2`)
  - လည်ပတ်နေသော Docker daemon လိုအပ်သည်။
  - `APALACHE_ALLOW_DOCKER=0` ဖြင့် လှည့်ပြန်ခြင်းကို ပိတ်ပါ။

ဥပမာများ-

```bash
APALACHE_BIN=/opt/apalache/bin/apalache-mc bash scripts/formal/sumeragi_apalache.sh fast
APALACHE_DOCKER_IMAGE=ghcr.io/apalache-mc/apalache:0.52.2 bash scripts/formal/sumeragi_apalache.sh frontier-deep
```

## မှတ်ချက်- ဤမော်ဒယ်သည် လည်ပတ်နိုင်သော Rust မော်ဒယ်စမ်းသပ်မှုများကို ဖြည့်စွက်ပေးပါသည်။
  `crates/iroha_core/src/sumeragi/main_loop/tests/state_machine_model_tests.rs`
  နှင့်
  `crates/iroha_core/src/sumeragi/main_loop/tests/state_machine_fairness_model_tests.rs`။
- စစ်ဆေးမှုများသည် `.cfg` ဖိုင်များတွင် အဆက်မပြတ်တန်ဖိုးများဖြင့် ကန့်သတ်ထားသည်။
- PR CI သည် ဤစစ်ဆေးမှုများကို `.github/workflows/pr.yml` မှတစ်ဆင့် လုပ်ဆောင်သည်။
  `ci/check_sumeragi_formal.sh`။
