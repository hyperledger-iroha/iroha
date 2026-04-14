<!-- Auto-generated stub for Mongolian (mn) translation. Replace this content with the full translation. -->

---
lang: mn
direction: ltr
source: docs/formal/sumeragi/README.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 56f1412b2db729ba69057ce15ac8bae707310fd5a6d01be2da816fdee18218f7
source_last_modified: "2026-02-23T14:48:46.580877+00:00"
translation_last_reviewed: 2026-04-02
translator: machine-google-reviewed
---

# Sumeragi Албан ёсны загвар (TLA+ / Apalache)

Энэ лавлах нь Sumeragi замын аюулгүй байдал, амьдрах чадварт зориулсан хязгаарлагдмал албан ёсны загварыг агуулдаг.

## Хамрах хүрээ

Энэхүү загвар нь дараахь зүйлийг агуулна.
- фазын явц (`Propose`, `Prepare`, `CommitVote`, `NewView`, `Committed`),
- саналын болон ирцийн босго (`CommitQuorum`, `ViewQuorum`),
- NPoS маягийн үүрэг хамгаалагчдын жигнэсэн гадасны чуулга (`StakeQuorum`),
- RBC-ийн учир шалтгааны хамаарал (`Init -> Chunk -> Ready -> Deliver`) толгой/тодорхой баримттай,
- Шударга ахиц дэвшлийн үйл ажиллагаан дээр ҮСТ болон сул шударга байдлын таамаглал.

Энэ нь утасны формат, гарын үсэг, сүлжээний дэлгэрэнгүй мэдээллийг зориудаар хийсвэрлэдэг.

## Файлууд

- `Sumeragi.tla`: протоколын загвар ба шинж чанарууд.
- `Sumeragi_fast.cfg`: жижиг CI-д ээлтэй параметрийн багц.
- `Sumeragi_deep.cfg`: илүү том стресс параметрийн багц.

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

## Гүйж байна

Хадгалах сангийн үндэсээс:

```bash
bash scripts/formal/sumeragi_apalache.sh fast
bash scripts/formal/sumeragi_apalache.sh deep
```

### Хуулбарлах боломжтой орон нутгийн тохиргоо (Docker шаардлагагүй)Энэ репозиторийн ашигладаг Apalache хэрэгслийн гинжийг суулгана уу:

```bash
bash scripts/formal/install_apalache.sh 0.52.2
```

Гүйгч энэ суулгацыг дараах хаягаар автоматаар илрүүлдэг:
`target/apalache/toolchains/v0.52.2/bin/apalache-mc`.
Суулгасны дараа `ci/check_sumeragi_formal.sh` нэмэлт env varsгүйгээр ажиллах ёстой:

```bash
bash ci/check_sumeragi_formal.sh
```

Хэрэв Apalache `PATH`-д байхгүй бол та:

- `APALACHE_BIN`-г гүйцэтгэх замд тохируулах, эсвэл
- Docker нөөцийг ашиглах (`docker` боломжтой үед анхдагчаар идэвхждэг):
  - зураг: `APALACHE_DOCKER_IMAGE` (өгөгдмөл `ghcr.io/apalache-mc/apalache:latest`)
  - ажиллаж байгаа Docker демон шаардлагатай
  - `APALACHE_ALLOW_DOCKER=0`-ээр нөөцийг идэвхгүй болгох.

Жишээ нь:

```bash
APALACHE_BIN=/opt/apalache/bin/apalache-mc bash scripts/formal/sumeragi_apalache.sh fast
APALACHE_DOCKER_IMAGE=ghcr.io/apalache-mc/apalache:latest bash scripts/formal/sumeragi_apalache.sh deep
```

## Тэмдэглэл

- Энэ загвар нь гүйцэтгэх боломжтой Rust загварын туршилтуудыг нөхдөг (орлохгүй).
  `crates/iroha_core/src/sumeragi/main_loop/tests/state_machine_model_tests.rs`
  болон
  `crates/iroha_core/src/sumeragi/main_loop/tests/state_machine_fairness_model_tests.rs`.
- Шалгалтууд нь `.cfg` файлуудын тогтмол утгуудаар хязгаарлагддаг.
- PR CI эдгээр шалгалтыг `.github/workflows/pr.yml`-ээр дамжуулан явуулдаг
  `ci/check_sumeragi_formal.sh`.