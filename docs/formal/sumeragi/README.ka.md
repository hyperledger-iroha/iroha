<!-- Auto-generated stub for Georgian (ka) translation. Replace this content with the full translation. -->

---
lang: ka
direction: ltr
source: docs/formal/sumeragi/README.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: e89f83a4ce35b7cab8d3bfcee27eafb761f6a281c445a7cae13ae9d228760fe7
source_last_modified: "2026-04-30T20:10:10.884040+00:00"
translation_last_reviewed: 2026-05-01
translator: machine-google-reviewed
---

# Sumeragi ფორმალური მოდელი (TLA+ / Apalache)

ეს დირექტორია შეიცავს შემოსაზღვრულ ფორმალურ მოდელებს Sumeragi უსაფრთხოებისა და სიცოცხლისუნარიანობისთვის.

## სფერო

`Sumeragi.tla` აღწერს ჩადენის გზას:
- ფაზის პროგრესირება (`Propose`, `Prepare`, `CommitVote`, `NewView`, `Committed`),
- ხმის მიცემის და კვორუმის ბარიერი (`CommitQuorum`, `ViewQuorum`),
- შეწონილი ფსონის კვორუმი (`StakeQuorum`) NPoS-ის სტილის მცველებისთვის,
- RBC მიზეზობრიობა (`Init -> Chunk -> Ready -> Deliver`) სათაურით/დაჯესტის მტკიცებულებებით,
- GST და სუსტი სამართლიანობის ვარაუდები პატიოსანი პროგრესის ქმედებებზე.

`SumeragiFrontierRecovery.tla` აფიქსირებს ფოკუსირებულ Taira Hang კლასს ერთის გარშემო
მომლოდინე მომიჯნავე სასაზღვრო ბლოკი:
- ხმის მიცემის მტკიცებულება ქვემოთ ან კვორუმში,
- ხმის რიგის ჩამორჩენა და ადგილობრივი გადინება,
- დაკარგული და ადგილობრივი დატვირთვის მდგომარეობა,
- ახალი და შემორჩენილი საზღვრის აღდგენის საკუთრება,
- კვორუმი-გადაგეგმა მარკერი/ფანჯრის ტემპი,
- სამომავლო საზღვრის/ახალი ხედვის მტკიცებულება, რომელსაც შეუძლია ადგილობრივი საზღვრის გადატანა,
- დეტერმინისტული პოსტ-GST ჩადენა, ხელახალი გადაცემა, შემოსაზღვრული ხედი-როტაცია და
  ნულოვანი მტკიცებულების ვარდნის შედეგები.

ორივე მოდელი განზრახ აბსტრაქტებს მავთულის ფორმატებს, ECDSA/ხელმოწერას
გადამოწმება და სრული ქსელის დეტალები.

## ფაილები- `Sumeragi.tla`: პროტოკოლის მოდელი და თვისებები.
- `Sumeragi_fast.cfg`: უფრო მცირე CI-მეგობრული პარამეტრების ნაკრები.
- `Sumeragi_deep.cfg`: სტრესის უფრო დიდი პარამეტრის ნაკრები.
- `SumeragiFrontierRecovery.tla`: ფოკუსირებული საზღვრის აღდგენის მოდელი.
- `SumeragiFrontierRecovery_fast.cfg`: უფრო მცირე CI-მეგობრული სასაზღვრო პარამეტრების ნაკრები.
- `SumeragiFrontierRecovery_deep.cfg`: უფრო დიდი სასაზღვრო ჩამორჩენა/ფანჯრის/ხედის შეკრული ნაკრები.
- `SumeragiFrontierRecovery_wide.cfg`: მექანიკური უფრო ფართო საზღვრების კომპლექტი.
- `SumeragiFrontierRecovery_bug_stale_owner.cfg`: მოსალოდნელი წარუმატებლობის მოძველებული მფლობელის მუტაცია.
- `SumeragiFrontierRecovery_bug_vote_queue.cfg`: მოსალოდნელი წარუმატებლობა ხმის რიგის მუტაცია.

## თვისებები

უცვლელები:
- `TypeInvariant`
- `CommitImpliesQuorum`
- `CommitImpliesStakeQuorum`
- `CommitImpliesDelivered`
- `DeliverImpliesEvidence`

დროებითი საკუთრება:
- `EventuallyCommit` (`[] (gst => <> committed)`), პოსტ-GST სამართლიანობის კოდირებით
  ფუნქციონირებს `Next`-ში (ჩართულია დროის ამოწურვა/შეცდომის თავიდან აცილების დაცვა
  პროგრესის მოქმედებები). ეს ინარჩუნებს მოდელს შესამოწმებლად Apalache 0.52.x-ით, რაც
  არ აქვს `WF_` სამართლიანობის ოპერატორების მხარდაჭერა შემოწმებული დროითი თვისებების შიგნით.

