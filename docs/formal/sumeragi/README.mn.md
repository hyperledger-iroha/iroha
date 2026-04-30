<!-- Auto-generated stub for Mongolian (mn) translation. Replace this content with the full translation. -->

---
lang: mn
direction: ltr
source: docs/formal/sumeragi/README.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: e89f83a4ce35b7cab8d3bfcee27eafb761f6a281c445a7cae13ae9d228760fe7
source_last_modified: "2026-04-30T20:10:10.884040+00:00"
translation_last_reviewed: 2026-05-01
translator: machine-google-reviewed
---

# Sumeragi Албан ёсны загвар (TLA+ / Apalache)

Энэ лавлах нь Sumeragi аюулгүй байдал, ашиглалтын хязгаарлагдмал албан ёсны загваруудыг агуулдаг.

## Хамрах хүрээ

`Sumeragi.tla` нь амлалтын замыг агуулна:
- фазын явц (`Propose`, `Prepare`, `CommitVote`, `NewView`, `Committed`),
- саналын болон ирцийн босго (`CommitQuorum`, `ViewQuorum`),
- NPoS маягийн үүрэг хамгаалагчдын жигнэсэн гадасны чуулга (`StakeQuorum`),
- RBC-ийн учир шалтгааны хамаарал (`Init -> Chunk -> Ready -> Deliver`) толгой/тодорхой баримттай,
- Шударга ахиц дэвшлийн үйл ажиллагаан дээр ҮСТ болон сул шударга байдлын таамаглал.

`SumeragiFrontierRecovery.tla` нь нэг орчимд төвлөрсөн Taira өлгүүрийн ангийг авдаг
хүлээгдэж буй залгаа хилийн блок:
- дор эсвэл чуулгад санал өгөх нотлох баримт,
- саналын дарааллын хоцрогдол, орон нутгийн урсгал,
- орон нутгийн ачааллын төлөв дутуу,
- шинэ ба хуучирсан хилийн нөхөн сэргээлтийн эзэмшил,
- чуулгын хуваарийг өөрчлөх тэмдэг/цонхны хурд,
- орон нутгийн хилийг сэргээж чадах ирээдүйн хил/шинэ үзэл баримтлал,
- тодорхойлогч дараах GST commit, дахин дамжуулах, хязгаарлагдмал харах-эргэлт, болон
  нотлох баримтгүй үр дүн.

Хоёр загвар хоёулаа утсан формат, ECDSA/гарын үсэг зэргийг санаатайгаар хийсвэрлэдэг
баталгаажуулалт, сүлжээний дэлгэрэнгүй мэдээлэл.

## Файлууд- `Sumeragi.tla`: протоколын загвар ба шинж чанарууд.
- `Sumeragi_fast.cfg`: жижиг CI-д ээлтэй параметрийн багц.
- `Sumeragi_deep.cfg`: илүү том стресс параметрийн багц.
- `SumeragiFrontierRecovery.tla`: төвлөрсөн хилийг сэргээх загвар.
- `SumeragiFrontierRecovery_fast.cfg`: жижиг CI-д ээлтэй хилийн параметрийн багц.
- `SumeragiFrontierRecovery_deep.cfg`: илүү том хил хязгаар/цонх/харагдах хязгаарлагдмал багц.
- `SumeragiFrontierRecovery_wide.cfg`: гарын авлагын илүү өргөн хүрээтэй.
- `SumeragiFrontierRecovery_bug_stale_owner.cfg`: хүлээгдэж буй бүтэлгүйтэл хуучирсан эзэмшигчийн мутаци.
- `SumeragiFrontierRecovery_bug_vote_queue.cfg`: хүлээгдэж буй бүтэлгүйтэл саналын дарааллын мутаци.

## Properties

Инвариантууд:
- `TypeInvariant`
- `CommitImpliesQuorum`
- `CommitImpliesStakeQuorum`
- `CommitImpliesDelivered`
- `DeliverImpliesEvidence`

Түр зуурын өмч:
- `EventuallyCommit` (`[] (gst => <> committed)`), GST-ийн дараах шударга байдлыг кодчилсон
  `Next`-д ажиллах горимд (хугацаа хэтэрсэн/гажигнаас урьдчилан сэргийлэх хамгаалалт идэвхжсэн)
  үйл ажиллагааны ахиц дэвшил). Энэ нь загварыг Apalache 0.52.x-ээр шалгах боломжтой болгодог
  шалгагдсан түр зуурын шинж чанар дотор `WF_` шударга операторуудыг дэмждэггүй.

