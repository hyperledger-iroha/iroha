<!-- Auto-generated stub for Uzbek (uz) translation. Replace this content with the full translation. -->

---
lang: uz
direction: ltr
source: docs/formal/sumeragi/README.md
status: needs-review
generator: scripts/sync_docs_i18n.py
source_hash: 11eb72b5851bd4763895248c9253df49c337fb2b0921b008672e86ae77caf21a
source_last_modified: "2026-06-21T13:31:16.238431+00:00"
translation_last_reviewed: null
translator: machine-google-reviewed
---

# Sumeragi Rasmiy Model (TLA+ / Apalache)

Ushbu katalogda Sumeragi xavfsizligi va hayotiyligi uchun cheklangan rasmiy modellar mavjud.

## Qo'llash doirasi

`Sumeragi.tla` majburiyat yo'lini ushlaydi:
- faza progressiyasi (`Propose`, `Prepare`, `CommitVote`, `NewView`, `Committed`),
- ovoz berish va kvorum chegaralari (`CommitQuorum`, `ViewQuorum`),
- NPoS uslubidagi qo'riqchilar uchun og'irlikdagi ulush kvorumu (`StakeQuorum`),
- qizil qon tanachalarining sababi (`Init -> Chunk -> Ready -> Deliver`) sarlavha/digest dalillari bilan,
- GST va halol taraqqiyot harakatlariga nisbatan zaif adolatli taxminlar.

`SumeragiFrontierRecovery.tla` diqqat markazida bo'lgan Taira hang sinfini bir atrofida suratga oladi
kutilayotgan chegara bloki:
- quyida yoki kvorumda ovoz berish to'g'risidagi dalillar;
- ovoz berish navbatining kechikishi va mahalliy drenaj,
- yo'qolgan va mahalliy foydali yuk holati,
- yangi va eskirgan chegarani tiklash egaligi,
- kvorum-qayta rejalashtirish belgisi/oyna tezligi,
- mahalliy chegarani qayta tiklashi mumkin bo'lgan kelajakdagi chegara/yangi ko'rinishdagi dalillar;
- deterministik post-GST commit, retransmit, chegaralangan ko'rish-aylanish, va
  nol dalilli pasayish natijalari.

Ikkala model ham ECDSA/imzo formatlarini ataylab mavhumlashtiradi
tekshirish va toʻliq tarmoq tafsilotlari.

## fayl- `Sumeragi.tla`: protokol modeli va xususiyatlari.
- `Sumeragi_fast.cfg`: kichikroq CI-do'st parametrlar to'plami.
- `Sumeragi_deep.cfg`: kattaroq kuchlanish parametrlari to'plami.
- `SumeragiFrontierRecovery.tla`: yo'naltirilgan chegarani tiklash modeli.
- `SumeragiFrontierRecovery_fast.cfg`: kichikroq CI-do'st chegara parametrlari to'plami.
- `SumeragiFrontierRecovery_deep.cfg`: kattaroq chegara to'plami/oyna/ko'rinishga bog'langan.
- `SumeragiFrontierRecovery_wide.cfg`: qo'lda kengroq chegaralangan to'plam.
- `SumeragiFrontierRecovery_bug_stale_owner.cfg`: eskirgan egasining kutilgan muvaffaqiyatsiz mutatsiyasi.
- `SumeragiFrontierRecovery_bug_vote_queue.cfg`: kutilgan muvaffaqiyatsiz ovoz navbat mutatsiyasi.

## Xususiyatlar

Invariantlar:
- `TypeInvariant`
- `CommitImpliesQuorum`
- `CommitImpliesStakeQuorum`
- `CommitImpliesDelivered`
- `DeliverImpliesEvidence`

Vaqtinchalik mulk:
- `EventuallyCommit` (`[] (gst => <> committed)`), GSTdan keyingi adolat kodlangan
  `Next` da operativ ravishda (vaqt tugashi/nosozlikni oldini olish himoyasi yoqilgan
  harakatlarning rivojlanishi). Bu modelni Apalache 0.52.x bilan tekshirilishini ta'minlaydi
  tekshirilgan vaqtinchalik xususiyatlar ichida `WF_` adolat operatorlarini qo'llab-quvvatlamaydi.

