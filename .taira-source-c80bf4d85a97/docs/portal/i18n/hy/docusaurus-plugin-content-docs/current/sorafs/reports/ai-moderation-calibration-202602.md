---
lang: hy
direction: ltr
source: docs/portal/docs/sorafs/reports/ai-moderation-calibration-202602.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: AI Moderation Calibration Report (2026-02)
summary: Baseline calibration dataset, thresholds, and scoreboard for the first MINFO-1 governance release.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# AI Moderation Calibration Report - 2026 թվականի փետրվար

Այս զեկույցը փաթեթավորում է **MINFO-1**-ի առաջին տրամաչափման արտեֆակտները: Այն
տվյալների բազան, մանիֆեստը և արդյունքների ցուցատախտակը պատրաստվել են 2026-02-05-ին, վերանայվել են կազմակերպության կողմից
Նախարարության խորհուրդը 2026-02-10-ին և խարսխված կառավարման DAG-ում բարձրության վրա
`912044`.

## Տվյալների հավաքածուի մանիֆեստ

- **Տվյալների հավաքածուի հղում՝** `c0956583-355a-43cc-9a60-e3a5d9a0f7d0`
- **Slug:** `ai-moderation-calibration-202602`
- **Գրառումներ.** մանիֆեստ 480, կտոր 12,800, մետատվյալներ 920, աուդիո 160
- **Պիտակի խառնուրդ.** անվտանգ 68%, կասկածելի 19%, էսկալացիա 13%
- **Արտեֆակտ մարսողություն.** `9c4f86a3c099a48d0e3d7cfbf14d22bb9492960c41cba3858f0722519ff612ab`
- **Բաշխում.** `sora://datasets/ministry/ai-moderation/calibration/2026-02.tar.zst`

Ամբողջական մանիֆեստն ապրում է `docs/examples/ai_moderation_calibration_manifest_202602.json`-ում
և պարունակում է կառավարման ստորագրությունը, գումարած վազող հեշը, որը գրավվել է թողարկման պահին
ժամանակ.

## Ցուցատախտակի ամփոփում

Կալիբրացիաներն իրականացվել են opset 17-ով և դետերմինիստական սերմերի խողովակաշարով: Այն
ամբողջական ցուցատախտակ JSON (`docs/examples/ai_moderation_calibration_scorecard_202602.json`)
գրանցում է հեշերը և հեռաչափության մարսումները. ստորև բերված աղյուսակը ընդգծում է ամենաշատը
կարևոր չափումներ.

| Մոդել (ընտանիք) | Բրիեր | ECE | AUROC | Precision@Quarantine | Recall@Escalate |
| --------------- | ----- | --- | ----- | --------------------- | ---------------- |
| ViT-H/14 Անվտանգություն (տեսողություն) | 0,141 | 0,031 | 0,987 | 0,964 | 0,912 |
| LLaVA-1.6 34B Անվտանգություն (մուլտիմոդալ) | 0,118 | 0,028 | 0,978 | 0,942 | 0,904 |
| Ընկալման համույթ (պերցեպտուալ) | 0,162 | 0,047 | 0,953 | 0,883 | 0,861 |

Համակցված չափումներ՝ `Brier = 0.126`, `ECE = 0.034`, `AUROC = 0.982`: Դատավճիռը
չափաբերման պատուհանի վրա բաշխվածությունը եղել է 91.2%, կարանտին 6.8%,
աճել 2.0%-ով, որը համապատասխանում է մանիֆեստում գրանցված քաղաքականության ակնկալիքներին
ամփոփում. Կեղծ դրական հետևանքները մնացել են զրոյական մակարդակում, իսկ դրեյֆի միավորը (7.1%)
ընկել է զգոնության 20% շեմից բավականին ցածր:

## Շեմեր և գրանցում

- `thresholds.quarantine = 0.42`
- `thresholds.escalate = 0.78`
- Կառավարման միջնորդություն՝ `MINFO-2026-02-07`
- Ստորագրված է `ministry-council-seat-03`-ի կողմից `2026-02-10T11:33:12Z`-ով

CI-ն ստորագրված փաթեթը պահեց `artifacts/ministry/ai_moderation/2026-02/`-ում
չափավոր վազող երկուականների կողքին: Մանիֆեստի ամփոփում և ցուցատախտակ
Վերոնշյալ հեշերը պետք է հղում կատարվեն աուդիտի և բողոքարկման ժամանակ:

## վահանակներ և ահազանգեր

Moderation SRE-ները պետք է ներմուծեն Grafana վահանակը
`dashboards/grafana/ministry_moderation_overview.json` և համապատասխանությունը
Prometheus ահազանգման կանոններ `dashboards/alerts/ministry_moderation_rules.yml`-ում
(փորձարկման ծածկույթն ապրում է `dashboards/alerts/tests/ministry_moderation_rules.test.yml`-ի ներքո):
Այս արտեֆակտները ծանուցումներ են արձակում կուլ տալու ախոռների, շեղումների և կարանտինի մասին
հերթերի աճ՝ բավարարելով մոնիտորինգի պահանջները
the [AI Moderation Runner Specification] (../../ministry/ai-moderation-runner.md):