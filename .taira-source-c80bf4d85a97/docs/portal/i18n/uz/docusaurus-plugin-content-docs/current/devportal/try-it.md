---
lang: uz
direction: ltr
source: docs/portal/docs/devportal/try-it.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
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
| `TRYIT_PROXY_TARGET` | Proksi so'rovlarni | ga yo'naltiradigan asosiy Torii URL manzili **Majburiy** |
| `TRYIT_PROXY_LISTEN` | Mahalliy rivojlanish uchun manzilni tinglang (`host:port` yoki `[ipv6]:port` formati) | `127.0.0.1:8787` |
| `TRYIT_PROXY_ALLOWED_ORIGINS` | Proksi-serverni chaqirishi mumkin bo'lgan manbalarning vergul bilan ajratilgan ro'yxati | `http://localhost:3000` |
| `TRYIT_PROXY_CLIENT_ID` | Har bir yuqori oqim so'rovi uchun `X-TryIt-Client` da joylashtirilgan identifikator | `docs-portal` |
| `TRYIT_PROXY_BEARER` | Birlamchi tashuvchi token Torii | ga yoʻnaltirildi _bo'sh_ |
| `TRYIT_PROXY_ALLOW_CLIENT_AUTH` | Yakuniy foydalanuvchilarga `X-TryIt-Auth` | orqali o'z tokenlarini etkazib berishga ruxsat bering `0` |
| `TRYIT_PROXY_MAX_BODY` | Maksimal so'rov tanasi hajmi (bayt) | `1048576` |
| `TRYIT_PROXY_TIMEOUT_MS` | Millisekundlarda yuqori oqim vaqti tugashi | `10000` |
| `TRYIT_PROXY_RATE_LIMIT` | Mijoz IP uchun tarif oynasiga ruxsat berilgan so'rovlar | `60` |
| `TRYIT_PROXY_RATE_WINDOW_MS` | Tezlikni cheklash uchun surma oynasi (ms) | `60000` |
| `TRYIT_PROXY_METRICS_LISTEN` | Prometheus uslubidagi ko'rsatkichlar so'nggi nuqtasi uchun ixtiyoriy tinglash manzili (`host:port` yoki `[ipv6]:port`) | _bo'sh (o'chirilgan)_ |
| `TRYIT_PROXY_METRICS_PATH` | Ko'rsatkichlar so'nggi nuqtasi tomonidan xizmat ko'rsatadigan HTTP yo'li | `/metrics` |

Proksi shuningdek, `GET /healthz` ni ochib beradi, tuzilgan JSON xatolarini qaytaradi va
jurnal chiqishidan tashuvchi tokenlarini o'zgartiradi.

Swagger va docs foydalanuvchilariga proksi-serverni ochishda `TRYIT_PROXY_ALLOW_CLIENT_AUTH=1` ni yoqing.
RapiDoc panellari foydalanuvchi tomonidan taqdim etilgan tokenlarni yuborishi mumkin. Proksi-server hali ham tarif cheklovlarini qo'llaydi,
hisob ma'lumotlarini o'zgartiradi va so'rov standart tokenni yoki har bir so'rovni bekor qilishni ishlatganligini qayd qiladi.
`TRYIT_PROXY_CLIENT_ID` ni `X-TryIt-Client` sifatida yubormoqchi bo'lgan yorliqni o'rnating
(standart `docs-portal`). Proksi-server qo'ng'iroq qiluvchi tomonidan taqdim etilgan qirqadi va tasdiqlaydi
`X-TryIt-Client` qiymatlari, bu standart holatga qaytadi, shuning uchun bosqichli shlyuzlar
Brauzer metama'lumotlarini korrelyatsiya qilmasdan kelib chiqishini tekshirish.

## Proksi-serverni mahalliy sifatida ishga tushiring

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

