<!-- Auto-generated stub for Kazakh (kk) translation. Replace this content with the full translation. -->

---
lang: kk
direction: ltr
source: docs/formal/sumeragi/README.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: e89f83a4ce35b7cab8d3bfcee27eafb761f6a281c445a7cae13ae9d228760fe7
source_last_modified: "2026-04-30T20:10:10.884040+00:00"
translation_last_reviewed: 2026-05-01
translator: machine-google-reviewed
---

№ Sumeragi Ресми үлгі (TLA+ / Апалач)

Бұл анықтамалық Sumeragi қауіпсіздік пен өмір сүруге арналған шектеулі ресми үлгілерді қамтиды.

## Ауқым

`Sumeragi.tla` орындау жолын жазады:
- фазалық прогрессия (`Propose`, `Prepare`, `CommitVote`, `NewView`, `Committed`),
- дауыс беру және кворум шекті мәндері (`CommitQuorum`, `ViewQuorum`),
- NPoS үлгісіндегі бақылаушылар үшін үлес салмағының кворумы (`StakeQuorum`),
- Тақырып/дайджест дәлелдері бар қызыл қан клеткаларының себептілігі (`Init -> Chunk -> Ready -> Deliver`),
- GST және адал прогресс әрекеттеріне қатысты әлсіз әділеттілік болжамдары.

`SumeragiFrontierRecovery.tla` фокусталған Taira ілу класын бір айналасында түсіреді
күтудегі іргелес шекаралық блок:
- төмен немесе кворумда дауыс беру туралы дәлелдер,
- дауыс беру кезегінің артта қалуы және жергілікті ағызу,
- жергілікті пайдалы жүктеме күйінің жоқтығы,
- жаңа және ескі шекараны қалпына келтіру меншік құқығы,
- кворум-қайта жоспарлау маркері/терезенің жылдамдығы,
- болашақ шекара/жергілікті шекараны бекіте алатын жаңа көрініс,
- детерминирленген пост-GST commit, retransmit, шектелген көрініс-айналдыру және
  нөлдік дәлелдемелерді жоғалту нәтижелері.

Екі модель де сым пішімдерін, ECDSA/қолтаңбаны әдейі алып тастайды
тексеру және толық желі мәліметтері.

## Файлдар- `Sumeragi.tla`: протокол үлгісі және қасиеттері.
- `Sumeragi_fast.cfg`: кішірек CI қолайлы параметрлер жинағы.
- `Sumeragi_deep.cfg`: үлкенірек кернеу параметрінің жинағы.
- `SumeragiFrontierRecovery.tla`: бағытталған шекараны қалпына келтіру моделі.
- `SumeragiFrontierRecovery_fast.cfg`: кішірек CI үшін қолайлы шекаралық параметрлер жинағы.
- `SumeragiFrontierRecovery_deep.cfg`: үлкенірек шекаралық кешігу/терезе/көрініске байланысты жиын.
- `SumeragiFrontierRecovery_wide.cfg`: қолмен кеңірек шекарамен шектелген жиын.
- `SumeragiFrontierRecovery_bug_stale_owner.cfg`: күтілетін сәтсіздік ескірген иесінің мутациясы.
- `SumeragiFrontierRecovery_bug_vote_queue.cfg`: күтілетін сәтсіздік дауысы-кезегі мутациясы.

## Қасиеттер

Инварианттар:
- `TypeInvariant`
- `CommitImpliesQuorum`
- `CommitImpliesStakeQuorum`
- `CommitImpliesDelivered`
- `DeliverImpliesEvidence`

Уақытша қасиет:
- `EventuallyCommit` (`[] (gst => <> committed)`), GST-тен кейінгі әділеттілік кодталған
  `Next` жүйесінде операциялық түрде (қосылған күйде күту уақыты/ақаулық алдын алу қорғаныстары
  прогресс әрекеттері). Бұл үлгіні Apalache 0.52.x көмегімен тексеруге мүмкіндік береді
  тексерілген уақытша сипаттардағы `WF_` әділеттілік операторларына қолдау көрсетпейді.

Шекаралық қалпына келтіру инварианттары:
- `TypeInvariant`
- `CommitImpliesVoteQuorum`
- `CommitImpliesPayloadAvailability`
- `VoteBackedNotDroppedAsZeroEvidenceZombie`
- `PostGstVoteBackedFrontierHasProgress`, бұл терминалды жоққа шығарады
  `pending /\ voteBacked /\ ~committed` қалпына келтірілмейтін GST-тен кейінгі күй,
  орындау, қайта жіберу, айналдыру немесе шектелген көшу.Шекараны қалпына келтірудің уақытша қасиеті:
- `PostGstVoteBackedFrontierEventuallyResolves`: GST кейін, әрбір шешілмеген
  Дауыс берген күтудегі шекаралық мемлекет ақыр соңында міндеттемеге, пайдалы жүктемеге жетеді
  қалпына келтіру, кворумды қайта жіберу, болашақ шекаралық реанкор немесе шектелген көрініс
  айналу.
- `RecoveredPayloadEventuallyAdvances`: дауыс берген шекаралық мемлекет
  қалпына келтірілген пайдалы жүк міндеттемесіз мәңгі күте алмайды,
  қайта жіберу, реанкер немесе айналдыру.
- `QuorumRetransmitEventuallyLeavesPending`: кворум ретрансляциясы іске қосылғаннан кейін
  дауыспен қолдау көрсетілетін шекаралық мемлекет үшін күтудегі орауыш ақырында тазартылуы керек.
- `FutureFrontierEvidenceEventuallyReanchors`: кейінірек шекара/жаңа көрініс дәлелі
  күтудегі қаптаманы тазалау керек немесе шекаралық реанкер ретінде тұтынылуы керек.

## Болжам картасы

Шекаралық модель әдейі шекті. Бұл іске асыру
ол абстракциялайды:| Үлгі тұжырымдамасы | Іске асыру беті |
| --- | --- |
| `pending`, `contiguous`, `payloadState` | `PendingBlock` өңдеу және жергілікті пайдалы жүктемені тексеру `crates/iroha_core/src/sumeragi/main_loop/reschedule.rs`, сонымен қатар `proposal_handlers.rs` ішінде BlockCreated/шекара иелігін материалдандыру. |
| `commitVotes`, `queuedVotes` | `reschedule_defers_vote_backed_quorum_timeout_while_vote_queue_backlogged` және `reschedule_ignores_quorum_timeout_vote_queue_backlog` арқылы `crates/iroha_core/src/sumeragi/main_loop/tests.rs` арқылы орындалған дауыстарды санау және дауыстарды енгізу қақпасы. |
| `recoveryOwner` | `frontier_slot_has_active_owner_state_for_view(...)` ішіндегі белсенді/ескірген шекара иесінің күйі, `maybe_yield_stale_frontier_owner_for_fresh_proposal(...)` ішіндегі ескірген иесінің кірісі және `drop_superseded_contiguous_frontier_owner_state(...)` ішіндегі тазалауды ауыстырады. |
| `quorumRescheduleArmed`, `quorumWindowAge` | `reschedule_stale_pending_blocks_with_now(...)` ішіндегі дауыспен қолдау көрсетілетін кворумды қайта жоспарлау жылдамдығы; регрессияны қамту `reschedule_skips_vote_backed_retransmit_while_frontier_quorum_timeout_window_owned` қамтиды. |
| `payloadRecovered` | `request_frontier_owner_body_repair(...)`, `handle_frontier_body_gap_with_topology(...)` және `stale_frontier_rbc_repair_is_actionable(...)` нұсқаларында шекаралық корпусты жөндеу және ескірген РБК жөндеуге рұқсат. |
| `quorumRetransmitted`, `rotated` | Кворум қайта жіберу мақсатты таңдауы, `rebroadcast_pending_block_updates(...)` және `reschedule_stale_pending_blocks_with_now(...)` ішіндегі детерминирленген көріністі өзгерту қоңыраулары. |
| `futureFrontierEvidence` | `on_pacemaker_propose_ready(...)` ішіндегі болашақ жаңа көрініс/жоғары шекаралық кворум дәлелі, `pacemaker_reanchors_frontier_when_future_new_view_quorum_exists` қамтылған. |

