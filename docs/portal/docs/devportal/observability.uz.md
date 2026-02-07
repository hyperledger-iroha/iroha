---
lang: uz
direction: ltr
source: docs/portal/docs/devportal/observability.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 5f6d17a605e4b90d9bb9cc041055c43b2f1b384fd13f732c0a56e5de5fe78bbd
source_last_modified: "2025-12-29T18:16:35.105743+00:00"
translation_last_reviewed: 2026-02-07
id: observability
title: Portal Observability & Analytics
sidebar_label: Observability
description: Telemetry, release tagging, and verification automation for the developer portal.
translator: machine-google-reviewed
---

DOCS-SORA yoʻl xaritasi analitika, sintetik zondlar va uzilgan aloqani talab qiladi.
har bir oldindan ko'rish qurilishi uchun avtomatlashtirish. Ushbu eslatma hozirda sanitariya-tesisat haqida ma'lumot beradi
portal bilan jo'natiladi, shuning uchun operatorlar tashrif buyuruvchilarni oqizmasdan monitoring qilishlari mumkin
ma'lumotlar.

## Teglarni chiqarish

- `DOCS_RELEASE_TAG=<identifier>` sozlang (`GIT_COMMIT` yoki `dev` ga qaytadi) qachon
  portal qurish. Qiymat `<meta name="sora-release">` ichiga kiritiladi
  shuning uchun problar va asboblar paneli joylashtirishni ajrata oladi.
- `npm run build` `build/release.json` chiqaradi (yozgan:
  `scripts/write-checksums.mjs`) tegni, vaqt tamg'asini va ixtiyoriyni tavsiflaydi
  `DOCS_RELEASE_SOURCE`. Xuddi shu fayl oldindan ko'rish artefaktlariga birlashtirilgan va
  havola tekshiruvi hisobotiga havola qilingan.

## Maxfiylikni saqlaydigan tahlillar

- `DOCS_ANALYTICS_ENDPOINT=<https://collector.example/ingest>` ni sozlang
  engil vaznli trekerni yoqing. Foydali yuklar `{ hodisa, yo'l, mahalliy parametr,
  reliz, ts }` with no referrer or IP metadata, and `navigator.sendBeacon`
  navigatsiyalarni blokirovka qilmaslik uchun imkon qadar foydalaniladi.
- `DOCS_ANALYTICS_SAMPLE_RATE` (0–1) bilan namuna olishni nazorat qilish. Kuzatuvchi saqlaydi
  oxirgi yuborilgan yo'l va hech qachon bir xil navigatsiya uchun takroriy hodisalarni chiqarmaydi.
- Amalga oshirish `src/components/AnalyticsTracker.jsx` da yashaydi va shunday
  `src/theme/Root.js` orqali global o'rnatilgan.

## Sintetik zondlar

- `npm run probe:portal` umumiy marshrutlarga qarshi GET so'rovlarini chiqaradi
  (`/`, `/norito/overview`, `/reference/torii-swagger` va boshqalar) va tasdiqlaydi
  `sora-release` meta yorlig'i `--expect-release` (yoki)
  `DOCS_RELEASE_TAG`). Misol:

```bash
PORTAL_BASE_URL="https://docs.staging.sora" \
DOCS_RELEASE_TAG="preview-42" \
npm run probe:portal -- --expect-release=preview-42
```

Muvaffaqiyatsizliklar to'g'risida har bir yo'lda xabar beriladi, bu esa probning muvaffaqiyati haqida kompakt-diskni ochishni osonlashtiradi.

## Buzilgan havolalarni avtomatlashtirish

- `npm run check:links` `build/sitemap.xml` ni skanerlaydi, har bir kirish xaritasini
  mahalliy fayl (`index.html` zaxiralarini tekshirish) va yozadi
  `build/link-report.json` reliz metamaʼlumotlari, jamilar, nosozliklar,
  va `checksums.sha256` SHA-256 barmoq izi (`manifest.id` sifatida ochilgan)
  shuning uchun har bir hisobotni artefakt manifestiga bog'lash mumkin.
