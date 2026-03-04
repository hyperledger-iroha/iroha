---
lang: kk
direction: ltr
source: docs/portal/docs/sns/onboarding-kit.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: SNS metrics & onboarding kit
description: Dashboard, pricing, and automation artifacts referenced by roadmap item SN-8.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# SNS метрикалары және қосу жинағы

Жол картасының **SN-8** тармағы екі уәдені біріктіреді:

1. Тіркеулерді, жаңартуларды, ARPU, дауларды және
   `.sora`, `.nexus` және `.dao` үшін терезелерді мұздату.
2. Тіркеушілер мен басқарушылар DNS, баға және
   Кез келген жұрнақ іске қосылмас бұрын API интерфейстері тұрақты.

Бұл бет бастапқы нұсқаны көрсетеді
[`docs/source/sns/onboarding_kit.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sns/onboarding_kit.md)
сондықтан сыртқы шолушылар бірдей процедураны орындай алады.

## 1. Метрикалық жинақ

### Grafana бақылау тақтасы және порталды ендіру

- `dashboards/grafana/sns_suffix_analytics.json` Grafana (немесе басқа) ішіне импорттау
  аналитикалық хост) стандартты API арқылы:

```bash
curl -H "Content-Type: application/json" \
     -H "Authorization: Bearer ${GRAFANA_TOKEN}" \
     -X POST https://grafana.sora.net/api/dashboards/db \
     --data-binary @dashboards/grafana/sns_suffix_analytics.json
```

- Дәл сол JSON осы портал бетінің iframe мүмкіндігін береді (**SNS KPI бақылау тақтасын** қараңыз).
  Бақылау тақтасын соққан сайын жүгіріңіз
  `npm run build && npm run serve-verified-preview` ішіндегі `docs/portal`
  Grafana және ендірілген синхрондауды растаңыз.

### Панельдер мен дәлелдер

| Панель | Көрсеткіштер | Басқару дәлелдері |
|-------|---------|---------------------|
| Тіркеулер және жаңартулар | `sns_registrar_status_total` (сәттілік + жаңарту шешуші белгілері) | Әр жұрнақ өткізу қабілеті + SLA бақылауы. |
| ARPU / таза бірлік | `sns_bulk_release_payment_net_units`, `sns_bulk_release_payment_gross_units` | Қаржы тіркеуші манифесттерін кіріске сәйкестендіруі мүмкін. |
| Даулар және қатулар | `guardian_freeze_active`, `sns_dispute_outcome_total`, `sns_governance_activation_total` | Белсенді қатып қалуларды, арбитраждың каденциясын және қамқоршының жұмыс жүктемесін көрсетеді. |
| SLA/қате бағалары | `torii_request_duration_seconds`, `sns_registrar_status_total{status="error"}` | Тұтынушыларға әсер етпес бұрын API регрессияларын бөлектейді. |
| Жаппай манифест трекері | `sns_bulk_release_manifest_total`, `manifest_id` белгілері бар төлем көрсеткіштері | CSV тамшыларын есеп айырысу билеттеріне қосады. |

Ай сайынғы KPI кезінде Grafana (немесе ендірілген iframe) ішінен PDF/CSV экспорттау
қарастырыңыз және оны сәйкес қосымша жазбаға тіркеңіз
`docs/source/sns/regulatory/<suffix>/YYYY-MM.md`. Стюардтар сонымен қатар SHA-256-ны басып алады
`docs/source/sns/reports/` бойынша экспортталған буманың (мысалы,
`steward_scorecard_2026q1.md`) аудиттер дәлелдер жолын қайталай алады.

### Қосымшаны автоматтандыру

Қосымша файлдарды тікелей бақылау тақтасының экспортынан жасаңыз, осылайша шолушылар келесі ақпаратты алады
дәйекті дайджест:

```bash
cargo xtask sns-annex \
  --suffix .sora \
  --cycle 2026-03 \
  --dashboard dashboards/grafana/sns_suffix_analytics.json \
  --dashboard-artifact artifacts/sns/regulatory/.sora/2026-03/sns_suffix_analytics.json \
  --output docs/source/sns/reports/.sora/2026-03.md \
  --regulatory-entry docs/source/sns/regulatory/eu-dsa/2026-03.md \
  --portal-entry docs/portal/docs/sns/regulatory/eu-dsa-2026-03.md
```

- Көмекші экспортты хэштейді, UID/тегтер/панельдер санын жазып алады және
  `docs/source/sns/reports/.<suffix>/<cycle>.md` астында Markdown қосымшасы (қараңыз
  `.sora/2026-03` үлгісі осы құжатпен бірге жасалған).
- `--dashboard-artifact` экспортты көшіреді
  `artifacts/sns/regulatory/<suffix>/<cycle>/` сондықтан қосымша сілтеме жасайды
  канондық дәлелдер жолы; көрсету қажет болғанда ғана `--dashboard-label` пайдаланыңыз
  диапазоннан тыс мұрағатта.
- `--regulatory-entry` басқару хаттамасын көрсетеді. Көмекші кірістіреді (немесе
  ауыстырады) қосымша жолын, бақылау тақтасын жазатын `KPI Dashboard Annex` блогын
  артефакт, дайджест және уақыт белгісі қайта іске қосылғаннан кейін дәлелдер синхрондалады.
- `--portal-entry` Docusaurus көшірмесін сақтайды (`docs/portal/docs/sns/regulatory/*.md`)
  шолушылардың бөлек қосымша жиынтықтарын қолмен ажыратудың қажеті болмайтындай реттелген.
- `--regulatory-entry`/`--portal-entry` өткізіп жіберсеңіз, жасалған файлды мына жерге тіркеңіз
  жазбаларды қолмен және әлі де Grafana ішінен түсірілген PDF/CSV суреттерін жүктеп салыңыз.
- Қайталанатын экспорттар үшін жұрнақ/цикл жұптарын тізімдеңіз
  `docs/source/sns/regulatory/annex_jobs.json` және іске қосыңыз
  `python3 scripts/run_sns_annex_jobs.py --verbose`. Көмекші әр кірген сайын жүреді,
  бақылау тақтасының экспортын көшіреді (әдепкі бойынша `dashboards/grafana/sns_suffix_analytics.json`
  анықталмаған кезде) және әрбір нормативтік құжаттың ішіндегі қосымша блогын жаңартады (және,
  қол жетімді болса, портал) бір жолдамадағы жазба.
- Тапсырмалар тізімінің сұрыпталған/қайталанбаған күйінде қалуын, әрбір жадта сәйкес `sns-annex` маркерін және қосымша штрихтың бар екенін дәлелдеу үшін `python3 scripts/check_sns_annex_schedule.py --jobs docs/source/sns/regulatory/annex_jobs.json --regulatory-root docs/source/sns/regulatory --report-root docs/source/sns/reports` (немесе `make check-sns-annex`) іске қосыңыз. Көмекші басқару пакеттерінде пайдаланылатын тіл/хэш қорытындыларының жанына `artifacts/sns/annex_schedule_summary.json` жазады.
Бұл қолмен көшіру/қою қадамдарын жояды және SN-8 қосымшасының дәлелдерін біркелкі сақтайды
күзет кестесі, маркер және CI-дегі локализация дрейфі.

## 2. Борттық жинақтың құрамдас бөліктері

### Суффикс сымдары

- Тіркеу схемасы + селектор ережелері:
  [`docs/source/sns/registry_schema.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sns/registry_schema.md)
  және [`docs/source/sns/local_to_global_toolkit.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sns/local_to_global_toolkit.md).
- DNS скелет көмекшісі:
  [`scripts/sns_zonefile_skeleton.py`](https://github.com/hyperledger-iroha/iroha/blob/master/scripts/sns_zonefile_skeleton.py)
  репетиция ағынымен түсірілген
  [шлюз/DNS runbook](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs_gateway_dns_owner_runbook.md).
- Әрбір тіркеушіні іске қосу үшін астына қысқа ескертпе жіберіңіз
  `docs/source/sns/reports/` селектор үлгілерін, GAR дәлелдеулерін және DNS хэштерін қорытындылайды.

### Бағаны анықтау парағы

| Белгі ұзындығы | Базалық алым (АҚШ доллары эквиві) |
|-------------|--------------------|
| 3 | $240 |
| 4 | $90 |
| 5 | $30 |
| 6–9 | $12 |
| 10+ | $8 |

Суффикс коэффициенттері: `.sora` = 1,0×, `.nexus` = 0,8×, `.dao` = 1,3×.  
Мерзімді көбейткіштер: 2‑жыл −5%, 5‑жыл −12%; жеңілдік терезесі = 30 күн, өтеу
= 60 күн (20% комиссия, минимум $5, максимум $200). Келісілген ауытқуларды жазыңыз
тіркеуші билеті.

### Премиум аукциондар және жаңартулар

1. **Премиум пул** — мөрленген өтінім/анықтау (SN-3). Өтінімдерді қадағалаңыз
   `sns_premium_commit_total` және манифестті астында жариялаңыз
   `docs/source/sns/reports/`.
2. **Нидерланды қайта ашу** — жеңілдік + өтеу мерзімі аяқталғаннан кейін 7 күндік голландтық сатылымды бастаңыз
   10× кезінде бұл тәулігіне 15% ыдырайды. Белгі `manifest_id` арқылы көрсетіледі, сондықтан
   бақылау тақтасы ілгерілей алады.
3. **Жаңартулар** — `sns_registrar_status_total{resolver="renewal"}` мониторы және
   автожаңарту тексеру парағын алу (хабарландырулар, SLA, қосымша төлем рельстері)
   тіркеуші билетінің ішінде.

### Әзірлеуші API және автоматтандыру

- API келісімшарттары: [`docs/source/sns/registrar_api.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sns/registrar_api.md).
- Жаппай көмекші және CSV схемасы:
  [`docs/source/sns/bulk_onboarding_toolkit.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sns/bulk_onboarding_toolkit.md).
- Мысал пәрмені:

```bash
python3 scripts/sns_bulk_onboard.py registrations.csv \
  --ndjson artifacts/sns/releases/2026q2/requests.ndjson \
  --submission-log artifacts/sns/releases/2026q2/submissions.log \
  --submit-torii-url https://torii.sora.net \
  --submit-token-file ~/.config/sora/tokens/registrar.token
```

KPI бақылау тақтасының сүзгісіне манифест идентификаторын (`--submission-log` шығысы) қосыңыз
сондықтан қаржы әр шығарылымдағы кіріс панелін салыстыра алады.

### Дәлелдер жинағы

1. Контактілері, суффикс ауқымы және төлем рельстері бар тіркеуші билеті.
2. DNS/resolver дәлелдері (аймақтық қаңқалар + GAR дәлелдері).
3. Бағалау жұмыс парағы + басқару бекіткен кез келген қайта анықтау.
4. API/CLI түтін сынағы артефактілері (`curl` үлгілері, CLI транскрипттері).
5. KPI бақылау тақтасының скриншоты + CSV экспорты, ай сайынғы қосымшаға тіркелген.

## 3. Іске қосу бақылау парағын

| Қадам | Иесі | Артефакт |
|------|-------|----------|
| Бақылау тақтасы импортталды | Өнім талдаулары | Grafana API жауабы + бақылау тақтасының UID |
| Портал ендірілгені расталды | Docs/DevRel | `npm run build` журналдары + алдын ала қарау скриншоты |
| DNS репетициясы аяқталды | Networking/Ops | `sns_zonefile_skeleton.py` шығыстары + runbook журналы |
| Тіркеуші автоматтандыру құрғақ жұмыс | Тіркеуші Eng | `sns_bulk_onboard.py` жіберулер журналы |
| Берілген басқару дәлелдері | Басқару кеңесі | Қосымша сілтеме + Экспортталған бақылау тақтасының SHA-256 |

Тіркеушіні немесе суффиксті белсендірмес бұрын тексеру тізімін толтырыңыз. Қол қойған
bundle SN-8 жол картасының қақпасын тазартады және аудиторларға қашан бір анықтама береді
нарықтың іске қосылуын қарастыру.