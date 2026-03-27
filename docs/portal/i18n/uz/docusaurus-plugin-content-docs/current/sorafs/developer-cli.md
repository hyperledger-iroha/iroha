---
id: developer-cli
lang: uz
direction: ltr
source: docs/portal/docs/sorafs/developer-cli.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: SoraFS CLI Cookbook
sidebar_label: CLI Cookbook
description: Task-focused walkthrough of the consolidated `sorafs_cli` surface.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

::: Eslatma Kanonik manba
:::

Konsolidatsiyalangan `sorafs_cli` yuzasi (`sorafs_car` sandiq bilan ta'minlangan.
`cli` xususiyati yoqilgan) SoraFS tayyorlash uchun zarur bo'lgan har bir qadamni ochib beradi.
artefaktlar. To'g'ridan-to'g'ri umumiy ish oqimlariga o'tish uchun ushbu oshxona kitobidan foydalaning; bilan bog'lang
manifest quvur liniyasi va operativ kontekst uchun orkestratorning ish kitoblari.

## Paket yuklari

Deterministik CAR arxivlari va parcha rejalarini yaratish uchun `car pack` dan foydalaning. The
Agar tutqich berilmasa, buyruq avtomatik ravishda SF-1 chunkerini tanlaydi.

```bash
sorafs_cli car pack \
  --input fixtures/video.mp4 \
  --car-out artifacts/video.car \
  --plan-out artifacts/video.plan.json \
  --summary-out artifacts/video.car.json
```

- Standart chunker tutqichi: `sorafs.sf1@1.0.0`.
- Katalogga kiritilgan ma'lumotlar leksikografik tartibda ko'rib chiqiladi, shuning uchun nazorat summalari barqaror bo'lib qoladi
  platformalar bo'ylab.
- JSON xulosasi foydali yuk dayjestlarini, har bir parcha metama'lumotlarini va ildizni o'z ichiga oladi
  CID ro'yxatga olish kitobi va orkestr tomonidan tan olingan.

## Manifestlarni qurish

```bash
sorafs_cli manifest build \
  --summary artifacts/video.car.json \
  --pin-min-replicas 4 \
  --pin-storage-class hot \
  --pin-retention-epoch 96 \
  --manifest-out artifacts/video.manifest.to \
  --manifest-json-out artifacts/video.manifest.json
```

- `--pin-*` opsiyalari to'g'ridan-to'g'ri `PinPolicy` maydonlariga xaritada
  `sorafs_manifest::ManifestBuilder`.
- CLI SHA3 qismini qayta hisoblashini xohlasangiz, `--chunk-plan` ni taqdim eting
  topshirishdan oldin hazm qilish; aks holda u ichiga o'rnatilgan dayjestni qayta ishlatadi
  xulosa.
- JSON chiqishi to'g'ridan-to'g'ri farqlar uchun Norito foydali yukini aks ettiradi.
  sharhlar.

## Belgi uzoq muddatli kalitlarsiz namoyon bo'ladi

```bash
sorafs_cli manifest sign \
  --manifest artifacts/video.manifest.to \
  --bundle-out artifacts/video.manifest.bundle.json \
  --signature-out artifacts/video.manifest.sig \
  --identity-token-env SIGSTORE_ID_TOKEN
```

- Inline tokenlar, muhit o'zgaruvchilari yoki faylga asoslangan manbalarni qabul qiladi.
- Kelib chiqishi meta-ma'lumotlarini qo'shadi (`token_source`, `token_hash_hex`, parcha dayjest)
  agar `--include-token=true` bo'lmasa, xom JWTni saqlamasdan.
- CIda yaxshi ishlaydi: sozlash orqali GitHub Actions OIDC bilan birlashtiring
  `--identity-token-provider=github-actions`.

## Manifestlarni Torii ga yuboring

```bash
sorafs_cli manifest submit \
  --manifest artifacts/video.manifest.to \
  --chunk-plan artifacts/video.plan.json \
  --torii-url https://gateway.example/v1 \
  --authority <i105-account-id> \
  --private-key ed25519:0123...beef \
  --alias-namespace sora \
  --alias-name video::launch \
  --alias-proof fixtures/alias_proof.bin \
  --summary-out artifacts/video.submit.json
```

- Taxallus isbotlari uchun Norito dekodlashni amalga oshiradi va ularning talablarga mos kelishini tekshiradi.
  Torii ga POST qilishdan oldin manifest dayjest.
- Mos kelmaslik hujumlarining oldini olish uchun SHA3 dayjestini rejadan qayta hisoblab chiqadi.
- Javoblar sarlavhalari HTTP holatini, sarlavhalarni va ro'yxatga olish kitobining foydali yuklarini oladi
  keyinchalik audit.

## CAR tarkibi va dalillarini tekshiring

```bash
sorafs_cli proof verify \
  --manifest artifacts/video.manifest.to \
  --car artifacts/video.car \
  --summary-out artifacts/video.verify.json
```

- PoR daraxtini qayta quradi va foydali yuk dayjestlarini manifest xulosasi bilan taqqoslaydi.
- Replikatsiya dalillarini topshirishda talab qilinadigan hisoblar va identifikatorlarni oladi
  boshqaruvga.

## Oqimli telemetriya

```bash
sorafs_cli proof stream \
  --manifest artifacts/video.manifest.to \
  --gateway-url https://gateway.example/v1/sorafs/proof/stream \
  --provider-id provider::alpha \
  --samples 32 \
  --stream-token "$(cat stream.token)" \
  --summary-out artifacts/video.proof_stream.json \
  --governance-evidence-dir artifacts/video.proof_stream_evidence
```

- Har bir oqim isboti uchun NDJSON elementlarini chiqaradi (bilan takrorlashni o'chiring
  `--emit-events=false`).
- Muvaffaqiyat/muvaffaqiyatsizliklar sonini, kechikish gistogrammalarini va namunaviy xatoliklarni umumlashtiradi
  Xulosa JSON, shuning uchun asboblar paneli jurnallarni qirib tashlamasdan natijalarni rejalashtirishi mumkin.
- Shlyuz nosozliklar yoki mahalliy PoR tekshiruvi haqida xabar berganda noldan farq qiladi
  (`--por-root-hex` orqali) dalillarni rad etadi. bilan chegaralarni sozlang
  Mashqlar uchun `--max-failures` va `--max-verification-failures`.
- Bugungi kunda PoR ni qo'llab-quvvatlaydi; PDP va PoTR bir xil konvertdan bir marta SF-13/SF-14 foydalanadi
  yer.
- `--governance-evidence-dir` ko'rsatilgan xulosani, metama'lumotlarni yozadi (vaqt tamg'asi,
  CLI versiyasi, shlyuz URL manzili, manifest dayjesti) va manifestning nusxasi
  ta'minlangan katalog, shuning uchun boshqaruv paketlari proof-streamni arxivlashi mumkin
  yugurishni takrorlamasdan dalil.

## Qo'shimcha havolalar

- `docs/source/sorafs_cli.md` - to'liq bayroq hujjatlari.
- `docs/source/sorafs_proof_streaming.md` - isbotlangan telemetriya sxemasi va Grafana
  asboblar paneli shabloni.
- `docs/source/sorafs/manifest_pipeline.md` - manifest bo'yicha chuqur sho'ng'in
  tarkibi va AVTOMOBIL boshqaruvi.