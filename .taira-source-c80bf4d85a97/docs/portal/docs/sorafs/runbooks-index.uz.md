---
lang: uz
direction: ltr
source: docs/portal/docs/sorafs/runbooks-index.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 9c3d1e36d99e18b5986e911a6b240393a92140324142f9edb778d2f966b1712e
source_last_modified: "2026-01-05T09:28:11.909605+00:00"
translation_last_reviewed: 2026-02-07
id: runbooks-index
title: Operator Runbooks Index
description: Canonical entry point for the migrated SoraFS operator runbooks.
sidebar_label: Runbook Index
translator: machine-google-reviewed
---

> `docs/source/sorafs/runbooks/` ostida yashaydigan egasi kitobini aks ettiradi.
> Har bir yangi SoraFS operatsion qoʻllanmasi chop etilgandan soʻng shu yerda bogʻlanishi kerak.
> portal qurilishi.

Ushbu sahifadan qaysi runbooklar ko'chirishni yakunlaganligini tekshirish uchun foydalaning
manba yo'li va portal nusxasi, shuning uchun sharhlovchilar kerakli joyga o'tishlari mumkin
beta-versiyani oldindan ko'rish paytida qo'llanma.

## Beta-versiyani ko'rib chiqish xosti

DocOps to'lqini hozirda sharhlovchi tomonidan tasdiqlangan beta-versiyani oldindan ko'rish xostini ilgari surdi
`https://docs.iroha.tech/`. Operatorlar yoki sharhlovchilarni ko'chirilganga ko'rsatganda
runbook, o'sha xost nomiga havola qiling, shunda ular nazorat summasi bilan himoyalangan portaldan foydalanadilar
surat. Nashr qilish/orqaga qaytarish tartib-qoidalari mavjud
[`devportal/preview-host-exposure`](../devportal/preview-host-exposure.md).

| Runbook | Ega(lar)i | Portal nusxasi | Manba |
|---------|----------|-------------|--------|
| Gateway & DNS boshlanishi | Networking TL, Ops Automation, Docs/DevRel | [`sorafs/gateway-dns-runbook`](./gateway-dns-runbook.md) | `docs/source/sorafs_gateway_dns_design_runbook.md` |
| SoraFS operatsiyalar kitobi | Docs/DevRel | [`sorafs/operations-playbook`](./operations-playbook.md) | `docs/source/sorafs/operations_playbook.md` |
| Imkoniyatlarni moslashtirish | G'aznachilik / SRE | [`sorafs/capacity-reconciliation`](./capacity-reconciliation.md) | `docs/source/sorafs/runbooks/capacity_reconciliation.md` |
| Ro'yxatga olish kitobi operatsiyalari | Asboblar WG | [`sorafs/pin-registry-ops`](./pin-registry-ops.md) | `docs/source/sorafs/pin_registry_ops.md` |
| Tugun operatsiyalari nazorat ro'yxati | Saqlash jamoasi, SRE | [`sorafs/node-operations`](./node-operations.md) | `docs/source/sorafs/runbooks/sorafs_node_ops.md` |
| Eʼtiroz va bekor qilish kitobi | Boshqaruv Kengashi | [`sorafs/dispute-revocation-runbook`](./dispute-revocation-runbook.md) | `docs/source/sorafs/dispute_revocation_runbook.md` |
| Manifest o'yin kitobini sahnalashtirish | Docs/DevRel | [`sorafs/staging-manifest-playbook`](./staging-manifest-playbook.md) | `docs/source/sorafs/staging_manifest_playbook.md` |
| Taikai langari kuzatilishi | Media platformasi WG / DA dasturi / Networking TL | [`sorafs/taikai-anchor-runbook`](./taikai-anchor-runbook.md) | `docs/source/taikai_anchor_monitoring.md` |

## Tekshirish roʻyxati

- [x] Ushbu indeksga portal yaratish havolalari (yon paneldagi yozuv).
- [x] Har bir ko'chirilgan runbook sharhlovchilarni ushlab turish uchun kanonik manba yo'lini ko'rsatadi
  doc ko'rib chiqish paytida moslashtirilgan.
- [x] DocOps oldindan ko'rish quvur liniyasi bloklari ro'yxatga olingan ish kitobi yo'q bo'lganda birlashadi
  portal chiqishidan.

Kelajakdagi migratsiya (masalan, yangi tartibsizlik mashqlari yoki boshqaruv ilovalari)
qatorni yuqoridagi jadvalga o'tkazing va ichiga o'rnatilgan DocOps nazorat ro'yxatini yangilang
`docs/examples/docs_preview_request_template.md`.