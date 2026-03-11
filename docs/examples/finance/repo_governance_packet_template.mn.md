---
lang: mn
direction: ltr
source: docs/examples/finance/repo_governance_packet_template.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: cd018a94197722adfbb9d54bf02f1c486147078174ba4c81f32e9d93b8c3f6d5
source_last_modified: "2026-01-22T16:26:46.473419+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Репо засаглалын багц загвар (замын зураг F1)

Замын зургийн зүйлд шаардагдах олдворын багцыг бэлтгэхдээ энэ загварыг ашиглана уу
F1 (репо амьдралын мөчлөгийн баримтжуулалт ба багаж хэрэгсэл). Зорилго нь шүүмжлэгчдийг хүлээлгэн өгөх явдал юм a
Оролцоо, хэш, нотлох баримтын багц бүрийг жагсаасан ганц Markdown файл
засаглалын зөвлөл саналд дурдсан байтыг дахин тоглуулж болно.

> Загварыг өөрийн нотлох баримтын лавлах руу хуулна уу (жишээ нь
> `artifacts/finance/repo/2026-03-15/packet.md`), орлуулагчийг солих ба
> доор дурдсан хэшлэгдсэн олдворуудын хажууд байршуулах/байршуулах.

## 1. Мета өгөгдөл

| Талбай | Үнэ цэнэ |
|-------|-------|
| Гэрээ/тодорхойлогчийг өөрчлөх | `<repo-yyMMdd-XX>` |
| Бэлтгэсэн / огноо | `<desk lead> – 2026-03-15T10:00Z` |
| Хянсан | `<dual-control reviewer(s)>` |
| Төрөл өөрчлөх | `Initiation / Haircut update / Substitution matrix change / Margin policy` |
| Асран хамгаалагч(ууд) | `<custodian id(s)>` |
| Холбоотой санал / бүх нийтийн санал асуулга | `<governance ticket id or GAR link>` |
| Нотлох лавлах | ``artifacts/finance/repo/<slug>/`` |

## 2. Зааварчилгааны ачаалал

Ширээнүүд дээр гарын үсэг зурсан үе шаттай Norito зааврыг тэмдэглэ.
`iroha app repo ... --output`. Оруулга бүр нь ялгарсан хэшийг агуулсан байх ёстой
файл болон санал хураалтын дараа ирүүлэх үйл ажиллагааны товч тайлбар
дамжуулдаг.

| Үйлдэл | Файл | SHA-256 | Тэмдэглэл |
|--------|------|---------|-------|
| Санаачлах | `instructions/initiate.json` | `<sha256>` | Ширээ + эсрэг талын зөвшөөрсөн бэлэн мөнгө/барьцааны хөлийг агуулна. |
| Маржин дуудлага | `instructions/margin_call.json` | `<sha256>` | Дуудлага үүсгэсэн хэмнэл + оролцогчийн ID-г авдаг. |
| Тайвшрах | `instructions/unwind.json` | `<sha256>` | Нөхцөл хангагдсаны дараа урвуу хөлийн баталгаа. |

```bash
# Example hash helper (repeat per instruction file)
sha256sum artifacts/finance/repo/<slug>/instructions/initiate.json \
  | tee artifacts/finance/repo/<slug>/hashes/initiate.sha256
```

## 2.1 Кастодианы талархал (зөвхөн гурван талын)

Репо нь `--custodian` ашиглах болгонд энэ хэсгийг бөглөнө үү. Засаглалын багц
кастодиан бүрээс гарын үсэг зурсан мэдэгдлийг хавсаргасан байх ёстой
`docs/source/finance/repo_ops.md`-ийн §2.8-д иш татсан файл.

| Кастодиан | Файл | SHA-256 | Тэмдэглэл |
|----------|------|---------|-------|
| `<i105...>` | `custodian_ack_<custodian>.md` | `<sha256>` | Хамгаалалтын цонх, чиглүүлэлтийн бүртгэл, өрмийн харилцагчийг хамарсан гарын үсэг зурсан SLA. |

> Хүлээн зөвшөөрлийг бусад нотлох баримтын хажууд хадгална уу (`artifacts/finance/repo/<slug>/`)
> тиймээс `scripts/repo_evidence_manifest.py` нь файлыг ижил модонд бичдэг
> үе шаттай зааварчилгаа болон тохиргооны хэсгүүд. Харна уу
> `docs/examples/finance/repo_custodian_ack_template.md` бөглөхөд бэлэн
> засаглалын нотлох гэрээнд тохирсон загвар.

