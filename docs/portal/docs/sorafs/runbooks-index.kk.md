---
lang: kk
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

> `docs/source/sorafs/runbooks/` астында өмір сүретін иесінің кітапшасын көрсетеді.
> Әрбір жаңа SoraFS операциялық нұсқаулығы жарияланғаннан кейін осында сілтеме болуы керек
> портал құрастыру.

Бұл бетті қай runbook файлдарынан тасымалдауды аяқтағанын тексеру үшін пайдаланыңыз
шолушылардың тікелей қалағанға өтуі үшін бастапқы жол және портал көшірмелері
бета-алдын ала қарау кезінде нұсқаулық.

## Бета алдын ала қарау хосты

DocOps толқыны қазір шолушы мақұлдаған бета-алдын ала қарау хостын алға жылжытты
`https://docs.iroha.tech/`. Операторларды немесе шолушыларды көшірілгенге көрсеткенде
runbook, сол хост атауына сілтеме жасай отырып, олар бақылау қосындысы бар порталды қолданады
сурет. Жариялау/қайтару процедуралары тірі
[`devportal/preview-host-exposure`](../devportal/preview-host-exposure.md).

| Runbook | Ие(лер) | Портал көшірмесі | Дереккөз |
|---------|----------|-------------|--------|
| Шлюз және DNS бастауы | Networking TL, Ops Automation, Docs/DevRel | [`sorafs/gateway-dns-runbook`](./gateway-dns-runbook.md) | `docs/source/sorafs_gateway_dns_design_runbook.md` |
| SoraFS операцияларды орындау кітабы | Docs/DevRel | [`sorafs/operations-playbook`](./operations-playbook.md) | `docs/source/sorafs/operations_playbook.md` |
| Сыйымдылықты салыстыру | Қазынашылық / МКҚК | [`sorafs/capacity-reconciliation`](./capacity-reconciliation.md) | `docs/source/sorafs/runbooks/capacity_reconciliation.md` |
| Тіркеу операцияларын бекіту | Құралдар WG | [`sorafs/pin-registry-ops`](./pin-registry-ops.md) | `docs/source/sorafs/pin_registry_ops.md` |
| Түйін операцияларын бақылау тізімі | Сақтау тобы, SRE | [`sorafs/node-operations`](./node-operations.md) | `docs/source/sorafs/runbooks/sorafs_node_ops.md` |
| Даулар және кері қайтарып алу Runbook | Басқару кеңесі | [`sorafs/dispute-revocation-runbook`](./dispute-revocation-runbook.md) | `docs/source/sorafs/dispute_revocation_runbook.md` |
| Сахналық манифест ойын кітабы | Docs/DevRel | [`sorafs/staging-manifest-playbook`](./staging-manifest-playbook.md) | `docs/source/sorafs/staging_manifest_playbook.md` |
| Тайкай зәкірінің бақылау мүмкіндігі | Медиа платформа WG / DA бағдарламасы / Networking TL | [`sorafs/taikai-anchor-runbook`](./taikai-anchor-runbook.md) | `docs/source/taikai_anchor_monitoring.md` |

## Тексеруді тексеру парағы

- [x] Портал құрастыру сілтемелері осы индекске (бүйірлік тақта жазбасы).
- [x] Әрбір көшірілген runbook шолушыларды сақтау үшін канондық бастапқы жолды тізімдейді
  құжатты тексеру кезінде тураланады.
- [x] DocOps алдын ала қарау конвейер блоктары тізімде берілген жұмыс кітабы жоқ болғанда біріктіріледі
  портал шығысынан.

Болашақ көші-қон (мысалы, жаңа хаос жаттығулары немесе басқару қосымшалары) қосу керек
жолды жоғарыдағы кестеге орнатыңыз және ендірілген DocOps бақылау тізімін жаңартыңыз
`docs/examples/docs_preview_request_template.md`.