Soketni ulashdan oldin skript buni tasdiqlaydi
`static/openapi/torii.json` da yozilgan dayjestga mos keladi
`static/openapi/manifest.json`. Agar fayllar siljib ketsa, buyruq bilan chiqadi
xato va sizga `npm run sync-openapi -- --latest` ni ishga tushirishni buyuradi. Eksport
`TRYIT_PROXY_ALLOW_STALE_SPEC=1` faqat favqulodda holatlar uchun; proksi bo'ladi
ogohlantirishni yozib oling va parvarishlash oynalarida tiklanishingiz uchun davom eting.

## Portal vidjetlarini ulash

Ishlab chiquvchi portalini yaratganingizda yoki xizmat ko'rsatayotganingizda, vidjetlar uchun URL manzilini o'rnating
proksi uchun foydalanish kerak:

```bash
export TRYIT_PROXY_PUBLIC_URL="http://localhost:8787"
export TRYIT_PROXY_DEFAULT_BEARER="Bearer eyJhbGciOi..." # Optional
npm run start
```

Quyidagi komponentlar ushbu qiymatlarni `docusaurus.config.js` dan o'qiydi:

- **Swagger UI** — `/reference/torii-swagger` da ko'rsatilgan; ga oldindan ruxsat beradi
  token mavjud bo'lganda tashuvchi sxemasi, `X-TryIt-Client` bilan teglar so'rovlari,
  `X-TryIt-Auth` in'ektsiya qiladi va proksi orqali qo'ng'iroqlarni qayta yozadi.
  `TRYIT_PROXY_PUBLIC_URL` o'rnatildi.
- **RapiDoc** — `/reference/torii-rapidoc` da ko'rsatilgan; token maydonini aks ettiradi,
  Swagger paneli bilan bir xil sarlavhalarni qayta ishlatadi va proksi-serverni nishonga oladi
  URL sozlanganda avtomatik ravishda.
- **Konsolni sinab ko'ring** — API umumiy ko'rinish sahifasiga o'rnatilgan; maxsus yuborish imkonini beradi
  so'rovlar, sarlavhalarni ko'rish va javob organlarini tekshirish.

Ikkala panelda ham o'qiladigan **surat tanlash moslamasi** mavjud
`docs/portal/static/openapi/versions.json`. Ushbu indeksni to'ldiring
`npm run sync-openapi -- --version=<label> --mirror=current --latest` shunday
sharhlovchilar tarixiy xususiyatlar o'rtasida o'tishlari mumkin, yozilgan SHA-256 dayjestiga qarang,
va foydalanishdan oldin reliz surati imzolangan manifestga ega ekanligini tasdiqlang
interaktiv vidjetlar.

Har qanday vidjetda tokenni o'zgartirish faqat joriy brauzer seansiga ta'sir qiladi; the
proksi hech qachon davom etmaydi yoki taqdim etilgan tokenni qayd qilmaydi.

## Qisqa muddatli OAuth tokenlari

Ko'rib chiquvchilarga uzoq muddatli Torii tokenlarini tarqatmaslik uchun sinab ko'ring.
konsolni OAuth serveringizga kiriting. Quyidagi muhit o'zgaruvchilari mavjud bo'lganda
portal qurilma kodiga kirish vidjetini taqdim etadi, qisqa muddatli tashuvchi tokenlarini zarb qiladi,
va ularni avtomatik ravishda konsol formasiga kiritadi.

