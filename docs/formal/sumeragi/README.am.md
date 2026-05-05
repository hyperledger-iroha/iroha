<!-- Auto-generated stub for Amharic (Ethiopian) (am) translation. Replace this content with the full translation. -->

---
lang: am
direction: ltr
source: docs/formal/sumeragi/README.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: e89f83a4ce35b7cab8d3bfcee27eafb761f6a281c445a7cae13ae9d228760fe7
source_last_modified: "2026-04-30T20:10:10.884040+00:00"
translation_last_reviewed: 2026-05-01
translator: machine-google-reviewed
---

# Sumeragi መደበኛ ሞዴል (TLA+ / Apalache)

ይህ ማውጫ ለSumeragi ደህንነት እና ህያውነት የታሰሩ መደበኛ ሞዴሎችን ይዟል።

## ወሰን

`Sumeragi.tla` የተፈፀመውን መንገድ ይይዛል፡-
- የደረጃ እድገት (`Propose`፣ `Prepare`፣ `CommitVote`፣ `NewView`፣ `Committed`)፣
- ድምጽ እና የኮረም ገደቦች (`CommitQuorum`፣ `ViewQuorum`)፣
- ለNPoS አይነት የጥብቅና ጥበቃዎች (`StakeQuorum`) ክብደት ያለው የካስማ ኮረም፣
- RBC መንስኤ (`Init -> Chunk -> Ready -> Deliver`) ከአርዕስት/የመፍጨት ማስረጃ ጋር፣
- GST እና ደካማ የፍትሃዊነት ግምቶች በታማኝነት የእድገት እርምጃዎች ላይ።

`SumeragiFrontierRecovery.tla` በአንድ ዙሪያ ያተኮረውን Taira hang ክፍል ይይዛል
በመጠባበቅ ላይ ያለ የድንበር ማገጃ;
- ከታች ወይም በምልአተ ጉባኤው ድምጽ መስጠት፣
- የድምፅ ወረፋ የኋላ መዝገብ እና የአካባቢ ፍሳሽ ፣
- የጎደለ ከአካባቢያዊ የመጫኛ ሁኔታ ጋር፣
- ትኩስ እና የቆየ የድንበር መልሶ ማግኛ ባለቤትነት፣
- ምልአተ ጉባኤ-ዳግም መርሐግብር ምልክት ማድረጊያ/መስኮት መራመድ፣
- የወደፊቱን የድንበር/የአዲስ እይታ ማስረጃ የአካባቢውን ድንበር እንደገና ሊያስተካክል ይችላል ፣
- የሚወስን ድህረ-GST ቁርጠኝነት፣ እንደገና ማስተላለፍ፣ የታሰረ እይታ-ማሽከርከር እና
  ዜሮ-ማስረጃ ጠብታ ውጤቶች.

ሁለቱም ሞዴሎች ሆን ብለው የሽቦ ቅርጸቶችን፣ ECDSA/ፊርማን ያርቁ
ማረጋገጫ, እና ሙሉ የአውታረ መረብ ዝርዝሮች.

## ፋይሎች- `Sumeragi.tla`: የፕሮቶኮል ሞዴል እና ባህሪያት.
- `Sumeragi_fast.cfg`: አነስ CI-ተስማሚ መለኪያ ስብስብ.
- `Sumeragi_deep.cfg`: ትልቅ የጭንቀት መለኪያ ስብስብ.
- `SumeragiFrontierRecovery.tla`: ያተኮረ የድንበር መልሶ ማግኛ ሞዴል.
- `SumeragiFrontierRecovery_fast.cfg`: አነስ CI-ተስማሚ የድንበር መለኪያ ስብስብ.
- `SumeragiFrontierRecovery_deep.cfg`፡ ትልቅ የድንበር መዝገብ/መስኮት/የእይታ የታሰረ ስብስብ።
- `SumeragiFrontierRecovery_wide.cfg`: በእጅ ሰፊ ድንበር የታሰረ ስብስብ.
- `SumeragiFrontierRecovery_bug_stale_owner.cfg`: የሚጠበቀው-ውድቀት የቆየ-የባለቤት ሚውቴሽን.
- `SumeragiFrontierRecovery_bug_vote_queue.cfg`: የሚጠበቀው-ውድቀት የድምጽ-ወረፋ ሚውቴሽን.

## ንብረቶች

ተለዋዋጮች
- `TypeInvariant`
- `CommitImpliesQuorum`
- `CommitImpliesStakeQuorum`
- `CommitImpliesDelivered`
- `DeliverImpliesEvidence`

