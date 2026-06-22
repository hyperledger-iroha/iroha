<!-- Auto-generated stub for Armenian (hy) translation. Replace this content with the full translation. -->

---
lang: hy
direction: ltr
source: docs/formal/sumeragi/README.md
status: needs-review
generator: scripts/sync_docs_i18n.py
source_hash: 11eb72b5851bd4763895248c9253df49c337fb2b0921b008672e86ae77caf21a
source_last_modified: "2026-06-21T13:31:16.238431+00:00"
translation_last_reviewed: null
translator: machine-google-reviewed
---

# Sumeragi պաշտոնական մոդել (TLA+ / Apalache)

Այս գրացուցակը պարունակում է սահմանափակված պաշտոնական մոդելներ Sumeragi անվտանգության և կենսունակության համար:

## Շրջանակ

`Sumeragi.tla`-ը գրավում է կատարման ուղին՝
- փուլային առաջընթաց (`Propose`, `Prepare`, `CommitVote`, `NewView`, `Committed`),
- ձայնի և քվորումի շեմերը (`CommitQuorum`, `ViewQuorum`),
- կշռված ցցի քվորում (`StakeQuorum`) NPoS ոճի պարտավորությունների պահակների համար,
- RBC-ի պատճառահետևանքային կապ (`Init -> Chunk -> Ready -> Deliver`) վերնագրով/դիզեստի վկայությամբ,
- GST և թույլ արդարության ենթադրություններ ազնիվ առաջընթացի գործողությունների նկատմամբ:

`SumeragiFrontierRecovery.tla`-ը լուսանկարում է կենտրոնացված Taira hang դասը մեկի շուրջը
սպասվող հարակից սահմանային բլոկ.
- քվեարկել ապացույցներ ստորև կամ քվորումում,
- քվեարկության հերթերի կուտակում և տեղական արտահոսք,
- բացակայում է ընդդեմ տեղական ծանրաբեռնվածության վիճակի,
- թարմ ընդդեմ հնացած սահմանի վերականգնման սեփականություն,
- քվորում-վերակազմակերպել մարկեր/պատուհանի տեմպը,
- ապագա սահմանների/նոր հայացքների ապացույցներ, որոնք կարող են կրկին ամրացնել տեղական սահմանը,
- որոշիչ post-GST commit, retransmit, bounded view-rotation, and
  զրոյական ապացույցների անկման արդյունքները:

Երկու մոդելներն էլ միտումնավոր աբստրակտ հեռացնում են մետաղալարերի ձևաչափերը, ECDSA/ստորագրությունը
ստուգում և ամբողջական ցանցային մանրամասներ:

## Ֆայլեր- `Sumeragi.tla`. արձանագրության մոդել և հատկություններ:
- `Sumeragi_fast.cfg`. ավելի փոքր CI-նպաստ պարամետրերի հավաքածու:
- `Sumeragi_deep.cfg`. ավելի մեծ սթրեսի պարամետրերի հավաքածու:
- `SumeragiFrontierRecovery.tla`՝ կենտրոնացված սահմանի վերականգնման մոդել:
- `SumeragiFrontierRecovery_fast.cfg`. ավելի փոքր CI-նպաստավոր սահմանային պարամետրերի հավաքածու:
- `SumeragiFrontierRecovery_deep.cfg`. ավելի մեծ սահմանային հետնամաս/պատուհան/դիտել կապված հավաքածու:
- `SumeragiFrontierRecovery_wide.cfg`. ձեռքով ավելի լայն սահմանային հավաքածու:
- `SumeragiFrontierRecovery_bug_stale_owner.cfg`. ակնկալվող ձախողում հնացած սեփականատիրոջ մուտացիա:
- `SumeragiFrontierRecovery_bug_vote_queue.cfg`. ակնկալվող ձախողում ձայների հերթի մուտացիա:

## Հատկություններ

Անփոփոխներ:
- `TypeInvariant`
- `CommitImpliesQuorum`
- `CommitImpliesStakeQuorum`
- `CommitImpliesDelivered`
- `DeliverImpliesEvidence`

Ժամանակավոր հատկություն.
- `EventuallyCommit` (`[] (gst => <> committed)`), GST-ից հետո կոդավորված արդարությամբ
  գործառնական `Next`-ում (ժամկետի/անսարքության կանխարգելման պահակները միացված են
  առաջընթացի գործողություններ): Սա թույլ է տալիս ստուգել մոդելը Apalache 0.52.x-ով, որը
  չի աջակցում `WF_` արդարության օպերատորներին ստուգված ժամանակային հատկությունների ներսում:

