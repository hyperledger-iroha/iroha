---
id: preview-host-exposure
lang: uz
direction: ltr
source: docs/portal/docs/devportal/preview-host-exposure.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: Preview host exposure guide
sidebar_label: Preview host exposure
description: Publish and verify the beta preview host before sending invites.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

DOCS‑SORA yoʻl xaritasi har bir umumiy koʻrishni bir xilda harakatlanishini talab qiladi.
Tekshiruvchilar mahalliy darajada foydalanadigan nazorat summasi bilan tasdiqlangan to'plam. Ushbu runbook dan foydalaning
ko'rib chiquvchi ishga tushirilgandan so'ng (va taklifni tasdiqlash chiptasi) qo'yish tugallangan
beta-versiyasini onlayn ko'rish.

## Old shartlar

- Sharhlovchini ishga tushirish to'lqini tasdiqlandi va oldindan ko'rish kuzatuvchisiga kirdi.
- Eng so'nggi portal tuzilishi `docs/portal/build/` va nazorat summasi ostida mavjud
  tasdiqlangan (`build/checksums.sha256`).
- SoraFS hisob ma'lumotlarini oldindan ko'rish (Torii URL, vakolat, shaxsiy kalit, taqdim etilgan
  epoch) muhit o'zgaruvchilarida yoki JSON konfiguratsiyasida saqlanadi
  [`docs/examples/sorafs_preview_publish.json`](../../../examples/sorafs_preview_publish.json).
- DNS o'zgartirish chiptasi kerakli host nomi bilan ochilgan (`docs-preview.sora.link`,
  `docs.iroha.tech` va boshqalar) qo'ng'iroq bo'yicha kontaktlar.

## 1-qadam - To'plamni yarating va tekshiring

```bash
cd docs/portal
export DOCS_RELEASE_TAG="preview-$(date -u +%Y%m%dT%H%M%SZ)"
npm ci
npm run build
./scripts/preview_verify.sh --build-dir build
```

Tekshirish skripti nazorat summasi manifesti yoʻq boʻlganda yoki davom etishni rad etadi
o'zgartirilgan, har bir oldindan ko'rish artefaktini tekshirgan holda.

## 2-qadam - SoraFS artefaktlarini to'plang

Statik saytni deterministik CAR/manifest juftligiga aylantiring. `ARTIFACT_DIR`
sukut bo'yicha `docs/portal/artifacts/`.

```bash
./scripts/sorafs-pin-release.sh \
  --alias docs-preview.sora \
  --alias-namespace docs \
  --alias-name preview \
  --pin-label docs-preview \
  --skip-submit

node scripts/generate-preview-descriptor.mjs \
  --manifest artifacts/checksums.sha256 \
  --archive artifacts/sorafs/portal.tar.gz \
  --out artifacts/sorafs/preview-descriptor.json
```

Yaratilgan `portal.car`, `portal.manifest.*`, deskriptor va nazorat summasini biriktiring
oldindan ko'rish to'lqin chiptasiga manifest.

## 3-qadam – Oldindan koʻrish taxallusni nashr qilish

PIN-yordamchini **siz** `--skip-submit` ochishga tayyor bo'lgach qayta ishga tushiring
uy egasi. JSON konfiguratsiyasini yoki aniq CLI bayroqlarini taqdim eting:

```bash
./scripts/sorafs-pin-release.sh \
  --alias docs-preview.sora \
  --alias-namespace docs \
  --alias-name preview \
  --pin-label docs-preview \
  --config ~/secrets/sorafs_preview_publish.json
```

Buyruq `portal.pin.report.json` deb yozadi,
`portal.manifest.submit.summary.json` va `portal.submit.response.json`, qaysi
taklif dalillar to'plami bilan jo'natish kerak.

## 4-qadam – DNS kesish rejasini yarating

```bash
node scripts/generate-dns-cutover-plan.mjs \
  --dns-hostname docs.iroha.tech \
  --dns-zone sora.link \
  --dns-change-ticket DOCS-SORA-Preview \
  --dns-cutover-window "2026-03-05 18:00Z" \
  --dns-ops-contact "pagerduty:sre-docs" \
  --manifest artifacts/sorafs/portal.manifest.to \
  --cache-purge-endpoint https://cache.api/purge \
  --cache-purge-auth-env CACHE_PURGE_TOKEN \
  --out artifacts/sorafs/portal.dns-cutover.json
```

Olingan JSON-ni Ops bilan baham ko'ring, shunda DNS kaliti aniq ma'lumotga murojaat qiladi
manifest hazm qilish. Oldingi identifikatorni orqaga qaytarish manbai sifatida qayta ishlatganda,
`--previous-dns-plan path/to/previous.json` qo'shing.

## 5-qadam - O'rnatilgan xostni tekshirish

```bash
npm run probe:portal -- \
  --base-url=https://docs-preview.sora.link \
  --expect-release="$DOCS_RELEASE_TAG"
```

Tekshiruv taqdim etilgan nashr yorlig'i, CSP sarlavhalari va imzo metama'lumotlarini tasdiqlaydi.
Auditorlar ko'rishlari uchun buyruqni ikkita mintaqadan takrorlang (yoki jingalak chiqishni biriktiring).
chekka kesh issiq ekanligini.

## Dalillar to'plami

Oldindan ko'rish to'lqin chiptasiga quyidagi artefaktlarni qo'shing va ularga murojaat qiling
taklif elektron pochtasi:

| Artefakt | Maqsad |
|----------|---------|
| `build/checksums.sha256` | To'plam CI tuzilishiga mos kelishini isbotlaydi. |
| `artifacts/sorafs/portal.tar.gz` + `portal.manifest.to` | Kanonik SoraFS foydali yuk + manifest. |
| `portal.pin.report.json`, `portal.manifest.submit.summary.json`, `portal.submit.response.json` | Manifest yuborilishini ko'rsatadi + taxallusni bog'lash muvaffaqiyatli. |
| `artifacts/sorafs/portal.dns-cutover.json` | DNS metamaʼlumotlari (chipta, oyna, kontaktlar), marshrutni ilgari surish (`Sora-Route-Binding`) xulosasi, `route_plan` koʻrsatkichi (JSON rejasi + sarlavha shablonlari), keshni tozalash maʼlumotlari va Ops uchun orqaga qaytarish koʻrsatmalari. |
| `artifacts/sorafs/preview-descriptor.json` | Arxiv + nazorat summasini bir-biriga bog'laydigan imzolangan deskriptor. |
| `probe` chiqishi | Jonli xost kutilgan reliz yorlig'ini reklama qilishini tasdiqlaydi. |

Mezbon jonli efirga chiqqandan so‘ng, [taklifni oldindan ko‘rish kitobi](./public-preview-invite.md) ga amal qiling.
havolani tarqatish, takliflarni qayd qilish va telemetriyani kuzatish.