საზღვრის აღდგენის უცვლელები:
- `TypeInvariant`
- `CommitImpliesVoteQuorum`
- `CommitImpliesPayloadAvailability`
- `VoteBackedNotDroppedAsZeroEvidenceZombie`
- `PostGstVoteBackedFrontierHasProgress`, რომელიც გამორიცხავს ტერმინალს
  პოსტ-GST მდგომარეობა, სადაც `pending /\ voteBacked /\ ~committed` არ არის აღდგენა,
  ჩადენა, ხელახალი გადაცემა, როტაცია ან შეზღუდული წვეთი გადასვლა.საზღვრის აღდგენის დროებითი ქონება:
- `PostGstVoteBackedFrontierEventuallyResolves`: GST-ის შემდეგ, ყველა გადაუჭრელი
  ხმებით მხარდაჭერილი მომლოდინე სასაზღვრო სახელმწიფო საბოლოოდ აღწევს ვალდებულებას, დატვირთვას
  აღდგენა, კვორუმის ხელახალი გადაცემა, მომავლის საზღვრის რეანკორი ან შემოსაზღვრული ხედი
  როტაცია.
- `RecoveredPayloadEventuallyAdvances`: ხმებით მხარდაჭერილი სასაზღვრო სახელმწიფო, რომელსაც აქვს
  ამოღებული ტვირთი ვერ დარჩება სამუდამოდ მოლოდინის რეჟიმში ვალდებულების გარეშე,
  ხელახალი გადაცემა, რეანკორირება ან როტაცია.
- `QuorumRetransmitEventuallyLeavesPending`: კვორუმის ხელახალი გადაცემის გაშვების შემდეგ
  ხმებით მხარდაჭერილი სასაზღვრო სახელმწიფოსთვის, მოსალოდნელი შეფუთვა საბოლოოდ უნდა გაიწმინდოს.
- `FutureFrontierEvidenceEventuallyReanchors`: მოგვიანებით სასაზღვრო/ახალი ხედვის მტკიცებულება
  ან უნდა გაწმინდოს მომლოდინე შეფუთვა ან მოხმარდეს როგორც სასაზღვრო სამაგრი.

## ვარაუდის რუკა

სასაზღვრო მოდელი განზრახ სასრულია. ეს არის განხორციელება
ზედაპირები მას აბსტრაქტებს:| მოდელის კონცეფცია | განხორციელების ზედაპირი |
| --- | --- |
| `pending`, `contiguous`, `payloadState` | `PendingBlock` მართვა და ადგილობრივი დატვირთვის შემოწმება `crates/iroha_core/src/sumeragi/main_loop/reschedule.rs`-ში, პლუს BlockCreated/საზღვრის მფლობელობის მატერიალიზაცია `proposal_handlers.rs`-ში. |
| `commitVotes`, `queuedVotes` | ხმების დათვლა და ხმების შემოსვლის კარიბჭე, რომელსაც ახორციელებენ `reschedule_defers_vote_backed_quorum_timeout_while_vote_queue_backlogged` და `reschedule_ignores_quorum_timeout_vote_queue_backlog` `crates/iroha_core/src/sumeragi/main_loop/tests.rs`-ში. |
| `recoveryOwner` | აქტიური/მოძველებული საზღვრის მფლობელის მდგომარეობა `frontier_slot_has_active_owner_state_for_view(...)`-ში, შემორჩენილი მფლობელის სარგებელი `maybe_yield_stale_frontier_owner_for_fresh_proposal(...)`-ში და ანაცვლებს გაწმენდას `drop_superseded_contiguous_frontier_owner_state(...)`-ში. |
| `quorumRescheduleArmed`, `quorumWindowAge` | კენჭისყრით მხარდაჭერილი კვორუმის გადასინჯვის ტემპი `reschedule_stale_pending_blocks_with_now(...)`-ში; რეგრესიის დაფარვა მოიცავს `reschedule_skips_vote_backed_retransmit_while_frontier_quorum_timeout_window_owned`. |
| `payloadRecovered` | სხეულის ზუსტი სასაზღვრო შეკეთება და შემორჩენილი RBC-ის შეკეთება `request_frontier_owner_body_repair(...)`, `handle_frontier_body_gap_with_topology(...)` და `stale_frontier_rbc_repair_is_actionable(...)`. |
| `quorumRetransmitted`, `rotated` | Quorum ხელახლა გადასცემს სამიზნე არჩევანს, `rebroadcast_pending_block_updates(...)` და დეტერმინისტული ხედვის შეცვლის ზარებს `reschedule_stale_pending_blocks_with_now(...)`-ში. |
| `futureFrontierEvidence` | მომავალი ახალი ხედვის / უმაღლესი საზღვრის კვორუმის მტკიცებულება `on_pacemaker_propose_ready(...)`-ში, დაფარული `pacemaker_reanchors_frontier_when_future_new_view_quorum_exists`-ით. |