Хилийн нөхөн сэргээх инвариантууд:
- `TypeInvariant`
- `CommitImpliesVoteQuorum`
- `CommitImpliesPayloadAvailability`
- `VoteBackedNotDroppedAsZeroEvidenceZombie`
- `PostGstVoteBackedFrontierHasProgress`, энэ нь терминалыг үгүйсгэдэг
  `pending /\ voteBacked /\ ~committed` нь сэргээгдэхгүй байгаа GST-ийн дараах муж,
  commit, дахин дамжуулах, эргүүлэх, эсвэл хязгаарлагдмал дусал шилжих.Хилийн нөхөн сэргээх түр зуурын өмч:
- `PostGstVoteBackedFrontierEventuallyResolves`: GST-ийн дараа шийдэгдээгүй
  саналаар дэмжигдсэн хүлээгдэж буй хилийн муж нь эцэст нь амлалт, ачаалалд хүрдэг
  сэргээх, чуулгын дахин дамжуулалт, ирээдүйн хил хязгаар, эсвэл хязгаарлагдмал үзэл
  эргэлт.
- `RecoveredPayloadEventuallyAdvances`: саналаар дэмжигдсэн хилийн муж
  нөхөн сэргээгдсэн ачааг үүрд хүлээхгүйгээр хүлээх боломжгүй,
  дахин дамжуулах, дахин холбох, эргүүлэх.
- `QuorumRetransmitEventuallyLeavesPending`: чуулгын дахин дамжуулалт идэвхгүй болсны дараа
  саналаар дэмжигдсэн хилийн муж улсын хувьд хүлээгдэж буй боодол нь эцэстээ цэвэрлэгдэх ёстой.
- `FutureFrontierEvidenceEventuallyReanchors`: хожим хилийн/шинэ үзэл баримтлал
  хүлээгдэж буй боодолыг цэвэрлэх эсвэл хилийн зурвас болгон ашиглах ёстой.

## Таамаглалын зураг

Хилийн загвар нь санаатайгаар хязгаарлагдмал байдаг. Эдгээр нь хэрэгжилт юм
Энэ нь хийсвэрлэдэг гадаргуу:| Загварын үзэл баримтлал | Хэрэгжүүлэх гадаргуу |
| --- | --- |
| `pending`, `contiguous`, `payloadState` | `PendingBlock` `crates/iroha_core/src/sumeragi/main_loop/reschedule.rs`-д `PendingBlock` харьцах болон орон нутгийн даацыг шалгах, мөн `proposal_handlers.rs`-д BlockCreated/хилийн эзэмшлийн материаллаг байдал. |
| `commitVotes`, `queuedVotes` | `crates/iroha_core/src/sumeragi/main_loop/tests.rs`-д `reschedule_defers_vote_backed_quorum_timeout_while_vote_queue_backlogged` болон `reschedule_ignores_quorum_timeout_vote_queue_backlog`-ийн гүйцэтгэсэн саналын тоолох болон саналын нэвтрэх хаалт. |
| `recoveryOwner` | `frontier_slot_has_active_owner_state_for_view(...)`-д идэвхтэй/хуучирсан хилийн эзэмшигчийн төлөв, `maybe_yield_stale_frontier_owner_for_fresh_proposal(...)`-д хуучирсан эзэмшигчийн гарц, `drop_superseded_contiguous_frontier_owner_state(...)`-д цэвэрлэгээг орлуулна. |
| `quorumRescheduleArmed`, `quorumWindowAge` | `reschedule_stale_pending_blocks_with_now(...)` дахь саналаар дэмжигдсэн чуулгын хуваарийг өөрчлөх; регрессийн хамрах хүрээ нь `reschedule_skips_vote_backed_retransmit_while_frontier_quorum_timeout_window_owned` багтана. |
| `payloadRecovered` | `request_frontier_owner_body_repair(...)`, `handle_frontier_body_gap_with_topology(...)`, `stale_frontier_rbc_repair_is_actionable(...)` дугаарт хилийн биеийн засвар, хуучирсан улаан эсийн засварыг яг таг авна. |
| `quorumRetransmitted`, `rotated` | Чуулгын дахин дамжуулалтын зорилтот сонголт, `rebroadcast_pending_block_updates(...)` болон `reschedule_stale_pending_blocks_with_now(...)` доторх тодорхойлогч харах-өөрчлөх дуудлага. |
| `futureFrontierEvidence` | `on_pacemaker_propose_ready(...)`-д `pacemaker_reanchors_frontier_when_future_new_view_quorum_exists`-д хамрагдсан ирээдүйн шинэ / дээд хязгаарын чуулгын нотлох баримтууд. |

