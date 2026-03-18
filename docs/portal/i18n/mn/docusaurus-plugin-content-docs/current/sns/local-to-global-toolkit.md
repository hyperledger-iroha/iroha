---
lang: mn
direction: ltr
source: docs/portal/docs/sns/local-to-global-toolkit.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: Local → Global Address Toolkit
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

Энэ хуудас [`docs/source/sns/local_to_global_toolkit.md`](../../../source/sns/local_to_global_toolkit.md) толин тусгал
моно репо-оос. Энэ нь **ADDR-5c** замын зураглалын зүйлд шаардлагатай CLI туслах болон runbook-уудыг багцалдаг.

## Тойм

- `scripts/address_local_toolkit.sh` нь `iroha` CLI-г ороож дараахь зүйлийг үйлдвэрлэдэг.
  - `audit.json` — `iroha tools address audit --format json`-аас бүтэцлэгдсэн гаралт.
  - `normalized.txt` — Local-domain сонгогч бүрийн хувьд сонгосон I105 / хоёр дахь хамгийн сайн шахсан (`sora`) литералуудыг хөрвүүлсэн.
- Скриптийг хаяг хүлээн авах хяналтын самбартай хослуулах (`dashboards/grafana/address_ingest.json`)
  болон Alertmanager дүрмүүд (`dashboards/alerts/address_ingest_rules.yml`) нь Local-8 /
  Орон нутгийн-12 зүсэлт нь аюулгүй. Орон нутгийн-8 ба Орон нутгийн-12 мөргөлдөөний самбар дээр нэмээд үзээрэй
  `AddressLocal8Resurgence`, `AddressLocal12Collision`, `AddressInvalidRatioSlo` дохиолол өмнө нь
  илэрхий өөрчлөлтийг дэмжих.
- [Хаяг харуулах удирдамж](address-display-guidelines.md) болон
  UX болон ослын хариу контекст зориулсан [Хаяг Манифест runbook](../../../source/runbooks/address_manifest_ops.md).

## Хэрэглээ

```bash
scripts/address_local_toolkit.sh \
  --input fixtures/address/local_digest_examples.txt \
  --output-dir artifacts/address_migration \
  --network-prefix 753 \
  --format i105
```

Сонголтууд:

- I105-ийн оронд `i105` гаралтын хувьд `--format i105`.
- `domainless output (default)` нь нүцгэн литералуудыг ялгаруулдаг.
- `--audit-only` дарж хувиргах алхамыг алгасах.
- `--allow-errors` нь алдаатай мөр гарч ирэх үед үргэлжлүүлэн сканнердах (CLI-д тохирсон).

Скрипт нь гүйлтийн төгсгөлд олдворын замыг бичдэг. Хоёр файлыг хавсаргана уу
тэгийг нотолсон Grafana дэлгэцийн агшингийн хажууд таны өөрчлөлтийн удирдлагын тасалбар
≥30 хоногийн дотор орон нутгийн-8 илрүүлэлт ба Орон нутгийн-12-ын мөргөлдөөн тэг.

## CI интеграцчилал

1. Скриптийг тусгай зориулалтын ажилд ажиллуулж, үр дүнг нь байршуулна уу.
2. `audit.json` Локал сонгогчид (`domain.kind = local12`) мэдээлэх үед блок нэгтгэдэг.
   өгөгдмөл `true` утгаараа (зөвхөн хөгжүүлэгч/туршилтын кластер дээрх `false`-г хүчингүй болгоно.
   регрессийг оношлох) ба нэмнэ
   `iroha tools address normalize`-аас CI хүртэл регресс
   үйлдвэрлэлд хүрэхээс өмнө оролдлого амжилтгүй болсон.

Дэлгэрэнгүй мэдээллийг эх бичиг баримтаас харна уу, жишээ нотлох баримтын хяналтын хуудас болон захиалгын тайланг хэрэглэгчдэд зарлахдаа дахин ашиглах боломжтой.