## სირბილი

საცავიდან ფესვიდან:

```bash
bash scripts/formal/sumeragi_apalache.sh fast
bash scripts/formal/sumeragi_apalache.sh deep
bash scripts/formal/sumeragi_apalache.sh frontier-fast
bash scripts/formal/sumeragi_apalache.sh frontier-deep
bash scripts/formal/sumeragi_apalache.sh frontier-wide
```

მორბენალი ადგენს გამოკვეთილ Apalache `--length`-ს თითოეული რეჟიმისთვის:| რეჟიმი | სიგრძე | განკუთვნილი გამოყენება |
| --- | ---: | --- |
| `fast` | 10 | CI commit-path შემოწმება |
| `deep` | 10 | უფრო დიდი commit-path შემოწმება |
| `frontier-fast` | 10 | CI სასაზღვრო შემოწმება |
| `frontier-deep` | 12 | უფრო დიდი სასაზღვრო შემოწმება |
| `frontier-wide` | 14 | მექანიკური/ღამის სასაზღვრო სტრესის შემოწმება |

`APALACHE_LENGTH=<n>` უგულებელყოფს თითო რეჟიმის ნაგულისხმევს ლოკალური შესწავლისას
კონტრმაგალითი ან შეზღუდული მტკიცებულების გაფართოება.

### რეპროდუცირებადი ადგილობრივი დაყენება (არ არის საჭირო Docker)

დააინსტალირეთ დამაგრებული ადგილობრივი Apalache ინსტრუმენტების ჯაჭვი, რომელიც გამოიყენება ამ საცავში:

```bash
bash scripts/formal/install_apalache.sh 0.52.2
```

Runner ავტომატურად ამოიცნობს ამ ინსტალაციას:
`target/apalache/toolchains/v0.52.2/bin/apalache-mc`.
ინსტალაციის შემდეგ, `ci/check_sumeragi_formal.sh` უნდა მუშაობდეს დამატებითი env vars-ის გარეშე:

```bash
bash ci/check_sumeragi_formal.sh
```

მოსალოდნელი წარუმატებლობის მუტაციები განზრახ სცილდება ნორმალურ CI-ს. მათ უნდა
წარუმატებლობა Apalache-ში და გამოსადეგია მოდელის შეცვლისას:

```bash
bash ci/check_sumeragi_formal_expected_failures.sh
```

თუ Apalache არ არის `PATH`-ში, შეგიძლიათ:

- დააყენეთ `APALACHE_BIN` შესრულებად გზაზე, ან
- გამოიყენეთ Docker სარეზერვო საშუალება (ჩართულია ნაგულისხმევად, როდესაც ხელმისაწვდომია `docker`):
  - სურათი: `APALACHE_DOCKER_IMAGE` (ნაგულისხმევი `ghcr.io/apalache-mc/apalache:0.52.2`)
  - მოითხოვს გაშვებულ Docker დემონს
  - გამორთეთ სარეზერვო საშუალება `APALACHE_ALLOW_DOCKER=0`-ით.

მაგალითები:

```bash
APALACHE_BIN=/opt/apalache/bin/apalache-mc bash scripts/formal/sumeragi_apalache.sh fast
APALACHE_DOCKER_IMAGE=ghcr.io/apalache-mc/apalache:0.52.2 bash scripts/formal/sumeragi_apalache.sh frontier-deep
```

## შენიშვნები- ეს მოდელი ავსებს (არ ცვლის) შესრულებადი Rust მოდელის ტესტებს
  `crates/iroha_core/src/sumeragi/main_loop/tests/state_machine_model_tests.rs`
  და
  `crates/iroha_core/src/sumeragi/main_loop/tests/state_machine_fairness_model_tests.rs`.
- ჩეკები შემოიფარგლება მუდმივი მნიშვნელობებით `.cfg` ფაილებში.
- PR CI აწარმოებს ამ შემოწმებებს `.github/workflows/pr.yml`-ში მეშვეობით
  `ci/check_sumeragi_formal.sh`.