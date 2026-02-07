---
lang: hy
direction: ltr
source: docs/examples/soranet_incentive_parliament_packet/README.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 29808ff4511c668963b3c8c4326cca49e033bea91b1b9aa56968ef494648f18e
source_last_modified: "2026-01-22T14:35:37.885694+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# SoraNet ռելեի խրախուսական խորհրդարանի փաթեթ

Այս փաթեթը ներառում է արտեֆակտները, որոնք պահանջվում են Սորա խորհրդարանի կողմից հաստատելու համար
ավտոմատ ռելեի վճարումներ (SNNet-7):

- `reward_config.json` - Norito-շարունակվող պարգևատրման շարժիչի կոնֆիգուրացիա, պատրաստ
  պետք է կլանվի `iroha app sorafs incentives service init`-ով: Այն
  `budget_approval_id`-ը համապատասխանում է կառավարման արձանագրության մեջ նշված հեշին:
- `shadow_daemon.json` - շահառու և պարտատոմսերի քարտեզագրում, որը սպառվում է կրկնապատկերով
  զենք ու զրահ (`shadow-run`) և արտադրական դեմոն:
- `economic_analysis.md` - արդարության ամփոփում 2025-10 թվականների համար -> 2025-11
  ստվերային մոդելավորում.
- `rollback_plan.md` - գործառնական խաղագիրք՝ ավտոմատ վճարումները անջատելու համար:
- Աջակցող արտեֆակտներ՝ `docs/examples/soranet_incentive_shadow_run.{json,pub,sig}`,
  `dashboards/grafana/soranet_incentives.json`,
  `dashboards/alerts/soranet_incentives_rules.yml`.

## Ամբողջականության ստուգումներ

```bash
shasum -a 256 docs/examples/soranet_incentive_parliament_packet/* \
  docs/examples/soranet_incentive_shadow_run.json \
  docs/examples/soranet_incentive_shadow_run.sig
```

Համեմատեք ամփոփումները խորհրդարանի արձանագրություններում գրանցված արժեքների հետ։ Ստուգեք
ստվերային ստորագրությունը, ինչպես նկարագրված է
`docs/source/soranet/reports/incentive_shadow_run.md`.

## Փաթեթի թարմացում

1. Թարմացրեք `reward_config.json`-ը, երբ պարգևը կշռում է, բազային վճարում կամ
   հաստատման հեշի փոփոխություն:
2. Վերագործարկեք 60-օրյա ստվերային մոդելավորումը, թարմացրեք `economic_analysis.md`-ը
   նոր բացահայտումներ և կատարեք JSON + անջատված ստորագրության զույգը:
3. Ներկայացրե՛ք թարմացված փաթեթը Խորհրդարանին աստղադիտարանի վահանակի հետ միասին
   արտահանում, երբ պահանջում են նորացված հաստատում: