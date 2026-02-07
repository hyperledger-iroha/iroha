---
lang: kk
direction: ltr
source: docs/source/crypto/gost_performance.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 7fab384ae80e1993b1e54d6addc82fd3dc652fb6e3958bea6a04e057a1805b57
source_last_modified: "2025-12-29T18:16:35.939573+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# ГОСТ өнімділігінің жұмыс процесі

Бұл жазбада өнімділік конвертін қалай қадағалап, қалай орындайтынымызды құжаттайды
TC26 ГОСТ қол қою сервері.

## Жергілікті жерде жүгіру

```bash
make gost-bench                     # run benches + tolerance check
make gost-bench GOST_BENCH_ARGS="--tolerance 0.30"  # override guard
make gost-dudect                    # run the constant-time timing guard
./scripts/update_gost_baseline.sh   # bench + rebaseline helper
```

Сахна артында екі нысан да `scripts/gost_bench.sh` деп атайды, ол:

1. `cargo bench -p iroha_crypto --bench gost_sign --features gost -- --noplot` орындайды.
2. `gost_perf_check` параметрін `target/criterion` қарсы іске қосып, медианаларды
   тіркелген бастапқы сызық (`crates/iroha_crypto/benches/gost_perf_baseline.json`).
3. Қол жетімді болған кезде Markdown жиынын `$GITHUB_STEP_SUMMARY` ішіне енгізеді.

Регрессияны/жақсартуды мақұлдағаннан кейін негізгі сызықты жаңарту үшін келесі әрекеттерді орындаңыз:

```bash
make gost-bench-update
```

немесе тікелей:

```bash
./scripts/gost_bench.sh --write-baseline \
  --baseline crates/iroha_crypto/benches/gost_perf_baseline.json
```

`scripts/update_gost_baseline.sh` орындық + тексерушіні іске қосады, негізгі JSON файлын қайта жазады және басып шығарады
жаңа медианалар. Әрқашан жаңартылған JSON файлын шешім жазбасымен бірге орындаңыз
`crates/iroha_crypto/docs/gost_backend.md`.

### Ағымдағы анықтамалық медианалар

| Алгоритм | Медиана (μs) |
|----------------------|-------------|
| ed25519 | 69,67 |
| gost256_paramset_a | 1136,96 |
| gost256_paramset_b | 1129,05 |
| gost256_paramset_c | 1133,25 |
| gost512_paramset_a | 8944,39 |
| gost512_paramset_b | 8963,60 |
| secp256k1 | 160,53 |

## CI

`.github/workflows/gost-perf.yml` бірдей сценарийді пайдаланады және сонымен қатар dudect уақыт қорғаушысын іске қосады.
Өлшенген медиана бастапқы мәннен конфигурацияланған төзімділіктен асып кетсе, CI сәтсіз аяқталады
(әдепкі бойынша 20%) немесе уақыт қорғаушысы ағып кетуді анықтағанда, регрессиялар автоматты түрде ұсталады.

## Жиынтық шығару

`gost_perf_check` салыстыру кестесін жергілікті түрде басып шығарады және сол мазмұнды келесіге қосады
`$GITHUB_STEP_SUMMARY`, сондықтан CI жұмыс журналдары мен іске қосу қорытындылары бірдей сандарды бөліседі.