ጊዜያዊ ንብረት;
- `EventuallyCommit` (`[] (gst => <> committed)`)፣ ከጂኤስቲ በኋላ ፍትሃዊነት በኮድ
  በ `Next` ውስጥ የሚሰራ (የጊዜ ማብቂያ/የጥፋት ቅድመ-መከላከያ ነቅቷል)
  የእድገት እርምጃዎች). ይሄ ሞዴሉን ከ Apalache 0.52.x ጋር መፈተሽ እንዲችል ያደርገዋል, ይህም
  በተረጋገጡ ጊዜያዊ ንብረቶች ውስጥ የ`WF_` የፍትሃዊነት ኦፕሬተሮችን አይደግፍም።

የድንበር ማገገሚያ ልዩነቶች
- `TypeInvariant`
- `CommitImpliesVoteQuorum`
- `CommitImpliesPayloadAvailability`
- `VoteBackedNotDroppedAsZeroEvidenceZombie`
- ተርሚናል የሚገዛው `PostGstVoteBackedFrontierHasProgress`
  የድህረ-GST ሁኔታ `pending /\ voteBacked /\ ~committed` ምንም ማገገሚያ የሌለው፣
  መፈጸም፣ እንደገና ማስተላለፍ፣ ማሽከርከር ወይም የታሰረ-ጠብታ ሽግግር።የድንበር ማገገሚያ ጊዜያዊ ንብረት፡
- `PostGstVoteBackedFrontierEventuallyResolves`: ከ GST በኋላ, እያንዳንዱ ያልተፈታ
  በድምፅ የተደገፈ በመጠባበቅ ላይ ያለ የድንበር ግዛት በመጨረሻ ቁርጠኝነት ላይ ይደርሳል
  ማገገሚያ፣ ምልአተ ጉባኤ እንደገና ማስተላለፍ፣ የወደፊት የድንበር መልሶ ማቋረጫ ወይም የታሰረ እይታ
  ማሽከርከር.
- `RecoveredPayloadEventuallyAdvances`፡ ያለው በድምፅ የሚደገፍ የድንበር ግዛት
  የተመለሰው ጭነት ያለ ቁርጠኝነት ለዘላለም በመጠባበቅ ላይ ሊሆን አይችልም ፣
  እንደገና ማስተላለፍ፣ መልሕቅ ማድረግ ወይም ማሽከርከር።
- `QuorumRetransmitEventuallyLeavesPending`፡ አንዴ ምልአተ ጉባኤው ተቋርጧል
  በድምፅ ለሚደገፍ የድንበር ግዛት፣ በመጠባበቅ ላይ ያለው ጥቅል በመጨረሻ ማጽዳት አለበት።
- `FutureFrontierEvidenceEventuallyReanchors`: በኋላ ድንበር / አዲስ እይታ ማስረጃ
  በመጠባበቅ ላይ ያለውን መጠቅለያ ማጽዳት ወይም እንደ የድንበር ሪከርክ መጠቀም አለበት.

## ግምት ካርታ

የድንበር አምሳያው ሆን ተብሎ የተገደበ ነው። አተገባበሩም እነዚህ ናቸው።
ያብራራል፡-| የሞዴል ጽንሰ-ሐሳብ | የትግበራ ወለል |
| --- | --- |
| `pending`፣ `contiguous`፣ `payloadState` | የ`PendingBlock` አያያዝ እና የአከባቢ ክፍያ ፍተሻዎች በ`crates/iroha_core/src/sumeragi/main_loop/reschedule.rs`፣ በተጨማሪም BlockCreated/የድንበር ባለቤትነት በ`proposal_handlers.rs`። |
| `commitVotes`, `queuedVotes` | በ`reschedule_defers_vote_backed_quorum_timeout_while_vote_queue_backlogged` እና በ`reschedule_ignores_quorum_timeout_vote_queue_backlog` በ`crates/iroha_core/src/sumeragi/main_loop/tests.rs` የተፈፀመ የድምፅ ቆጠራ እና የድምፅ ማስገቢያ ጨዋታ። |
| `recoveryOwner` | በ`frontier_slot_has_active_owner_state_for_view(...)` ውስጥ የገባ/የቆየ የድንበር ባለቤት ሁኔታ፣ በ`maybe_yield_stale_frontier_owner_for_fresh_proposal(...)` ውስጥ የቆየ ባለቤት ምርት እና በ`drop_superseded_contiguous_frontier_owner_state(...)` ውስጥ ጽዳትን ይተካል። |
| `quorumRescheduleArmed`, `quorumWindowAge` | በድምጽ የተደገፈ ምልአተ ጉባኤን በ`reschedule_stale_pending_blocks_with_now(...)` ውስጥ እንደገና ቀጠሮ ማስያዝ; የማገገሚያ ሽፋን `reschedule_skips_vote_backed_retransmit_while_frontier_quorum_timeout_window_owned` ያካትታል። |
| `payloadRecovered` | ትክክለኛ የድንበር አካል ጥገና እና የቆየ RBC ጥገና በ`request_frontier_owner_body_repair(...)`፣ `handle_frontier_body_gap_with_topology(...)` እና `stale_frontier_rbc_repair_is_actionable(...)`። |
| `quorumRetransmitted`, `rotated` | ምልአተ ጉባኤ የዒላማ ምርጫን፣ `rebroadcast_pending_block_updates(...)` እና የመወሰን የእይታ ለውጥ ጥሪዎችን በ`reschedule_stale_pending_blocks_with_now(...)` እንደገና ያስተላልፋል። |
| `futureFrontierEvidence` | በ`on_pacemaker_propose_ready(...)` ውስጥ የወደፊት አዲስ እይታ/ከፍተኛ የድንበር ምልአተ ጉባኤ ማስረጃ በ`pacemaker_reanchors_frontier_when_future_new_view_quorum_exists` የተሸፈነ። |

