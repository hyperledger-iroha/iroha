<!-- Auto-generated stub for Azerbaijani (az) translation. Replace this content with the full translation. -->

---
lang: az
direction: ltr
source: docs/formal/sumeragi/README.md
status: needs-review
generator: scripts/sync_docs_i18n.py
source_hash: 11eb72b5851bd4763895248c9253df49c337fb2b0921b008672e86ae77caf21a
source_last_modified: "2026-06-21T13:31:16.238431+00:00"
translation_last_reviewed: null
translator: machine-google-reviewed
---

# Sumeragi Formal Model (TLA+ / Apalache)

Bu kataloq Sumeragi təhlükəsizlik və canlılıq üçün məhdud rəsmi modelləri ehtiva edir.

## Əhatə dairəsi

`Sumeragi.tla` öhdəlik yolunu tutur:
- faza irəliləməsi (`Propose`, `Prepare`, `CommitVote`, `NewView`, `Committed`),
- səsvermə və kvorum hədləri (`CommitQuorum`, `ViewQuorum`),
- NPoS tipli mühafizəçilər üçün ölçülmüş pay kvorumu (`StakeQuorum`),
- Başlıq/həzm sübutu ilə RBC səbəb əlaqəsi (`Init -> Chunk -> Ready -> Deliver`),
- Dürüst irəliləyiş hərəkətləri üzərində GST və zəif ədalətlilik fərziyyələri.

`SumeragiFrontierRecovery.tla` bir ətrafında fokuslanmış Taira asma sinifini çəkir
gözləyən bitişik sərhəd bloku:
- aşağıda və ya yetərsayda səs vermə sübutu,
- səsvermə növbəsi və yerli boşalma,
- itkin və yerli faydalı yük vəziyyəti,
- təzə və köhnəlmiş sərhəd bərpa mülkiyyəti,
- kvorum-yenidən planlaşdırma markeri/pəncərə pacing,
- gələcək sərhəd/yerli sərhədi bərpa edə biləcək yeni baxış dəlilləri,
- deterministik post-GST commit, retransmit, məhdud baxış-fırlanma və
  sıfır sübut düşməsi nəticələri.

Hər iki model məftil formatlarını, ECDSA/imzanı qəsdən abstrakt edir
yoxlama və tam şəbəkə təfərrüatları.

## Fayllar- `Sumeragi.tla`: protokol modeli və xüsusiyyətləri.
- `Sumeragi_fast.cfg`: daha kiçik CI dostu parametrlər dəsti.
- `Sumeragi_deep.cfg`: daha böyük gərginlik parametrləri dəsti.
- `SumeragiFrontierRecovery.tla`: fokuslanmış sərhəd bərpa modeli.
- `SumeragiFrontierRecovery_fast.cfg`: daha kiçik CI-dostluq sərhəd parametrləri dəsti.
- `SumeragiFrontierRecovery_deep.cfg`: daha böyük sərhəd geriliyi/pəncərə/baxışla bağlı dəst.
- `SumeragiFrontierRecovery_wide.cfg`: əl ilə daha geniş sərhəd dəsti.
- `SumeragiFrontierRecovery_bug_stale_owner.cfg`: gözlənilən uğursuzluq köhnəlmiş sahibi mutasiyası.
- `SumeragiFrontierRecovery_bug_vote_queue.cfg`: gözlənilən uğursuzluq səs növbəsi mutasiyası.

## Xüsusiyyətlər

İnvariantlar:
- `TypeInvariant`
- `CommitImpliesQuorum`
- `CommitImpliesStakeQuorum`
- `CommitImpliesDelivered`
- `DeliverImpliesEvidence`

Müvəqqəti mülkiyyət:
- `EventuallyCommit` (`[] (gst => <> committed)`), GST-dən sonrakı ədalətlə kodlaşdırılmış
  `Next`-də operativ olaraq (vaxt aşımı/nöqsandan qorunma qoruyucuları aktivləşdirilib
  irəliləyiş tədbirləri). Bu, modeli Apalache 0.52.x ilə yoxlanıla bilir
  yoxlanılan müvəqqəti xassələrdə `WF_` ədalət operatorlarını dəstəkləmir.

