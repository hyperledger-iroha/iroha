---
lang: mn
direction: ltr
source: docs/source/ministry/referendum_packet.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 922d972376b67a2f8c0c03ded95db6576e16e229e4bcb62d920b0ffda49c93ac
source_last_modified: "2025-12-29T18:16:35.980526+00:00"
translation_last_reviewed: 2026-02-07
title: Referendum Packet Workflow (MINFO-4)
summary: Produce the complete referendum dossier (`ReferendumPacketV1`) combining the proposal, neutral summary, sortition artefacts, and impact report.
translator: machine-google-reviewed
---

# Ард нийтийн санал асуулгын багцын ажлын урсгал (MINFO-4)

Замын зургийн зүйл **MINFO-4 — Хяналтын самбар ба бүх нийтийн санал асуулга нэгтгэгч** одоо байна
шинэ `ReferendumPacketV1` Norito схем болон CLI туслахуудаар хангагдсан
доор тайлбарласан. Ажлын урсгал нь бодлогын шүүгчид шаардагдах эд өлгийн зүйл бүрийг багцалдаг
засаглал, аудиторууд, ил тод байдал зэрэг нь нэг JSON баримт бичигт санал өгдөг
порталууд нотлох баримтыг тодорхой байдлаар давтаж болно.

## оролт

1. **Хэлэлцэх асуудлын санал** — `cargo xtask ministry-agenda validate`-д ашигласан ижил JSON.
2. **Сайн дурын ажилтны товч мэдээлэл** — дамжуулсны дараа боловсруулсан өгөгдлийн багц
   `cargo xtask ministry-transparency volunteer-validate`.
3. **AI moderation manifest** — засаглалын гарын үсэгтэй `ModerationReproManifestV1`.
4. **Ангилах хураангуй** — ялгаруулсан детерминист олдвор
   `cargo xtask ministry-agenda sortition`. JSON дагаж байна
   [`PolicyJurySortitionV1`](./policy_jury_ballots.md) тиймээс засаглал
   POP агшин зуурын хураангуй болон хүлээлгийн жагсаалт/ажилтгүй утсыг хуулбарлах.
5. **Нөлөөллийн тайлан** — hash-family/report-оор үүсгэгдсэн
   `cargo xtask ministry-agenda impact`.

## CLI хэрэглээ

```bash
cargo xtask ministry-panel packet \
  --proposal artifacts/ministry/proposals/AC-2026-001.json \
  --volunteer artifacts/ministry/volunteer_briefs.json \
  --ai-manifest artifacts/ministry/ai_manifest.json \
  --panel-round RP-2026-05 \
  --sortition artifacts/ministry/agenda_sortition_2026Q1.json \
  --impact artifacts/ministry/impact/AC-2026-001.json \
  --summary-out artifacts/ministry/review_panel_summary.json \
  --output artifacts/ministry/referendum_packets/AC-2026-001.json
```

`packet` дэд команд нь төвийг сахисан хураангуй синтезаторыг (MINFO-4a) ажиллуулж, дахин ашигладаг.
одоо байгаа сайн дурын хэрэгсэл, гарцыг баяжуулж байна:

- `ReferendumSortitionEvidence` — алгоритм, үр, жагсаалтын задаргаа
  ангилах олдвор.
- `ReferendumPanelist[]` — сонгогдсон зөвлөлийн гишүүн бүр дээр нэмээд Merkle баталгаа
  тэдний сугалааны аудит хийх шаардлагатай болсон.
- `ReferendumImpactSummary` — гэр бүл бүрийн хэшийн нийт дүн болон зөрчлийн жагсаалт
  нөлөөллийн тайлан.

Бие даасан `ReviewPanelSummaryV1` хэрэгтэй үед `--summary-out`-г ашиглаарай
файл; эс бөгөөс пакет нь хураангуйг `review_summary` доор оруулдаг.

## Гаралтын бүтэц

`ReferendumPacketV1` амьдардаг
`crates/iroha_data_model/src/ministry/mod.rs` бөгөөд SDK дээр ашиглах боломжтой.
Гол хэсгүүдэд:

- `proposal` — анхны `AgendaProposalV1` объект.
- `review_summary` — MINFO-4a-аас гаргасан тэнцвэртэй хураангуй.
- `sortition` / `panelists` - суудалтай зөвлөлд дахин давтагдах баталгаа.
- `impact_summary` — хэш гэр бүл бүрт давхардсан/бодлогын зөрчлийн нотлох баримт.

Бүрэн дээжийг `docs/examples/ministry/referendum_packet_example.json` харна уу.
Үүсгэсэн багцыг бүх нийтийн санал асуулгын хуудас бүрт гарын үсэг зурсан AI-тай хамт хавсаргана уу
онцлох хэсгүүдийн иш татсан ил тод, ил тод олдворууд.