## በመሮጥ ላይ

ከማከማቻ ስር፡

```bash
bash scripts/formal/sumeragi_apalache.sh fast
bash scripts/formal/sumeragi_apalache.sh deep
bash scripts/formal/sumeragi_apalache.sh frontier-fast
bash scripts/formal/sumeragi_apalache.sh frontier-deep
bash scripts/formal/sumeragi_apalache.sh frontier-wide
```

ሯጩ ለእያንዳንዱ ሁነታ ግልጽ Apalache `--length` ያዘጋጃል፡| ሁነታ | ርዝመት | የታሰበ አጠቃቀም |
| --- | ---: | --- |
| `fast` | 10 | CI ቁርጠኝነት-መንገድ ማረጋገጥ |
| `deep` | 10 | ትልቅ የቁርጥ መንገድ ፍተሻ |
| `frontier-fast` | 10 | CI ድንበር ማረጋገጥ |
| `frontier-deep` | 12 | ትልቅ የድንበር ፍተሻ |
| `frontier-wide` | 14 | በእጅ/በሌሊት የድንበር ጭንቀት ማረጋገጥ |

`APALACHE_LENGTH=<n>` በአገር ውስጥ አንድን ሲያስሱ በየሞድ ነባሪውን ይሽራል።
ተቃራኒ ምሳሌ ወይም የታሰረ ማስረጃን ማስፋት።

### ሊባዛ የሚችል የአካባቢ ማዋቀር (አይ18NT00000004X አያስፈልግም)

በዚህ ማከማቻ ጥቅም ላይ የዋለውን የተሰካውን የአካባቢ Apalache መሣሪያ ሰንሰለት ጫን፡-

```bash
bash scripts/formal/install_apalache.sh 0.52.2
```

ሯጩ ይህንን ጭነት በሚከተለው ላይ በራስ-ሰር ያውቀዋል።
`target/apalache/toolchains/v0.52.2/bin/apalache-mc`.
ከተጫነ በኋላ, `ci/check_sumeragi_formal.sh` ያለ ተጨማሪ env vars መስራት አለበት:

```bash
bash ci/check_sumeragi_formal.sh
```

የሚጠበቀው-ውድቀት ሚውቴሽን ሆን ተብሎ ከመደበኛ CI ውጪ ናቸው። አለባቸው
በ Apalache ስር አለመሳካት እና ሞዴሉን ሲቀይሩ ጠቃሚ ናቸው:

```bash
bash ci/check_sumeragi_formal_expected_failures.sh
```

Apalache በ `PATH` ውስጥ ካልሆነ የሚከተሉትን ማድረግ ይችላሉ:

- `APALACHE_BIN` ወደ ተፈፃሚው መንገድ ያዘጋጁ ፣ ወይም
- የDocker ውድቀትን ይጠቀሙ (`docker` ሲገኝ በነባሪነት የነቃ)
  - ምስል፡ `APALACHE_DOCKER_IMAGE` (ነባሪ `ghcr.io/apalache-mc/apalache:0.52.2`)
  - የሚያሄድ Docker ዴሞን ይፈልጋል
  - በ`APALACHE_ALLOW_DOCKER=0` መመለስን ያሰናክሉ።

ምሳሌዎች፡-

```bash
APALACHE_BIN=/opt/apalache/bin/apalache-mc bash scripts/formal/sumeragi_apalache.sh fast
APALACHE_DOCKER_IMAGE=ghcr.io/apalache-mc/apalache:0.52.2 bash scripts/formal/sumeragi_apalache.sh frontier-deep
```

## ማስታወሻዎች- ይህ ሞዴል የ Rust ሞዴል ሙከራዎችን ያሟላል (አይተካም)
  `crates/iroha_core/src/sumeragi/main_loop/tests/state_machine_model_tests.rs`
  እና
  `crates/iroha_core/src/sumeragi/main_loop/tests/state_machine_fairness_model_tests.rs`.
- ቼኮች በ `.cfg` ፋይሎች ውስጥ በቋሚ እሴቶች የተገደቡ ናቸው።
- PR CI እነዚህን ቼኮች በ `.github/workflows/pr.yml` በኩል ያካሂዳል
  `ci/check_sumeragi_formal.sh`.