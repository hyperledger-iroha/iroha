---
lang: ba
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

# Референдум Пакет эш ағымы (МИНФО-4)

Юл картаһы пункты **МИНФО-4 — панелендә тикшерелгән & референдум синтезаторы** хәҙер
тормошҡа ашырылған яңы `ReferendumPacketV1` Norito схемаһы плюс CLI ярҙамсылары
түбәндә һүрәтләнгән. Эш ағымы өйөмдәре һәр артефакт өсөн кәрәкле сәйәсәт-плейсы
тауыш бирә бер JSON документы шулай идара итеү, аудиторҙар, һәм асыҡлыҡ
порталдары дәлилдәрҙе детерминистик рәүештә ҡабатлай ала.

##

1. **Көндәлек тәҡдим** — шул уҡ JSON `cargo xtask ministry-agenda validate` өсөн ҡулланылған.
2. **Ирекле трусик** — курированный мәғлүмәттәр йыйылмаһы етештерелгәндән һуң линтинг аша .
   `cargo xtask ministry-transparency volunteer-validate`.
3. **AI самаһы манифест** — идара итеү өсөн ҡул ҡуйылған `ModerationReproManifestV1`.
4. **Сортировка резюме** — детерминистик артефакт
   `cargo xtask ministry-agenda sortition`. JSON эйәреп килә
   [`PolicyJurySortitionV1`] (./policy_jury_ballots.md) шулай идара итеү ала
   тергеҙергә POP снимок deragest һәм көтөү исемлеге/файловер проводка.
5. **Импактик отчет ** — хеш-ғаилә/отчет аша генерацияланған
   `cargo xtask ministry-agenda impact`.

## CLI ҡулланыу

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

`packet` подкомманд нейтраль-йомғаҡлау синтезаторы (MINFO-4a) эшләй, ҡабаттан файҙалана.
ғәмәлдәге ирекмәндәр ҡоролмалары, һәм сығышты байыта:

- `ReferendumSortitionEvidence` — алгоритм, орлоҡ, һәм ростер һеңдереүҙең .
  сортировка артефакт.
- `ReferendumPanelist[]` — һәр береһе һайланған совет ағзаһы плюс Меркл иҫбатлау
  кәрәк, уларҙы аудит нигеҙендә.
- `ReferendumImpactSummary` — 2012 йылдан алып хэш-ғаилә һәм конфликт исемлеге.
  йоғонто отчеты.

Ҡулланыу `--summary-out` ҡасан һеҙгә кәрәк, автономлы `ReviewPanelSummaryV1` .
файл; юғиһә пакет `review_summary` буйынса резюме индереү.

## Сығыш структураһы

`ReferendumPacketV1` 1990 йылда йәшәй.
`crates/iroha_data_model/src/ministry/mod.rs` һәм SDKs буйынса мөмкин.
Төп бүлектәрҙә:

- `proposal` — `AgendaProposalV1` объекты.
- `review_summary` — MINFO-4a тарафынан сығарылған баланслы резюме.
- `sortition` / `panelists` — ултырған совет өсөн ҡабатланған дәлилдәр.
- `impact_summary` — дубликаты/сәйәсәт конфликты дәлилдәре бер хеш-ғаилә.

Ҡарағыҙ `docs/examples/ministry/referendum_packet_example.json` өсөн тулы өлгө.
Беркетелгән генерацияланған пакет һәр референдум досье менән бергә ҡул ҡуйылған AI .
асыҡ һәм асыҡлыҡ артефакттары өҫтөнлөклө бүлектәре менән һылтанма яһай.