Sərhəd bərpa invariantları:
- `TypeInvariant`
- `CommitImpliesVoteQuorum`
- `CommitImpliesPayloadAvailability`
- `VoteBackedNotDroppedAsZeroEvidenceZombie`
- Terminalı istisna edən `PostGstVoteBackedFrontierHasProgress`
  `pending /\ voteBacked /\ ~committed`-in bərpa olunmadığı GST-dən sonrakı vəziyyət,
  törətmək, təkrar ötürmək, fırlanma və ya məhdud-damcı keçid.Sərhəd bərpasının müvəqqəti mülkiyyəti:
- `PostGstVoteBackedFrontierEventuallyResolves`: GST-dən sonra hər həll olunmamış
  Səslə dəstəklənən gözlənilən sərhəd dövləti nəhayət öhdəlik götürür, faydalı yükə çatır
  bərpa, kvorumun təkrar ötürülməsi, gələcək-sərhəd reanchor və ya məhdud görünüş
  fırlanma.
- `RecoveredPayloadEventuallyAdvances`: səsvermə ilə dəstəklənən sərhəd dövləti
  bərpa edilmiş faydalı yük öhdəlik olmadan əbədi olaraq gözlənilə bilməz,
  retransmit, reanchor və ya fırlanma.
- `QuorumRetransmitEventuallyLeavesPending`: kvorum təkrar ötürülməsi işə salındıqdan sonra
  səsvermə ilə dəstəklənən sərhəd dövləti üçün gözlənilən sarğı sonda təmizlənməlidir.
- `FutureFrontierEvidenceEventuallyReanchors`: daha sonra sərhəd/yeni görünüş sübutu
  ya gözlənilən sarğı təmizləməli, ya da sərhəd reankeri kimi istehlak edilməlidir.

## Fərziyyə xəritəsi

Sərhəd modeli qəsdən sonludur. Bunlar icrasıdır
mücərrəd etdiyi səthlər:| Model konsepsiyası | İcra səthi |
| --- | --- |
| `pending`, `contiguous`, `payloadState` | `PendingBlock` `crates/iroha_core/src/sumeragi/main_loop/reschedule.rs`-də işləmə və yerli faydalı yükün yoxlanılması, üstəgəl `proposal_handlers.rs`-də BlockCreated/sərhəd mülkiyyətinin reallaşdırılması. |
| `commitVotes`, `queuedVotes` | `crates/iroha_core/src/sumeragi/main_loop/tests.rs`-də `reschedule_defers_vote_backed_quorum_timeout_while_vote_queue_backlogged` və `reschedule_ignores_quorum_timeout_vote_queue_backlog` tərəfindən icra edilən səslərin hesablanması və səslərin daxil olması. |
| `recoveryOwner` | `frontier_slot_has_active_owner_state_for_view(...)`-də aktiv/köhnə sərhəd sahibi vəziyyəti, `maybe_yield_stale_frontier_owner_for_fresh_proposal(...)`-də köhnə sahibinin gəliri və `drop_superseded_contiguous_frontier_owner_state(...)`-də təmizlənməni əvəz edir. |
| `quorumRescheduleArmed`, `quorumWindowAge` | `reschedule_stale_pending_blocks_with_now(...)`-də səsvermə ilə dəstəklənən kvorum sürətinin dəyişdirilməsi; reqressiya əhatə dairəsinə `reschedule_skips_vote_backed_retransmit_while_frontier_quorum_timeout_window_owned` daxildir. |
| `payloadRecovered` | `request_frontier_owner_body_repair(...)`, `handle_frontier_body_gap_with_topology(...)` və `stale_frontier_rbc_repair_is_actionable(...)`-də dəqiq sərhəd korpusunun təmiri və köhnə RBC təmiri qəbulu. |
| `quorumRetransmitted`, `rotated` | Kvorum təkrar ötürmə hədəf seçimi, `rebroadcast_pending_block_updates(...)` və `reschedule_stale_pending_blocks_with_now(...)`-də deterministik görünüş dəyişikliyi zəngləri. |
| `futureFrontierEvidence` | `on_pacemaker_propose_ready(...)`, `pacemaker_reanchors_frontier_when_future_new_view_quorum_exists` ilə əhatə olunmuş gələcək yeni baxış / daha yüksək sərhəd kvorum sübutu. |

