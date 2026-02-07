---
lang: hy
direction: ltr
source: docs/examples/sns_training_template.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: dd9da5045f5f40dbc31837145ad13bf79b4d751b0803c0b6d69bab49885ed1b4
source_last_modified: "2025-12-29T18:16:35.079313+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# SNS Training Slide Կաղապար

Markdown-ի այս ուրվագիծը արտացոլում է այն սլայդները, որոնց պետք է հարմարվեն վարողները
նրանց լեզվական խմբերը: Պատճենեք այս բաժինները Keynote/PowerPoint/Google-ում
Անհրաժեշտության դեպքում սահում և տեղայնացնում է կետերը, սքրինշոթները և դիագրամները:

## Վերնագրի սլայդ
- Ծրագիր՝ «Sora Name Service inboarding»
- Ենթագրեր. նշեք վերջածանց + ցիկլը (օրինակ՝ `.sora — 2026‑03`)
- Ներկայացնողներ + պատկանելություններ

## KPI կողմնորոշում
- Սքրինշոթ կամ ներկառուցված `docs/portal/docs/sns/kpi-dashboard.md`
- Բլետների ցանկ, որը բացատրում է վերջածանցների ֆիլտրերը, ARPU աղյուսակը, սառեցման հետքերը
- Զանգեր PDF/CSV արտահանման համար

## Դրսեւորված կյանքի ցիկլը
- Դիագրամ՝ գրանցող → Torii → կառավարում → DNS/gateway
- `docs/source/sns/registry_schema.md` հղումով քայլեր
- Օրինակ մանիֆեստի հատված ծանոթագրություններով

## Վեճերի և սառեցման վարժանքներ
- Խնամակալի միջամտության հոսքի դիագրամ
- Ստուգաթերթի հղում՝ `docs/source/sns/governance_playbook.md`
- Սառեցման տոմսերի ժամանակացույցի օրինակ

## Հավելվածի գրավում
- Հրամանի հատված, որը ցույց է տալիս `cargo xtask sns-annex ... --portal-entry ...`
- Հիշեցում Grafana JSON արխիվին `artifacts/sns/regulatory/<suffix>/<cycle>/` տակ
- Հղում դեպի `docs/source/sns/reports/.<suffix>/<cycle>.md`

## Հաջորդ քայլերը
- Մարզման հետադարձ կապի հղում (տես `docs/examples/sns_training_eval_template.md`)
- Slack/Matrix ալիքի բռնակներ
- Առաջիկա կարևոր ամսաթվերը