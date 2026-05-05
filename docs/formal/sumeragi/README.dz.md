<!-- Auto-generated stub for Dzongkha (dz) translation. Replace this content with the full translation. -->

---
lang: dz
direction: ltr
source: docs/formal/sumeragi/README.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: e89f83a4ce35b7cab8d3bfcee27eafb761f6a281c445a7cae13ae9d228760fe7
source_last_modified: "2026-04-30T20:10:10.884040+00:00"
translation_last_reviewed: 2026-05-01
translator: machine-google-reviewed
---

# Sumeragi ལུགས་མཐུན་དཔེ་ཚད་ (ཊི་ཨེལ་ཨེ་+ / ཨ་པ་ལ་ཆི)

སྣོད་ཐོ་འདི་ནང་ Sumeragi ཉེན་སྲུང་དང་སྲོག་ལྡན་གྱི་དོན་ལུ་ ཚད་འཛིན་འབད་ཡོད་པའི་ལུགས་མཐུན་དཔེ་ཚད་ཚུ་ཡོདཔ་ཨིན།

## ཁྱབ་ཁོངས།

`Sumeragi.tla` གིས་ ཁས་བླངས་འགྲུལ་ལམ་འདི་འཛིན་བཟུང་འབདཝ་ཨིན།
- དུས་རིམ་འཕེལ་རིམ་ (`Propose`, `Prepare`, `CommitVote`, `NewView`, `Committed`),
- ཚོགས་རྒྱན་དང་ཚོགས་གྲངས་ཚད་གཞི་ (`CommitQuorum`, `ViewQuorum`),
- ལྗིད་ཚད་ཅན་གྱི་བགོ་བཤའ་ཚད་གཞི་ (`StakeQuorum`) ཨེན་པི་ཨོ་ཨེསི་བཟོ་རྣམ་གྱི་ཁས་བླངས་སྲུང་སྐྱོབ་ཚུ་གི་དོན་ལུ་,
- RBC རྒྱུ་རྐྱེན་ (`Init -> Chunk -> Ready -> Deliver`) མགོ་ཡིག་/བཞུ་བའི་སྒྲུབ་བྱེད།,
- དྲང་བདེན་གྱི་ཡར་རྒྱས་ཀྱི་བྱ་སྤྱོད་ཚུ་ལས་ ཇི་ཨེསི་ཊི་དང་ དྲང་བདེན་གྱི་ བསམ་ཚུལ་ཞན་ཁོག་ཚུ།

`SumeragiFrontierRecovery.tla` གིས་ གཅིག་གི་མཐའ་འཁོར་ལུ་ གཙོ་བོར་བཏོན་ཡོད་པའི་ ཊའི་ར་ ཧང་སྡེ་ཚན་འདི་ འཛིན་བཟུང་འབདཝ་ཨིན།
བསྒུག་སྡོད་མི་ མཐུད་མཚམས་བཀག་ཆ་:
- འོག་ལུ་ཡང་ན་ ཚོགས་རྒྱན་བཙུགས་ནིའི་སྒྲུབ་བྱེད།
- ཚོགས་རྒྱན་བང་རིམ་རྒྱབ་ལོག་དང་ས་གནས་ཀྱི་ཆུ་འགྲོ།,
- བརླག་སྟོར་ཞུགསཔ་ vs. ས་གནས་ཀྱི་ payload གནས་སྟངས།,
- གསརཔ་ vs. རྙིངམ་ས་མཚམས་སླར་གསོའི་བདག་དབང་།,
- quorum-བསྐྱར་ལས་རིམ་རྟགས་/སྒོ་སྒྲིག་གོམ་པ་,,
- མ་འོངས་པའི་ས་མཚམས་/ས་གནས་ཀྱི་ས་མཚམས་ལོག་སྟེ་གཞི་བཙུགས་འབད་ཚུགས་པའི་ མཐོང་སྣང་གསརཔ་གི་སྒྲུབ་བྱེད།
- གཏན་འབེབས་ཅན་གྱི་ ཇི་ཨེསི་ཊི་གི་ཤུལ་ལས་ ཁས་བླངས་འབད་ནི་དང་ ལོག་སྤེལ་ནི་ ཚད་འཛིན་མཐོང་སྣང་བསྒྱིར་ནི་ དེ་ལས་
  ཀླད་ཀོར་སྒྲུབ་བྱེད་བཏོན་པའི་གྲུབ་འབྲས།

