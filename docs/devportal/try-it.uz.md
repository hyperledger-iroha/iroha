---
lang: uz
direction: ltr
source: docs/devportal/try-it.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 791d88296d9e52d3272ce3ac324e498fa3c622c323edc8c988302efe5092f0b4
source_last_modified: "2025-12-29T18:16:35.067551+00:00"
translation_last_reviewed: 2026-02-07
title: Try It Sandbox Guide
summary: How to run the Torii staging proxy and developer portal sandbox.
translator: machine-google-reviewed
---

Ishlab chiquvchilar portali Torii REST API uchun "Sinab ko'ring" konsolini yuboradi. Ushbu qo'llanma
qo'llab-quvvatlovchi proksi-serverni ishga tushirish va konsolni sahnalashtirishga qanday ulash kerakligini tushuntiradi
hisob ma'lumotlarini oshkor qilmasdan shlyuz.

## Old shartlar

- Iroha omborini tekshirish (ish maydoni ildizi).
- Node.js 18.18+ (portal bazasiga mos keladi).
- Torii so'nggi nuqtasini ish stantsiyangizdan olish mumkin (staging yoki mahalliy).

## 1. OpenAPI suratini yarating (ixtiyoriy)

Konsol bir xil OpenAPI foydali yukini portal ma'lumotnoma sahifalari kabi qayta ishlatadi. Agar
Torii marshrutlarini o'zgartirdingiz, suratni qayta yarating:

```bash
cargo xtask openapi
```

Vazifa `docs/portal/static/openapi/torii.json` deb yozadi.

## 2. Try It proksi-serverini ishga tushiring

Repozitoriy ildizidan:

```bash
cd docs/portal

export TRYIT_PROXY_TARGET="https://torii.staging.sora"
export TRYIT_PROXY_ALLOWED_ORIGINS="http://localhost:3000"
# Optional defaults
export TRYIT_PROXY_BEARER="sora-dev-token"
export TRYIT_PROXY_LISTEN="127.0.0.1:8787"

npm run tryit-proxy
```

### Atrof-muhit o'zgaruvchilari

| O'zgaruvchi | Tavsif |
|----------|-------------|
| `TRYIT_PROXY_TARGET` | Torii asosiy URL (majburiy). |
| `TRYIT_PROXY_ALLOWED_ORIGINS` | Proksi-serverdan foydalanishga ruxsat berilgan manbalar roʻyxati vergul bilan ajratilgan (birlamchi `http://localhost:3000`). |
| `TRYIT_PROXY_BEARER` | Barcha proksi-server so‘rovlariga qo‘llaniladigan ixtiyoriy standart ko‘rsatuvchi token. |
| `TRYIT_PROXY_ALLOW_CLIENT_AUTH` | Qo'ng'iroq qiluvchining `Authorization` sarlavhasini so'zma-so'z yo'naltirish uchun `1` ga sozlang. |
| `TRYIT_PROXY_RATE_LIMIT` / `TRYIT_PROXY_RATE_WINDOW_MS` | Xotira tezligini cheklovchi sozlamalari (standart: 60 soniyada 60 ta so'rov). |
| `TRYIT_PROXY_MAX_BODY` | Maksimal soʻrov yuki qabul qilindi (bayt, standart 1MiB). |
| `TRYIT_PROXY_TIMEOUT_MS` | Torii so'rovlari uchun yuqori oqim kutish vaqti (standart 10000ms). |

Proksi quyidagilarni ochib beradi:

- `GET /healthz` - tayyorlikni tekshirish.
- `/proxy/*` - yo'l va so'rovlar qatorini saqlaydigan proksi-so'rovlar.

## 3. Portalni ishga tushiring

Alohida terminalda:

```bash
cd docs/portal
export TRYIT_PROXY_PUBLIC_URL="http://localhost:8787"
npm run start
```

`http://localhost:3000/api/overview` ga tashrif buyuring va Sinab ko'ring konsolidan foydalaning. Xuddi shunday
muhit o'zgaruvchilari Swagger UI va RapiDoc o'rnatishlarini sozlaydi.

## 4. Birlik testlarini ishga tushirish

Proksi tez tugunga asoslangan test to'plamini namoyish etadi:

```bash
npm run test:tryit-proxy
```

Sinovlar manzilni tahlil qilish, kelib chiqishni boshqarish, tarifni cheklash va tashuvchini o'z ichiga oladi
in'ektsiya.

## 5. Problarni avtomatlashtirish va ko'rsatkichlar

`/healthz` va namunali yakuniy nuqtani tekshirish uchun birlashtirilgan probdan foydalaning:

```bash
TRYIT_PROXY_PUBLIC_URL="https://docs.sora.example/proxy" \
TRYIT_PROXY_SAMPLE_PATH="/v1/status" \
npm run probe:tryit-proxy
```

Atrof-muhit tugmalari:

- `TRYIT_PROXY_SAMPLE_PATH` — mashq qilish uchun ixtiyoriy Torii marshruti (`/proxy`siz).
- `TRYIT_PROXY_SAMPLE_METHOD` — sukut bo'yicha `GET`; yozish marshrutlari uchun `POST` ga sozlangan.
- `TRYIT_PROXY_PROBE_TOKEN` — namunaviy chaqiruv uchun vaqtinchalik tashuvchi tokenini kiritadi.
- `TRYIT_PROXY_PROBE_TIMEOUT_MS` — standart 5s kutish vaqtini bekor qiladi.
- `TRYIT_PROXY_PROBE_METRICS_FILE` — `probe_success`/`probe_duration_seconds` uchun Prometheus matn fayli manzili.
- `TRYIT_PROXY_PROBE_LABELS` — vergul bilan ajratilgan `key=value` juftliklar koʻrsatkichlarga qoʻshilgan (birlamchi `job=tryit-proxy` va `instance=<proxy URL>`).

`TRYIT_PROXY_PROBE_METRICS_FILE` o'rnatilganda, skript faylni qayta yozadi
atomik tarzda sizning node_exporter/matn fayli kollektoringiz har doim to'liq ko'radi
foydali yuk. Misol:

```bash
TRYIT_PROXY_PUBLIC_URL="https://docs.sora.example/proxy" \
TRYIT_PROXY_PROBE_METRICS_FILE="/var/lib/node_exporter/textfile_collector/tryit.prom" \
TRYIT_PROXY_PROBE_LABELS="job=tryit-proxy,cluster=staging" \
npm run probe:tryit-proxy
```

Olingan ko'rsatkichlarni Prometheus ga yo'naltiring va namunadagi ogohlantirishni qayta ishlating.
`probe_success` `0` ga tushganda dasturchi-portal hujjatlari sahifasiga.

## 6. Ishlab chiqarishni qattiqlashtirishni nazorat qilish ro'yxati

Mahalliy rivojlanishdan tashqari proksi-serverni nashr qilishdan oldin:

- TLS-ni proksi-serverdan oldin tugatish (teskari proksi yoki boshqariladigan shlyuz).
- Strukturaviy ro'yxatga olishni sozlang va kuzatuv quvurlariga yo'naltiring.
- Tokenlarni aylantiring va ularni maxfiy menejeringizda saqlang.
- Proksi-serverning `/healthz` so'nggi nuqtasini va umumiy kechikish ko'rsatkichlarini kuzatib boring.
- Tarif chegaralarini Torii bosqichli kvotalaringizga moslang; `Retry-After` ni sozlang
  mijozlarga bostirishni bildirish uchun xatti-harakatlar.