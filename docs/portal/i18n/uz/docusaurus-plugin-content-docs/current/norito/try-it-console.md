---
lang: uz
direction: ltr
source: docs/portal/docs/norito/try-it-console.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: Norito Try-It Console
description: Use the developer-portal proxy, Swagger, and RapiDoc widgets to send real Torii / Norito-RPC requests directly from the documentation site.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

Portal trafikni Torii ga uzatuvchi uchta interaktiv sirtni birlashtiradi:

- `/reference/torii-swagger` da **Swagger UI** imzolangan OpenAPI spetsifikatsiyasini ko‘rsatadi va `TRYIT_PROXY_PUBLIC_URL` o‘rnatilganda avtomatik ravishda proksi-server orqali so‘rovlarni qayta yozadi.
- `/reference/torii-rapidoc` da **RapiDoc** fayl yuklash va `application/x-norito` uchun yaxshi ishlaydigan kontent turi selektorlari bilan bir xil sxemani ochib beradi.
- **Norito umumiy koʻrinish sahifasidagi sinov muhiti** maxsus REST soʻrovlari va OAuth qurilmasiga kirish uchun engil shaklni taqdim etadi.

Barcha uchta vidjet mahalliy **Try-It proksi-serveriga** (`docs/portal/scripts/tryit-proxy.mjs`) so‘rov yuboradi. Proksi-server `static/openapi/torii.json` `static/openapi/manifest.json` da imzolangan dayjestga mos kelishini tekshiradi, tezlikni cheklovchini kiritadi, jurnallardagi `X-TryIt-Auth` sarlavhalarini o'zgartiradi va har bir yuqori oqim qo'ng'irog'ini `X-TryIt-Client` bilan teglaydi, shuning uchun I010 manbasi1NT0 trafigini tekshiradi.

## Proksi-serverni ishga tushiring

```bash
cd docs/portal
npm install
export TRYIT_PROXY_TARGET="https://torii.devnet.sora.example"
export TRYIT_PROXY_ALLOWED_ORIGINS="http://localhost:3000"
export TRYIT_PROXY_PUBLIC_URL="http://localhost:8787"
# Optional, use short-lived tokens only:
export TRYIT_PROXY_BEARER="Bearer eyJhbGciOi..."
export TRYIT_PROXY_CLIENT_ID="docs-portal"
export DOCS_TRYIT_ALLOW_DEFAULT_BEARER=1
npm run tryit-proxy
```