དཔེ་ཚད་གཉིས་ཆ་ར་གིས་ ཤེས་བཞིན་དུ་ ཐག་རིང་རྩ་སྒྲིག་ཚུ་ བཅུད་བསྡུས་འབདཝ་ཨིན། ECDSA/མཚན་རྟགས་
བདེན་དཔྱད་དང་ ཡོངས་འབྲེལ་གྱི་ཁ་གསལ་ཆ་ཚང་།

## ཡིག་སྣོད་ཚུ།- `Sumeragi.tla`: མཐུན་སྒྲིག་དཔེ་ཚད་དང་རྒྱུ་དངོས་ཚུ།
- `Sumeragi_fast.cfg`: CI-མཐུན་འབྲེལ་ཚད་གཞི་ཆ་ཚན་ཆུང་བ།
- `Sumeragi_deep.cfg`: གནོན་ཤུགས་ཚད་བཟུང་ཆ་ཚན་སྦོམ་ཡོདཔ།
- `SumeragiFrontierRecovery.tla`: གཙོ་བོར་བཏོན་པའི་ས་མཚམས་སླར་གསོའི་དཔེ་ཚད།
- `SumeragiFrontierRecovery_fast.cfg`: CI-མཐུན་འབྲེལ་ཅན་གྱི་མཐའ་མཚམས་ཚད་གཞི་ཆ་ཚན་ཆུང་བ།
- `SumeragiFrontierRecovery_deep.cfg`: མཐའ་མཚམས་རྒྱབ་ལོག་/སྒོ་སྒྲིག་/མཐོང་སྣང་མཐའ་མཚམས་ཆ་ཚན་སྦོམ།
- `SumeragiFrontierRecovery_wide.cfg`: ལག་ཐོག་རྒྱ་ཆེ་བའི་ས་མཚམས་མཐུད་སྒྲིག་ཆ་ཚན།
- `SumeragiFrontierRecovery_bug_stale_owner.cfg`: རེ་བ་-འཐུས་ཤོར་རྙིངམ་-ཇོ་བདག་རིགས་འགྱུར།
- `SumeragiFrontierRecovery_bug_vote_queue.cfg`: རེ་བ་-འཐུས་ཤོར་ཚོགས་རྒྱན་-བང་རིམ་འགྱུར་བ།

## རྒྱུ་དངོས་ཚུ།

འགྱུར་ལྡོག་མེད་མི་ཚུ་:
- `TypeInvariant`
- `CommitImpliesQuorum`
- `CommitImpliesStakeQuorum`
- `CommitImpliesDelivered`
- `DeliverImpliesEvidence`

དུས་སྐབས་ཀྱི་རྒྱུ་དངོས།
- `EventuallyCommit` (`[] (gst => <> committed)`), ཇི་ཨེསི་ཊི་རྗེས་མའི་དྲང་བདེན་ཨེན་ཀོ་ཌི་འབད་ཡོདཔ།
  ལག་ལེན་འཐབ་ཐོག་ལས་ `Next` ནང་ལུ་ (དུས་ཚོད་རྫོགས་/འཛོལ་བ་སྔོན་འགོག་སྲུང་སྐྱོབ་ཚུ་ ལྕོགས་ཅན་བཟོ་ཡོདཔ།
  ཡར་རྒྱས་ཀྱི་བྱ་བ་ཚུ་)། འདི་གིས་ དཔེ་ཚད་འདི་ ཨ་པ་ལ་ཆི་ ༠.༥༢.ཨེགསི་དང་གཅིག་ཁར་ བརྟག་ཞིབ་འབད་ཚུགསཔ་སྦེ་བཞགཔ་ཨིན།
  བརྟག་ཞིབ་འབད་ཡོད་པའི་དུས་སྐབས་རྒྱུ་དངོས་ཚུ་གི་ནང་འཁོད་ལུ་ `WF_` དྲང་བདེན་བཀོལ་སྤྱོད་པ་ཚུ་ལུ་རྒྱབ་སྐྱོར་མི་འབད།

མཐའ་མཚམས་སླར་གསོའི་འགྱུར་ལྡོག་མེད་པ།
- `TypeInvariant`
- `CommitImpliesVoteQuorum`
- `CommitImpliesPayloadAvailability`
- `VoteBackedNotDroppedAsZeroEvidenceZombie`
- `PostGstVoteBackedFrontierHasProgress`, དེ་གིས་ ཊར་མི་ནཱལ་ཅིག་ བཀག་ཆ་འབདཝ་ཨིན།
  ཇི་ཨེསི་ཊི་རྗེས་ཀྱི་མངའ་སྡེ་ `pending /\ voteBacked /\ ~committed` ལུ་སླར་གསོ་མེདཔ་ཨིན།
  commit, retransmit, བསྒྱིར་ནི་ ཡང་ན་ ཚད་འཛིན་-བཀོག་བཞག་འཕོ་སོར་།མཐའ་མཚམས་སླར་གསོ་གནས་སྐབས་ཀྱི་རྒྱུ་དངོས།