## Гүйж байна

Хадгалах сангийн үндэсээс:

```bash
bash scripts/formal/sumeragi_apalache.sh fast
bash scripts/formal/sumeragi_apalache.sh deep
bash scripts/formal/sumeragi_apalache.sh frontier-fast
bash scripts/formal/sumeragi_apalache.sh frontier-deep
bash scripts/formal/sumeragi_apalache.sh frontier-wide
```

Гүйгч нь горим бүрийн хувьд тодорхой Apalache `--length` тохируулдаг:| Горим | Урт | Зориулалтын хэрэглээ |
| --- | ---: | --- |
| `fast` | 10 | CI commit-path check |
| `deep` | 10 | Томоохон гүйцэтгэх замын шалгалт |
| `frontier-fast` | 10 | CI хилийн шалгалт |
| `frontier-deep` | 12 | Илүү том хилийн шалгалт |
| `frontier-wide` | 14 | Гараар/шөнийн хилийн стресс шалгах |

`APALACHE_LENGTH=<n>` нь локал мэдээллийг судлах үед горим бүрийн өгөгдмөлийг хүчингүй болгодог.
эсрэг жишээ эсвэл хязгаарлагдмал нотлох баримтыг өргөжүүлэх.

### Хуулбарлах боломжтой орон нутгийн тохиргоо (Docker шаардлагагүй)

Энэ репозиторийн ашигладаг локал Apalache хэрэгслийн гинжийг суулгана уу:

```bash
bash scripts/formal/install_apalache.sh 0.52.2
```

Гүйгч энэ суулгацыг дараах хаягаар автоматаар илрүүлдэг:
`target/apalache/toolchains/v0.52.2/bin/apalache-mc`.
Суулгасны дараа `ci/check_sumeragi_formal.sh` нэмэлт env varsгүйгээр ажиллах ёстой:

```bash
bash ci/check_sumeragi_formal.sh
```

Хүлээгдэж буй бүтэлгүйтлийн мутаци нь хэвийн CI-ээс зориудаар байна. Тэд тэгэх ёстой
Apalache-ийн дор бүтэлгүйтсэн бөгөөд загварыг өөрчлөхөд хэрэгтэй:

```bash
bash ci/check_sumeragi_formal_expected_failures.sh
```

Хэрэв Apalache `PATH`-д байхгүй бол та:

- `APALACHE_BIN`-г гүйцэтгэх замд тохируулах, эсвэл
- Docker нөөцийг ашиглах (`docker` боломжтой үед анхдагчаар идэвхждэг):
  - зураг: `APALACHE_DOCKER_IMAGE` (өгөгдмөл `ghcr.io/apalache-mc/apalache:0.52.2`)
  - ажиллаж байгаа Docker демон шаардлагатай
  - `APALACHE_ALLOW_DOCKER=0`-ийн тусламжтайгаар буцаалтыг идэвхгүй болгох.

Жишээ нь:

```bash
APALACHE_BIN=/opt/apalache/bin/apalache-mc bash scripts/formal/sumeragi_apalache.sh fast
APALACHE_DOCKER_IMAGE=ghcr.io/apalache-mc/apalache:0.52.2 bash scripts/formal/sumeragi_apalache.sh frontier-deep
```

## Тэмдэглэл- Энэ загвар нь гүйцэтгэх боломжтой Rust загварын туршилтуудыг нөхдөг (орлохгүй).
  `crates/iroha_core/src/sumeragi/main_loop/tests/state_machine_model_tests.rs`
  болон
  `crates/iroha_core/src/sumeragi/main_loop/tests/state_machine_fairness_model_tests.rs`.
- Шалгалтууд нь `.cfg` файлуудын тогтмол утгуудаар хязгаарлагддаг.
- PR CI эдгээр шалгалтыг `.github/workflows/pr.yml`-ээр дамжуулан явуулдаг
  `ci/check_sumeragi_formal.sh`.