| O'zgaruvchi | Maqsad | Standart |
| --- | --- | --- |
| `DOCS_OAUTH_DEVICE_CODE_URL` | OAuth Device Authorization endpoint (`/oauth/device/code`) | _bo'sh (o'chirilgan)_ |
| `DOCS_OAUTH_TOKEN_URL` | `grant_type=urn:ietf:params:oauth:grant-type:device_code` | ni qabul qiluvchi token oxirgi nuqtasi _bo'sh_ |
| `DOCS_OAUTH_CLIENT_ID` | OAuth mijoz identifikatori hujjatlarni oldindan koʻrish uchun roʻyxatdan oʻtgan | _bo'sh_ |
| `DOCS_OAUTH_SCOPE` | Kirish paytida boʻsh joy bilan ajratilgan doiralar soʻralgan | `openid profile offline_access` |
| `DOCS_OAUTH_AUDIENCE` | Tokenni | ga ulash uchun ixtiyoriy API auditoriyasi _bo'sh_ |
| `DOCS_OAUTH_POLL_INTERVAL_MS` | Tasdiqlashni kutayotganda minimal so'rov oralig'i (ms) | `5000` (qiymatlari <5000ms rad etiladi) |
| `DOCS_OAUTH_DEVICE_CODE_TTL_SECONDS` | Qayta qurilma kodining amal qilish muddati tugash oynasi (sekundlar) | `600` (300 dan 900 gacha qolishi kerak) |
| `DOCS_OAUTH_TOKEN_TTL_SECONDS` | Qayta kirish tokenining ishlash muddati (sekundlar) | `900` (300 dan 900 gacha qolishi kerak) |
| `DOCS_OAUTH_ALLOW_INSECURE` | OAuth qoʻllanilishini ataylab oʻtkazib yuboradigan mahalliy oldindan koʻrish uchun `1` ga oʻrnating | _belgilanmagan_ |

Konfiguratsiyaga misol:

```bash
export DOCS_OAUTH_DEVICE_CODE_URL="https://auth.dev.sora.example/oauth/device/code"
export DOCS_OAUTH_TOKEN_URL="https://auth.dev.sora.example/oauth/token"
export DOCS_OAUTH_CLIENT_ID="docs-preview"
export DOCS_OAUTH_SCOPE="torii openid offline_access"
# Optional audience and polling tweaks
export DOCS_OAUTH_AUDIENCE="https://torii.devnet.sora.example"
export DOCS_OAUTH_POLL_INTERVAL_MS="6000"
```

`npm run start` yoki `npm run build` ishga tushirilganda portal ushbu qiymatlarni o'rnatadi.
`docusaurus.config.js` da. Mahalliy oldindan koʻrish vaqtida “Try it” kartasi a koʻrsatadi
"Qurilma kodi bilan tizimga kirish" tugmasi. Foydalanuvchilar OAuth-da ko'rsatilgan kodni kiritadilar
tekshirish sahifasi; qurilma oqimi vidjetdan muvaffaqiyatli o'tgandan keyin:

- “Try it console” maydoniga chiqarilgan tashuvchi tokenini kiritadi,
- mavjud `X-TryIt-Client` va `X-TryIt-Auth` sarlavhalari bilan so'rovlarni teglar,
- qolgan xizmat muddatini ko'rsatadi va
- token muddati tugashi bilan avtomatik ravishda tozalaydi.

Bearerni qo'lda kiritish mavjud bo'lib qoladi - OAuth o'zgaruvchilarini o'tkazib yuboring
sharhlovchilarni vaqtinchalik tokenni o'zlari joylashtirishga yoki eksport qilishga majburlamoqchi
`DOCS_OAUTH_ALLOW_INSECURE=1`, anonim kirish uchun ajratilgan mahalliy oldindan ko'rish uchun
qabul qilinadi. OAuth sozlanmagan tuzilmalar endi talabni qondira olmaydi
DOCS-1b yo'l xaritasi darvozasi.

📌 [Xavfsizlikni mustahkamlash va ruchka sinovlari roʻyxatini] koʻrib chiqing (./security-hardening.md)
portalni laboratoriyadan tashqariga chiqarishdan oldin; u tahdid modelini hujjatlashtiradi,
CSP/Ishonchli turlar profili va hozirda DOCS-1b ga o'tadigan penetratsion test bosqichlari.

## Norito-RPC namunalari

