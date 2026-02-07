---
lang: hy
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

> Հայելի է սեփականատիրոջ մատյան, որն ապրում է `docs/source/sorafs/runbooks/`-ի ներքո:
> Յուրաքանչյուր նոր SoraFS գործառնական ուղեցույց պետք է կապված լինի այստեղ, երբ այն հրապարակվի:
> պորտալի կառուցում:

Օգտագործեք այս էջը՝ ստուգելու համար, թե որ runbook-ներն են ավարտել միգրացիան
աղբյուրի ուղին և պորտալի պատճենը, որպեսզի գրախոսները կարողանան անմիջապես անցնել ցանկալիին
ուղեցույց բետա նախադիտման ընթացքում:

## Բետա նախադիտման հաղորդավար

DocOps ալիքն այժմ առաջ է քաշել վերանայողների կողմից հաստատված բետա նախադիտման հոսթինգը
`https://docs.iroha.tech/`. Երբ օպերատորներին կամ վերանայողներին մատնանշում եք միգրացված
runbook, հղում կատարել այդ հոսթի անվանմանը, որպեսզի նրանք գործադրեն checksum-gated պորտալը
ակնթարթ. Հրապարակման/վերադարձի ընթացակարգերը գործում են
[`devportal/preview-host-exposure`](../devportal/preview-host-exposure.md):

| Runbook | Սեփականատեր(ներ) | Պորտալի պատճենը | Աղբյուր |
|---------|----------|-------------|--------|
| Gateway & DNS մեկնարկ | Networking TL, Ops Automation, Docs/DevRel | [`sorafs/gateway-dns-runbook`](./gateway-dns-runbook.md) | `docs/source/sorafs_gateway_dns_design_runbook.md` |
| SoraFS գործողությունների գրքույկ | Փաստաթղթեր/DevRel | [`sorafs/operations-playbook`](./operations-playbook.md) | `docs/source/sorafs/operations_playbook.md` |
| Կարողությունների հաշտեցում | Գանձապետարան / SRE | [`sorafs/capacity-reconciliation`](./capacity-reconciliation.md) | `docs/source/sorafs/runbooks/capacity_reconciliation.md` |
| Փին ռեեստրի օպերացիա | Գործիքավորում WG | [`sorafs/pin-registry-ops`](./pin-registry-ops.md) | `docs/source/sorafs/pin_registry_ops.md` |
| Հանգույցի գործառնությունների ստուգաթերթ | Storage Team, SRE | [`sorafs/node-operations`](./node-operations.md) | `docs/source/sorafs/runbooks/sorafs_node_ops.md` |
| Վեճերի և չեղյալ հայտարարման գրքույկ | Կառավարման խորհուրդ | [`sorafs/dispute-revocation-runbook`](./dispute-revocation-runbook.md) | `docs/source/sorafs/dispute_revocation_runbook.md` |
| Բեմականացման մանիֆեստի խաղագիրք | Փաստաթղթեր/DevRel | [`sorafs/staging-manifest-playbook`](./staging-manifest-playbook.md) | `docs/source/sorafs/staging_manifest_playbook.md` |
| Taikai խարիսխի դիտելիություն | Մեդիա հարթակ WG / DA ծրագիր / Ցանցային TL | [`sorafs/taikai-anchor-runbook`](./taikai-anchor-runbook.md) | `docs/source/taikai_anchor_monitoring.md` |

## Ստուգման ստուգաթերթ

- [x] Պորտալի կառուցման հղումներ դեպի այս ինդեքսը (կողագոտու մուտք):
- [x] Յուրաքանչյուր գաղթած վազքագիրք թվարկում է կանոնական աղբյուրի ուղին՝ գրախոսներ պահելու համար
  հավասարեցված փաստաթղթերի վերանայման ժամանակ:
- [x] DocOps-ի նախադիտման խողովակաշարի բլոկները միաձուլվում են, երբ բացակայում է թվարկված runbook-ը
  պորտալի ելքից:

Ապագա միգրացիաները (օրինակ՝ նոր քաոսի զորավարժությունները կամ կառավարման հավելվածները) պետք է ավելացնեն a
տող դեպի վերևի աղյուսակը և թարմացրեք ներկառուցված DocOps ստուգաթերթը
`docs/examples/docs_preview_request_template.md`.