- `PostGstVoteBackedFrontierEventuallyResolves`: ཇི་ཨེསི་ཊི་གི་ཤུལ་ལས་ ཐག་མ་བཅད་མི་ག་ར་
  ཚོགས་རྒྱན་རྒྱབ་སྐྱོར་འབད་མི་ བསྒུག་སྡོད་མི་ ས་མཚམས་མངའ་སྡེ་འདི་ མཐའ་མཇུག་ཁར་ ཁས་བླངས་ལུ་ལྷོདཔ་ཨིན།
  སླར་གསོ། ཚད་གཞི་བསྐྱར་གཏོང་། མ་འོངས་པའི་ས་མཚམས་བསྐྱར་སྒྲིག ཡང་ན་ ཚད་འཛིན་མཐོང་སྣང་།
  བསྒྱིར།
- `RecoveredPayloadEventuallyAdvances`: འོས་ཤོག་རྒྱབ་སྐྱོར་ཡོད་པའི་ས་མཚམས་མངའ་སྡེ།
  ཁས་བླངས་མ་འབད་བར་ པེ་ལོཌ་འདི་ རྟག་བུ་རང་ བསྒུག་སྡོད་མི་ཚུགས།
  retransmit, བསྐྱར་སྒྲིག་ ཡང་ན་ བསྒྱིར།
- `QuorumRetransmitEventuallyLeavesPending`: ཚར་གཅིག་ ཚད་གཞི་བསྐྱར་སྤེལ་འབད་ཚར་བའི་ཤུལ་ལས་
  ཚོགས་རྒྱན་རྒྱབ་སྐྱོར་འབད་མི་ ས་མཚམས་མངའ་སྡེ་གི་དོན་ལུ་ བསྒུག་སྡོད་མི་ བཀབ་ཆ་འདི་ མཐའ་མཇུག་ལུ་ བསལ་དགོཔ་ཨིན།
- `FutureFrontierEvidenceEventuallyReanchors`: ཤུལ་ལས་ས་མཚམས་/མཐོང་སྣང་གསརཔ་གི་སྒྲུབ་བྱེད།
  ཡང་ན་ བསྒུག་སྡོད་མི་ བཤུད་སྒྲིལ་འདི་ བཏོན་གཏང་དགོཔ་ཨིན་ ཡང་ན་ མཐའ་མཚམས་ལོག་བཙུགས་མི་སྦེ་ བཀོལ་སྤྱོད་འབད་དགོ།

## ཚོད་དཔག་ས་ཁྲ།

