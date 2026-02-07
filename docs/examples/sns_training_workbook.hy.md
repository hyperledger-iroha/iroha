---
lang: hy
direction: ltr
source: docs/examples/sns_training_workbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 7d6965998c392217380a1722e49098f831438e2f4499b9e3258398a66f905a35
source_last_modified: "2025-12-29T18:16:35.080069+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# SNS Training Workbook Կաղապար

Օգտագործեք այս աշխատանքային գրքույկը որպես կանոնական թերթիկ յուրաքանչյուր ուսումնական խմբի համար: Փոխարինել
տեղապահներ (`<...>`) նախքան մասնակիցներին բաշխելը:

## Նիստի մանրամասները
- վերջածանց՝ `<.sora | .nexus | .dao>`
- Ցիկլ՝ `<YYYY-MM>`
- Լեզուն՝ `<ar/es/fr/ja/pt/ru/ur>`
- Օժանդակող՝ `<name>`

## Լաբորատորիա 1 - KPI արտահանում
1. Բացեք պորտալի KPI վահանակը (`docs/portal/docs/sns/kpi-dashboard.md`):
2. Զտել ըստ `<suffix>` վերջածանցի և `<window>` ժամանակային միջակայքի:
3. Արտահանել PDF + CSV snapshots:
4. Արտահանված JSON/PDF-ի SHA-256-ը գրանցեք այստեղ՝ `______________________`:

## Լաբորատորիա 2 — Մանիֆեստ փորված
1. Վերցրեք նմուշի մանիֆեստը `artifacts/sns/training/<suffix>/<cycle>/manifests/<lang>.json`-ից:
2. Վավերացնել `cargo run --bin sns_manifest_check -- --input <file>`-ով:
3. Ստեղծեք լուծիչի կմախք `scripts/sns_zonefile_skeleton.py`-ով:
4. Տեղադրեք տարբերությունների ամփոփագիրը.
   ```
   <git diff output>
   ```

## Լաբորատորիա 3 — Վեճերի մոդելավորում
1. Օգտագործեք Guardian CLI-ը՝ սառեցումը սկսելու համար (գործի ID `<case-id>`):
2. Գրանցեք վեճի հեշը՝ `______________________`:
3. Վերբեռնեք ապացույցների մատյանը `artifacts/sns/training/<suffix>/<cycle>/logs/`:

## Լաբորատորիա 4 — Հավելվածի ավտոմատացում
1. Արտահանեք Grafana վահանակը JSON և պատճենեք այն `artifacts/sns/regulatory/<suffix>/<cycle>/sns_suffix_analytics.json`-ում:
2. Վազել:
   ```bash
   cargo xtask sns-annex \
     --suffix <suffix> \
     --cycle <cycle> \
     --dashboard artifacts/sns/regulatory/<suffix>/<cycle>/sns_suffix_analytics.json \
     --dashboard-artifact artifacts/sns/regulatory/<suffix>/<cycle>/sns_suffix_analytics.json \
     --output docs/source/sns/reports/<suffix>/<cycle>.md \
     --regulatory-entry docs/source/sns/regulatory/<memo>.md \
     --portal-entry docs/portal/docs/sns/regulatory/<memo-id>.md
   ```
3. Տեղադրեք հավելվածի ուղին + SHA-256 ելք՝ `________________________________`:

## Հետադարձ գրառումներ
- Ի՞նչն էր անհասկանալի:
- Ո՞ր լաբորատորիաներն են աշխատել ժամանակի ընթացքում:
- Գործիքավորման վրիպակներ նկատվե՞լ են:

Ավարտված աշխատանքային գրքույկները վերադարձրեք վարողին. տակ են պատկանում
`artifacts/sns/training/<suffix>/<cycle>/workbooks/`.