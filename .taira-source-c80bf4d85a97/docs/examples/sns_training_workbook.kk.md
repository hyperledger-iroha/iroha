---
lang: kk
direction: ltr
source: docs/examples/sns_training_workbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 7d6965998c392217380a1722e49098f831438e2f4499b9e3258398a66f905a35
source_last_modified: "2025-12-29T18:16:35.080069+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# SNS оқыту жұмыс кітабының үлгісі

Бұл жұмыс кітабын әрбір оқу когорты үшін канондық үлестірме ретінде пайдаланыңыз. Ауыстыру
Қатысушыларға таратпастан бұрын толтырғыштар (`<...>`).

## Сеанс мәліметтері
- Суффикс: `<.sora | .nexus | .dao>`
- Цикл: `<YYYY-MM>`
- Тіл: `<ar/es/fr/ja/pt/ru/ur>`
- Фасилитатор: `<name>`

## 1-зертхана — KPI экспорты
1. Порталдың KPI бақылау тақтасын (`docs/portal/docs/sns/kpi-dashboard.md`) ашыңыз.
2. `<suffix>` жұрнағы және `<window>` уақыт ауқымы бойынша сүзіңіз.
3. PDF + CSV суреттерін экспорттаңыз.
4. Экспортталған JSON/PDF файлының SHA-256 файлын мына жерге жазыңыз: `______________________`.

## 2-зертхана — Манифест жаттығуы
1. `artifacts/sns/training/<suffix>/<cycle>/manifests/<lang>.json` үлгісінен манифест үлгісін алыңыз.
2. `cargo run --bin sns_manifest_check -- --input <file>` арқылы растаңыз.
3. `scripts/sns_zonefile_skeleton.py` көмегімен шешуші қаңқаны жасаңыз.
4. Айырмашылық қорытындыны қойыңыз:
   ```
   <git diff output>
   ```

## 3-зертхана — Дауларды модельдеу
1. Қауіпсіздікті бастау үшін CLI қамқоршысын пайдаланыңыз (жағдай идентификаторы `<case-id>`).
2. Дау хэшін жазыңыз: `______________________`.
3. Дәлелдер журналын `artifacts/sns/training/<suffix>/<cycle>/logs/` жүйесіне жүктеңіз.

## Зертхана 4 — Қосымшаны автоматтандыру
1. Grafana бақылау тақтасын JSON экспорттаңыз және оны `artifacts/sns/regulatory/<suffix>/<cycle>/sns_suffix_analytics.json` ішіне көшіріңіз.
2. Іске қосу:
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
3. Қосымша жолын + SHA-256 шығысын қойыңыз: `________________________________`.

## Кері байланыс жазбалары
- Не түсініксіз болды?
- Уақыт өте келе қандай зертханалар жұмыс істеді?
- Құрал қателері байқалды ма?

Аяқталған жұмыс дәптерін жүргізушіге қайтару; астына жатады
`artifacts/sns/training/<suffix>/<cycle>/workbooks/`.