Chegarani tiklash invariantlari:
- `TypeInvariant`
- `CommitImpliesVoteQuorum`
- `CommitImpliesPayloadAvailability`
- `VoteBackedNotDroppedAsZeroEvidenceZombie`
- `PostGstVoteBackedFrontierHasProgress`, bu terminalni istisno qiladi
  `pending /\ voteBacked /\ ~committed` qayta tiklanmagan GSTdan keyingi holat,
  commit, retransmit, aylanish yoki chegaralangan o'tish.Chegarani tiklash vaqtinchalik mulki:
- `PostGstVoteBackedFrontierEventuallyResolves`: GST dan keyin, har bir hal qilinmagan
  ovoz bilan qo'llab-quvvatlangan kutilayotgan chegara holati oxir-oqibat majburiyatga, foydali yukga etadi
  qayta tiklash, kvorumni qayta uzatish, kelajak chegarasi yoki chegaralangan ko'rinish
  aylanish.
- `RecoveredPayloadEventuallyAdvances`: ovoz bilan qo'llab-quvvatlangan chegara davlati
  tiklangan foydali yuk majburiyatsiz abadiy qolishi mumkin emas,
  qayta uzatish, reanchor yoki aylanish.
- `QuorumRetransmitEventuallyLeavesPending`: kvorum retranslyatsiyasi o'chirilgandan keyin
  ovoz bilan qo'llab-quvvatlangan chegara davlat uchun, kutilayotgan o'ram oxir-oqibat tozalanishi kerak.
- `FutureFrontierEvidenceEventuallyReanchors`: keyinroq chegara/yangi ko'rinishdagi dalillar
  kutilayotgan o'ramni tozalash kerak yoki chegara o'tkazgich sifatida iste'mol qilinishi kerak.

## Taxminlar xaritasi

Chegara modeli qasddan cheklangan. Bular amalga oshirish
u abstraktlashtiradigan yuzalar:| Model tushunchasi | Amalga oshirish yuzasi |
| --- | --- |
| `pending`, `contiguous`, `payloadState` | `PendingBlock` `crates/iroha_core/src/sumeragi/main_loop/reschedule.rs` da `PendingBlock` ishlov berish va mahalliy foydali yuklarni tekshirish, shuningdek, `proposal_handlers.rs` da BlockCreated/chegara egaligini amalga oshirish. |
| `commitVotes`, `queuedVotes` | `crates/iroha_core/src/sumeragi/main_loop/tests.rs` da `reschedule_defers_vote_backed_quorum_timeout_while_vote_queue_backlogged` va `reschedule_ignores_quorum_timeout_vote_queue_backlog` tomonidan amalga oshirilgan ovozlarni sanash va ovozlarni kiritish rejimi. |
| `recoveryOwner` | `frontier_slot_has_active_owner_state_for_view(...)` da faol/eskirgan chegara egasi holati, `maybe_yield_stale_frontier_owner_for_fresh_proposal(...)` da eskirgan egalik holati va `drop_superseded_contiguous_frontier_owner_state(...)` da tozalash o‘rnini bosadi. |
| `quorumRescheduleArmed`, `quorumWindowAge` | `reschedule_stale_pending_blocks_with_now(...)` da ovoz berish bilan tasdiqlangan kvorum tezligini o'zgartirish; regressiya qamroviga `reschedule_skips_vote_backed_retransmit_while_frontier_quorum_timeout_window_owned` kiradi. |
| `payloadRecovered` | `request_frontier_owner_body_repair(...)`, `handle_frontier_body_gap_with_topology(...)` va `stale_frontier_rbc_repair_is_actionable(...)` da aniq chegara tanasini ta'mirlash va eskirgan RBC ta'mirlashni qabul qilish. |
| `quorumRetransmitted`, `rotated` | Kvorum qayta uzatish maqsadli tanlash, `rebroadcast_pending_block_updates(...)` va `reschedule_stale_pending_blocks_with_now(...)` da deterministik ko‘rinishni o‘zgartirish chaqiruvlari. |
| `futureFrontierEvidence` | `on_pacemaker_propose_ready(...)` da `pacemaker_reanchors_frontier_when_future_new_view_quorum_exists` bilan qamrab olingan kelajakdagi yangi ko'rinish / yuqori chegaraviy kvorum dalillari. |

