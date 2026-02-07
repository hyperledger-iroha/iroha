---
id: training-collateral
lang: kk
direction: ltr
source: docs/portal/docs/sns/training-collateral.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: SNS Training Collateral
description: Curriculum, localization workflow, and annex evidence capture required by SN-8.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

> Айналар `docs/source/sns/training_collateral.md`. Брифинг кезінде осы бетті пайдаланыңыз
> әр жұрнақты іске қосу алдында тіркеуші, DNS, қамқоршы және қаржы топтары.

## 1. Оқу жоспарының суреті

| Трек | Мақсаттар | Алдын ала оқулар |
|-------|------------|-----------|
| Тіркеуші операциялары | Манифесттерді жіберіңіз, KPI бақылау тақталарын бақылаңыз, қателерді арттырыңыз. | `sns/onboarding-kit`, `sns/kpi-dashboard`. |
| DNS және шлюз | Шешуші қаңқаларды қолданыңыз, қатып қалуды/қайтаруды қайталаңыз. | `sorafs/gateway-dns-runbook`, тікелей режим саясат үлгілері. |
| Қамқоршылар және кеңес | Дауларды орындаңыз, басқару қосымшаларын, журнал қосымшаларын жаңартыңыз. | `sns/governance-playbook`, басқарушы көрсеткіштер жүйесі. |
| Қаржы және аналитика | ARPU/көлемдік көрсеткіштерді түсіріңіз, қосымша жинақтарын жариялаңыз. | `finance/settlement-iso-mapping`, KPI бақылау тақтасы JSON. |

### Модуль ағыны

1. **M1 — KPI бағдары (30 мин):** Жүру жұрнағы сүзгілері, экспорттар және қашқын
   есептегіштерді мұздату. Жеткізу мүмкіндігі: SHA-256 дайджесті бар PDF/CSV суреттері.
2. **M2 — Манифесттің өмірлік циклі (45 мин):** Тіркеуші манифесттерін құру және тексеру,
   `scripts/sns_zonefile_skeleton.py` арқылы шешуші қаңқаларды жасаңыз. Жеткізу мүмкіндігі:
   git diff қаңқа + GAR дәлелдерін көрсетеді.
3. **M3 — Дау жаттығулары (40 мин):** Қамқоршыны қатыру + апелляция, басып алуды имитациялау
   `artifacts/sns/training/<suffix>/<cycle>/logs/` астындағы қамқоршы CLI журналдары.
4. **M4 — Қосымшаны түсіру (25 мин):** JSON бақылау тақтасын экспорттау және іске қосу:

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

   Жеткізу мүмкіндігі: жаңартылған қосымша Markdown + реттеуші + порталдың жадынама блоктары.

## 2. Локализацияның жұмыс процесі

- Тілдер: `ar`, `es`, `fr`, `ja`, `pt`, `ru`, I180NI05X00.
- Әрбір аударма бастапқы файлдың жанында тұрады
  (`docs/source/sns/training_collateral.<lang>.md`). `status` + жаңарту
  Жаңартқаннан кейін `translation_last_reviewed`.
- Әр тілдегі активтер келесіге жатады
  `artifacts/sns/training/<suffix>/<lang>/<cycle>/` (слайдтар/, жұмыс дәптері/,
  жазбалар/, журналдар/).
- Ағылшын тілін өңдегеннен кейін `python3 scripts/sync_docs_i18n.py --lang <code>` іске қосыңыз
  аудармашылар жаңа хэшті көруі үшін көз.

### Жеткізуді тексеру парағы

1. Локализацияланғаннан кейін аударма түйінін (`status: complete`) жаңартыңыз.
2. Слайдтарды PDF форматына экспорттаңыз және әр тілге арналған `slides/` каталогына жүктеңіз.
3. Жазба ≤10мин KPI шолуы; тіл таблосынан сілтеме.
4. Слайд/жұмыс кітабы бар `sns-training` тегі бар файлды басқару билеті
   дайджесттер, жазу сілтемелері және қосымша дәлелдер.

## 3. Оқыту активтері

- Слайд құрылымы: `docs/examples/sns_training_template.md`.
- Жұмыс кітабының үлгісі: `docs/examples/sns_training_workbook.md` (әр қатысушыға бір).
- Шақыру + еске салғыштар: `docs/examples/sns_training_invite_email.md`.
- Бағалау формасы: `docs/examples/sns_training_eval_template.md` (жауаптар
  `artifacts/sns/training/<suffix>/<cycle>/feedback/` астында мұрағатталған).

## 4. Жоспарлау және көрсеткіштер

| Цикл | Терезе | Көрсеткіштер | Ескертпелер |
|-------|--------|---------|-------|
| 2026-03 | KPI шолуын жариялау | Қатысу %, қосымша дайджест журналында | `.sora` + `.nexus` когорттар |
| 2026-06 | Pre `.dao` GA | Қаржылық дайындық ≥90% | Саясатты жаңартуды қосу |
| 2026-09 | Кеңейту | Дауларды шешу <20мин, SLA қосымшасы ≤2күн | SN-7 ынталандыруларымен туралаңыз |

`docs/source/sns/reports/sns_training_feedback.md` ішінде анонимді кері байланысты түсіріңіз
сондықтан кейінгі когорттар локализацияны және зертханаларды жақсарта алады.