- Sahifa etishmayotganda skript noldan farq qiladi, shuning uchun CI nashrlarni bloklashi mumkin
  eskirgan yoki buzilgan marshrutlar. Hisobotlarda nomzod yo'llariga iqtibos keltiriladi,
  bu hujjatlar daraxtiga marshrutlash regressiyalarini kuzatishga yordam beradi.

## Grafana asboblar paneli va ogohlantirishlar

- `dashboards/grafana/docs_portal.json` **Docs Portal Publishing**-ni nashr etadi
  Grafana doska. U quyidagi panellarni yuboradi:
  - *Gateway Redusals (5m)* `torii_sorafs_gateway_refusals_total` dan foydalanadi
    `profile`/`reason`, shuning uchun SRElar noto'g'ri siyosat surishlari yoki token xatolarini aniqlay oladi.
  - *Alias Keshni yangilash natijalari* va *Alias Proof Age p90* trek
    DNS uzilishidan oldin yangi dalillar mavjudligini isbotlash uchun `torii_sorafs_alias_cache_*`
    tugadi.
  - *Ro‘yxatga olish kitobi manifestlari soni* va *Faol taxalluslar soni* statistik ma’lumotlarni aks ettiradi.
    boshqaruv har bir nashrni tekshirishi mumkin bo'lgan pin-registrlar to'plami va umumiy taxalluslar.
  - *Gateway TLS muddati (soat)* nashr qilish shlyuzi TLS qachon ta'kidlanadi
    sertifikat muddati tugaydi (ogohlantirish chegarasi 72 soatda).
  - *Replikatsiya SLA natijalari* va *Replikatsiyaning orqada qolishi* kuzatib boring
    `torii_sorafs_replication_*` telemetriyasi barcha replikalarning GA talablariga javob berishiga ishonch hosil qilish
    nashrdan keyin bar.
- O'rnatilgan shablon o'zgaruvchilaridan (`profile`, `reason`) foydalaning.
  `docs.sora` nashriyot profili yoki barcha shlyuzlar bo'ylab ko'tarilishlarni tekshirib ko'ring.
- PagerDuty marshrutlash asboblar paneli panellaridan dalil sifatida foydalanadi: ogohlantirishlar nomli
  `DocsPortal/GatewayRefusals`, `DocsPortal/AliasCache` va
  `DocsPortal/TLSExpiry` mos keladigan seriyalar buzilganda yong'in
  chegaralar. Ogohlantirishning ish kitobini ushbu sahifaga bog'lang, shunda muhandislar qo'ng'iroq qilishlari mumkin
  aniq Prometheus so'rovlarini takrorlang.

## Uni birlashtirish

1. `npm run build` vaqtida reliz/tahlil muhiti oʻzgaruvchilarini oʻrnating va
   Qurilishdan keyingi bosqich `checksums.sha256`, `release.json` va
   `link-report.json`.
2. `npm run probe:portal` ni oldindan ko'rish xost nomiga qarshi ishga tushiring
   `--expect-release` xuddi shu tegga ulangan. stdoutni nashr qilish uchun saqlang
   nazorat ro'yxati.
3. Sayt xaritasidagi buzilgan yozuvlar va arxivda tezda ishlamay qolishi uchun `npm run check:links` ni ishga tushiring
   yaratilgan JSON hisobotini oldindan ko'rish artefaktlari bilan birga. CI ni tushiradi
   `artifacts/docs_portal/link-report.json` da so'nggi hisobot, shuning uchun boshqaruv mumkin
   dalillar to'plamini to'g'ridan-to'g'ri qurilish jurnallaridan yuklab oling.
4. Maxfiylikni saqlaydigan kollektoringizga tahlil soʻnggi nuqtasini yoʻnaltiring (Ishonchli,
   o'z-o'zidan yotqizilgan OTEL ingest va h.k.) va tanlab olish stavkalari hujjatlashtirilganligiga ishonch hosil qiling
   bo'shating, shunda asboblar paneli hisoblarni to'g'ri talqin qiladi.
5. CI allaqachon bu bosqichlarni oldindan ko'rish/joylashtirish ish oqimlari orqali o'tkazmoqda
   (`.github/workflows/docs-portal-preview.yml`,
   `.github/workflows/docs-portal-deploy.yml`), shuning uchun mahalliy quruq yugurishlar faqat kerak
   sirga xos xulq-atvorni qamrab oladi.