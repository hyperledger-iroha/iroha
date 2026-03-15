---
lang: uz
direction: ltr
source: docs/portal/versioned_docs/version-2025-q2/devportal/try-it.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: c389a2121f577bcf8893a0d5c0b898ec2ff5330f2f1727de3387da98f8369915
source_last_modified: "2025-12-29T18:16:35.904297+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Sinab ko'ring

Ishlab chiquvchi portali ixtiyoriy “Sinab ko‘ring” konsolini yuboradi, shunda siz Torii raqamiga qo‘ng‘iroq qilishingiz mumkin.
hujjatlarni tark etmasdan so'nggi nuqtalar. Konsol so'rovlarni uzatadi
to'plamdagi proksi-server orqali brauzerlar CORS chegaralarini chetlab o'tishlari mumkin
tarif cheklovlarini va autentifikatsiyani amalga oshirish.

## Old shartlar

- Node.js 18.18 yoki yangiroq (portalni yaratish talablariga mos keladi)
- Torii staging muhitiga tarmoqqa kirish
- Siz mashq qilishni rejalashtirgan Torii marshrutlarini chaqira oladigan tashuvchi tokeni

Barcha proksi-server konfiguratsiyasi muhit o'zgaruvchilari orqali amalga oshiriladi. Quyidagi jadval
eng muhim tugmalar ro'yxati:

| O'zgaruvchi | Maqsad | Standart |
| --- | --- | --- |
| `TRYIT_PROXY_TARGET` | Proksi-server so'rovlarni | ga yo'naltiradigan asosiy Torii URL manzili **Majburiy** |
| `TRYIT_PROXY_LISTEN` | Mahalliy rivojlanish uchun manzilni tinglang (`host:port` yoki `[ipv6]:port` formati) | `127.0.0.1:8787` |
| `TRYIT_PROXY_ALLOWED_ORIGINS` | Proksi-serverni chaqirishi mumkin bo'lgan manbalarning vergul bilan ajratilgan ro'yxati | `http://localhost:3000` |
| `TRYIT_PROXY_BEARER` | Birlamchi tashuvchi tokeni Torii | ga yoʻnaltirildi _bo'sh_ |
| `TRYIT_PROXY_ALLOW_CLIENT_AUTH` | Yakuniy foydalanuvchilarga `X-TryIt-Auth` | orqali o'z tokenlarini etkazib berishga ruxsat bering `0` |
| `TRYIT_PROXY_MAX_BODY` | Maksimal so'rov tanasi hajmi (bayt) | `1048576` |
| `TRYIT_PROXY_TIMEOUT_MS` | Millisekundlarda yuqori oqim vaqti tugashi | `10000` |
| `TRYIT_PROXY_RATE_LIMIT` | Mijoz IP uchun tarif oynasiga ruxsat berilgan so'rovlar | `60` |
| `TRYIT_PROXY_RATE_WINDOW_MS` | Tezlikni cheklash uchun surma oynasi (ms) | `60000` |

Proksi, shuningdek, `GET /healthz`ni fosh qiladi, tuzilgan JSON xatolarini qaytaradi va
jurnal chiqishidan tashuvchi tokenlarini o'zgartiradi.

## Mahalliy proksi-serverni ishga tushiring

Portalni birinchi marta o'rnatganingizda bog'liqliklarni o'rnating:

```bash
cd docs/portal
npm install
```

Proksi-serverni ishga tushiring va uni Torii misolingizga yo'naltiring:

```bash
export TRYIT_PROXY_TARGET="https://torii.devnet.sora.example"
export TRYIT_PROXY_ALLOWED_ORIGINS="http://localhost:3000"
# Optional: preset a bearer token for the Swagger / RapiDoc panels
export TRYIT_PROXY_BEARER="Bearer eyJhbGciOi..."
npm run tryit-proxy
```

Skript bog'langan manzilni qayd qiladi va so'rovlarni `/proxy/*` manziliga yuboradi.
sozlangan Torii kelib chiqishi.

## Portal vidjetlarini ulash

Ishlab chiquvchi portalini yaratganingizda yoki xizmat ko'rsatayotganingizda, vidjetlar uchun URL manzilini o'rnating
proksi uchun foydalanish kerak:

```bash
export TRYIT_PROXY_PUBLIC_URL="http://localhost:8787"
export TRYIT_PROXY_DEFAULT_BEARER="Bearer eyJhbGciOi..." # Optional
npm run start
```

Quyidagi komponentlar ushbu qiymatlarni `docusaurus.config.js` dan o'qiydi:

- **Swagger UI** — `/reference/torii-swagger` da ko'rsatilgan; so'rovdan foydalanadi
  tokenlarni avtomatik ravishda biriktirish uchun interceptor.
