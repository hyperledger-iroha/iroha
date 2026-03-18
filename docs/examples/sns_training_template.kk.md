---
lang: kk
direction: ltr
source: docs/examples/sns_training_template.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: dd9da5045f5f40dbc31837145ad13bf79b4d751b0803c0b6d69bab49885ed1b4
source_last_modified: "2025-12-29T18:16:35.079313+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# SNS жаттығу слайдының үлгісі

Бұл Markdown контуры фасилитаторлар бейімделуі керек слайдтарды көрсетеді
олардың тілдік топтары. Бұл бөлімдерді Keynote/PowerPoint/Google ішіне көшіріңіз
Қажет болса, таңбалау нүктелерін, скриншоттарды және диаграммаларды слайдтар және локализациялаңыз.

## Тақырыптық слайд
- Бағдарлама: «Sora Name Service onboarding»
- Субтитр: жұрнақты + циклды көрсетіңіз (мысалы, `.sora — 2026‑03`)
- Баяндамашылар + серіктестіктер

## KPI бағдары
- `docs/portal/docs/sns/kpi-dashboard.md` скриншоты немесе ендіру
- Суффикс сүзгілерін түсіндіретін таңбалар тізімі, ARPU кестесі, мұздату трекері
- PDF/CSV экспорттау үшін хабарламалар

## Манифесттің өмірлік циклі
- Диаграмма: тіркеуші → Torii → басқару → DNS/шлюз
- `docs/source/sns/registry_schema.md` сілтеме жасайтын қадамдар
- Аннотациялары бар манифест үзіндісі

## Дау және мұздату жаттығулары
- Қамқоршының араласуының схемасы
- `docs/source/sns/governance_playbook.md` сілтемесі бар бақылау парағы
- Мысал билетті тоқтату хронологиясы

## Қосымшаны түсіру
- `cargo xtask sns-annex ... --portal-entry ...` көрсететін пәрмен үзіндісі
- `artifacts/sns/regulatory/<suffix>/<cycle>/` астында Grafana JSON мұрағатына арналған еске салу
- `docs/source/sns/reports/.<suffix>/<cycle>.md` сілтемесі

## Келесі қадамдар
- Жаттығу бойынша кері байланыс сілтемесі (`docs/examples/sns_training_eval_template.md` қараңыз)
- Slack/Matrix арналарының тұтқалары
- Алдағы маңызды күндер