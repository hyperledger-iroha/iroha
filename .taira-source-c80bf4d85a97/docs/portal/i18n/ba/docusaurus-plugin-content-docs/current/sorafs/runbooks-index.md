---
id: runbooks-index
lang: ba
direction: ltr
source: docs/portal/docs/sorafs/runbooks-index.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: Operator Runbooks Index
description: Canonical entry point for the migrated SoraFS operator runbooks.
sidebar_label: Runbook Index
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

> Көҙгөләр хужаһы баш китабы, улар йәшәй аҫтында I18NI0000000012X.
> Һәр яңы I18NT000000000000000000 операциялар етәксеһе бында бәйләнергә тейеш, бер тапҡыр ул баҫылып сыға.
> портал төҙөү.

Был битте ҡулланып, ниндәй runbooks миграцияны тамамлағанын раҫлау өсөн
сығанаҡ юлы, һәм портал күсермәһе шулай рецензенттар туранан-тура теләккә һикерә ала
етәкселек ваҡытында бета-ҡараш.

## Бета алдан ҡарау хост

DocOps тулҡын хәҙер пропагандалау рецензент-раҫланған бета-ҡараш хост .
`https://docs.iroha.tech/`. Операторҙарҙы йәки рецензенттарҙы миграцияға күрһәткәндә
runbook, һылтанма, тип хост исеме, шулай итеп, улар чемпионат-ҡапҡа порталын тормошҡа ашыра
снимок. Нәшриәт/кире кире ҡайтарыу процедуралары йәшәй.
[`devportal/preview-host-exposure`] (../devportal/preview-host-exposure.md).

| Ранбук | Хужа(тар) | Портал күсермәһе | Сығанаҡ |
|--------|-----------|-------------|---------|
| Ҡапҡа & DNS старт | Селтәрҙәр ТЛ, Опс автоматлаштырыу, Док/ДевРел | [`sorafs/gateway-dns-runbook`] (I18NU000000003X) | `docs/source/sorafs_gateway_dns_design_runbook.md` |
| SoraFS операциялар плейбук | Док/ДевРел | [`sorafs/operations-playbook`] (./operations-playbook.md) | `docs/source/sorafs/operations_playbook.md` |
| Ҡыйыулыҡ ярашыу | Ҡаҙна / СРЭ | [`sorafs/capacity-reconciliation`] (./capacity-reconciliation.md) | `docs/source/sorafs/runbooks/capacity_reconciliation.md` |
| Пен реестр опстары | Ҡолғау WG | [`sorafs/pin-registry-ops`] (./pin-registry-ops.md) | `docs/source/sorafs/pin_registry_ops.md` |
| Төйөн операциялары тикшерелгән исемлек | Һаҡлау командаһы, SRE | [`sorafs/node-operations`] (./node-operations.md) | `docs/source/sorafs/runbooks/sorafs_node_ops.md` |
| Бәхәс & тартып алыу runbook | Идара итеү советы | [`sorafs/dispute-revocation-runbook`] (./dispute-revocation-runbook.md) | `docs/source/sorafs/dispute_revocation_runbook.md` |
| Стажлау манифест плейбук | Док/ДевРел | [`sorafs/staging-manifest-playbook`] (./staging-manifest-playbook.md X) | `docs/source/sorafs/staging_manifest_playbook.md` |
| Тайкай якорь күҙәтеүсәнлеге | Медиа платформаһы WG / DA программаһы / Селтәрҙәр ТЛ | [`sorafs/taikai-anchor-runbook`] (./taikai-anchor-runbook.md) | I18NI000000030X |

## Тикшереү исемлеге

- [x] Портал был индексҡа һылтанмалар төҙөү (ян панелендә инеү).
- [x] Һәр миграцияланған runbook-та рецензенттарҙы һаҡлау өсөн канонлы сығанаҡ юлы исемлеге .
  тура килтерелгән ваҡытында doc отзывтар.
- [x] DocOps алдан ҡарау торба үткәргес блоктар берләшә, ҡасан исемлеккә индерелгән runbook юҡ
  порталь сығышынан.

Киләсәктә миграциялар (мәҫәлән, яңы хаос күнекмәләр йәки идара итеү ҡушымталары) өҫтәргә тейеш
рәт өҫтәге таблицаға һәм яңыртыу DocOps тикшерелгән исемлеге 2000 йылда 2000 й.
`docs/examples/docs_preview_request_template.md`.