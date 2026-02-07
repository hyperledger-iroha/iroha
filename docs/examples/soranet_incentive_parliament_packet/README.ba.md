---
lang: ba
direction: ltr
source: docs/examples/soranet_incentive_parliament_packet/README.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 29808ff4511c668963b3c8c4326cca49e033bea91b1b9aa56968ef494648f18e
source_last_modified: "2026-01-22T14:35:37.885694+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# СорНет реле стимул парламент пакеты

Был өйөмдә Сора парламенты талап иткән артефактты раҫлай.
автоматик реле түләүҙәре (SNNet-7):

- I18NI000000002X - I18NT000000000000000-се сериялы бүләк двигателе конфигурацияһы, әҙер
  `iroha app sorafs incentives service init` тарафынан ингестироваться. 1990 й.
  `budget_approval_id` матчтар хеш идара итеү минуттарында исемлеккә индерелгән.
- I18NI000000005X - реплей ҡулланған бенефициар һәм облигациялар картаһы
  жгут (`shadow-run`) һәм етештереү демоны.
- `economic_analysis.md` - 2025-10 йылдарҙағы ғәҙеллек резюмеһы -> 2025-11.
  күләгә моделләштереү.
- `rollback_plan.md` - автоматик түләүҙәрҙе өҙөү өсөн оператив плейбук.
- Ярҙамсы артефакттар: I18NI000000009X, .
  `dashboards/grafana/soranet_incentives.json`,
  `dashboards/alerts/soranet_incentives_rules.yml`.

## Бөтөнлөк тикшерә

```bash
shasum -a 256 docs/examples/soranet_incentive_parliament_packet/* \
  docs/examples/soranet_incentive_shadow_run.json \
  docs/examples/soranet_incentive_shadow_run.sig
```

Парламент минуттарында теркәлгән ҡиммәттәр менән дарыуҙарҙы сағыштырығыҙ. Тикшерергә
күләгә-йүгереү ҡултамғаһы, нисек һүрәтләнгән .
`docs/source/soranet/reports/incentive_shadow_run.md`.

## Пакетты яңыртыу

1. Яңыртыу I18NI000000013X ҡасан да булһа бүләк ауырлыҡтары, база түләү, йәки
   раҫлау хеш үҙгәрештәр.
2. 60 көнлөк күләгә моделләштереүен яңынан эшләтеү, яңыртыу I18NI000000014X менән .
   яңы табыштар, һәм JSON + айырым ҡултамға парын ҡылған.
3. Яңыртылған өйөмөн Парламентҡа обсерватория приборҙар таҡтаһы менән бергә тәҡдим итеү
   экспорты яңыртылған раҫлау эҙләгәндә.