## 3. Тохиргооны хэсэг

Кластер дээр буух `[settlement.repo]` TOML блокыг (үүнд орно) буулгана уу.
`collateral_substitution_matrix`). Хэшгийг хэсэгчилсэн байдлаар хадгалаарай
аудиторууд репо захиалах үед идэвхтэй байсан ажиллах цагийн бодлогыг баталгаажуулах боломжтой
батлагдсан.

```toml
[settlement.repo]
eligible_collateral = ["bond#wonderland", "note#wonderland"]
default_margin_percent = "0.025"

[settlement.repo.collateral_substitution_matrix]
"bond#wonderland" = ["bill#wonderland"]
```

`SHA-256 (config snippet): <sha256>`

### 3.1 Зөвшөөрлийн дараах тохиргооны агшин зураг

Бүх нийтийн санал асуулга эсвэл засаглалын санал хураалт дууссаны дараа `[settlement.repo]`
өөрчлөлт гарсан тул үе тэнгийн хүн бүрээс `/v1/configuration` агшин зуурын зургийг аваарай.
Аудиторууд батлагдсан бодлого нь кластер даяар хэрэгжиж байгааг нотолж чадна (харна уу
`docs/source/finance/repo_ops.md` нотлох баримтын ажлын урсгалын §2.9).

```bash
mkdir -p artifacts/finance/repo/<slug>/config/peers
curl -fsSL https://peer01.example/v1/configuration \
  | jq '.' \
  > artifacts/finance/repo/<slug>/config/peers/peer01.json
```

| Үе тэнгийн / эх сурвалж | Файл | SHA-256 | Блокны өндөр | Тэмдэглэл |
|-------------|------|---------|--------------|-------|
| `peer01` | `config/peers/peer01.json` | `<sha256>` | `<block-height>` | Тохиргоог нэвтрүүлсний дараа шууд авсан агшин зуурын зураг. |
| `peer02` | `config/peers/peer02.json` | `<sha256>` | `<block-height>` | `[settlement.repo]` шаталсан TOML-тэй тохирч байгааг баталгаажуулна. |

`hashes.txt` (эсвэл түүнтэй дүйцэхүйц) доторх идентификаторуудын хажууд задаргаа бичнэ үү.
хураангуй) ингэснээр хянагчид өөрчлөлтийг ямар зангилаанууд залгисныг хянах боломжтой. Хормын хувилбарууд
TOML хэсэгчилсэн хэсгийн хажууд `config/peers/` доор амьдардаг бөгөөд түүнийг авах болно
автоматаар `scripts/repo_evidence_manifest.py`.

## 4. Детерминист туршилтын олдворууд

Хамгийн сүүлийн үеийн гаралтыг хавсаргана уу:

- `cargo test -p iroha_core -- repo_deterministic_lifecycle_proof_matches_fixture`
- `cargo test --package integration_tests --test repo`

Бүртгэлийн файлын замууд + логин багцад зориулсан хэшүүд эсвэл таны CI-ийн үйлдвэрлэсэн JUnit XML
систем.

| Олдвор | Файл | SHA-256 | Тэмдэглэл |
|----------|------|---------|-------|
| Амьдралын мөчлөгийн баталгааны бүртгэл | `tests/repo_lifecycle.log` | `<sha256>` | `--nocapture` гаралтаар авсан. |
| Интеграцийн туршилтын бүртгэл | `tests/repo_integration.log` | `<sha256>` | Орлуулах + захын хэмжилтийн хамрах хүрээ багтана. |

## 5. Lifecycle Proof Snapshot

Пакет бүр нь экспортолсон амьдралын мөчлөгийн тодорхойлогч агшин агшныг агуулсан байх ёстой
`repo_deterministic_lifecycle_proof_matches_fixture`. -ээр оосорыг ажиллуул
Экспортын товчлууруудыг идэвхжүүлсэн тул хянагчид JSON фреймийг ялгаж, ялгах боломжтой
бэхэлгээг `crates/iroha_core/tests/fixtures/` (харна уу
`docs/source/finance/repo_ops.md` §2.7).

```bash
REPO_PROOF_SNAPSHOT_OUT=artifacts/finance/repo/<slug>/repo_proof_snapshot.json \
REPO_PROOF_DIGEST_OUT=artifacts/finance/repo/<slug>/repo_proof_digest.txt \
cargo test -p iroha_core \
  -- --exact smartcontracts::isi::repo::tests::repo_deterministic_lifecycle_proof_matches_fixture
```

