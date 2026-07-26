---
lang: mn
direction: ltr
source: docs/examples/soranet_testnet_operator_kit/02-checklist.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 65b7cc2b6c8fdcc55dfebfbfb5fda52c44c82804a556cad70a06ddeb53cb2531
source_last_modified: "2025-12-29T18:16:35.090122+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

- [ ] Техник хангамжийн үзүүлэлтүүдийг шалгана уу: 8+ цөм, 16 GiB RAM, NVMe санах ой ≥ 500 МБ/с.
- [ ] Хоёр IPv4 + IPv6 хаягийг баталгаажуулж, дээд тал нь QUIC/UDP 443-г зөвшөөрнө.
- [ ] Релей таних түлхүүрүүдэд зориулсан HSM эсвэл тусгай хамгаалалттай анклаваар хангана.
- [ ] Каноник татгалзах каталогийг синк хийх (`governance/compliance/soranet_opt_outs.json`).
- [ ] Зохицуулалтын блокийг найруулагчийн тохиргоонд нэгтгэх (`03-config-example.toml`-г үзнэ үү).
- [ ] харьяалал/дэлхий дахины нийцлийн гэрчилгээг авч, `attestations` жагсаалтыг бөглөнө үү.
- [ ] `cargo xtask soranet-testnet-metrics --input 07-metrics-sample.json` (эсвэл таны шууд хормын хувилбар) ажиллуулж, тэнцсэн/бүтэлгүйтлийн тайланг шалгана уу.
- [ ] Буухиа хүлээн авах нийгмийн хариуцлагыг үүсгэж, засаглалын гарын үсэгтэй дугтуй авах.
- [ ] Хамгаалалтын тодорхойлогч үрийг импортлох ба нийтлэгдсэн хэш гинжин хэлхээтэй тулгарах.
- [ ] `cargo test -p sorafs_orchestrator -- --ignored guard_rotation_smoke` ажиллуулна уу.
- [ ] Хуурай ажиллагаатай телеметрийн экспорт: Prometheus хусах ажлыг орон нутагт амжилттай хийж байгаа эсэхийг шалгаарай.
- [ ] Өрөмдлөгийн цонхны хуваарь гаргаж, хурдасгах харилцагчдыг бүртгэнэ.
- [ ] Өрөмдлөгийн нотлох баримтын багцад гарын үсэг зурна уу: `cargo xtask soranet-testnet-drill-bundle --log evidence/drills-log.json --signing-key <path> --window-start <YYYY-MM-DD> --window-end <YYYY-MM-DD> --attachment guard-rotation=<path> --attachment exit-bond=<path> --out evidence/drills-signed.json`.