---
lang: mn
direction: ltr
source: docs/examples/taikai_anchor_lineage_packet.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: a2037fed472e37a06559e7cd871c1b916b514b9804f309413fc369d5ded662b6
source_last_modified: "2025-12-29T18:16:35.095373+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Taikai Anchor Lineage Пакет Загвар (SN13-C)

Замын зургийн зүйл **SN13-C — Manifests & SoraNS зангуу** нь бусад нэр бүрийг шаарддаг
детерминист нотолгооны багцыг тээвэрлэх эргэлт. Энэ загварыг өөрийн файл руу хуулна уу
олдворын лавлах (жишээ нь
`artifacts/taikai/anchor/<event>/<alias>/<timestamp>/packet.md`) ба солих
багцыг засаглалд оруулахаас өмнө орлуулагчид.

## 1. Мета өгөгдөл

| Талбай | Үнэ цэнэ |
|-------|-------|
| Үйл явдлын ID | `<taikai.event.launch-2026-07-10>` |
| Дамжуулах / дамжуулах | `<main-stage>` |
| Alias ​​namespace / name | `<sora / docs>` |
| Нотлох лавлах | `artifacts/taikai/anchor/<event>/<alias>/2026-07-10T18-00Z/` |
| Оператортой харилцах | `<name + email>` |
| GAR / RPT тасалбар | `<governance ticket or GAR digest>` |

## Багцын туслах (заавал биш)

Дамрын олдворуудыг хуулж, өмнө нь JSON (заавал гарын үсэг зурсан) хураангуйг гарга
үлдсэн хэсгүүдийг бөглөх:

```bash
cargo xtask taikai-anchor-bundle \
  --spool config/da_manifests/taikai \
  --copy-dir artifacts/taikai/anchor/<event>/<alias>/<timestamp>/spool \
  --out artifacts/taikai/anchor/<event>/<alias>/<timestamp>/anchor_bundle.json \
  --signing-key <hex-ed25519-optional>
```

Туслах `taikai-anchor-request-*`, `taikai-trm-state-*`,
`taikai-lineage-*`, дугтуй, харуулууд Taikai дамар лавлахаас гарсан
(`config.da_ingest.manifest_store_dir/taikai`) тиймээс нотлох баримт хавтас аль хэдийн
доор дурдсан яг файлуудыг агуулна.

## 2. Удам угсааны дэвтэр ба зөвлөгөө

Диск дээрх удамшлын дэвтэр болон үүнд зориулж бичсэн JSON Torii зөвлөмжийг хавсаргана уу.
цонх. Эдгээр нь шууд ирдэг
`config.da_ingest.manifest_store_dir/taikai/taikai-trm-state-<alias>.json` ба
`taikai-lineage-<lane>-<epoch>-<sequence>-<storage_ticket>-<fingerprint>.json`.

| Олдвор | Файл | SHA-256 | Тэмдэглэл |
|----------|------|---------|-------|
| Удам угсааны дэвтэр | `taikai-trm-state-docs.json` | `<sha256>` | Өмнөх манифест дижест/цонхыг нотолно. |
| Удам угсааны зөвлөгөө | `taikai-lineage-l1-140-6a-b2b.json` | `<sha256>` | SoraNS зангуу руу байршуулахаас өмнө авсан. |

```bash
sha256sum artifacts/taikai/anchor/<event>/<alias>/<ts>/taikai-trm-state-*.json \
  | tee artifacts/taikai/anchor/<event>/<alias>/<ts>/hashes/lineage.sha256
```

## 3. Зангууны ачааг барих

Torii зангууны үйлчилгээнд хүргэсэн POST ачааллыг тэмдэглэ. Ачаалал
`envelope_base64`, `ssm_base64`, `trm_base64`, доторлогоо багтана
`lineage_hint` объект; Аудитууд нь энэ баримтад тулгуурладаг бөгөөд энэ нь байсан санааг нотлох болно
SoraNS руу илгээсэн. Torii одоо энэ JSON-г автоматаар гэж бичдэг
`taikai-anchor-request-<lane>-<epoch>-<sequence>-<ticket>-<fingerprint>.json`
Taikai дамар лавлах дотор (`config.da_ingest.manifest_store_dir/taikai/`), тийм
операторууд HTTP бүртгэлийг хусахын оронд шууд хуулж болно.

| Олдвор | Файл | SHA-256 | Тэмдэглэл |
|----------|------|---------|-------|
| Anchor POST | `requests/2026-07-10T18-00Z.json` | `<sha256>` | `taikai-anchor-request-*.json` (Тайкай дамар) -аас хуулбарласан түүхий хүсэлт. |

## 4. Манифест дижест хүлээн зөвшөөрөлт

| Талбай | Үнэ цэнэ |
|-------|-------|
| Шинэ манифест тойм | `<hex digest>` |
| Өмнөх манифест дижест (санавараас) | `<hex digest>` |
| Цонхны эхлэл / төгсгөл | `<start seq> / <end seq>` |
| Хүлээн авах цагийн тэмдэг | `<ISO8601>` |

Дээр бичсэн дэвтэр/санамж хэшийг лавлана уу, ингэснээр хянагчид үүнийг баталгаажуулах боломжтой
солигдсон цонх.

## 5. Метрик / `taikai_alias_rotations`

- `taikai_trm_alias_rotations_total` агшин зуурын зураг: `<Prometheus query + export path>`
- `/status taikai_alias_rotations` хогийн цэг (хоолын нэрээр): `<file path + hash>`

Тоолуурыг харуулсан Prometheus/Grafana экспорт эсвэл `curl` гаралтыг өгнө үү
өсөлт ба энэ нэрийн `/status` массив.

## 6. Нотлох баримтын лавлах манифест

Нотолгооны лавлахын тодорхойлогч манифест үүсгэх (дамар файл,
ачаалал авах, хэмжүүрийн агшин зуурын агшин) гэх мэт) тул засаглал нь хэш бүрийг шалгахгүйгээр баталгаажуулах боломжтой
архивыг задлах.

```bash
python3 scripts/repo_evidence_manifest.py \
  --root artifacts/taikai/anchor/<event>/<alias>/<ts> \
  --agreement-id <event/alias/window> \
  --output artifacts/taikai/anchor/<event>/<alias>/<ts>/manifest.json
```

| Олдвор | Файл | SHA-256 | Тэмдэглэл |
|----------|------|---------|-------|
| Нотлох баримт | `manifest.json` | `<sha256>` | Үүнийг засаглалын багц / GAR-д хавсаргана уу. |

## 7. Хяналтын хуудас

- [ ] Удам угсааны дэвтэр хуулсан + хэш хийсэн.
- [ ] Удам угсааны зөвлөмжийг хуулсан + хэш хийсэн.
- [ ] Anchor POST ачааллыг барьж аваад хэш хийсэн.
- [ ] Манифест дижест хүснэгтийг бөглөсөн.
- [ ] Хэмжилтийн агшин зуурын агшин зургуудыг экспортолсон (`taikai_trm_alias_rotations_total`, `/status`).
- [ ] `scripts/repo_evidence_manifest.py`-ээр үүсгэсэн манифест.
- [ ] Хэш + холбоо барих мэдээллийн хамт засаглалд байршуулсан пакет.

Энэ загварыг бусад нэрийн эргэлт болгонд хадгалах нь SoraNS засаглалыг хадгалж байдаг
хуулбарлах боломжтой, удамшлын зөвлөмжийг GAR/RPT нотлох баримттай шууд холбоно.