Սահմանի վերականգնման անփոփոխներ.
- `TypeInvariant`
- `CommitImpliesVoteQuorum`
- `CommitImpliesPayloadAvailability`
- `VoteBackedNotDroppedAsZeroEvidenceZombie`
- `PostGstVoteBackedFrontierHasProgress`, որը բացառում է տերմինալը
  հետ-GST վիճակ, որտեղ `pending /\ voteBacked /\ ~committed`-ը վերականգնում չունի,
  commit, retransmit, rotation կամ bounded-drop անցում:Սահմանի վերականգնման ժամանակավոր հատկություն.
- `PostGstVoteBackedFrontierEventuallyResolves`. GST-ից հետո՝ ամեն չլուծված
  քվեարկությամբ ապահովված առկախ սահմանամերձ պետությունը, ի վերջո, հասնում է պարտավորություններին, ծանրաբեռնվածությանը
  վերականգնում, քվորումի վերահեռարձակում, ապագա-սահմանային վերահաստատում կամ սահմանափակված տեսք
  ռոտացիան.
- `RecoveredPayloadEventuallyAdvances`. ձայների աջակցությամբ սահմանամերձ պետություն, որն ունի
  վերականգնված բեռը չի կարող ընդմիշտ առկախ մնալ առանց պարտավորությունների,
  վերահաղորդում, վերահաստատում կամ պտտում:
- `QuorumRetransmitEventuallyLeavesPending`. քվորումի վերահեռարձակումն սկսելուց հետո
  քվեներով ապահովված սահմանամերձ պետության համար սպասող փաթաթանն ի վերջո պետք է մաքրվի:
- `FutureFrontierEvidenceEventuallyReanchors`. ավելի ուշ սահմանային/նոր տեսքի ապացույց
  պետք է կա՛մ մաքրի սպասող փաթաթան, կա՛մ օգտագործվի որպես սահմանային ամրակ:

## Ենթադրությունների քարտեզ

Սահմանային մոդելը միտումնավոր վերջավոր է: Սրանք իրականացումն են
մակերեսները, որոնք նա վերացում է.| Մոդելի հայեցակարգ | Իրականացման մակերեսը |
| --- | --- |
| `pending`, `contiguous`, `payloadState` | `PendingBlock` բեռնաթափման և տեղական օգտակար բեռների ստուգումներ `crates/iroha_core/src/sumeragi/main_loop/reschedule.rs`-ում, գումարած BlockCreated/սահմանային սեփականության նյութականացում `proposal_handlers.rs`-ում: |
| `commitVotes`, `queuedVotes` | Ձայների հաշվառում և ձայների ներթափանցման անցում, որն իրականացվում է `reschedule_defers_vote_backed_quorum_timeout_while_vote_queue_backlogged`-ի և `reschedule_ignores_quorum_timeout_vote_queue_backlog`-ի կողմից `crates/iroha_core/src/sumeragi/main_loop/tests.rs`-ում: |
| `recoveryOwner` | Ակտիվ/հնացած սահմանային սեփականատիրոջ վիճակը `frontier_slot_has_active_owner_state_for_view(...)`-ում, հնացած սեփականատիրոջ եկամտաբերությունը `maybe_yield_stale_frontier_owner_for_fresh_proposal(...)`-ում և փոխարինել մաքրումը `drop_superseded_contiguous_frontier_owner_state(...)`-ում: |
| `quorumRescheduleArmed`, `quorumWindowAge` | `reschedule_stale_pending_blocks_with_now(...)`-ում քվորումի վերափոխման տեմպը ձայնով ապահովված է; ռեգրեսիայի ծածկույթը ներառում է `reschedule_skips_vote_backed_retransmit_while_frontier_quorum_timeout_window_owned`: |
| `payloadRecovered` | Սահմանային մարմնի ճշգրիտ վերանորոգում և հնացած RBC-ի վերանորոգման ընդունում `request_frontier_owner_body_repair(...)`, `handle_frontier_body_gap_with_topology(...)` և `stale_frontier_rbc_repair_is_actionable(...)`-ում: |
| `quorumRetransmitted`, `rotated` | Քվորումը վերահեռարձակում է թիրախային ընտրությունը, `rebroadcast_pending_block_updates(...)` և `reschedule_stale_pending_blocks_with_now(...)`-ում դիտման փոփոխության որոշիչ զանգեր: |
| `futureFrontierEvidence` | Ապագա նոր դիտման / ավելի բարձր սահմանի քվորումի ապացույց `on_pacemaker_propose_ready(...)`-ում, որը ծածկված է `pacemaker_reanchors_frontier_when_future_new_view_quorum_exists`-ով: |