Эсвэл бэхэлгээг сэргээж, тэдгээрийг хуулж авахын тулд бэхлэгдсэн туслагчийг ашиглана уу
нэг алхамаар нотлох баримтын багц:

```bash
scripts/regen_repo_proof_fixture.sh --toolchain <toolchain> \
  --bundle-dir artifacts/finance/repo/<slug>
```

| Олдвор | Файл | SHA-256 | Тэмдэглэл |
|----------|------|---------|-------|
| Snapshot JSON | `repo_proof_snapshot.json` | `<sha256>` | Баталгаажуулах бэхэлгээнээс ялгардаг амьдралын мөчлөгийн каноник хүрээ. |
| Диджест файл | `repo_proof_digest.txt` | `<sha256>` | `crates/iroha_core/tests/fixtures/repo_lifecycle_proof.digest`-аас толин тусгалтай том зургаан өнцөгт хуваарь; өөрчлөгдөөгүй байсан ч хавсаргана. |

## 6. Нотлох баримт

Нотлох баримтын лавлахыг бүхэлд нь манифест үүсгэснээр аудиторууд шалгах боломжтой
архивыг задлахгүйгээр хэш. Туслах нь тайлбарласан ажлын урсгалыг тусгадаг
`docs/source/finance/repo_ops.md`-д §3.2.

```bash
python3 scripts/repo_evidence_manifest.py \
  --root artifacts/finance/repo/<slug> \
  --agreement-id <repo-identifier> \
  --output artifacts/finance/repo/<slug>/manifest.json
```

| Олдвор | Файл | SHA-256 | Тэмдэглэл |
|----------|------|---------|-------|
| Нотлох баримт | `manifest.json` | `<sha256>` | Шалгалтын дүнг засаглалын тасалбар / бүх нийтийн санал асуулгын тэмдэглэлд оруулна. |

## 7. Telemetry & Event Snapshot

Холбогдох `AccountEvent::Repo(*)` оруулгууд болон аливаа хяналтын самбар эсвэл CSV-г экспортлох
`docs/source/finance/repo_ops.md`-д иш татсан экспорт. Файлуудыг бичнэ үү +
Хэшийг энд оруулснаар хянагч нар шууд нотлох баримт руу орох боломжтой.

| Экспорт | Файл | SHA-256 | Тэмдэглэл |
|--------|------|---------|-------|
| Репо үйл явдлууд JSON | `evidence/repo_events.ndjson` | `<sha256>` | Түүхий Torii үйл явдлын урсгалыг ширээний данс руу шүүсэн. |
| Телеметрийн CSV | `evidence/repo_margin_dashboard.csv` | `<sha256>` | Repo Margin самбар ашиглан Grafana-с экспортолсон. |

## 8. Зөвшөөрөл, гарын үсэг

- **Хос удирдлагатай гарын үсэг зурсан хүмүүс:** `<names + timestamps>`
- **GAR / минутын хураангуй:** Гарын үсэг зурсан GAR PDF-ийн `<sha256>` эсвэл байршуулах минут.
- **Хадгалах байршил:** `governance://finance/repo/<slug>/packet/`

## 9. Хяналтын хуудас

Зүйл бүрийг дууссаны дараа тэмдэглээрэй.

- [ ] Зааварчилгааны ачааллыг шаталсан, хэш хийсэн, хавсаргасан.
- [ ] Тохируулгын хэсэгчилсэн хэш бүртгэгдсэн.
- [ ] Тодорхойлогч тестийн бүртгэлийг авсан + хэш хийсэн.
- [ ] Амьдралын мөчлөгийн агшин зуурын агшин + тоймыг экспортолсон.
- [ ] Нотлох баримт үүсгэсэн ба хэш бүртгэгдсэн.
- [ ] Үйл явдал/телеметрийн экспортыг авсан + хэш хийсэн.
- [ ] Хос хяналтын баталгаажуулалтыг архивласан.
- [ ] GAR/минут байршуулсан; дээр тэмдэглэсэн задаргаа.

Энэхүү загварыг багц бүрийн хажууд байлгах нь засаглалын DAG-ыг хадгалж байдаг
тодорхойлогч бөгөөд аудиторуудад репо амьдралын мөчлөгийн зөөврийн манифест өгдөг
шийдвэрүүд.