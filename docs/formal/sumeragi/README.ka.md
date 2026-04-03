<!-- Auto-generated stub for Georgian (ka) translation. Replace this content with the full translation. -->

---
lang: ka
direction: ltr
source: docs/formal/sumeragi/README.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 56f1412b2db729ba69057ce15ac8bae707310fd5a6d01be2da816fdee18218f7
source_last_modified: "2026-02-23T14:48:46.580877+00:00"
translation_last_reviewed: 2026-04-02
translator: machine-google-reviewed
---

# Sumeragi ფორმალური მოდელი (TLA+ / Apalache)

ეს დირექტორია შეიცავს შემოსაზღვრულ ფორმალურ მოდელს Sumeragi commit-path უსაფრთხოებისა და სიცოცხლისუნარიანობისთვის.

## სფერო

მოდელი ასახავს:
- ფაზის პროგრესირება (`Propose`, `Prepare`, `CommitVote`, `NewView`, `Committed`),
- ხმის მიცემის და კვორუმის ბარიერი (`CommitQuorum`, `ViewQuorum`),
- შეწონილი ფსონის კვორუმი (`StakeQuorum`) NPoS-ის სტილის დავალებების მცველებისთვის,
- RBC მიზეზობრიობა (`Init -> Chunk -> Ready -> Deliver`) სათაურით/დაჯესტის მტკიცებულებებით,
- GST და სუსტი სამართლიანობის ვარაუდები პატიოსანი პროგრესის ქმედებებზე.

იგი განზრახ აბსტრაქტებს მავთულის ფორმატებს, ხელმოწერებს და ქსელის სრულ დეტალებს.

## ფაილები

- `Sumeragi.tla`: პროტოკოლის მოდელი და თვისებები.
- `Sumeragi_fast.cfg`: უფრო მცირე CI-მეგობრული პარამეტრების ნაკრები.
- `Sumeragi_deep.cfg`: სტრესის უფრო დიდი პარამეტრის ნაკრები.

## თვისებები

უცვლელები:
- `TypeInvariant`
- `CommitImpliesQuorum`
- `CommitImpliesStakeQuorum`
- `CommitImpliesDelivered`
- `DeliverImpliesEvidence`

დროებითი საკუთრება:
- `EventuallyCommit` (`[] (gst => <> committed)`), პოსტ-GST სამართლიანობის კოდირებით
  ფუნქციონირებს `Next`-ში (ჩართულია დროის ამოწურვა/შეცდომის წინასწარი დაცვა
  პროგრესის მოქმედებები). ეს ინარჩუნებს მოდელს შესამოწმებლად Apalache 0.52.x-ით, რაც
  არ აქვს `WF_` სამართლიანობის ოპერატორების მხარდაჭერა შემოწმებული დროითი თვისებების შიგნით.

## სირბილი

საცავიდან ფესვიდან:

```bash
bash scripts/formal/sumeragi_apalache.sh fast
bash scripts/formal/sumeragi_apalache.sh deep
```

### რეპროდუცირებადი ადგილობრივი დაყენება (არ არის საჭირო Docker)დააინსტალირეთ დამაგრებული ადგილობრივი Apalache ინსტრუმენტების ჯაჭვი, რომელიც გამოიყენება ამ საცავში:

```bash
bash scripts/formal/install_apalache.sh 0.52.2
```

Runner ავტომატურად ამოიცნობს ამ ინსტალაციას:
`target/apalache/toolchains/v0.52.2/bin/apalache-mc`.
ინსტალაციის შემდეგ, `ci/check_sumeragi_formal.sh` უნდა მუშაობდეს დამატებითი env vars-ის გარეშე:

```bash
bash ci/check_sumeragi_formal.sh
```

თუ Apalache არ არის `PATH`-ში, შეგიძლიათ:

- დააყენეთ `APALACHE_BIN` შესრულებად გზაზე, ან
- გამოიყენეთ Docker სარეზერვო საშუალება (ჩართულია ნაგულისხმევად, როდესაც `docker` ხელმისაწვდომია):
  - სურათი: `APALACHE_DOCKER_IMAGE` (ნაგულისხმევი `ghcr.io/apalache-mc/apalache:latest`)
  - მოითხოვს გაშვებულ Docker დემონს
  - გამორთეთ სარეზერვო საშუალება `APALACHE_ALLOW_DOCKER=0`-ით.

მაგალითები:

```bash
APALACHE_BIN=/opt/apalache/bin/apalache-mc bash scripts/formal/sumeragi_apalache.sh fast
APALACHE_DOCKER_IMAGE=ghcr.io/apalache-mc/apalache:latest bash scripts/formal/sumeragi_apalache.sh deep
```

## შენიშვნები

- ეს მოდელი ავსებს (არ ცვლის) შესრულებადი Rust მოდელის ტესტებს
  `crates/iroha_core/src/sumeragi/main_loop/tests/state_machine_model_tests.rs`
  და
  `crates/iroha_core/src/sumeragi/main_loop/tests/state_machine_fairness_model_tests.rs`.
- ჩეკები შემოიფარგლება მუდმივი მნიშვნელობებით `.cfg` ფაილებში.
- PR CI აწარმოებს ამ შემოწმებებს `.github/workflows/pr.yml`-ში მეშვეობით
  `ci/check_sumeragi_formal.sh`.