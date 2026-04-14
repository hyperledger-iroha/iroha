<!-- Auto-generated stub for Uzbek (uz) translation. Replace this content with the full translation. -->

---
lang: uz
direction: ltr
source: docs/formal/sumeragi/README.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 56f1412b2db729ba69057ce15ac8bae707310fd5a6d01be2da816fdee18218f7
source_last_modified: "2026-02-23T14:48:46.580877+00:00"
translation_last_reviewed: 2026-04-02
translator: machine-google-reviewed
---

# Sumeragi Rasmiy Model (TLA+ / Apalache)

Ushbu katalogda Sumeragi commit-yo'l xavfsizligi va hayotiyligi uchun cheklangan rasmiy model mavjud.

## Qo'llash doirasi

Model quyidagilarni qamrab oladi:
- faza progressiyasi (`Propose`, `Prepare`, `CommitVote`, `NewView`, `Committed`),
- ovoz berish va kvorum chegaralari (`CommitQuorum`, `ViewQuorum`),
- NPoS uslubidagi qo'riqchilar uchun og'irlikdagi ulush kvorumu (`StakeQuorum`),
- qizil qon tanachalarining sababi (`Init -> Chunk -> Ready -> Deliver`), sarlavha/digest dalillari bilan,
- GST va halol taraqqiyot harakatlariga nisbatan zaif adolatli taxminlar.

U tel formatlarini, imzolarni va tarmoq tafsilotlarini ataylab olib tashlaydi.

## fayl

- `Sumeragi.tla`: protokol modeli va xususiyatlari.
- `Sumeragi_fast.cfg`: kichikroq CI-do'st parametrlar to'plami.
- `Sumeragi_deep.cfg`: kattaroq kuchlanish parametrlari to'plami.

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

## Yugurish

Repozitoriy ildizidan:

```bash
bash scripts/formal/sumeragi_apalache.sh fast
bash scripts/formal/sumeragi_apalache.sh deep
```

### Qayta tiklanadigan mahalliy sozlash (Docker shart emas)Ushbu ombor tomonidan ishlatiladigan mahkamlangan mahalliy Apalache asboblar zanjirini o'rnating:

```bash
bash scripts/formal/install_apalache.sh 0.52.2
```

Yuguruvchi ushbu o'rnatishni quyidagi manzilda avtomatik ravishda aniqlaydi:
`target/apalache/toolchains/v0.52.2/bin/apalache-mc`.
O'rnatishdan so'ng `ci/check_sumeragi_formal.sh` qo'shimcha env variantlarisiz ishlashi kerak:

```bash
bash ci/check_sumeragi_formal.sh
```

Agar Apalache `PATH` da bo'lmasa, siz:

- `APALACHE_BIN` ni bajariladigan yo'lga o'rnating yoki
- Docker zaxirasidan foydalaning (`docker` mavjud bo'lganda sukut bo'yicha yoqilgan):
  - rasm: `APALACHE_DOCKER_IMAGE` (standart `ghcr.io/apalache-mc/apalache:latest`)
  - ishlaydigan Docker demonini talab qiladi
  - `APALACHE_ALLOW_DOCKER=0` bilan qayta tiklashni o'chirib qo'ying.

Misollar:

```bash
APALACHE_BIN=/opt/apalache/bin/apalache-mc bash scripts/formal/sumeragi_apalache.sh fast
APALACHE_DOCKER_IMAGE=ghcr.io/apalache-mc/apalache:latest bash scripts/formal/sumeragi_apalache.sh deep
```

## Eslatmalar

- Ushbu model Rust modelining bajariladigan sinovlarini to'ldiradi (almashtirmaydi).
  `crates/iroha_core/src/sumeragi/main_loop/tests/state_machine_model_tests.rs`
  va
  `crates/iroha_core/src/sumeragi/main_loop/tests/state_machine_fairness_model_tests.rs`.
- Tekshiruvlar `.cfg` fayllaridagi doimiy qiymatlar bilan chegaralangan.
- PR CI bu tekshiruvlarni `.github/workflows/pr.yml` orqali amalga oshiradi
  `ci/check_sumeragi_formal.sh`.