---
lang: kk
direction: ltr
source: docs/source/i3_slo_harness.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: df3e3ac15baf47a6c53001acabcac7987a2386c2b772b1d8625eb60598f95a60
source_last_modified: "2025-12-29T18:16:35.966039+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

% Iroha 3 SLO жабдығы

Iroha 3 шығарылым желісі маңызды Nexus жолдары үшін нақты SLO-ларды қамтиды:

- қорытынды слот ұзақтығы (NX‑18 каденциясы)
- дәлелдемелерді тексеру (коммит сертификаттары, JDG аттестациялары, көпір дәлелдері)
- дәлелді соңғы нүктені өңдеу (тексеру кідірісі арқылы Axum жолы проксиі)
- комиссиялық және стекинг жолдары (төлеуші/демеуші және облигация/қиғаш ағындар)

## Бюджеттер

Бюджеттер `benchmarks/i3/slo_budgets.json` жүйесінде тұрады және тікелей орындықпен салыстырылады
I3 жиынтығындағы сценарийлер. Міндеттер әр қоңырауға арналған p99 мақсаттары:

- Төлем/төлем: бір қоңырауға 50 мс (`fee_payer`, `fee_sponsor`, `staking_bond`, `staking_slash`)
- Сертификаттау / JDG / көпірді тексеру: 80 мс (`commit_cert_verify`, `jdg_attestation_verify`,
  `bridge_proof_verify`)
- Сертификаттау жинағы: 80 мс (`commit_cert_assembly`)
- Қолжетімді жоспарлаушы: 50 мс (`access_scheduler`)
- Соңғы нүкте проксиі: 120 мс (`torii_proof_endpoint`)

Жану жылдамдығы туралы кеңестер (`burn_rate_fast`/`burn_rate_slow`) 14.4/6.0 кодтайды
пейджинг пен билет ескертулеріне арналған көп терезе қатынасы.

## Әбзел

Жіпті `cargo xtask i3-slo-harness` арқылы іске қосыңыз:

```bash
cargo xtask i3-slo-harness \
  --iterations 64 \
  --sample-count 5 \
  --out-dir artifacts/i3_slo/latest
```

Шығарулар:

- `bench_report.json|csv|md` — өңделмеген I3 орындық жинағының нәтижелері (git хэш + сценарийлері)
- `slo_report.json|md` — мақсатқа сәйкес өту/сәтсіз/бюджеттік қатынасы бар SLO бағалауы

Жабдық бюджеттер файлын тұтынады және `benchmarks/i3/slo_thresholds.json` талап етеді
орындық жүгіру кезінде мақсат кері кеткенде тез істен шығады.

## Телеметрия және бақылау тақталары

- Түпкілікті: `histogram_quantile(0.99, rate(iroha_slot_duration_ms_bucket[5m]))`
- Дәлелдеуді тексеру: `histogram_quantile(0.99, sum by (le) (rate(zk_verify_latency_ms_bucket{status="Verified"}[5m])))`

Grafana бастапқы панельдері `dashboards/grafana/i3_slo.json` ішінде тұрады. Prometheus
жану жылдамдығы туралы ескертулер `dashboards/alerts/i3_slo_burn.yml` файлында берілген
жоғары бюджеттер (соңғы 2 секунд, дәлелді растау 80 мс, дәлелдеу соңғы нүкте проксиі
120 мс).

## Операциялық жазбалар

- Түнгі уақытта әбзелді жүгіртіңіз; `artifacts/i3_slo/<stamp>/slo_report.md` жариялау
  басқару дәлелдері үшін орындық артефактілермен қатар.
- Егер бюджет сәтсіз болса, сценарийді анықтау үшін бағаны белгілеуді пайдаланыңыз, содан кейін егжей-тегжейлі өтіңіз
  тірі метрикамен байланыстыру үшін сәйкес Grafana панеліне/ескертуге.
- Дәлелдеу соңғы нүктесі SLO әр бағытта жол бермеу үшін растау кідірісін прокси ретінде пайдаланады
  түбегейлі соққы; эталондық мақсат (120 мс) сақтау/DoS сәйкес келеді
  proof API-дегі қоршаулар.