Norito-RPC so'rovlari JSON marshrutlari kabi bir xil proksi-server va OAuth sanitariya-tesisatini ulashadi,
ular shunchaki `Content-Type: application/x-norito` ni o'rnatadilar va yuboradilar
NRPC spetsifikatsiyasida tasvirlangan oldindan kodlangan Norito foydali yuk
(`docs/source/torii/nrpc_spec.md`).
Ombor `fixtures/norito_rpc/` ostida kanonik foydali yuklarni yuboradi, shuning uchun portal
mualliflar, SDK egalari va sharhlovchilar CI foydalanadigan aniq baytlarni takrorlashlari mumkin.

### Try It konsolidan Norito foydali yukini yuboring

1. `fixtures/norito_rpc/transfer_asset.norito` kabi moslamani tanlang. Bular
   fayllar xom Norito konvertlari; **base64-ularni kodlamang.
2. Swagger yoki RapiDoc-da NRPC oxirgi nuqtasini toping (masalan
   `POST /v1/pipeline/submit`) va **Content-Type** selektorini
   `application/x-norito`.
3. Soʻrovning asosiy muharririni **ikliklik** ga almashtiring (Swaggerning “Fayl” rejimi yoki
   RapiDoc-ning "Ikkilik/Fayl" selektori) va `.norito` faylini yuklang. Vidjet
   baytlarni proksi-server orqali o'zgartirmasdan uzatadi.
4. So'rovni yuboring. Agar Torii `X-Iroha-Error-Code: schema_mismatch` qaytarsa,
   ikkilik foydali yuklarni qabul qiluvchi so'nggi nuqtaga qo'ng'iroq qilayotganingizni tekshiring va
   `fixtures/norito_rpc/schema_hashes.json` da yozilgan sxema xashini tasdiqlang
   siz urgan Torii tuzilishiga mos keladi.

Konsol eng so'nggi faylni xotirada saqlaydi, shuning uchun uni qayta yuborishingiz mumkin
turli avtorizatsiya tokenlari yoki Torii xostlarini ishlatishda foydali yuk. Qo'shish
Sizning ish oqimingiz uchun `scripts/run_norito_rpc_fixtures.sh --note "<ticket>"` ishlab chiqaradi
NRPC-4 qabul qilish rejasida ko'rsatilgan dalillar to'plami (log + JSON xulosasi),
Bu ko'rib chiqish paytida "Try It" javobini skrinshot qilish bilan yaxshi mos keladi.

### CLI misoli (jingalak)

Xuddi shu moslamalarni `curl` orqali portaldan tashqarida takrorlash mumkin, bu foydali
proksi-serverni tekshirish yoki shlyuz javoblarini tuzatishda:

```bash
TORII="https://torii.devnet.sora.example"
TOKEN="Bearer $(cat ~/.config/torii/devnet.token)"
curl \
  -H "Content-Type: application/x-norito" \
  -H "Authorization: ${TOKEN}" \
  --data-binary @fixtures/norito_rpc/transfer_asset.norito \
  "${TORII}/v1/pipeline/submit"
```

Armaturani `transaction_fixtures.manifest.json` ro'yxatidagi har qanday yozuvga almashtiring
yoki o'zingizning foydali yukingizni `cargo xtask norito-rpc-fixtures` bilan kodlang. Qachon Torii
kanareyka rejimida siz `curl`-ni sinab ko'ring proksi-serverda ko'rsatishingiz mumkin
(`https://docs.sora.example/proxy/v1/pipeline/submit`) xuddi shunday mashq qilish
portal vidjetlari foydalanadigan infratuzilma.

## Kuzatish va operatsiyalarHar bir so'rov bir marta usul, yo'l, kelib chiqishi, yuqori oqim holati va
autentifikatsiya manbai (`override`, `default` yoki `client`). Tokenlar hech qachon
saqlangan - tashuvchi sarlavhalari va `X-TryIt-Auth` qiymatlari avval tahrirlanadi
logging - shuning uchun siz tashvishlanmasdan stdout-ni markaziy kollektorga yuborishingiz mumkin
sirlar oshkor bo'ladi.

### Salomatlik tekshiruvlari va ogohlantirish

O'rnatish paytida yoki jadval bo'yicha birlashtirilgan tekshiruvni ishga tushiring:

