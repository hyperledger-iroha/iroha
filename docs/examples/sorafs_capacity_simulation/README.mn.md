---
lang: mn
direction: ltr
source: docs/examples/sorafs_capacity_simulation/README.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 727a648141405b0c8f12a131ff903d3e7ce5b74a7f899dd99fe9aa6490b55ef2
source_last_modified: "2025-12-29T18:16:35.080764+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# SoraFS Хүчин чадлын симуляцийн хэрэгсэл

Энэхүү лавлах нь SF-2c багтаамжийн зах зээлд дахин олдох боломжтой олдворуудыг нийлүүлдэг
симуляци. Хэрэгслийн хэрэгсэл нь квотын тохиролцоо, бүтэлгүйтлийн зохицуулалт, таслах ажлыг гүйцэтгэдэг
үйлдвэрлэлийн CLI туслах болон хөнгөн шинжилгээний скрипт ашиглан засч залруулах.

## Урьдчилсан нөхцөл

- Ажлын талбарын гишүүдэд `cargo run`-г ажиллуулах чадвартай Rust хэрэгслийн гинж.
- Python 3.10+ (зөвхөн стандарт номын сан).

## Хурдан эхлэл

```bash
# 1. Generate canonical CLI artefacts
./run_cli.sh ./artifacts

# 2. Aggregate the results and emit Prometheus metrics
./analyze.py --artifacts ./artifacts
```

`run_cli.sh` скрипт нь дараахийг бүтээхийн тулд `sorafs_manifest_stub capacity`-г дууддаг:

- Квотын тохиролцооны багцад зориулсан тодорхой үйлчилгээ үзүүлэгчийн мэдэгдлүүд.
- Хэлэлцээрийн хувилбарт тохирсон хуулбарлах дараалал.
- Ачаалах цонхонд зориулсан телеметрийн агшин зуурын зургууд.
- Таслах хүсэлтийг барьж буй маргааны ачаалал.

Скрипт нь Norito байт (`*.to`), base64 ачааллыг (`*.b64`), Torii хүсэлтийг бичдэг.
Сонгосон олдворын дор биетүүд болон хүний унших боломжтой хураангуй (`*_summary.json`)
лавлах.

`analyze.py` нь үүсгэсэн хураангуйг хэрэглэж, нэгтгэсэн тайлан гаргадаг
(`capacity_simulation_report.json`) ба Prometheus текст файлыг гаргадаг
(`capacity_simulation.prom`) дараахь зүйлийг агуулна.

- `sorafs_simulation_quota_*` хэмжигч нь тохиролцсон хүчин чадал, хуваарилалтыг тодорхойлдог.
  үйлчилгээ үзүүлэгч бүрийн хувьцаа.
- `sorafs_simulation_failover_*` хэмжигч нь сул зогсолтын дельта болон сонгосон хэмжигдэхүүнийг тодруулдаг.
  орлуулах үйлчилгээ үзүүлэгч.
- `sorafs_simulation_slash_requested` олборлосон засварын хувийг бүртгэж байна
  маргааны ачааллаас.

Grafana багцыг `dashboards/grafana/sorafs_capacity_simulation.json` дотор импортлох
үүсгэсэн текст файлыг хусдаг Prometheus өгөгдлийн эх үүсвэр рүү чиглүүл.
жишээ нь зангилаа экспортлогч текст файл цуглуулагчаар дамжуулан). Runbook хаягаар
`docs/source/sorafs/runbooks/sorafs_capacity_simulation.md` бүрэн дундуур явдаг
Prometheus тохиргооны зөвлөмжийг багтаасан ажлын урсгал.

## Бэхэлгээ

- `scenarios/quota_negotiation/` — Үйлчилгээ үзүүлэгчийн мэдэгдлийн үзүүлэлтүүд болон хуулбарлах дараалал.
- `scenarios/failover/` - Анхдагч тасалдал болон бүтэлгүйтэлд зориулсан телеметрийн цонхнууд.
- `scenarios/slashing/` — Хуулбарлах ижил дарааллыг харуулсан маргааны тодорхойлолт.

Эдгээр бэхэлгээг `crates/sorafs_car/tests/capacity_simulation_toolkit.rs`-д баталгаажуулсан
CLI схемтэй синхрончлогдсон хэвээр байхын тулд.