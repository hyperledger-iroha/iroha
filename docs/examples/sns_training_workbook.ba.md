---
lang: ba
direction: ltr
source: docs/examples/sns_training_workbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 7d6965998c392217380a1722e49098f831438e2f4499b9e3258398a66f905a35
source_last_modified: "2025-12-29T18:16:35.080069+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# SNS уҡытыу эш дәфтәре ҡалыптары

Был эш дәфтәрен һәр уҡытыу когортаһы өсөн канонлы таратыу булараҡ ҡулланығыҙ. Алмаштырырға
урындар (`<...>`) йыйылыусыларға таратҡансы.

## Сессия реквизиттары
- Суффикс: `<.sora | .nexus | .dao>`
- цикл: `<YYYY-MM>`
- Тел: `<ar/es/fr/ja/pt/ru/ur>`
- Фасилитатор: `<name>`

## 1-се лаборатория — KPI экспорты
1. Портал KPI приборҙар таҡтаһын асырға (`docs/portal/docs/sns/kpi-dashboard.md`).
2. `<suffix>` ялғауы һәм `<window>` ваҡыт диапазоны менән фильтр.
3. Экспорт PDF + CSV снимоктары.
4. Яҙма SHA-256 экспортланған JSON/PDF бында: `______________________`.

## 2-се лаборатория — Манифест бура
1. I18NI000000012X-тан өлгө күрһәтеү.
2. `cargo run --bin sns_manifest_check -- --input <file>` менән раҫлау.
.
4. Дифф резюмеһын йәбештерегеҙ:
   ```
   <git diff output>
   ```

## 3-сө лаборатория — бәхәс моделләштереү
1. Ҡулланыу опекун CLI башлау өсөн туңдырыу (осраҡ id `<case-id>`).
2. Бәхәс хеш яҙма: `______________________`.
3. Дәлилдәрҙе `artifacts/sns/training/<suffix>/<cycle>/logs/`-ҡа тейәгеҙ.

## 4-се лаборатория — Ҡушымта автоматлаштырыу
1. Экспорт I18NT00000000000X приборҙар таҡтаһы JSON һәм уны күсерергә I18NI000000018X.
2. Йүгерергә:
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
3. Ҡушымта юлды йәбештереү + SHA-256 сығыш: I18NI000000019X.

## Кире бәйләнеш иҫкәрмәләр
- Нимә асыҡланманы?
- Ҡайһы лабораториялар ваҡыт үткән һайын йүгерҙе?
- Күҙәтелгән ҡораллы ҡомаҡтар?

Ҡайтарыу тамамланған эш дәфтәрҙәрен ярҙамсыға; улар 2012 йылда ҡарай.
`artifacts/sns/training/<suffix>/<cycle>/workbooks/`.