```bash
# Ensure the proxy responds to /healthz and forwards a sample request.
TRYIT_PROXY_PUBLIC_URL="https://docs.sora.example/proxy" \
TRYIT_PROXY_SAMPLE_PATH="/v1/status" \
npm run probe:tryit-proxy
```

Atrof-muhit tugmalari:

- `TRYIT_PROXY_SAMPLE_PATH` — mashq qilish uchun ixtiyoriy Torii marshruti (`/proxy`siz).
- `TRYIT_PROXY_SAMPLE_METHOD` — sukut bo'yicha `GET`; yozish marshrutlari uchun `POST` ga sozlangan.
- `TRYIT_PROXY_PROBE_TOKEN` — namunaviy chaqiruv uchun vaqtinchalik tashuvchi tokenini kiritadi.
- `TRYIT_PROXY_PROBE_TIMEOUT_MS` — standart 5s kutish vaqtini bekor qiladi.
- `TRYIT_PROXY_PROBE_METRICS_FILE` — `probe_success`/`probe_duration_seconds` uchun ixtiyoriy Prometheus matn fayli manzili.
- `TRYIT_PROXY_PROBE_LABELS` — vergul bilan ajratilgan `key=value` juftliklar koʻrsatkichlarga qoʻshilgan (birlamchi `job=tryit-proxy` va `instance=<proxy URL>`).
- `TRYIT_PROXY_PROBE_METRICS_URL` - `TRYIT_PROXY_METRICS_LISTEN` yoqilganda muvaffaqiyatli javob berishi kerak bo'lgan ixtiyoriy ko'rsatkichlar oxirgi nuqtasi URL (masalan, `http://localhost:9798/metrics`).

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

`TRYIT_PROXY_METRICS_LISTEN` sozlanganda, o'rnating
`TRYIT_PROXY_PROBE_METRICS_URL` metrikaning so'nggi nuqtasiga, shuning uchun prob tezda ishlamay qoladi
agar qirqish yuzasi yo'qolsa (masalan, noto'g'ri sozlangan kirish yoki etishmayotgan
xavfsizlik devori qoidalari). Oddiy ishlab chiqarish sozlamalari
`TRYIT_PROXY_PROBE_METRICS_URL="http://127.0.0.1:9798/metrics"`.

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

### Ko'rsatkichlarning so'nggi nuqtasi va asboblar paneli

Oldin `TRYIT_PROXY_METRICS_LISTEN=127.0.0.1:9798` (yoki har qanday xost/port juftligini) o'rnating
Prometheus formatidagi ko'rsatkichlar so'nggi nuqtasini ochish uchun proksi-serverni ishga tushirish. Yo'l
sukut bo'yicha `/metrics`, lekin orqali bekor qilinishi mumkin
`TRYIT_PROXY_METRICS_PATH=/custom`. Har bir qirqish har bir usul uchun hisoblagichlarni qaytaradi
so'rovlar yig'indisi, tarif limitini rad etishlar, yuqoridagi xatolar/vaqt tugashlari, proksi natijalari,
va kechikish haqida xulosalar:

```bash
export TRYIT_PROXY_METRICS_LISTEN="127.0.0.1:9798"
npm run tryit-proxy &
curl http://127.0.0.1:9798/metrics | head -n 5
# HELP tryit_proxy_requests_total Requests handled by method
tryit_proxy_requests_total{method="GET"} 12
tryit_proxy_rate_limited_total 1
```

Prometheus/OTLP kollektorlarini ko'rsatkichlarning so'nggi nuqtasiga qarating va qayta foydalaning.
SRE quyruqni kuzatishi uchun mavjud `dashboards/grafana/docs_portal.json` panellari
jurnallarni tahlil qilmasdan kechikishlar va rad etish ko'rsatkichlari. Avtomatik proksi
operatorlarga qayta ishga tushirishni aniqlashga yordam berish uchun `tryit_proxy_start_timestamp_ms` ni nashr etadi.

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