## Yugurish

Repozitoriy ildizidan:

```bash
bash scripts/formal/sumeragi_apalache.sh fast
bash scripts/formal/sumeragi_apalache.sh deep
bash scripts/formal/sumeragi_apalache.sh frontier-fast
bash scripts/formal/sumeragi_apalache.sh frontier-deep
bash scripts/formal/sumeragi_apalache.sh frontier-wide
```

Yuguruvchi har bir rejim uchun aniq Apalache `--length` ni o'rnatadi:| Rejim | Uzunlik | Foydalanish maqsadi |
| --- | ---: | --- |
| `fast` | 10 | CI commit-path tekshiruvi |
| `deep` | 10 | Kattaroq majburiyat yo'lini tekshirish |
| `frontier-fast` | 10 | CI chegara tekshiruvi |
| `frontier-deep` | 12 | Kattaroq chegara tekshiruvi |
| `frontier-wide` | 14 | Qo'lda/tungi chegara stressini tekshirish |

`APALACHE_LENGTH=<n>` mahalliy o'rganilayotganda har bir rejim uchun standartni bekor qiladi.
qarshi misol yoki chegaralangan isbotni kengaytirish.

### Qayta tiklanadigan mahalliy sozlash (Docker shart emas)

Ushbu ombor tomonidan ishlatiladigan mahkamlangan mahalliy Apalache asboblar zanjirini o'rnating:

```bash
bash scripts/formal/install_apalache.sh 0.52.2
```

Yuguruvchi ushbu o'rnatishni quyidagi manzilda avtomatik ravishda aniqlaydi:
`target/apalache/toolchains/v0.52.2/bin/apalache-mc`.
O'rnatishdan so'ng, `ci/check_sumeragi_formal.sh` qo'shimcha env variantlarisiz ishlashi kerak:

```bash
bash ci/check_sumeragi_formal.sh
```

Kutilgan muvaffaqiyatsiz mutatsiyalar ataylab normal CI dan tashqarida. Ular kerak
Apalache ostida muvaffaqiyatsizlikka uchraydi va modelni o'zgartirishda foydalidir:

```bash
bash ci/check_sumeragi_formal_expected_failures.sh
```

Agar Apalache `PATH` da bo'lmasa, siz:

- `APALACHE_BIN` ni bajariladigan yo'lga o'rnating yoki
- Docker zaxirasidan foydalaning (`docker` mavjud bo'lganda sukut bo'yicha yoqilgan):
  - rasm: `APALACHE_DOCKER_IMAGE` (standart `ghcr.io/apalache-mc/apalache:0.52.2`)
  - ishlaydigan Docker demonini talab qiladi
  - `APALACHE_ALLOW_DOCKER=0` bilan qayta tiklashni o'chirib qo'ying.

Misollar:

```bash
APALACHE_BIN=/opt/apalache/bin/apalache-mc bash scripts/formal/sumeragi_apalache.sh fast
APALACHE_DOCKER_IMAGE=ghcr.io/apalache-mc/apalache:0.52.2 bash scripts/formal/sumeragi_apalache.sh frontier-deep
```

## Eslatmalar- Ushbu model Rust modelining bajariladigan sinovlarini to'ldiradi (almashtirmaydi).
  `crates/iroha_core/src/sumeragi/main_loop/tests/state_machine_model_tests.rs`
  va
  `crates/iroha_core/src/sumeragi/main_loop/tests/state_machine_fairness_model_tests.rs`.
- Tekshiruvlar `.cfg` fayllaridagi doimiy qiymatlar bilan chegaralangan.
- PR CI bu tekshiruvlarni `.github/workflows/pr.yml` orqali amalga oshiradi
  `ci/check_sumeragi_formal.sh`.
