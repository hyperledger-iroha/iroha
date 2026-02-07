---
lang: kk
direction: ltr
source: docs/portal/docs/nexus/settlement-faq.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: e992cea8d0c835b30bd9e91860f6b6f87bed79a2c25bd6d0544639685834f80c
source_last_modified: "2025-12-29T18:16:35.146583+00:00"
translation_last_reviewed: 2026-02-07
id: nexus-settlement-faq
title: Settlement FAQ
description: Operator-facing answers covering settlement routing, XOR conversion, telemetry, and audit evidence.
translator: machine-google-reviewed
---

Бұл бет ішкі есеп айырысу жиі қойылатын сұрақтарды көрсетеді (`docs/source/nexus_settlement_faq.md`)
осылайша портал оқырмандары бірдей нұсқаулықты зерттемей-ақ қарай алады
моно-репо. Ол есеп айырысу маршрутизаторы төлемдерді қалай өңдейді, қандай көрсеткіштерді түсіндіреді
бақылау үшін және SDK Norito пайдалы жүктемелерін қалай біріктіру керек.

## Ерекшеліктер

1. **Жолды салыстыру** — әрбір деректер кеңістігі `settlement_handle` деп жариялайды
   (`xor_global`, `xor_lane_weighted`, `xor_hosted_custody`, немесе
   `xor_dual_fund`). Төмендегі жолақтардың соңғы каталогын қараңыз
   `docs/source/project_tracker/nexus_config_deltas/`.
2. **Детерминистік түрлендіру** — маршрутизатор барлық есеп айырысуларды XOR арқылы түрлендіреді
   басқару бекіткен өтімділік көздері. Жеке жолдар XOR буферлерін алдын ала қаржыландырады;
   шаш қиюлары буферлер саясаттан тыс ауытқыған кезде ғана қолданылады.
3. **Телеметрия** — сағат `nexus_settlement_latency_seconds`, конверсия есептегіштері,
   және шаш үлгілері. Бақылау тақталары `dashboards/grafana/nexus_settlement.json` ішінде тұрады
   және `dashboards/alerts/nexus_audit_rules.yml` ішіндегі ескертулер.
4. **Дәлелдер** — мұрағат конфигурациялары, маршрутизатор журналдары, телеметрия экспорты және
   тексерулер үшін салыстыру есептері.
5. **SDK міндеттері** — әрбір SDK есеп айырысу көмекшілерін, жолақ идентификаторларын,
   және маршрутизатормен тепе-теңдікті сақтау үшін Norito пайдалы жүк кодерлері.

## Мысал ағындары

| Жолақ түрі | Қолға алу үшін дәлелдемелер | Бұл нені дәлелдейді |
|----------|--------------------|----------------|
| Жеке `xor_hosted_custody` | Маршрутизатор журналы + `nexus_settlement_latency_seconds{lane}` + `settlement_router_haircut_total{lane}` | CBDC буферлері дебеттік детерминирленген XOR және шаш қиюлары саясатта қалады. |
| Қоғамдық `xor_global` | Маршрутизатор журналы + DEX/TWAP анықтамасы + кідіріс/түрлендіру көрсеткіштері | Ортақ өтімділік жолы трансферді жарияланған TWAP бойынша нөлдік кесумен бағалады. |
| Гибридті `xor_dual_fund` | Маршрутизатор журналы жалпыға қарсы экрандалған бөлу + телеметриялық есептегіштерді | Қорғалған/қоғамдық араласқан басқару коэффициенттерін сақтады және әр аяққа қолданылған шаш қиюын жазды. |

## Толығырақ мәлімет керек пе?

- Толық жиі қойылатын сұрақтар: `docs/source/nexus_settlement_faq.md`
- Есеп айырысу маршрутизаторының ерекшелігі: `docs/source/settlement_router.md`
- CBDC саясатының ойын кітабы: `docs/source/cbdc_lane_playbook.md`
- Operations runbook: [Nexus операциялар](./nexus-operations)