---
lang: ba
direction: ltr
source: docs/examples/soranet_testnet_operator_kit/02-checklist.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 65b7cc2b6c8fdcc55dfebfbfb5fda52c44c82804a556cad70a06ddeb53cb2531
source_last_modified: "2025-12-29T18:16:35.090122+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

- [ ] аппарат спектрын тикшерергә: 8+ ядролар, 16 Гиб RAM, NVMe һаҡлау ≥ 500 МиБ/с.
- [ ] Ике IPv4 + IPv6 адрестарын раҫлағыҙ һәм өҫкө ағымда QUIC/UDP 443 рөхсәт итә.
- [ ] HSM йәки бағышланған хәүефһеҙ анклав тәьмин итеү өсөн реле идентификация асҡыстары.
- [ ] Синх. канонлы каталог (`governance/compliance/soranet_opt_outs.json`).
- [ ] Оркестратор конфигына үтәү блогын берләштереү (ҡара: `03-config-example.toml`).
- [ ] Йыйыу юрисдикцияһы/глобаль үтәү аттестациялары һәм `attestations` исемлеген тултырырға.
- [ ] Йүгереп I18NI000000004X (йәки һеҙҙең тере снимок) һәм тикшерергә үткәреү/уңышһыҙлыҡҡа осраған отчет.
- [ ] Реле ҡабул итеү КСО-ны генерациялау һәм идара итеүгә ҡул ҡуйылған конверт алыу.
- [ ] Импорт һаҡсыһы дескриптор орлоғо һәм баҫылған хеш-сылбырға ҡаршы раҫлау.
- [ ] Йүгереп `cargo test -p sorafs_orchestrator -- --ignored guard_rotation_smoke`.
- [ ] Ҡоро идара итеү телеметрия экспорты: тәьмин итеү I18NT000000000000 X scrape урындағы уңышҡа өлгәшә.
- [ ] Расписание браунут быраулау тәҙрәһе һәм рекордлы эскалация контакттары.
- [ ] Бурау дәлилдәренә ҡул ҡуйығыҙ: `cargo xtask soranet-testnet-drill-bundle --log evidence/drills-log.json --signing-key <path> --window-start <YYYY-MM-DD> --window-end <YYYY-MM-DD> --attachment guard-rotation=<path> --attachment exit-bond=<path> --out evidence/drills-signed.json`.