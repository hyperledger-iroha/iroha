<!-- Auto-generated stub for Kazakh (kk) translation. Replace this content with the full translation. -->

---
lang: kk
direction: ltr
source: docs/formal/sumeragi/README.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 56f1412b2db729ba69057ce15ac8bae707310fd5a6d01be2da816fdee18218f7
source_last_modified: "2026-02-23T14:48:46.580877+00:00"
translation_last_reviewed: 2026-04-02
translator: machine-google-reviewed
---

№ Sumeragi Ресми үлгі (TLA+ / Апалач)

Бұл каталогта Sumeragi жолдың қауіпсіздігі мен өмір сүру ұзақтығы үшін шектелген ресми үлгісі бар.

## Ауқым

Модель түсіреді:
- фазалық прогрессия (`Propose`, `Prepare`, `CommitVote`, `NewView`, `Committed`),
- дауыс беру және кворум шекті мәндері (`CommitQuorum`, `ViewQuorum`),
- NPoS стиліндегі бақылаушылар үшін үлес салмағының кворумы (`StakeQuorum`),
- Тақырып/дайджест дәлелдері бар қызыл қан клеткаларының себептілігі (`Init -> Chunk -> Ready -> Deliver`),
- GST және адал прогресс әрекеттеріне қатысты әлсіз әділеттілік болжамдары.

Ол сым пішімдерін, қолтаңбаларды және толық желі мәліметтерін әдейі алып тастайды.

## Файлдар

- `Sumeragi.tla`: протокол үлгісі және қасиеттері.
- `Sumeragi_fast.cfg`: кішірек CI қолайлы параметрлер жинағы.
- `Sumeragi_deep.cfg`: үлкенірек кернеу параметрлері жинағы.

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

## Жүгіру

Репозиторий түбірінен:

```bash
bash scripts/formal/sumeragi_apalache.sh fast
bash scripts/formal/sumeragi_apalache.sh deep
```

### Қайталанатын жергілікті орнату (Docker қажет емес)Осы репозиторий пайдаланатын бекітілген жергілікті Apalache құралдар тізбегін орнатыңыз:

```bash
bash scripts/formal/install_apalache.sh 0.52.2
```

Жүгіруші бұл орнатуды мына жерде автоматты түрде анықтайды:
`target/apalache/toolchains/v0.52.2/bin/apalache-mc`.
Орнатқаннан кейін `ci/check_sumeragi_formal.sh` қосымша параметрлерсіз жұмыс істеуі керек:

```bash
bash ci/check_sumeragi_formal.sh
```

Apalache `PATH` ішінде болмаса, сіз:

- `APALACHE_BIN` орындалатын жолға орнатыңыз немесе
- Docker резервтік нұсқасын пайдаланыңыз (`docker` қол жетімді болғанда әдепкі бойынша қосылады):
  - кескін: `APALACHE_DOCKER_IMAGE` (әдепкі `ghcr.io/apalache-mc/apalache:latest`)
  - іске қосылған Docker демонын қажет етеді
  - `APALACHE_ALLOW_DOCKER=0` көмегімен қалпына келтіруді өшіру.

Мысалдар:

```bash
APALACHE_BIN=/opt/apalache/bin/apalache-mc bash scripts/formal/sumeragi_apalache.sh fast
APALACHE_DOCKER_IMAGE=ghcr.io/apalache-mc/apalache:latest bash scripts/formal/sumeragi_apalache.sh deep
```

## Ескертулер

- Бұл модель орындалатын Rust үлгісі сынақтарын толықтырады (алмастырмайды).
  `crates/iroha_core/src/sumeragi/main_loop/tests/state_machine_model_tests.rs`
  және
  `crates/iroha_core/src/sumeragi/main_loop/tests/state_machine_fairness_model_tests.rs`.
- Тексерулер `.cfg` файлдарындағы тұрақты мәндермен шектелген.
- PR CI бұл тексерулерді `.github/workflows/pr.yml` арқылы жүзеге асырады
  `ci/check_sumeragi_formal.sh`.