ས་མཚམས་དཔེ་ཚད་འདི་ ཤེས་བཞིན་དུ་ ཚད་ལྡན་ཅིག་ཨིན། འདི་དག་ནི་ལག་བསྟར་བྱེད་དོ།
ཁ་ཐོག་ཚུ་ བཅུད་དོན་ཚུ་ཨིན།| དཔེ་སྟོན་བསམ་གཞི། | ལག་བསྟར་གྱི་ཕྱི་ངོས། |
| --- | --- |
| `pending`, `contiguous`, `payloadState` | `PendingBlock` འཛིན་སྐྱོང་དང་ `crates/iroha_core/src/sumeragi/main_loop/reschedule.rs` ནང་ལུ་ `PendingBlock` འཛིན་སྐྱོང་དང་ ས་གནས་ཀྱི་པེ་ལོཌི་ཞིབ་དཔྱད་ཚུ་ དེ་ལས་ `proposal_handlers.rs` ནང་ལུ་ BlockCreated/frontier བདག་དབང་དངོས་པོ་བཟོ་ནི། |
| `commitVotes`, `queuedVotes` | ཁས་བླངས་-ཚོགས་རྒྱན་རྩིས་རྐྱབ་ནི་དང་ ཚོགས་རྒྱན་འཛུལ་ཞུགས་སྒོ་སྒྲིག་འདི་ `reschedule_defers_vote_backed_quorum_timeout_while_vote_queue_backlogged` དང་ `reschedule_ignores_quorum_timeout_vote_queue_backlog` གིས་ `crates/iroha_core/src/sumeragi/main_loop/tests.rs` ནང་ལུ་ ལག་ལེན་འཐབ་ཡོདཔ་ཨིན། |
| `recoveryOwner` | `frontier_slot_has_active_owner_state_for_view(...)` ནང་ལུ་ ཤུགས་ལྡན་/རྙིངམ་གི་ས་མཚམས་ཇོ་བདག་གི་གནས་སྟངས་དང་ `maybe_yield_stale_frontier_owner_for_fresh_proposal(...)` ནང་ལུ་ རྙིངམ་གི་ཇོ་བདག་གི་ཐོན་ཤུགས་ དེ་ལས་ `drop_superseded_contiguous_frontier_owner_state(...)` ནང་ལུ་ གཙང་སྦྲ་འབད་ནི་འདི་ ཚབ་བཙུགས། |
| `quorumRescheduleArmed`, `quorumWindowAge` | `reschedule_stale_pending_blocks_with_now(...)` ནང་ ཚོགས་རྒྱན་རྒྱབ་སྐྱོར་འབད་མི་ ཚོགས་རྒྱན་བསྐྱར་སྒྲིག་འབད་ནི། འགྱུར་ལྡོག་ཁྱབ་ཚད་ནང་ `reschedule_skips_vote_backed_retransmit_while_frontier_quorum_timeout_window_owned` ཚུདཔ་ཨིན། |
| `payloadRecovered` | མཐའ་མཚམས་གཟུགས་པོའི་ཉམས་བཅོས་དང་ རྙིང་པའི་ RBC ཉམས་བཅོས་ཀྱི་འཛུལ་ཞུགས་ `request_frontier_owner_body_repair(...)`, `handle_frontier_body_gap_with_topology(...)`, དང་ `stale_frontier_rbc_repair_is_actionable(...)` ནང་། |
| `quorumRetransmitted`, `rotated` | ཚོགས་རྒྱན་བསྐྱར་གཏང་དམིགས་གཏད་སེལ་འཐུ་ `rebroadcast_pending_block_updates(...)` དང་ `reschedule_stale_pending_blocks_with_now(...)` ནང་ལུ་ གཏན་འབེབས་མཐོང་སྣང་བསྒྱུར་བཅོས་འབོད་བརྡ་ཚུ། |
| `futureFrontierEvidence` | མ་འོངས་པའི་མཐོང་སྣང་གསརཔ་ / མཐོ་བའི་མཐའ་མཚམས་ཀྱི་ ཚད་གཞི་གི་སྒྲུབ་བྱེད་ `on_pacemaker_propose_ready(...)` ནང་ `pacemaker_reanchors_frontier_when_future_new_view_quorum_exists` གིས་ཁྱབ་ཡོདཔ་ཨིན། |

## རྒྱུག་དོ།

མཛོད་ཁང་གི་རྩ་བ་ལས་:

```bash
bash scripts/formal/sumeragi_apalache.sh fast
bash scripts/formal/sumeragi_apalache.sh deep
bash scripts/formal/sumeragi_apalache.sh frontier-fast
bash scripts/formal/sumeragi_apalache.sh frontier-deep
bash scripts/formal/sumeragi_apalache.sh frontier-wide
```

རྒྱུག་མི་འདི་གིས་ ཐབས་ལམ་རེ་རེ་གི་དོན་ལུ་ གསལ་ཏོག་ཏོ་ཨ་པ་ལ་ཆི་ `--length` གཞི་སྒྲིག་འབདཝ་ཨིན།| ཐབས་ལམ་ | རིང་ཚད། | དམིགས་གཏད་ལག་ལེན་འཐབ་མི། |
| --- | ---: | --- |
| `fast` | ༡༠ | CI ཁས་བླངས་-འགྲུལ་ལམ་ཞིབ་དཔྱད་ |
| `deep` | ༡༠ | ཁས་ལེན་འགྲུལ་ལམ་སྦོམ་ཞིབ་དཔྱད་ |
| `frontier-fast` | ༡༠ | CI མཐའ་མཚམས་ཞིབ་དཔྱད། |
| `frontier-deep` | ༡༢ | མཐའ་མཚམས་ཞིབ་དཔྱད་སྦོམ་ |
| `frontier-wide` | ༡༤ | ལག་ཐོག་/མཚན་མོར་ས་མཚམས་གནོན་ཤུགས་བརྟག་དཔྱད། |

`APALACHE_LENGTH=<n>` གིས་ ཉེ་གནས་ལུ་འཚོལ་ཞིབ་འབད་བའི་སྐབས་ ཐབས་ལམ་རེ་རེ་གི་སྔོན་སྒྲིག་འདི་བཀག་ཆ་འབདཝ་ཨིན།
counterexample ཡང་ན་ ཚད་འཛིན་ཅན་གྱི་བདེན་ཁུངས་རྒྱ་སྐྱེད་གཏང་ནི།