## Qaçış

Repozitor kökündən:

```bash
bash scripts/formal/sumeragi_apalache.sh fast
bash scripts/formal/sumeragi_apalache.sh deep
bash scripts/formal/sumeragi_apalache.sh frontier-fast
bash scripts/formal/sumeragi_apalache.sh frontier-deep
bash scripts/formal/sumeragi_apalache.sh frontier-wide
```

Qaçışçı hər rejim üçün açıq Apalache `--length` təyin edir:| Rejim | Uzunluq | Təyinatlı istifadə |
| --- | ---: | --- |
| `fast` | 10 | CI commit-path yoxlaması |
| `deep` | 10 | Daha böyük icra yolu yoxlaması |
| `frontier-fast` | 10 | CI sərhəd yoxlaması |
| `frontier-deep` | 12 | Daha böyük sərhəd yoxlaması |
| `frontier-wide` | 14 | Əllə/gecə sərhəd gərginliyinin yoxlanılması |

`APALACHE_LENGTH=<n>` yerli olaraq kəşfiyyat zamanı hər rejim üçün defoltu ləğv edir.
əks nümunə və ya məhdud sübutu genişləndirmək.

### Təkrarlana bilən yerli quraşdırma (Docker tələb olunmur)

Bu depo tərəfindən istifadə edilən bərkidilmiş yerli Apalache alətlər silsiləsi quraşdırın:

```bash
bash scripts/formal/install_apalache.sh 0.52.2
```

Qaçışçı bu quraşdırmanı avtomatik olaraq aşkar edir:
`target/apalache/toolchains/v0.52.2/bin/apalache-mc`.
Quraşdırıldıqdan sonra `ci/check_sumeragi_formal.sh` əlavə env varyasyonları olmadan işləməlidir:

```bash
bash ci/check_sumeragi_formal.sh
```

Gözlənilən uğursuz mutasiyalar qəsdən normal CI-dən kənardadır. Onlar olmalıdır
Apalache altında uğursuz olur və modeli dəyişdirərkən faydalıdır:

```bash
bash ci/check_sumeragi_formal_expected_failures.sh
```

Apalache `PATH`-də deyilsə, aşağıdakıları edə bilərsiniz:

- icra edilə bilən yola `APALACHE_BIN` təyin edin və ya
- Docker ehtiyat funksiyasından istifadə edin (`docker` mövcud olduqda standart olaraq aktivdir):
  - şəkil: `APALACHE_DOCKER_IMAGE` (defolt `ghcr.io/apalache-mc/apalache:0.52.2`)
  - çalışan Docker demonu tələb olunur
  - `APALACHE_ALLOW_DOCKER=0` ilə geri dönüşü söndürün.

Nümunələr:

```bash
APALACHE_BIN=/opt/apalache/bin/apalache-mc bash scripts/formal/sumeragi_apalache.sh fast
APALACHE_DOCKER_IMAGE=ghcr.io/apalache-mc/apalache:0.52.2 bash scripts/formal/sumeragi_apalache.sh frontier-deep
```

## Qeydlər- Bu model icra edilə bilən Rust model testlərini tamamlayır (əvəz etmir).
  `crates/iroha_core/src/sumeragi/main_loop/tests/state_machine_model_tests.rs`
  və
  `crates/iroha_core/src/sumeragi/main_loop/tests/state_machine_fairness_model_tests.rs`.
- Çeklər `.cfg` fayllarında sabit dəyərlərlə məhdudlaşır.
- PR CI bu yoxlamaları `.github/workflows/pr.yml` vasitəsilə həyata keçirir
  `ci/check_sumeragi_formal.sh`.
