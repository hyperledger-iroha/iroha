---
lang: kk
direction: ltr
source: docs/source/i3_bench_suite.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: a3158cd70a42104bacaafc520fdcc10e20e3bc347d895be448fcb10da4f668bd
source_last_modified: "2025-12-29T18:16:35.965528+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Iroha 3 орындық люкс

Iroha 3 орындық люкс стекинг кезінде біз сенетін ыстық жолдарды еселейді, алым
зарядтау, дәлелді тексеру, жоспарлау және дәлелдеу соңғы нүктелері. Ол ретінде жұмыс істейді
Детерминирленген қондырғылары бар `xtask` командасы (бекітілген тұқымдар, бекітілген кілт материалы,
және тұрақты сұрау жүктемелері) нәтижелер хосттар арасында қайталанатын болады.

## Сюитті іске қосу

```bash
cargo xtask i3-bench-suite \
  --iterations 64 \
  --sample-count 5 \
  --json-out benchmarks/i3/latest.json \
  --csv-out benchmarks/i3/latest.csv \
  --markdown-out benchmarks/i3/latest.md \
  --threshold benchmarks/i3/thresholds.json \
  --allow-overwrite
```

Жалаулар:

- `--iterations` сценарий үлгісі бойынша итерацияларды басқарады (әдепкі: 64).
- `--sample-count` медиананы есептеу үшін әрбір сценарийді қайталайды (әдепкі: 5).
- `--json-out|--csv-out|--markdown-out` шығыс артефактілерін таңдайды (барлығы міндетті емес).
- `--threshold` медианаларды негізгі шектермен салыстырады (`--no-threshold` орнату
  өткізіп жіберу).
- `--flamegraph-hint` Markdown есебіне `cargo flamegraph` арқылы түсініктеме береді
  сценарийді профильдеу пәрмені.

CI желімі `ci/i3_bench_suite.sh` ішінде өмір сүреді және жоғарыдағы жолдарға әдепкі мән береді; орнату
Түнгі уақытта жұмыс уақытын реттеу үшін `I3_BENCH_ITERATIONS`/`I3_BENCH_SAMPLES`.

## Сценарийлер

- `fee_payer` / `fee_sponsor` / `fee_insufficient` — төлеуші және демеуші дебет
  және тапшылықтан бас тарту.
- `staking_bond` / `staking_slash` — бар және онсыз облигация/бөлу кезегі
  кесу.
- `commit_cert_verify` / `jdg_attestation_verify` / `bridge_proof_verify` —
  міндеттеме сертификаттары, JDG аттестациялары және көпір арқылы қолтаңбаны тексеру
  дәлелдейтін пайдалы жүктемелер.
- `commit_cert_assembly` — растау сертификаттары үшін дайджест жинағы.
- `access_scheduler` — қақтығысты ескеретін қатынас орнатуды жоспарлау.
- `torii_proof_endpoint` — Axum proof соңғы нүктені талдау + тексерудің айналмалы сапары.

Әрбір сценарий итерациядағы медиана наносекундтарды, өткізу қабілеттілігін және а
жылдам регрессиялар үшін детерминирленген бөлу есептегіші. Табалдырықтар тұрады
`benchmarks/i3/thresholds.json`; аппараттық құрал өзгерген кезде шекаралар сонда болады және
есеппен бірге жаңа артефакт жасаңыз.

## Ақаулықтарды жою

- Шулы регрессияларды болдырмау үшін дәлелдерді жинау кезінде процессор жиілігін/басқарушыны бекітіңіз.
- Зерттеу жұмысы үшін `--no-threshold` пайдаланыңыз, содан кейін бастапқы сызық болған кезде қайта қосыңыз.
  жаңартылды.
- Жалғыз сценарийді профильдеу үшін `--iterations 1` орнатыңыз және астында қайта іске қосыңыз
  `cargo flamegraph -p xtask -- i3-bench-suite --iterations 128 --sample-count 1 --no-threshold --flamegraph-hint`.