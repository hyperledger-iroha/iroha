---
lang: mn
direction: ltr
source: docs/source/i3_bench_suite.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: a3158cd70a42104bacaafc520fdcc10e20e3bc347d895be448fcb10da4f668bd
source_last_modified: "2025-12-29T18:16:35.965528+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Iroha 3 вандан люкс

Iroha 3 вандан иж бүрдэл нь бооцоо тавих үед бидний найддаг халуун замаас хэд дахин их, хураамж
цэнэглэх, нотлох баримтыг шалгах, хуваарь гаргах, баталгаажуулах эцсийн цэгүүд. Энэ нь хэлбэрээр ажилладаг
Тодорхойлогч бэхэлгээтэй `xtask` тушаал (тогтмол үр, үндсэн материал,
болон тогтвортой хүсэлтийн ачаалал) тул үр дүнг хостууд дээр дахин гаргах боломжтой.

## Suite-г ажиллуулж байна

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

Тугнууд:

- `--iterations` нь хувилбарт түүврийн давталтуудыг хянадаг (өгөгдмөл: 64).
- `--sample-count` медианыг тооцоолохын тулд хувилбар бүрийг давтана (өгөгдмөл: 5).
- `--json-out|--csv-out|--markdown-out` гаралтын олдворуудыг сонгоно (бүгд сонголттой).
- `--threshold` медианыг суурь хязгаартай харьцуулдаг (`--no-threshold` тохируулна)
  алгасах).
- `--flamegraph-hint` нь Markdown тайланд `cargo flamegraph` тайлбартай байдаг
  сценарийг профайл болгох команд.

CI цавуу нь `ci/i3_bench_suite.sh`-д амьдардаг бөгөөд дээрх замууд дээр анхдагчаар тохируулагддаг; тогтоосон
Шөнийн цагаар ажиллах цагийг тааруулахын тулд `I3_BENCH_ITERATIONS`/`I3_BENCH_SAMPLES`.

## Сценари

- `fee_payer` / `fee_sponsor` / `fee_insufficient` — төлбөр төлөгч ба спонсорын дебит
  болон дутагдлаас татгалзах.
- `staking_bond` / `staking_slash` — бонд/барьцаагаагүй дараалал
  цавчих.
- `commit_cert_verify` / `jdg_attestation_verify` / `bridge_proof_verify` —
  commit гэрчилгээ, JDG attestations, болон гүүр дээр гарын үсгийн баталгаажуулалт
  нотлох ачаалал.
- `commit_cert_assembly` — гэрчилгээ олгох угсралт.
- `access_scheduler` - зөрчилдөөнийг мэддэг хандалтын хуваарь.
- `torii_proof_endpoint` — Axum баталгаатай төгсгөлийн цэг задлан шинжилгээ + баталгаажуулах хоёр талт аялал.

Сценари бүр нэг давталт дахь дундаж наносекунд, дамжуулах чадвар болон a
хурдан регрессийн тодорхойлогч хуваарилалтын тоолуур. Босгогууд амьдардаг
`benchmarks/i3/thresholds.json`; Тоног төхөөрөмж өөрчлөгдөхөд тэнд овойлт үүсдэг ба
тайлангийн хамт шинэ олдворыг хийх.

## Алдааг олж засварлах

- Чимээ ихтэй регрессээс зайлсхийхийн тулд нотлох баримт цуглуулахдаа CPU-ийн давтамж/захирагчийг тогтооно.
- Хайгуулын гүйлтэд `--no-threshold`-г ашиглаад үндсэн үзүүлэлт болмогц дахин идэвхжүүлнэ.
  сэргээгдсэн.
- Нэг хувилбарыг профайл болгохын тулд `--iterations 1` тохиргоог хийж, доор дахин ажиллуулна уу.
  `cargo flamegraph -p xtask -- i3-bench-suite --iterations 128 --sample-count 1 --no-threshold --flamegraph-hint`.