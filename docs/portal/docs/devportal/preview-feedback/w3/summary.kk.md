---
lang: kk
direction: ltr
source: docs/portal/docs/devportal/preview-feedback/w3/summary.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 76a4303fa2657476a3f983f1aa5597c9ddb478f670d233b0a7cf4e3791419a72
source_last_modified: "2025-12-29T18:16:35.110857+00:00"
translation_last_reviewed: 2026-02-07
id: preview-feedback-w3-summary
title: W3 beta feedback & status
sidebar_label: W3 summary
description: Live digest for the 2026 beta preview wave (finance, observability, SDK, and ecosystem cohorts).
translator: machine-google-reviewed
---

| Элемент | Мәліметтер |
| --- | --- |
| Толқын | W3 — Бета когорттар (қаржы + операция + SDK серіктесі + экожүйенің қорғаушысы) |
| Шақыру терезесі | 2026‑02‑18 → 2026‑02‑28 |
| Артефакт тегі | `preview-20260218` |
| Трекер мәселесі | `DOCS-SORA-Preview-W3` |
| Қатысушылар | finance-beta-01, observability-ops-02, partner-sdk-03, ecosystem-advocate-04 |

## Ерекшеліктер

1. **Ұқсас дәлелдер құбыры.** `npm run preview:wave -- --wave preview-20260218 --invite-start 2026-02-18 --invite-end 2026-02-28 --report-date 2026-03-01 --notes "Finance/observability beta wave"` толқын сайынғы қорытындыны (`artifacts/docs_portal_preview/preview-20260218-summary.json`), дайджестті (`preview-20260218-digest.md`) жасайды және `docs/portal/src/data/previewFeedbackSummary.json` жаңартады, осылайша басқаруды бір шолушыға сене алады.
2. **Телеметрия + басқаруды қамту.** Төрт шолушының барлығы бақылау сомасымен бекітілген қатынасты мойындады, кері байланыс жіберді және уақытында кері қайтарылды; дайджест кері байланыс мәселелеріне сілтеме жасайды (`docs-preview/20260218` жинағы + `DOCS-SORA-Preview-20260218`) толқын кезінде жиналған Grafana жүгірістерімен қатар.
3. **Портал бетін жабу.** Жаңартылған портал кестесі енді кідіріс және жауап жылдамдығы көрсеткіштері бар жабық W3 толқынын көрсетеді және төмендегі жаңа журнал беті өңделмеген JSON журналын шығармайтын аудиторлар үшін оқиға уақыт кестесін көрсетеді.

## Әрекет элементтері

| ID | Сипаттама | Иесі | Күй |
| --- | --- | --- | --- |
| W3-A1 | Алдын ала қарау дайджестін түсіріп, трекерге тіркеңіз. | Docs/DevRel жетекші | ✅ Аяқталды 2026‑02‑28 |
| W3-A2 | Шақыру/дәлелдерді порталға + жол картасына/күйге айналдыру. | Docs/DevRel жетекші | ✅ Аяқталды 2026‑02‑28 |

## Шығу қорытындысы (28.02.2026)

- 2026‑02‑18 жіберілген шақырулар бірнеше минуттан кейін тіркеледі; 2026-02-28 соңғы телеметриялық тексеруден өткеннен кейін алдын ала қарау рұқсаты қайтарылды.
- Дайджест + қорытынды `artifacts/docs_portal_preview/` астында түсірілген, өңделмеген журнал қайта ойнату үшін `artifacts/docs_portal_preview/feedback_log.json` арқылы бекітілген.
- `DOCS-SORA-Preview-20260218` басқару трекерімен `docs-preview/20260218` бойынша берілген мәселені бақылау; CSP/Try it жазбалары бақылау/қаржы иелеріне бағытталады және дайджесттен сілтеме жасайды.
- Трекер жолы 🈴 Аяқталды деп жаңартылды және порталдың кері байланыс кестесі қалған DOCS-SORA бета-дайындық тапсырмасын орындай отырып, жабық толқынды көрсетеді.