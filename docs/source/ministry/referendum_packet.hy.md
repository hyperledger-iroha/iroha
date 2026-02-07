---
lang: hy
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

# Հանրաքվեի փաթեթի աշխատանքային հոսք (MINFO-4)

Ճանապարհային քարտեզի կետը **MINFO-4 — Վերանայման վահանակում և հանրաքվեի սինթեզատոր** այժմ է
իրականացվել է նոր `ReferendumPacketV1` Norito սխեմայի և CLI օգնականների կողմից
նկարագրված է ստորև: Աշխատանքային հոսքը միավորում է քաղաքական ժյուրիի համար պահանջվող յուրաքանչյուր արտեֆակտ
քվեարկում է մեկ JSON փաստաթղթի համար՝ կառավարում, աուդիտորներ և թափանցիկություն
պորտալները կարող են դետերմինիստորեն վերարտադրել ապացույցները:

## Մուտքեր

1. **Օրակարգի առաջարկ** — նույն JSON-ն օգտագործվում է `cargo xtask ministry-agenda validate`-ի համար:
2. **Կամավորների համառոտագրեր** — ընտրված տվյալների բազան, որն արտադրվում է միջով ծածկելուց հետո
   `cargo xtask ministry-transparency volunteer-validate`.
3. **AI մոդերացիայի մանիֆեստ** — կառավարման ստորագրված `ModerationReproManifestV1`:
4. **Տեսակավորման ամփոփագիր** — դետերմինիստական արտեֆակտ, որն արտանետվել է
   `cargo xtask ministry-agenda sortition`. JSON-ը հետևում է
   [`PolicyJurySortitionV1`](./policy_jury_ballots.md), որպեսզի կառավարումը կարողանա
   վերարտադրել POP ակնթարթային ակնարկը և սպասման ցուցակը/անջատող լարերը:
5. **Ազդեցության հաշվետվություն** — հաշ-ընտանիք/հաշվետվություն, որը ստեղծվել է միջոցով
   `cargo xtask ministry-agenda impact`.

## CLI-ի օգտագործում

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

`packet` ենթահրամանը գործարկում է չեզոք ամփոփիչ սինթեզատորը (MINFO-4a), նորից օգտագործում
առկա կամավորական հարմարանքները և հարստացնում է արդյունքը.

- `ReferendumSortitionEvidence` — ալգորիթմը, սերմերը և ցուցակը մարսվում են
  տեսակավորման արտեֆակտ.
- `ReferendumPanelist[]` — խորհրդի յուրաքանչյուր ընտրված անդամ գումարած Մերկլի ապացույցը
  անհրաժեշտ է ստուգել նրանց խաղարկությունը:
- `ReferendumImpactSummary` — յուրաքանչյուր հեշ-ընտանիքի գումարները և կոնֆլիկտների ցուցակները
  ազդեցության հաշվետվությունը։

Օգտագործեք `--summary-out`, երբ ձեզ դեռ պետք է ինքնուրույն `ReviewPanelSummaryV1`
ֆայլ; հակառակ դեպքում փաթեթն ամփոփում է `review_summary` տակ:

## Ելքի կառուցվածքը

`ReferendumPacketV1` ապրում է
`crates/iroha_data_model/src/ministry/mod.rs` և հասանելի է SDK-ներում:
Հիմնական բաժինները ներառում են.

- `proposal` — բնօրինակ `AgendaProposalV1` օբյեկտը:
- `review_summary` — MINFO-4a-ի կողմից թողարկված հավասարակշռված ամփոփագիր:
- `sortition` / `panelists` — վերարտադրվող ապացույցներ նստած խորհրդի համար:
- `impact_summary` — կրկնօրինակ/քաղաքականության կոնֆլիկտի վկայություն յուրաքանչյուր հեշ ընտանիքի համար:

Ամբողջական նմուշի համար տես `docs/examples/ministry/referendum_packet_example.json`:
Կցեք ստեղծված փաթեթը յուրաքանչյուր հանրաքվեի դոսյեին ստորագրված AI-ի հետ միասին
մանիֆեստի և թափանցիկության արտեֆակտներ, որոնք վկայակոչված են կարևորագույն հատվածում: