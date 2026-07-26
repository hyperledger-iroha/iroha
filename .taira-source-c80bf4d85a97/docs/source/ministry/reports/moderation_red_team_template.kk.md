---
lang: kk
direction: ltr
source: docs/source/ministry/reports/moderation_red_team_template.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 22bfdf5696bf3a58e7899e7d7b2ba77e404a05fa81304f12d6c78eeb1e8035e5
source_last_modified: "2025-12-29T18:16:35.982646+00:00"
translation_last_reviewed: 2026-02-07
title: Red-Team Drill Report Template
summary: Copy this file for every MINFO-9 drill to capture metadata, evidence, and remediation actions.
translator: machine-google-reviewed
---

> **Қолдану жолы:** бұл үлгіні әр жаттығудан кейін бірден `docs/source/ministry/reports/<YYYY-MM>-mod-red-team-<scenario>.md` үлгісіне көшіріңіз. Файл атауларын кіші әріппен, сызықшамен және Alertmanager жүйесінде тіркелген егжей-тегжейлі идентификатормен туралаңыз.

# Red-Team Drill Report — `<SCENARIO NAME>`

- **Бұрғылау идентификаторы:** `<YYYYMMDD>-<scenario>`
- **Күн және терезе:** `<YYYY-MM-DD HH:MMZ – HH:MMZ>`
- **Сценарий класы:** `smuggling | bribery | gateway | ...`
- **Операторлар:** `<names / handles>`
- **Бақылау тақталары тапсырмадан тоқтатылды:** `<git SHA>`
- **Дәлелдер жинағы:** `artifacts/ministry/red-team/<YYYY-MM>/<scenario>/`
- **SoraFS CID (міндетті емес):** `<cid>`  
- **Байланысты жол картасы элементтері:** `MINFO-9`, сонымен қатар кез келген байланыстырылған билеттер.

## 1. Мақсаттар мен кіру шарттары

- **Негізгі мақсаттар**
  - `<e.g. Verify denylist TTL enforcement under smuggling attack>`
- **Алғышарттар расталды**
  - `emergency_canon_policy.md` нұсқасы `<tag>`
  - `dashboards/grafana/ministry_moderation_overview.json` дайджест `<sha256>`
  - Қоңырау кезінде қайта анықтау өкілеттігі: `<name>`

## 2. Орындау хронологиясы

| Уақыт белгісі (UTC) | Актер | Әрекет / Команда | Нәтиже / Ескертулер |
|----------------|-------|------------------|----------------|
|  |  |  |  |

> Torii сұрау идентификаторларын, бөлік хэштерін, қайта анықтау мақұлдауларын және Alertmanager сілтемелерін қосыңыз.

## 3. Бақылаулар және метрика

| метрикалық | Мақсат | Байқалған | Өтті/өтпеген | Ескертпелер |
|--------|--------|----------|-----------|-------|
| Ескертуге жауап беру кідірісі | `<X> min` | `<Y> min` | ✅/⚠️ |  |
| Модерацияны анықтау жылдамдығы | `>= <value>` |  |  |  |
| Шлюз аномалиясын анықтау | `Alert fired` |  |  |  |

- `Grafana export:` `artifacts/.../dashboards/ministry_moderation_overview.json`
- `Alert bundle:` `artifacts/.../alerts/ministry_moderation_rules.yml`
- `Norito manifests:` `<path>`

## 4. Нәтижелер және түзетулер

| Ауырлық | табу | Иесі | Мақсатты күн | Күй / Сілтеме |
|----------|---------|-------|-------------|---------------|
| Жоғары |  |  |  |  |

Калибрлеудің қалай көрінетінін құжаттау, бас тарту саясаты немесе SDK/құрал өзгерту керек. GitHub/Jira мәселелеріне сілтеме және блокталған/бұғатталмаған күйлерді ескертіңіз.

## 5. Басқару және бекітулер

- **Оқиға командирінің қолтаңбасы:** `<name / timestamp>`
- **Басқару кеңесінің қарау күні:** `<meeting id>`
- **Кейінгі тексеру парағы:** `[ ] status.md updated`, `[ ] roadmap row updated`, `[ ] transparency packet annotated`

## 6. Қосымшалар

- `[ ] CLI logbook (`logs/.md`)`
- `[ ] Dashboard JSON export`
- `[ ] Alertmanager history`
- `[ ] SoraFS manifest / CAR`
- `[ ] Override audit log`

Дәлелдер жинағына және SoraFS суретіне жүктеп салынғаннан кейін әрбір тіркемені `[x]` деп белгілеңіз.

---

_Соңғы жаңартылған уақыты: {{ күні | әдепкі("2026-02-20") }}_