## Жүгіру

Репозиторий түбірінен:

```bash
bash scripts/formal/sumeragi_apalache.sh fast
bash scripts/formal/sumeragi_apalache.sh deep
bash scripts/formal/sumeragi_apalache.sh frontier-fast
bash scripts/formal/sumeragi_apalache.sh frontier-deep
bash scripts/formal/sumeragi_apalache.sh frontier-wide
```

Жүгіруші әрбір режим үшін анық Apalache `--length` орнатады:| Режим | Ұзындығы | Мақсатты пайдалану |
| --- | ---: | --- |
| `fast` | 10 | CI орындау жолын тексеру |
| `deep` | 10 | Үлкенірек тапсыру жолын тексеру |
| `frontier-fast` | 10 | CI шекаралық тексеру |
| `frontier-deep` | 12 | Үлкен шекаралық тексеру |
| `frontier-wide` | 14 | Қолмен/түнгі шекаралық кернеуді тексеру |

`APALACHE_LENGTH=<n>` файлды жергілікті түрде зерттегенде әр режимнің әдепкі мәнін қайта анықтайды.
қарсы мысал немесе шектелген дәлелдеуді кеңейту.

### Қайталанатын жергілікті орнату (Docker қажет емес)

Осы репозиторий пайдаланатын бекітілген жергілікті Apalache құралдар тізбегін орнатыңыз:

```bash
bash scripts/formal/install_apalache.sh 0.52.2
```

Жүгіруші бұл орнатуды мына жерде автоматты түрде анықтайды:
`target/apalache/toolchains/v0.52.2/bin/apalache-mc`.
Орнатқаннан кейін `ci/check_sumeragi_formal.sh` қосымша параметрлерсіз жұмыс істеуі керек:

```bash
bash ci/check_sumeragi_formal.sh
```

Күтілетін сәтсіздік мутациялары әдейі қалыпты CI шегінен тыс. Олар керек
Apalache астында сәтсіздікке ұшырайды және үлгіні өзгерту кезінде пайдалы:

```bash
bash ci/check_sumeragi_formal_expected_failures.sh
```

Apalache `PATH` ішінде болмаса, сіз:

- `APALACHE_BIN` орындалатын жолға орнатыңыз немесе
- Docker резервтік нұсқасын пайдаланыңыз (`docker` қол жетімді болғанда әдепкі бойынша қосылады):
  - кескін: `APALACHE_DOCKER_IMAGE` (әдепкі `ghcr.io/apalache-mc/apalache:0.52.2`)
  - іске қосылған Docker демоны қажет
  - `APALACHE_ALLOW_DOCKER=0` көмегімен қалпына келтіруді өшіру.

Мысалдар:

```bash
APALACHE_BIN=/opt/apalache/bin/apalache-mc bash scripts/formal/sumeragi_apalache.sh fast
APALACHE_DOCKER_IMAGE=ghcr.io/apalache-mc/apalache:0.52.2 bash scripts/formal/sumeragi_apalache.sh frontier-deep
```

## Ескертулер- Бұл модель орындалатын Rust үлгісі сынақтарын толықтырады (алмастырмайды).
  `crates/iroha_core/src/sumeragi/main_loop/tests/state_machine_model_tests.rs`
  және
  `crates/iroha_core/src/sumeragi/main_loop/tests/state_machine_fairness_model_tests.rs`.
- Тексерулер `.cfg` файлдарындағы тұрақты мәндермен шектелген.
- PR CI бұл тексерулерді `.github/workflows/pr.yml` арқылы жүзеге асырады
  `ci/check_sumeragi_formal.sh`.