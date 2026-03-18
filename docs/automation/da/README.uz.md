---
lang: uz
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

# Ma'lumotlar mavjudligi tahdid modelini avtomatlashtirish (DA-1)

Yo'l xaritasining DA-1 va `status.md` bandlari deterministik avtomatlashtirish tsiklini talab qiladi
da paydo bo'lgan Norito PDP/PoTR tahdid modeli xulosalarini ishlab chiqaradi.
`docs/source/da/threat_model.md` va Docusaurus oynasi. Ushbu katalog
tomonidan havola qilingan artefaktlarni suratga oladi:

- `cargo xtask da-threat-model-report [--out <path|->] [--seed <u64|0xhex>] [--config <path>]`
- `.github/workflows/da-threat-model-nightly.yml`
- `make docs-da-threat-model` (`scripts/docs/render_da_threat_model_tables.py` bilan ishlaydi)
- `cargo xtask da-commitment-reconcile --receipt <path> --block <path> [--json-out <path|->]`
- `cargo xtask da-privilege-audit --config <torii.toml> [--extra-path <path> ...] [--json-out <path|->]`

## Oqim

1. **Hisobotni yarating**
   ```bash
   cargo xtask da-threat-model-report \
     --config configs/da/threat_model.toml \
     --out artifacts/da/threat_model_report.json
   ```
   JSON xulosasi simulyatsiya qilingan replikatsiya xatosi tezligini, chunkerni qayd qiladi
   chegaralar va PDP/PoTR tizimi tomonidan aniqlangan har qanday siyosat buzilishi
   `integration_tests/src/da/pdp_potr.rs`.
2. **Markdown jadvallarini ko'rsating**
   ```bash
   make docs-da-threat-model
   ```
   Bu qayta yozish uchun `scripts/docs/render_da_threat_model_tables.py` ishlaydi
   `docs/source/da/threat_model.md` va `docs/portal/docs/da/threat-model.md`.
3. JSON hisobotini (va ixtiyoriy CLI jurnalini) nusxalash orqali **artefaktni arxivlang**
   `docs/automation/da/reports/<timestamp>-threat_model_report.json`. Qachon
   boshqaruv qarorlari ma'lum bir ishga tayanadi, jumladan git commit xesh va
   `<timestamp>-metadata.md` birodaridagi simulyator urug'i.

## Dalillarni kutish

- JSON fayllari git-da yashashi uchun <100 KiB bo'lishi kerak. Kattaroq ijro
  izlar tashqi xotiraga tegishli - metama'lumotlarda ularning imzolangan xeshiga havola
  agar kerak bo'lsa, eslatma.
- Har bir arxivlangan faylda urug', konfiguratsiya yo'li va simulyator versiyasi ko'rsatilgan bo'lishi kerak
  DA reliz eshiklarini tekshirish paytida takroriy takrorlash mumkin.
- `status.md` dan arxivlangan faylga yoki yo'l xaritasi yozuviga istalgan vaqtda havola qiling
  DA-1 qabul qilish mezonlarini oldindan belgilab, sharhlovchilar buni tekshirishlari mumkin
  jabduqni qayta ishga tushirmasdan asosiy chiziq.

## Majburiyatni solishtirish (Sequencer bo'lmasligi)

DA qabul qilish tushumlarini solishtirish uchun `cargo xtask da-commitment-reconcile` dan foydalaning
DA majburiyatlari yozuvlari, sekvenser qoldirilishi yoki o'zgartirilishi:

```bash
cargo xtask da-commitment-reconcile \
  --receipt artifacts/da/receipts/ \
  --block storage/blocks/ \
  --json-out artifacts/da/commitment_reconciliation.json
```

- Norito yoki JSON shaklida kvitansiya va majburiyatlarni qabul qiladi
  `SignedBlockWire`, `.norito` yoki JSON to'plamlari.
- Bloklash jurnalida biron bir chipta yo'q bo'lganda yoki xeshlar ajralib chiqqanda muvaffaqiyatsiz bo'ladi;
  `--allow-unexpected` qasddan qamrab olganingizda faqat blokirovka qilinadigan chiptalarga e'tibor bermaydi
  kvitansiya to'plami.
- Emissiya qilingan JSON-ni boshqaruv paketlariga/Alertmanager-ga qo'shib qo'ying
  ogohlantirishlar; sukut bo'yicha `artifacts/da/commitment_reconciliation.json`.

## Imtiyoz auditi (har chorakda foydalanishni tekshirish)

DA manifest/takrorlash kataloglarini skanerlash uchun `cargo xtask da-privilege-audit` dan foydalaning
(plyus ixtiyoriy qo'shimcha yo'llar) etishmayotgan, katalog bo'lmagan yoki dunyoda yozilishi mumkin
yozuvlar:

```bash
cargo xtask da-privilege-audit \
  --config configs/torii.dev.toml \
  --extra-path /var/lib/iroha/da-manifests \
  --json-out artifacts/da/privilege_audit.json
```

- Taqdim etilgan Torii konfiguratsiyasidan DA qabul qilish yo'llarini o'qiydi va Unixni tekshiradi
  ruxsatlar mavjud bo'lganda.
- Yo'qolgan/katalog bo'lmagan/dunyoda yozilmaydigan yo'llarni belgilab qo'yadi va nolga teng bo'lmagan chiqishni qaytaradi
  muammolar mavjud bo'lganda kod.
- JSON to'plamini imzolang va biriktiring (`artifacts/da/privilege_audit.json` by
  sukut bo'yicha) har chorakda paketlar va asboblar paneliga kirish-ko'rib chiqish.