## Վազում

Պահեստի արմատից.

```bash
bash scripts/formal/sumeragi_apalache.sh fast
bash scripts/formal/sumeragi_apalache.sh deep
bash scripts/formal/sumeragi_apalache.sh frontier-fast
bash scripts/formal/sumeragi_apalache.sh frontier-deep
bash scripts/formal/sumeragi_apalache.sh frontier-wide
```

Վազողը յուրաքանչյուր ռեժիմի համար սահմանում է բացահայտ Apalache `--length`.| Ռեժիմ | Երկարությունը | Նախատեսված օգտագործման |
| --- | ---: | --- |
| `fast` | 10 | CI commit-path check |
| `deep` | 10 | Ավելի մեծ պարտավորությունների ուղու ստուգում |
| `frontier-fast` | 10 | CI սահմանային ստուգում |
| `frontier-deep` | 12 | Ավելի մեծ սահմանային ստուգում |
| `frontier-wide` | 14 | Ձեռնարկ/գիշերային սահմանային լարվածության ստուգում |

`APALACHE_LENGTH=<n>`-ը անտեսում է մեկ ռեժիմի լռելյայնությունը, երբ տեղական ուսումնասիրություն է կատարվում
հակաօրինակ կամ սահմանափակված ապացույցի ընդլայնում:

### Վերարտադրվող տեղային կարգավորում (Docker չի պահանջվում)

Տեղադրեք ամրացված տեղական Apalache գործիքների շղթան, որն օգտագործվում է այս պահոցի կողմից.

```bash
bash scripts/formal/install_apalache.sh 0.52.2
```

The runner-ը ավտոմատ կերպով հայտնաբերում է այս տեղադրումը հետևյալ հասցեով.
`target/apalache/toolchains/v0.52.2/bin/apalache-mc`.
Տեղադրվելուց հետո `ci/check_sumeragi_formal.sh`-ը պետք է աշխատի առանց լրացուցիչ env vars-ի.

```bash
bash ci/check_sumeragi_formal.sh
```

Սպասվող ձախողման մուտացիաները դիտավորյալ գտնվում են նորմալ CI-ից դուրս: Նրանք պետք է
ձախողվում են Apalache-ի տակ և օգտակար են մոդելը փոխելու ժամանակ.

```bash
bash ci/check_sumeragi_formal_expected_failures.sh
```

Եթե Apalache-ը `PATH`-ում չէ, կարող եք.

- սահմանել `APALACHE_BIN` գործարկվող ուղու վրա, կամ
- օգտագործեք Docker հետադարձ կապը (միացված է լռելյայն, երբ `docker` հասանելի է):
  - պատկեր՝ `APALACHE_DOCKER_IMAGE` (կանխադրված `ghcr.io/apalache-mc/apalache:0.52.2`)
  - պահանջում է գործող Docker դեյմոն
  - անջատել հետադարձ կապը `APALACHE_ALLOW_DOCKER=0`-ով:

Օրինակներ.

```bash
APALACHE_BIN=/opt/apalache/bin/apalache-mc bash scripts/formal/sumeragi_apalache.sh fast
APALACHE_DOCKER_IMAGE=ghcr.io/apalache-mc/apalache:0.52.2 bash scripts/formal/sumeragi_apalache.sh frontier-deep
```

## Նշումներ- Այս մոդելը լրացնում է (չի փոխարինում) գործարկվող Rust մոդելի թեստերը
  `crates/iroha_core/src/sumeragi/main_loop/tests/state_machine_model_tests.rs`
  և
  `crates/iroha_core/src/sumeragi/main_loop/tests/state_machine_fairness_model_tests.rs`.
- Չեկերը սահմանափակված են հաստատուն արժեքներով `.cfg` ֆայլերում:
- PR CI-ն այս ստուգումները կատարում է `.github/workflows/pr.yml` միջոցով
  `ci/check_sumeragi_formal.sh`.
