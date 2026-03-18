---
lang: mn
direction: ltr
source: docs/automation/da/README.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 8757f0bf8699b532ece29437af953353526b3201b4b129ebec7d6bf5d224f038
source_last_modified: "2025-12-29T18:16:35.061402+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Өгөгдлийн хүртээмжийн аюулын загварын автоматжуулалт (DA-1)

Замын зургийн DA-1 ба `status.md` зүйл нь тодорхойлогч автоматжуулалтын давталтыг шаарддаг.
дээр гарсан Norito PDP/PoTR аюул заналын загварын хураангуйг гаргадаг.
`docs/source/da/threat_model.md` болон Docusaurus толь. Энэ лавлах
иш татсан олдворуудыг авдаг:

- `cargo xtask da-threat-model-report [--out <path|->] [--seed <u64|0xhex>] [--config <path>]`
- `.github/workflows/da-threat-model-nightly.yml`
- `make docs-da-threat-model` (`scripts/docs/render_da_threat_model_tables.py` ажилладаг)
- `cargo xtask da-commitment-reconcile --receipt <path> --block <path> [--json-out <path|->]`
- `cargo xtask da-privilege-audit --config <torii.toml> [--extra-path <path> ...] [--json-out <path|->]`

## Урсгал

1. **Тайлан гаргах**
   ```bash
   cargo xtask da-threat-model-report \
     --config configs/da/threat_model.toml \
     --out artifacts/da/threat_model_report.json
   ```
   JSON хураангуй нь дуурайлган хуулбарлах алдааны хувь, chunker-ийг бүртгэдэг
   босго, PDP/PoTR-аас илрүүлсэн аливаа бодлогын зөрчил
   `integration_tests/src/da/pdp_potr.rs`.
2. **Markdown хүснэгтүүдийг гаргах**
   ```bash
   make docs-da-threat-model
   ```
   Энэ нь дахин бичихийн тулд `scripts/docs/render_da_threat_model_tables.py`-г ажиллуулдаг
   `docs/source/da/threat_model.md` ба `docs/portal/docs/da/threat-model.md`.
3. JSON тайланг (болон нэмэлт CLI бүртгэл) хуулж **олдворыг архивлана уу**
   `docs/automation/da/reports/<timestamp>-threat_model_report.json`. Хэзээ
   засаглалын шийдвэрүүд нь тодорхой гүйлтэд тулгуурладаг, үүнд git commit hash болон
   `<timestamp>-metadata.md` дүүгийн симулятор үр.

## Нотлох баримтын хүлээлт

- JSON файлууд <100 КиБ хэвээр байх ёстой бөгөөд ингэснээр git-д амьдрах боломжтой. Илүү том гүйцэтгэл
  ул мөр нь гадаад санах ойд харьяалагддаг - мета өгөгдөл дэх тэдгээрийн гарын үсэгтэй хэшийг лавлана
  шаардлагатай бол тэмдэглэ.
- Архивлагдсан файл бүр нь үр, тохиргооны зам, симулятор хувилбарыг жагсаасан байх ёстой
  DA хувилбарын хаалгыг аудит хийх үед дахин давталтыг яг таг хуулбарлаж болно.
- `status.md`-аас архивлагдсан файл эсвэл замын газрын зураг руу хүссэн үедээ холбоно уу.
  DA-1-ийг хүлээн авах шалгуурыг ахиулах нь хянагчдыг баталгаажуулах боломжийг олгоно
  оосорыг дахин ажиллуулахгүйгээр үндсэн .

## Үүрэг даалгаврыг нэгтгэх (дараалагчийг орхигдуулсан)

DA хүлээн авсан баримтыг харьцуулахын тулд `cargo xtask da-commitment-reconcile` ашиглана уу
DA-ийн амлалтын бүртгэл, дараалал тогтоогчийн орхигдуулсан эсвэл хөндлөнгийн оролцоо:

```bash
cargo xtask da-commitment-reconcile \
  --receipt artifacts/da/receipts/ \
  --block storage/blocks/ \
  --json-out artifacts/da/commitment_reconciliation.json
```

- Norito эсвэл JSON маягтын баримт, амлалтуудыг хүлээн авна.
  `SignedBlockWire`, `.norito`, эсвэл JSON багцууд.
- Блокны бүртгэлээс тасалбар дутуу эсвэл хэш ялгарах үед амжилтгүй болно;
  Таныг санаатайгаар хамрах үед `--allow-unexpected` зөвхөн блоклох тасалбарыг үл тоомсорлодог.
  баримтын багц.
- Гаргасан JSON-г орхигдуулсан тохиолдолд удирдлагын пакет/Alertmanager-д хавсаргана уу
  сэрэмжлүүлэг; анхдагч нь `artifacts/da/commitment_reconciliation.json`.

## Давуу эрхийн аудит (Улирал тутмын хандалтын хяналт)

DA манифест/дахин тоглуулах лавлахуудыг скан хийхийн тулд `cargo xtask da-privilege-audit` ашиглана уу
(нэмэх нэмэлт замууд) байхгүй, лавлахгүй эсвэл дэлхийн хэмжээнд бичигдэх боломжтой
оруулгууд:

```bash
cargo xtask da-privilege-audit \
  --config configs/torii.dev.toml \
  --extra-path /var/lib/iroha/da-manifests \
  --json-out artifacts/da/privilege_audit.json
```

- Өгөгдсөн Torii тохиргооноос DA залгих замыг уншиж, Unix-ийг шалгана
  боломжтой бол зөвшөөрөл.
- Дутуу/санал биш/дэлхийд бичигдэх замуудыг тэмдэглэж, тэгээс өөр гарцыг буцаана
  асуудал гарсан тохиолдолд кодчилно.
- JSON багцад гарын үсэг зурж хавсаргана (`artifacts/da/privilege_audit.json` by
  анхдагч) пакетууд болон хяналтын самбаруудад улирал тутам хандалт хийх.