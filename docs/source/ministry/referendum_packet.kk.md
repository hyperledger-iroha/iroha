---
lang: kk
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

# Референдум пакетінің жұмыс процесі (MINFO-4)

Жол картасының тармағы **MINFO-4 — Шолу панелінде және референдум синтезаторында** қазір
жаңа `ReferendumPacketV1` Norito схемасымен және CLI көмекшілерімен орындалды
төменде сипатталған. Жұмыс процесі қазылар алқасына қажетті әрбір артефакттарды біріктіреді
басқару, аудиторлар және ашықтық үшін бір JSON құжатында дауыс береді
порталдар дәлелдемелерді анықтаушы түрде қайталай алады.

## Кіріс

1. **Күн тәртібі ұсынысы** — `cargo xtask ministry-agenda validate` үшін бірдей JSON пайдаланылады.
2. **Еріктілер туралы қысқаша ақпарат** — линтингтен кейін дайындалған кураторлық деректер жинағы
   `cargo xtask ministry-transparency volunteer-validate`.
3. **AI модерациясының манифесті** — басқару қол қойылған `ModerationReproManifestV1`.
4. **Сұрыптау қорытындысы** — шығаратын детерминирленген артефакт
   `cargo xtask ministry-agenda sortition`. JSON орындалады
   [`PolicyJurySortitionV1`](./policy_jury_ballots.md) сондықтан басқару
   POP снапшот дайджестін және күту тізімі/ауыспалы сымдарды қайта жасаңыз.
5. **Әсер туралы есеп** — хэш-отбасы/есеп арқылы жасалған
   `cargo xtask ministry-agenda impact`.

## CLI пайдалану

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

`packet` ішкі пәрмені бейтарап жиынтық синтезаторды (MINFO-4a) іске қосады, қайта пайдаланады
Қолданыстағы волонтерлік қондырғылар және өнімді келесілермен байытады:

- `ReferendumSortitionEvidence` — алгоритм, тұқым және тізімдік дайджесттер
  сұрыптау артефакті.
- `ReferendumPanelist[]` — әрбір таңдалған кеңес мүшесі және Merkle дәлелі
  олардың ұтысын тексеру қажет.
- `ReferendumImpactSummary` — отбасылық хэштердің жиынтықтары және қайшылықтар тізімі
  әсер ету туралы есеп.

Оқшауланған `ReviewPanelSummaryV1` қажет болғанда `--summary-out` пайдаланыңыз.
файл; әйтпесе пакет қорытындыны `review_summary` астына енгізеді.

## Шығару құрылымы

`ReferendumPacketV1` тұрады
`crates/iroha_data_model/src/ministry/mod.rs` және SDK файлдарында қол жетімді.
Негізгі бөлімдерге мыналар кіреді:

- `proposal` — бастапқы `AgendaProposalV1` нысаны.
- `review_summary` — MINFO-4a шығаратын теңдестірілген қорытынды.
- `sortition` / `panelists` — отырған кеңес үшін қайталанатын дәлелдер.
- `impact_summary` — әр хэш жанұясына қайталанатын/саясат қайшылықтары дәлелі.

Толық үлгіні `docs/examples/ministry/referendum_packet_example.json` қараңыз.
Жасалған пакетті референдумның әрбір құжатына қол қойылған AI-мен бірге тіркеңіз
манифест және мөлдірлік артефактілері маңызды сәттер бөліміне сілтеме жасайды.