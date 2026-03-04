---
lang: mn
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

> `docs/source/sorafs/runbooks/` дор амьдардаг эзэмшигчийн дэвтэрийг толин тусгал.
> Шинэ SoraFS үйлдлийн гарын авлага бүрийг нийтэлсэн бол энд холбогдох ёстой.
> портал бүтээх.

Энэ хуудсыг ашиглан аль runbook-ээс шилжилтийг хийж гүйцэтгэсэн болохыг шалгана уу
эх сурвалжийн зам, портал хуулбарыг ашиглан хянагчид хүссэн рүүгээ шууд шилжих боломжтой
бета урьдчилан үзэх үеийн хөтөч.

## Бета урьдчилан үзэх хост

DocOps долгион нь одоо хянагчаар батлагдсан бета урьдчилан үзэх хостыг сурталчилж байна
`https://docs.iroha.tech/`. Оператор эсвэл тоймчдыг шилжүүлсэн рүү чиглүүлэх үед
runbook, тухайн хостын нэрийг лавлаж, шалгах нийлбэртэй порталыг ашигладаг
агшин зуурын зураг. Нийтлэх/буцах процедурууд амьдардаг
[`devportal/preview-host-exposure`](../devportal/preview-host-exposure.md).

| Runbook | Эзэмшигч(үүд) | Портал хуулбар | Эх сурвалж |
|---------|----------|-------------|--------|
| Gateway & DNS эхлэл | Networking TL, Ops Automation, Docs/DevRel | [`sorafs/gateway-dns-runbook`](./gateway-dns-runbook.md) | `docs/source/sorafs_gateway_dns_design_runbook.md` |
| SoraFS үйлдлийн дэвтэр | Docs/DevRel | [`sorafs/operations-playbook`](./operations-playbook.md) | `docs/source/sorafs/operations_playbook.md` |
| Чадавхийг нэгтгэх | Төрийн сан / SRE | [`sorafs/capacity-reconciliation`](./capacity-reconciliation.md) | `docs/source/sorafs/runbooks/capacity_reconciliation.md` |
| Бүртгэлийн үйлдлүүдийг тогтоох | Багажны WG | [`sorafs/pin-registry-ops`](./pin-registry-ops.md) | `docs/source/sorafs/pin_registry_ops.md` |
| Зангилааны үйл ажиллагааны хяналтын хуудас | Хадгалах баг, SRE | [`sorafs/node-operations`](./node-operations.md) | `docs/source/sorafs/runbooks/sorafs_node_ops.md` |
| Маргаан ба хүчингүй болгох runbook | Засаглалын зөвлөл | [`sorafs/dispute-revocation-runbook`](./dispute-revocation-runbook.md) | `docs/source/sorafs/dispute_revocation_runbook.md` |
| Тайзны манифест тоглох ном | Docs/DevRel | [`sorafs/staging-manifest-playbook`](./staging-manifest-playbook.md) | `docs/source/sorafs/staging_manifest_playbook.md` |
| Тайкай зангууны ажиглалт | Media Platform WG / DA Program / Networking TL | [`sorafs/taikai-anchor-runbook`](./taikai-anchor-runbook.md) | `docs/source/taikai_anchor_monitoring.md` |

## Баталгаажуулах шалгах хуудас

- [x] Энэ индекс рүү портал үүсгэх холбоосууд (хажуугийн самбарын оруулга).
- [x] Шилжүүлсэн runbook бүр тоймчдыг хадгалах каноник эх замыг жагсаасан байдаг
  док-н шалгалтын явцад зэрэгцүүлсэн.
- [x] Жагсаалтад орсон runbook байхгүй үед DocOps урьдчилан харах дамжуулах хоолойн блокуудыг нэгтгэдэг
  портал гаралтаас.

Ирээдүйн шилжилт хөдөлгөөн (жишээ нь, эмх замбараагүй байдлын шинэ дасгалууд эсвэл засаглалын хавсралтууд) нэмэх хэрэгтэй.
Дээрх хүснэгтийн эгнээнд суулгасан DocOps шалгах хуудсыг шинэчил
`docs/examples/docs_preview_request_template.md`.