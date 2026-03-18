---
lang: kk
direction: ltr
source: docs/portal/docs/sns/local-to-global-toolkit.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: Local → Global Address Toolkit
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

Бұл бет [`docs/source/sns/local_to_global_toolkit.md`](../../../source/sns/local_to_global_toolkit.md) айналады
монореподан. Ол жол картасының **ADDR-5c** элементі талап ететін CLI көмекшілері мен жұмыс кітаптарын жинақтайды.

## Шолу

- `scripts/address_local_toolkit.sh` мыналарды шығару үшін `iroha` CLI орап алады:
  - `audit.json` — `iroha tools address audit --format json` ішінен құрылымдық шығыс.
  - `normalized.txt` — әрбір жергілікті домен селекторы үшін түрлендірілген таңдаулы I105 / екінші ең жақсы қысылған (`sora`) литералдар.
- Скриптті мекенжайды қабылдау бақылау тақтасымен жұптаңыз (`dashboards/grafana/address_ingest.json`)
  және Local-8 / дәлелдеу үшін Alertmanager ережелері (`dashboards/alerts/address_ingest_rules.yml`)
  Жергілікті-12 кесу қауіпсіз. Local-8 және Local-12 соқтығысқан панельдерді, плюс
  `AddressLocal8Resurgence`, `AddressLocal12Collision` және `AddressInvalidRatioSlo` ескертулері
  айқын өзгерістерге ықпал ету.
- [Мекенжайды көрсету нұсқауларына](address-display-guidelines.md) және
  UX және оқиғаға жауап беру контекстіне арналған [Мекенжай манифестінің жұмыс кітабы](../../../source/runbooks/address_manifest_ops.md).

## Қолдану

```bash
scripts/address_local_toolkit.sh \
  --input fixtures/address/local_digest_examples.txt \
  --output-dir artifacts/address_migration \
  --network-prefix 753 \
  --format i105
```

Опциялар:

- I105 орнына `i105` шығысы үшін `--format i105`.
- Жалаң литералдарды шығару үшін `domainless output (default)`.
- түрлендіру қадамын өткізіп жіберу үшін `--audit-only`.
- `--allow-errors` қате пішімделген жолдар пайда болған кезде сканерлеуді жалғастыру үшін (CLI әрекетіне сәйкес келеді).

Сценарий орындалудың соңында артефакт жолдарын жазады. Екі файлды да тіркеңіз
нөлді дәлелдейтін Grafana скриншотымен бірге өзгертуді басқару билеті
≥30 күн ішінде жергілікті-8 анықтау және нөлдік Жергілікті-12 соқтығысуы.

## CI интеграциясы

1. Сценарийді арнайы жұмыста іске қосыңыз және оның нәтижелерін жүктеңіз.
2. `audit.json` жергілікті селекторларды (`domain.kind = local12`) есептегенде, блок біріктіріледі.
   әдепкі `true` мәні бойынша (тек `false` үшін әзірлеу/сынақ кластерлерінде қайта анықтау
   регрессияларды диагностикалау) және қосыңыз
   `iroha tools address normalize` - CI, сондықтан регрессия
   әрекеттер өндіріске жеткенге дейін сәтсіздікке ұшырайды.

Қосымша мәліметтерді, үлгі дәлелдемелерді тексеру тізімдерін және тұтынушыларға қысқарту туралы хабарлағанда қайта пайдалануға болатын шығарылым жазбасының үзіндісін алу үшін бастапқы құжатты қараңыз.