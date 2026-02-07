---
id: nexus-settlement-faq
lang: mn
direction: ltr
source: docs/portal/docs/nexus/settlement-faq.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: Settlement FAQ
description: Operator-facing answers covering settlement routing, XOR conversion, telemetry, and audit evidence.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

Энэ хуудас нь дотоод төлбөр тооцооны түгээмэл асуултуудыг толилуулж байна (`docs/source/nexus_settlement_faq.md`)
Ингэснээр портал уншигчид ухахгүйгээр ижил удирдамжтай танилцах боломжтой
моно репо. Энэ нь Төлбөр тооцооны чиглүүлэгч төлбөрийг хэрхэн боловсруулдаг, ямар хэмжүүрүүдийг тайлбарладаг
хянах, мөн SDK-ууд Norito ачааллыг хэрхэн нэгтгэх ёстой.

## Онцлох үйл явдал

1. **Эгнээний зураглал** — өгөгдлийн орон зай бүр `settlement_handle` гэж зарладаг.
   (`xor_global`, `xor_lane_weighted`, `xor_hosted_custody`, эсвэл
   `xor_dual_fund`). Хамгийн сүүлийн үеийн эгнээний каталогийг доороос үзнэ үү
   `docs/source/project_tracker/nexus_config_deltas/`.
2. **Deterministic conversion** — чиглүүлэгч нь бүх тооцоог XOR руу хөрвүүлдэг
   засаглалын баталсан хөрвөх чадварын эх үүсвэр. Хувийн замууд нь XOR буферийг урьдчилан санхүүжүүлдэг;
   Үс засах нь зөвхөн буфер нь бодлогоос гадуур гарах үед л хамаарна.
3. **Телеметри** — `nexus_settlement_latency_seconds` цаг, хөрвүүлэх тоолуур,
   мөн үс засах хэмжигч. Хяналтын самбарууд `dashboards/grafana/nexus_settlement.json` дээр амьдардаг
   болон `dashboards/alerts/nexus_audit_rules.yml` дахь дохиолол.
4. **Нотлох баримт** — архивын тохиргоо, чиглүүлэгчийн бүртгэл, телеметрийн экспорт болон
   аудитын нэгтгэлийн тайлан.
5. **SDK-ийн үүрэг хариуцлага** — SDK бүр төлбөр тооцооны туслахууд, эгнээний ID,
   болон Norito даацын кодлогч нь чиглүүлэгчтэй тэгш байдлыг хадгалах.

## Урсгалын жишээ

| эгнээний төрөл | Баривчлах нотлох баримт | Энэ нь юуг баталж байна |
|----------|--------------------|----------------|
| Хувийн `xor_hosted_custody` | Чиглүүлэгчийн бүртгэл + `nexus_settlement_latency_seconds{lane}` + `settlement_router_haircut_total{lane}` | CBDC буфер нь дебит детерминистик XOR ба үсний засалтууд нь бодлогод үлддэг. |
| Нийтийн `xor_global` | Чиглүүлэгчийн бүртгэл + DEX/TWAP лавлагаа + хоцролт/хувиргах хэмжүүр | Хуваалцсан хөрвөх чадварын зам нь хэвлэгдсэн TWAP дээр үсээ огтлохгүйгээр шилжүүлгийг үнэлсэн. |
| Гибрид `xor_dual_fund` | Нийтийн ба хамгаалалттай хуваах + телеметрийн тоолуурыг харуулсан чиглүүлэгчийн бүртгэл | Хамгаалагдсан/олон нийтийн холимог засаглалын харьцааг хүндэтгэж, хөл тус бүрт үсний засалтыг тэмдэглэв. |

## Илүү дэлгэрэнгүй мэдээлэл хэрэгтэй байна уу?

- Бүрэн түгээмэл асуултууд: `docs/source/nexus_settlement_faq.md`
- Төлбөр тооцооны чиглүүлэгчийн үзүүлэлт: `docs/source/settlement_router.md`
- CBDC бодлогын заавар: `docs/source/cbdc_lane_playbook.md`
- Үйлдлийн дэвтэр: [Nexus үйлдлүүд](./nexus-operations)