- **RapiDoc** — `/reference/torii-rapidoc` da ko'rsatilgan; token maydonini aks ettiradi
  va proksi-serverga qarshi sinab ko'rish so'rovlarini qo'llab-quvvatlaydi.
- **Konsolni sinab ko'ring** — API umumiy ko'rinish sahifasiga o'rnatilgan; maxsus yuborish imkonini beradi
  so'rovlar, sarlavhalarni ko'rish va javob organlarini tekshirish.

Har qanday vidjetda tokenni o'zgartirish faqat joriy brauzer seansiga ta'sir qiladi; the
proksi hech qachon davom etmaydi yoki taqdim etilgan tokenni qayd qilmaydi.

## Kuzatish va operatsiyalar

Har bir so'rov bir marta usul, yo'l, kelib chiqishi, yuqori oqim holati va
autentifikatsiya manbai (`override`, `default` yoki `client`). Tokenlar hech qachon
saqlangan - tashuvchi sarlavhalari va `X-TryIt-Auth` qiymatlari avval tahrirlanadi
logging - shuning uchun siz tashvishlanmasdan stdout-ni markaziy kollektorga yuborishingiz mumkin
sirlar oshkor bo'ladi.

### Salomatlik tekshiruvlari va ogohlantirishO'rnatish paytida yoki jadval bo'yicha birlashtirilgan tekshiruvni ishga tushiring:

```bash
# Ensure the proxy responds to /healthz and forwards a sample request.
TRYIT_PROXY_PUBLIC_URL="https://docs.sora.example/proxy" \
TRYIT_PROXY_SAMPLE_PATH="/v2/status" \
npm run probe:tryit-proxy
```

Atrof-muhit tugmalari:

- `TRYIT_PROXY_SAMPLE_PATH` — mashq qilish uchun ixtiyoriy Torii marshruti (`/proxy`siz).
- `TRYIT_PROXY_SAMPLE_METHOD` — sukut bo'yicha `GET`; yozish marshrutlari uchun `POST` ga sozlangan.
- `TRYIT_PROXY_PROBE_TOKEN` — namunaviy chaqiruv uchun vaqtinchalik tashuvchi tokenini kiritadi.
- `TRYIT_PROXY_PROBE_TIMEOUT_MS` - standart 5s kutish vaqtini bekor qiladi.
- `TRYIT_PROXY_PROBE_METRICS_FILE` — `probe_success`/`probe_duration_seconds` uchun ixtiyoriy Prometheus matn fayli manzili.
- `TRYIT_PROXY_PROBE_LABELS` — vergul bilan ajratilgan `key=value` juftliklar koʻrsatkichlarga qoʻshilgan (birlamchi `job=tryit-proxy` va `instance=<proxy URL>`).

Natijalarni matn fayli kollektoriga probni yoziladigan joyga qaratib yuboring
yo'l (masalan, `/var/lib/node_exporter/textfile_collector/tryit.prom`) va
har qanday maxsus teg qo'shish:

```bash
TRYIT_PROXY_PUBLIC_URL="https://docs.sora.example/proxy" \
TRYIT_PROXY_PROBE_METRICS_FILE="/var/lib/node_exporter/textfile_collector/tryit.prom" \
TRYIT_PROXY_PROBE_LABELS="job=tryit-proxy,cluster=prod" \
npm run probe:tryit-proxy
```

Skript o'lchovlar faylini atomik tarzda qayta yozadi, shuning uchun kollektoringiz har doim a
to'liq yuk.

Ogohlantirishni engillashtirish uchun probni monitoring to'plamiga ulang. A Prometheus
ketma-ket ikkita xatolikdan keyingi sahifalar misoli:

```yaml
groups:
  - name: tryit-proxy
    rules:
      - alert: TryItProxyUnhealthy
        expr: probe_success{job="tryit-proxy"} == 0
        for: 2m
        labels:
          severity: page
        annotations:
          summary: Try It proxy is failing health checks
          description: |
            The try-it proxy at {{ $labels.instance }} is not responding to probe requests.
```

### Orqaga qaytarishni avtomatlashtirish

Maqsadli Torii URL manzilini yangilash yoki tiklash uchun boshqaruv yordamchisidan foydalaning. Ssenariy
oldingi konfiguratsiyani `.env.tryit-proxy.bak` da saqlaydi, shuning uchun orqaga qaytarishlar
yagona buyruq.

```bash
# Update TRYIT_PROXY_TARGET and back up the previous config.
npm run manage:tryit-proxy -- update --target https://torii.devnet.sora.example

# Roll back to the previously backed-up target.
npm run manage:tryit-proxy -- rollback
```

Env fayl yo'lini `--env` yoki `TRYIT_PROXY_ENV` bilan bekor qiling
konfiguratsiyani boshqa joyda saqlaydi.