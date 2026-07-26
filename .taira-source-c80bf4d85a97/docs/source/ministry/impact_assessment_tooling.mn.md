---
lang: mn
direction: ltr
source: docs/source/ministry/impact_assessment_tooling.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 89be62d7bb2bb79fd994d207489d310ef4c997be53447fbee8ac1f7b758d3beb
source_last_modified: "2025-12-29T18:16:35.978367+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Нөлөөллийн үнэлгээний хэрэгсэл (MINFO‑4b)

Замын зургийн лавлагаа: **MINFO‑4b — Нөлөөллийн үнэлгээний хэрэгсэл.**  
Эзэмшигч: Засаглалын зөвлөл / Аналитик

Энэхүү тэмдэглэл нь одоо `cargo xtask ministry-agenda impact` командыг баримтжуулж байна
бүх нийтийн санал асуулгын багцад шаардлагатай автоматжуулсан хэш-гэр бүлийн ялгааг гаргадаг. The
хэрэгсэл нь мөрийн хөтөлбөрийн баталгаажуулсан санал, давхардсан бүртгэл, болон
нэмэлт үгүйсгэх жагсаалт/бодлогын агшин агшин нь тоймчид яг аль нь болохыг харах боломжтой
хурууны хээ шинэ, аль нь одоо байгаа бодлоготой зөрчилдөж, хэдэн оруулгатай
хэш гэр бүл бүр хувь нэмрээ оруулдаг.

## оролт

1. **Хэлэлцэх асуудлын санал.** Дараах нэг буюу хэд хэдэн файл
   [`docs/source/ministry/agenda_council_proposal.md`](agenda_council_proposal.md).
   Тэдгээрийг `--proposal <path>`-ээр тодорхой дамжуулж эсвэл командыг a дээр зааж өгнө үү
   `--proposal-dir <dir>` болон тэр замын доорх `*.json` файл бүрээр дамжуулан лавлах
   орсон байна.
2. **Давхардсан бүртгэл (заавал биш).** таарч байгаа JSON файл
   `docs/examples/ministry/agenda_duplicate_registry.json`. Зөрчилдөөн байдаг
   `source = "duplicate_registry"` дор мэдээлсэн.
3. **Бодлогын агшин зураг (заавал биш).** Бүх зүйлийг жагсаасан хөнгөн манифест
   хурууны хээг ГАР/Яамны бодлогоор аль хэдийн хэрэгжүүлсэн. Ачаалагч хүлээж байна
   схемийг доор харуулав (харна уу
   [`docs/examples/ministry/policy_snapshot_example.json`](../../examples/ministry/policy_snapshot_example.json)
   бүрэн дээжийн хувьд):

```json
{
  "snapshot_id": "denylist-2026-03",
  "generated_at": "2026-03-31T12:00:00Z",
  "entries": [
    {
      "hash_family": "blake3-256",
      "hash_hex": "…",
      "policy_id": "denylist-2025-014-entry-01",
      "note": "Already quarantined by GAR case CSAM-2025-014."
    }
  ]
}
```

`hash_family:hash_hex` хурууны хээ нь саналын зорилттой таарч байгаа аливаа оруулга
`source = "policy_snapshot"` дор мэдээлсэн нь `policy_id` иш татсан.

## Хэрэглээ

```bash
cargo xtask ministry-agenda impact \
  --proposal docs/examples/ministry/agenda_proposal_example.json \
  --registry docs/examples/ministry/agenda_duplicate_registry.json \
  --policy-snapshot docs/examples/ministry/policy_snapshot_example.json \
  --out artifacts/ministry/impact/AC-2026-001.json
```

Нэмэлт саналыг давтан `--proposal` тугуудаар эсвэл дараах байдлаар хавсаргаж болно.
Бүх нийтийн санал асуулгын багцыг агуулсан лавлахыг нийлүүлж байна:

```bash
cargo xtask ministry-agenda impact \
  --proposal-dir artifacts/ministry/proposals/2026-03-31 \
  --registry state/agenda_duplicate_registry.json \
  --out artifacts/ministry/impact/2026-03-31.json
```

`--out`-г орхигдуулсан үед энэ тушаал нь үүсгэсэн JSON-г stdout руу хэвлэдэг.

## Гаралт

Тайлан нь гарын үсэг зурсан олдвор юм (үүнийг бүх нийтийн санал асуулгын багцын дор тэмдэглэнэ үү
`artifacts/ministry/impact/` лавлах) дараах бүтэцтэй:

```json
{
  "format_version": 1,
  "generated_at": "2026-03-31T12:34:56Z",
  "totals": {
    "proposals_analyzed": 4,
    "targets_analyzed": 17,
    "registry_conflicts": 2,
    "policy_conflicts": 1,
    "hash_families": [
      { "hash_family": "blake3-256", "targets": 12, "registry_conflicts": 2, "policy_conflicts": 0 },
      { "hash_family": "sha256", "targets": 5, "registry_conflicts": 0, "policy_conflicts": 1 }
    ]
  },
  "proposals": [
    {
      "proposal_id": "AC-2026-001",
      "action": "add-to-denylist",
      "total_targets": 2,
      "source_path": "docs/examples/ministry/agenda_proposal_example.json",
      "hash_families": [
        { "hash_family": "blake3-256", "targets": 2, "registry_conflicts": 1, "policy_conflicts": 0 }
      ],
      "conflicts": [
        {
          "source": "duplicate_registry",
          "hash_family": "blake3-256",
          "hash_hex": "0d714bed…1338d",
          "reference": "AC-2025-014",
          "note": "Already quarantined."
        }
      ],
      "registry_conflicts": 1,
      "policy_conflicts": 0
    }
  ]
}
```

Энэ JSON-ийг бүх нийтийн санал асуулгын материал болгонд төвийг сахисан хураангуйтай хамт хавсаргана уу
Хэлэлцүүлэгт оролцогчид, тангарагтны шүүгчид, засаглалын ажиглагчид яг тэсэлгээний радиусыг харж болно
санал бүр. Гаралт нь тодорхой (хэш гэр бүлээр эрэмблэгдсэн) бөгөөд аюулгүй
CI/runbook-д оруулах; Хэрэв давхардсан бүртгэл эсвэл бодлогын агшин зуурын зураг өөрчлөгдсөн бол,
тушаалыг дахин ажиллуулж, санал хураалт эхлэхээс өмнө шинэчлэгдсэн олдворыг хавсаргана уу.

> **Дараагийн алхам:** үүсгэсэн нөлөөллийн тайланг оруулах
> [`cargo xtask ministry-panel packet`](referendum_packet.md) тиймээс
> `ReferendumPacketV1` файл нь хэш-гэр бүлийн задаргаа болон
> хянагдаж буй саналын зөрчлийн нарийвчилсан жагсаалт.