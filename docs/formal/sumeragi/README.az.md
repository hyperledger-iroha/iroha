<!-- Auto-generated stub for Azerbaijani (az) translation. Replace this content with the full translation. -->

---
lang: az
direction: ltr
source: docs/formal/sumeragi/README.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 56f1412b2db729ba69057ce15ac8bae707310fd5a6d01be2da816fdee18218f7
source_last_modified: "2026-02-23T14:48:46.580877+00:00"
translation_last_reviewed: 2026-04-02
translator: machine-google-reviewed
---

# Sumeragi Formal Model (TLA+ / Apalache)

Bu kataloq Sumeragi commit-path təhlükəsizliyi və canlılığı üçün məhdud rəsmi modeldən ibarətdir.

## Əhatə dairəsi

Model çəkir:
- faza irəliləməsi (`Propose`, `Prepare`, `CommitVote`, `NewView`, `Committed`),
- səsvermə və kvorum hədləri (`CommitQuorum`, `ViewQuorum`),
- NPoS tipli mühafizəçilər üçün ölçülmüş pay kvorumu (`StakeQuorum`),
- Başlıq/həzm sübutu ilə RBC səbəb əlaqəsi (`Init -> Chunk -> Ready -> Deliver`),
- Dürüst irəliləyiş hərəkətləri üzərində GST və zəif ədalətlilik fərziyyələri.

O, qəsdən məftil formatlarını, imzaları və tam şəbəkə təfərrüatlarını mücərrəd edir.

## Fayllar

- `Sumeragi.tla`: protokol modeli və xüsusiyyətləri.
- `Sumeragi_fast.cfg`: daha kiçik CI dostu parametrlər dəsti.
- `Sumeragi_deep.cfg`: daha böyük gərginlik parametrləri dəsti.

## Xüsusiyyətlər

İnvariantlar:
- `TypeInvariant`
- `CommitImpliesQuorum`
- `CommitImpliesStakeQuorum`
- `CommitImpliesDelivered`
- `DeliverImpliesEvidence`

Müvəqqəti mülkiyyət:
- `EventuallyCommit` (`[] (gst => <> committed)`), GST-dən sonrakı ədalətlə kodlaşdırılmış
  `Next`-də operativ olaraq (fayda aşımı/nöqsandan qorunma qoruyucuları aktivləşdirilib
  irəliləyiş tədbirləri). Bu, modeli Apalache 0.52.x ilə yoxlanıla bilir
  yoxlanılan müvəqqəti xassələrdə `WF_` ədalət operatorlarını dəstəkləmir.

## Qaçış

Repozitor kökündən:

```bash
bash scripts/formal/sumeragi_apalache.sh fast
bash scripts/formal/sumeragi_apalache.sh deep
```

### Təkrarlana bilən yerli quraşdırma (Docker tələb olunmur)Bu depo tərəfindən istifadə edilən bərkidilmiş yerli Apalache alətlər silsiləsi quraşdırın:

```bash
bash scripts/formal/install_apalache.sh 0.52.2
```

Qaçışçı bu quraşdırmanı avtomatik olaraq aşkar edir:
`target/apalache/toolchains/v0.52.2/bin/apalache-mc`.
Quraşdırıldıqdan sonra `ci/check_sumeragi_formal.sh` əlavə env varyasyonları olmadan işləməlidir:

```bash
bash ci/check_sumeragi_formal.sh
```

Apalache `PATH`-də deyilsə, aşağıdakıları edə bilərsiniz:

- icra edilə bilən yola `APALACHE_BIN` təyin edin və ya
- Docker ehtiyatdan istifadə edin (`docker` mövcud olduqda standart olaraq aktivdir):
  - şəkil: `APALACHE_DOCKER_IMAGE` (defolt `ghcr.io/apalache-mc/apalache:latest`)
  - çalışan Docker demonu tələb olunur
  - `APALACHE_ALLOW_DOCKER=0` ilə geri dönüşü söndürün.

Nümunələr:

```bash
APALACHE_BIN=/opt/apalache/bin/apalache-mc bash scripts/formal/sumeragi_apalache.sh fast
APALACHE_DOCKER_IMAGE=ghcr.io/apalache-mc/apalache:latest bash scripts/formal/sumeragi_apalache.sh deep
```

## Qeydlər

- Bu model icra edilə bilən Rust model testlərini tamamlayır (əvəz etmir).
  `crates/iroha_core/src/sumeragi/main_loop/tests/state_machine_model_tests.rs`
  və
  `crates/iroha_core/src/sumeragi/main_loop/tests/state_machine_fairness_model_tests.rs`.
- Çeklər `.cfg` fayllarında sabit qiymətlərlə məhdudlaşır.
- PR CI bu yoxlamaları `.github/workflows/pr.yml` vasitəsilə həyata keçirir
  `ci/check_sumeragi_formal.sh`.