- `TRYIT_PROXY_TARGET` - siz mashq qilmoqchi bo'lgan Torii asosiy URL.
- `TRYIT_PROXY_ALLOWED_ORIGINS` konsolni o'rnatishi kerak bo'lgan har bir portal manbasini (mahalliy ishlab chiquvchi server, ishlab chiqarish xost nomi, oldindan ko'rish URL) o'z ichiga olishi kerak.
- `TRYIT_PROXY_PUBLIC_URL` `docusaurus.config.js` tomonidan iste'mol qilinadi va `customFields.tryIt` orqali vidjetlarga kiritiladi.
- `TRYIT_PROXY_BEARER` faqat `DOCS_TRYIT_ALLOW_DEFAULT_BEARER=1`da yuklanadi; aks holda foydalanuvchilar konsol yoki OAuth qurilma oqimi orqali o'z tokenlarini taqdim etishlari kerak.
- `TRYIT_PROXY_CLIENT_ID` har bir so'rov bo'yicha olib boriladigan `X-TryIt-Client` tegini o'rnatadi.
  Brauzerdan `X-TryIt-Client` ni etkazib berishga ruxsat berilgan, ammo qiymatlar kesilgan
  va agar ular nazorat belgilaridan iborat bo'lsa, rad etiladi.

Ishga tushganda proksi-server `verifySpecDigest`-ni ishga tushiradi va agar manifest eskirgan bo'lsa, tuzatish ko'rsatmasi bilan chiqadi. Eng yangi Torii spetsifikatsiyasini yuklab olish uchun `npm run sync-openapi -- --latest` ni ishga tushiring yoki favqulodda vaziyatni bekor qilish uchun `TRYIT_PROXY_ALLOW_STALE_SPEC=1` dan oʻting.

Atrof-muhit fayllarini qo'lda tahrirlamasdan proksi-maqsadni yangilash yoki orqaga qaytarish uchun yordamchidan foydalaning:

```bash
npm run manage:tryit-proxy -- update --target https://new.torii.example
npm run manage:tryit-proxy -- rollback
```

## Vidjetlarni ulash

Proksi-server tinglagandan keyin portalga xizmat ko'rsatish:

```bash
cd docs/portal
TRYIT_PROXY_PUBLIC_URL="http://localhost:8787" npm run start
```

`docusaurus.config.js` quyidagi tugmalarni ochib beradi:

| O'zgaruvchi | Maqsad |
| --- | --- |
| `TRYIT_PROXY_PUBLIC_URL` | URL Swagger, RapiDoc va Sinab ko'ring sinov maydoniga kiritilgan. Ruxsatsiz koʻrib chiqish vaqtida vidjetlarni yashirish uchun sozlanmagan holda qoldiring. |
| `TRYIT_PROXY_DEFAULT_BEARER` | Xotirada saqlanadigan ixtiyoriy standart token. Mahalliy ravishda `DOCS_SECURITY_ALLOW_INSECURE=1`dan oʻtmasangiz, `DOCS_TRYIT_ALLOW_DEFAULT_BEARER=1` va faqat HTTPS uchun CSP himoyasi (DOCS-1b) talab qilinadi. |
| `DOCS_OAUTH_*` | OAuth qurilma oqimini (`OAuthDeviceLogin` komponenti) yoqing, shunda sharhlovchilar portaldan chiqmasdan qisqa muddatli tokenlarni zarb qilishlari mumkin. |

OAuth o'zgaruvchilari mavjud bo'lganda, sinov maydoni sozlangan Auth serveri bo'ylab o'tadigan **Qurilma kodi bilan tizimga kirish** tugmachasini ko'rsatadi (aniq shakl uchun `config/security-helpers.js` ga qarang). Qurilma oqimi orqali chiqarilgan tokenlar faqat brauzer seansida keshlanadi.

## Norito-RPC foydali yuklarni yuborish

1. [Norito tezkor boshlash](./quickstart.md) da tasvirlangan CLI yoki parchalar bilan `.norito` foydali yukini yarating. Proksi-server `application/x-norito` jismlarini o'zgarishsiz yo'naltiradi, shuning uchun siz `curl` bilan joylashtirgan bir xil artefaktni qayta ishlatishingiz mumkin.
2. `/reference/torii-rapidoc` (ikkilik foydali yuklar uchun afzal) yoki `/reference/torii-swagger` ni oching.
3. Ochiladigan menyudan kerakli Torii suratini tanlang. Suratlar imzolanadi; panel `static/openapi/manifest.json` da yozilgan manifest dayjestini ko'rsatadi.
4. “Sinab ko‘ring” tortmasida `application/x-norito` kontent turini tanlang, **Faylni tanlang**-ni bosing va foydali yukingizni tanlang. Proksi-server so'rovni `/proxy/v1/pipeline/submit` ga qayta yozadi va uni `X-TryIt-Client=docs-portal-rapidoc` bilan teglaydi.
5. Norito javoblarini yuklab olish uchun `Accept: application/x-norito` ni o'rnating. Swagger/RapiDoc bir xil tortmada sarlavha selektorini ochib beradi va ikkilik faylni proksi-server orqali qaytaradi.

Faqat JSON marshrutlari uchun oʻrnatilgan “Try it” sinov maydoni koʻpincha tezroq boʻladi: yoʻlni kiriting (masalan, `/v1/accounts/i105.../assets`), HTTP usulini tanlang, kerak boʻlganda JSON korpusini joylashtiring va sarlavhalar, davomiylik va foydali yuklarni satrda tekshirish uchun **Soʻrov yuborish** tugmasini bosing.

## Nosozliklarni bartaraf etish

| Alomat | Ehtimoliy sabab | Tuzatish |
| --- | --- | --- |
| Brauzer konsoli CORS xatolarini ko'rsatadi yoki sinov muhiti proksi-server URL manzili yo'qligi haqida ogohlantiradi. | Proksi ishlamayapti yoki manba oq ro'yxatga kiritilmagan. | Proksi-serverni ishga tushiring, `TRYIT_PROXY_ALLOWED_ORIGINS` portal xostingizni qamrab olganligiga ishonch hosil qiling va `npm run start`-ni qayta ishga tushiring. |
| `npm run tryit-proxy` “digest nomuvofiqligi” bilan chiqadi. | Torii OpenAPI toʻplami yuqori oqimga oʻzgartirildi. | `npm run sync-openapi -- --latest` (yoki `--version=<tag>`) ni ishga tushiring va qayta urinib ko'ring. |
| Vidjetlar `401` yoki `403` qaytaradi. | Token yoʻq, muddati oʻtgan yoki qamrovlar yetarli emas. | OAuth qurilma oqimidan foydalaning yoki yaroqli tokenni sinov qutisiga joylashtiring. Statik tokenlar uchun siz `DOCS_TRYIT_ALLOW_DEFAULT_BEARER=1` ni eksport qilishingiz kerak. |
| Proksi-serverdan `429 Too Many Requests`. | IP uchun tarif chegarasidan oshib ketdi. | Ishonchli muhitlar yoki gaz kelebeği test skriptlari uchun `TRYIT_PROXY_RATE_LIMIT`/`TRYIT_PROXY_RATE_WINDOW_MS` ni ko'taring. Barcha tarif cheklovini rad etish `tryit_proxy_rate_limited_total`. |

## Kuzatish imkoniyati

- `npm run probe:tryit-proxy` (`scripts/tryit-proxy-probe.mjs` atrofidagi oʻram) `/healthz` ga qoʻngʻiroq qiladi, ixtiyoriy ravishda namunaviy marshrutni ishlatadi va `probe_success` / I100080 uchun Prometheus matn fayllarini chiqaradi. `TRYIT_PROXY_PROBE_METRICS_FILE` ni node_exporter bilan integratsiya qilish uchun sozlang.
- Hisoblagichlar (`tryit_proxy_requests_total`, `tryit_proxy_rate_limited_total`, `tryit_proxy_upstream_failures_total`) va kechikish gistogrammalarini ochish uchun `TRYIT_PROXY_METRICS_LISTEN=127.0.0.1:9798` ni sozlang. `dashboards/grafana/docs_portal.json` boshqaruv kengashi DOCS-SORA SLOlarni qo'llash uchun ushbu ko'rsatkichlarni o'qiydi.
- Ish vaqti jurnallari stdout-da jonli. Har bir yozuv so'rov identifikatori, yuqori oqim holati, autentifikatsiya manbai (`default`, `override` yoki `client`) va davomiylikni o'z ichiga oladi; sirlar emissiyadan oldin tahrir qilinadi.

Agar siz `application/x-norito` foydali yuklari Torii ga o'zgarmaganligini tekshirishingiz kerak bo'lsa, Jest to'plamini (`npm test -- tryit-proxy`) ishga tushiring yoki `docs/portal/scripts/__tests__/tryit-proxy.test.mjs` ostidagi armaturalarni tekshiring. Regressiya testlari siqilgan Norito ikkilik fayllarini, imzolangan OpenAPI manifestlarini va proksi-serverni pasaytirish yo'llarini qamrab oladi, shuning uchun NRPC chiqarilishi doimiy dalillar izini saqlab qoladi.