### བསྐྱར་བཟོ་འབད་བཏུབ་པའི་ཉེ་གནས་གཞི་སྒྲིག་ (Docker དགོས་མཁོ་མེདཔ་)

མཛོད་ཁང་འདི་གིས་ལག་ལེན་འཐབ་མི་ པིན་འབད་ཡོད་པའི་ཉེ་གནས་ཨ་པ་ལ་ཆི་ལག་ཆས་རྒྱུན་རིམ་འདི་གཞི་བཙུགས་འབད།

```bash
bash scripts/formal/install_apalache.sh 0.52.2
```

རྒྱུག་མི་གིས་ རང་བཞིན་གྱིས་ གཞི་བཙུགས་འདི་ ལུ་ ཤེས་རྟོགས་འབདཝ་ཨིན།
`target/apalache/toolchains/v0.52.2/bin/apalache-mc`.
གཞི་བཙུགས་འབད་བའི་ཤུལ་ལས་ `ci/check_sumeragi_formal.sh` འདི་ env vars ཁ་སྐོང་མེད་པར་ལཱ་འབད་དགོ།

```bash
bash ci/check_sumeragi_formal.sh
```

རེ་བ་བསྐྱེད་དེ་ཡོད་མི་འགྱུར་བཅོས་འདི་ སྤྱིར་བཏང་CIགི་ཕྱི་ཁར་ཨིན་མས། ཁོང་ཚོས་
ཨ་པ་ལ་ཆི་གི་འོག་ལུ་འཐུས་ཤོར་བྱུང་ཡོདཔ་དང་ དཔེ་ཚད་བསྒྱུར་བཅོས་འབད་བའི་སྐབས་ཕན་ཐོགས་ཡོདཔ་ཨིན།

```bash
bash ci/check_sumeragi_formal_expected_failures.sh
```

གལ་སྲིད་ཨ་པ་ལ་ཆི་ `PATH` ནང་མེད་པ་ཅིན་ ཁྱོད་ཀྱིས་འབད་ཚུགས།

- བཀོལ་སྤྱོད་འབད་བཏུབ་པའི་འགྲུལ་ལམ་ལུ་ `APALACHE_BIN` གཞི་སྒྲིག་འབད་ ཡང་ན་
- Docker ཕོལ་བེཀ་འདི་ལག་ལེན་འཐབ།
  - པར་རིས་: `APALACHE_DOCKER_IMAGE` (སྔོན་སྒྲིག་`ghcr.io/apalache-mc/apalache:0.52.2`)
  - གཡོག་བཀོལ་བའི་ Docker ཌེ་མཱོན་ཅིག་དགོཔ་ཨིན།
  - `APALACHE_ALLOW_DOCKER=0` དང་ཅིག་ཁར་ ཕོལབེཀ་ལྕོགས་མིན་བཟོ།

དཔེར་ན།

```bash
APALACHE_BIN=/opt/apalache/bin/apalache-mc bash scripts/formal/sumeragi_apalache.sh fast
APALACHE_DOCKER_IMAGE=ghcr.io/apalache-mc/apalache:0.52.2 bash scripts/formal/sumeragi_apalache.sh frontier-deep
```

## མཆན་འགྲེལ།- དཔེ་ཚད་འདི་གིས་ 2019 ནང་ ལག་ལེན་འཐབ་བཏུབ་པའི་ རསཊི་དཔེ་ཚད་བརྟག་དཔྱད་ཚུ་ མཐུན་སྒྲིག་འབདཝ་ཨིན།
  `crates/iroha_core/src/sumeragi/main_loop/tests/state_machine_model_tests.rs`
  དང་ །
  `crates/iroha_core/src/sumeragi/main_loop/tests/state_machine_fairness_model_tests.rs`.
- ཞིབ་དཔྱད་ཚུ་ `.cfg` ཡིག་སྣོད་ཚུ་ནང་ དུས་རྒྱུན་གནས་གོང་ཚུ་གིས་ ཚད་འཛིན་འབད་ཡོདཔ་ཨིན།
- PR CI གིས་ འ་ནི་ཞིབ་དཔྱད་ཚུ་ `.github/workflows/pr.yml` ནང་ལུ་བརྒྱུད་དེ་ གཡོག་བཀོལཝ་ཨིན།
  `ci/check_sumeragi_formal.sh`.