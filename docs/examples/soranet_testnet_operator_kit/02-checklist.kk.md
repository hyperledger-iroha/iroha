---
lang: kk
direction: ltr
source: docs/examples/soranet_testnet_operator_kit/02-checklist.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 65b7cc2b6c8fdcc55dfebfbfb5fda52c44c82804a556cad70a06ddeb53cb2531
source_last_modified: "2025-12-29T18:16:35.090122+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

- [ ] Аппараттық құралдың ерекшелігін қарап шығу: 8+ ядро, 16 ГБ жедел жады, NVMe жады ≥ 500 МБ/с.
- [ ] Екі IPv4 + IPv6 мекенжайларын және жоғары ағын QUIC/UDP 443 рұқсаттарын растаңыз.
- [ ] Релелік идентификациялық кілттер үшін HSM немесе арнайы қауіпсіз анклавты қамтамасыз ету.
- [ ] Синхрондау канондық бас тарту каталогы (`governance/compliance/soranet_opt_outs.json`).
- [ ] Оркестр конфигурациясына сәйкестік блогын біріктіріңіз (`03-config-example.toml` қараңыз).
- [ ] Юрисдикция/жаһандық сәйкестік аттестацияларын алыңыз және `attestations` тізімін толтырыңыз.
- [ ] `cargo xtask soranet-testnet-metrics --input 07-metrics-sample.json` (немесе сіздің тікелей суретіңізді) іске қосыңыз және өтпелі/өтпегені туралы есепті қарап шығыңыз.
- [ ] CSR релелік рұқсатын жасаңыз және басқару қол қойылған конвертті алыңыз.
- [ ] Қорғаушы дескриптор тұқымын импорттаңыз және жарияланған хэш тізбегімен салыстырыңыз.
- [ ] `cargo test -p sorafs_orchestrator -- --ignored guard_rotation_smoke` іске қосыңыз.
- [ ] Құрғақ іске қосылған телеметрия экспорты: Prometheus қырып алудың жергілікті деңгейде сәтті болуын қамтамасыз етіңіз.
- [ ] Бұрғылау терезесін жоспарлаңыз және кеңейту контактілерін жазыңыз.
- [ ] Бұрғылау дәлелдер жинағына қол қойыңыз: `cargo xtask soranet-testnet-drill-bundle --log evidence/drills-log.json --signing-key <path> --window-start <YYYY-MM-DD> --window-end <YYYY-MM-DD> --attachment guard-rotation=<path> --attachment exit-bond=<path> --out evidence/drills-signed.json`.