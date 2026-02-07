---
lang: mn
direction: ltr
source: docs/source/i3_slo_harness.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: df3e3ac15baf47a6c53001acabcac7987a2386c2b772b1d8625eb60598f95a60
source_last_modified: "2025-12-29T18:16:35.966039+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

% Iroha 3 SLO бэхэлгээ

Iroha 3 хувилбар нь чухал Nexus замуудын тодорхой SLO-г агуулдаг:

- эцсийн хугацаа (NX‑18 каденц)
- нотлох баримт (гэрчилгээ, JDG гэрчилгээ, гүүрийн баталгаа)
- баталгааны төгсгөлийн цэгийн зохицуулалт (Баталгаажуулалтын хоцролтоор дамжуулан Axum замын прокси)
- шимтгэл ба бооцооны зам (төлбөр төлөгч/ивээн тэтгэгч ба бонд/сашийн урсгал)

## Төсөв

Төсөв нь `benchmarks/i3/slo_budgets.json`-д амьдардаг бөгөөд вандан сандал руу шууд зураглана
I3 багц дахь хувилбарууд. Зорилтууд нь дуудлага тус бүрийн p99 зорилтууд юм:

- Төлбөр/төлбөр: дуудлага бүрт 50 мс (`fee_payer`, `fee_sponsor`, `staking_bond`, `staking_slash`)
- Commit cert / JDG / bridge verify: 80ms (`commit_cert_verify`, `jdg_attestation_verify`,
  `bridge_proof_verify`)
- Сертификатын угсралт: 80 мс (`commit_cert_assembly`)
- Хандалтын хуваарь: 50мс (`access_scheduler`)
- Баталгаажуулах эцсийн цэгийн прокси: 120 мс (`torii_proof_endpoint`)

Шатаах түвшний зөвлөмжүүд (`burn_rate_fast`/`burn_rate_slow`) 14.4/6.0 кодчилдог.
пейжинг хийх олон цонхны харьцаа, тасалбарын дохиолол.

## Уяа

Хүрээг `cargo xtask i3-slo-harness`-ээр ажиллуулна уу:

```bash
cargo xtask i3-slo-harness \
  --iterations 64 \
  --sample-count 5 \
  --out-dir artifacts/i3_slo/latest
```

Гаралт:

- `bench_report.json|csv|md` — түүхий I3 вандан багцын үр дүн (git hash + хувилбарууд)
- `slo_report.json|md` - Зорилтод тэнцсэн/унасан/төсвийн харьцаатай SLO үнэлгээ

Уяа нь төсвийн файлыг зарцуулж, `benchmarks/i3/slo_thresholds.json`-ийг хэрэгжүүлдэг.
вандан гүйлтийн үеэр зорилтот регресс үед хурдан бүтэлгүйтэх.

## Телеметр ба хяналтын самбар

- Эцсийн хугацаа: `histogram_quantile(0.99, rate(iroha_slot_duration_ms_bucket[5m]))`
- Баталгаажуулалт: `histogram_quantile(0.99, sum by (le) (rate(zk_verify_latency_ms_bucket{status="Verified"}[5m])))`

Grafana гарааны хавтан нь `dashboards/grafana/i3_slo.json`-д амьдардаг. Prometheus
шаталтын түвшний сэрэмжлүүлгийг `dashboards/alerts/i3_slo_burn.yml`-д өгөгдсөн
шатаасан төсөв (эцсийн 2 секунд, баталгаа баталгаажуулах 80 мс, баталгаа төгсгөлийн прокси
120 мс).

## Үйл ажиллагааны тэмдэглэл

- Шөнийн цагаар морины уяа сойлгыг ажиллуулах; `artifacts/i3_slo/<stamp>/slo_report.md` нийтлэх
  засаглалын нотлох баримтын сандал олдворуудын хажууд.
- Хэрэв төсөв бүтэлгүйтсэн бол нөхцөл байдлыг тодорхойлохын тулд жишиг тэмдэглэгээг ашиглана уу
  шууд хэмжигдэхүүнтэй уялдуулахын тулд тохирох Grafana самбар/сэрэмжлүүлэг рүү оруулна.
- Баталгаажуулах эцсийн цэгийн SLO нь маршрут тус бүрээс зайлсхийхийн тулд баталгаажуулалтын хоцролтыг прокси болгон ашигладаг
  зүрх сэтгэлийн цохилт; жишиг зорилт (120ms) нь хадгалах/DoS-тай таарч байна
  баталгаа API дээрх хашлага.