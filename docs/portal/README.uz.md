---
lang: uz
direction: ltr
source: docs/portal/README.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: c0320c0372f182add4c290a339bc4a0598ec00d0e95dd6601c811bf8134e96c0
source_last_modified: "2026-02-07T00:38:43.595197+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# SORA Nexus Dasturchilar portali

Ushbu katalog interaktiv dasturchi uchun Docusaurus ish maydoniga ega.
portal. Portal Norito qoʻllanmalarini, SDK tezkor ishga tushirishlarini va OpenAPIni birlashtiradi.
`cargo xtask openapi` tomonidan yaratilgan ma'lumotnoma, ularni SORA Nexus ichiga o'rash
docs dasturida foydalaniladigan brending.

## Old shartlar

- Node.js 18.18 yoki yangiroq (Docusaurus v3 bazaviy).
- Paketni boshqarish uchun ip 1.x yoki npm ≥ 9.
- Rust asboblar zanjiri (OpenAPI sinxronlash skripti tomonidan ishlatiladi).

## Bootstrap

```bash
cd docs/portal
npm install    # or yarn install
```

## Mavjud skriptlar

| Buyruq | Tavsif |
|---------|-------------|
| `npm run start` / `yarn start` | Jonli qayta yuklash bilan mahalliy ishlab chiqaruvchi serverni ishga tushiring (birlamchi `http://localhost:3000`). |
| `npm run build` / `yarn build` | `build/` da ishlab chiqarish qurilishini ishlab chiqaring. |
| `npm run serve` / `yarn serve` | Mahalliy eng so'nggi tuzilishga xizmat qiling (tutun sinovlari uchun foydali). |
| `npm run docs:version -- <label>` | Joriy hujjatlarni `versioned_docs/version-<label>` ichiga suratga oling (`docusaurus docs:version` atrofidagi oʻram). |
| `npm run sync-openapi` / `yarn sync-openapi` | `static/openapi/torii.json` ni `cargo xtask openapi` orqali qayta tiklang (spetsni qo'shimcha versiya suratlariga nusxalash uchun `--mirror=<label>` dan o'ting). |
| `npm run tryit-proxy` | “Sinab ko‘ring” konsolini ishga tushiruvchi proksi-serverni ishga tushiring (konfiguratsiya uchun pastga qarang). |
| `npm run probe:tryit-proxy` | Proksi-serverga (CI/monitoring yordamchisi) qarshi `/healthz` + namunali so‘rov tekshiruvini chiqaring. |
| `npm run manage:tryit-proxy -- <update|rollback>` | `.env` proksi-serverini zaxira nusxasi bilan yangilang yoki tiklang. |
| `npm run sync-i18n` | `i18n/` ostida yapon, ibroniy, ispan, portugal, frantsuz, rus, arab, urdu, birma, gruzin, arman, ozarbayjon, qozoq, boshqird, amxar, dzonxa, o‘zbek, mo‘g‘ul, an’anaviy xitoy va soddalashtirilgan xitoy tillariga tarjima qo‘shiqlari mavjudligiga ishonch hosil qiling. |
| `npm run sync-norito-snippets` | Kotodama namunaviy hujjatlarni + yuklab olinadigan parchalarni qayta tiklang (shuningdek, ishlab chiqaruvchi server plagini tomonidan avtomatik ravishda chaqiriladi). |
| `npm run test:tryit-proxy` | Proksi birlik testlarini Node test dasturi (`node --test`) orqali bajaring. |

OpenAPI sinxronlash skripti `cargo xtask openapi` dan foydalanishni talab qiladi.
ombor ildizi; u `static/openapi/` ga deterministik JSON faylini chiqaradi
va endi Torii routeri jonli spetsifikatsiyani ochishini kutmoqda (foydalanish
`cargo xtask openapi --allow-stub` faqat favqulodda to'ldiruvchi chiqishi uchun).

## Hujjatlar versiyalari va OpenAPI suratlari

- **Hujjatlar versiyasini kesish:** `npm run docs:version -- 2025-q3` (yoki har qanday kelishilgan teg) ni ishga tushiring.
  Yaratilgan `versioned_docs/version-<label>`, `versioned_sidebars`,
  va `versions.json`. Navbar versiyasi ochiladigan ro'yxati avtomatik ravishda paydo bo'ladi
  yangi surat.
- **OpenAPI artefaktlarini sinxronlash:** versiyani kesib bo'lgach, kanonikni yangilang
  spec va `cargo xtask openapi --sign <path-to-ed25519-key>` orqali manifest, keyin
  bilan mos suratga oling
  `npm run sync-openapi -- --version=2025-q3 --mirror=current --latest`. The
  skript `static/openapi/versions/2025-q3/torii.json` ni yozadi, spetsifikatsiyani aks ettiradi
  `versions/current/torii.json` ichiga, `versions.json` yangilanadi, yangilanadi
  `/openapi/torii.json` va imzolangan `manifest.json` ni har bir versiyaga klonlaydi
  katalogi, shuning uchun tarixiy xususiyatlar bir xil kelib chiqish metama'lumotlariga ega. Har qanday ta'minot
  yangi yaratilgan spetsifikatsiyani nusxalash uchun `--mirror=<label>` bayroqlari soni
  boshqa tarixiy suratlar.
- **CI kutilmalari:** taʼsir etuvchi hujjatlarda versiya oʻzgarishi ham boʻlishi kerak
  (agar mavjud bo'lsa) va yangilangan OpenAPI oniy tasvirlar, shuning uchun Swagger, RapiDoc,
  va Redoc panellari olish xatosisiz tarixiy spetsifikatsiyalar o‘rtasida almashishi mumkin.
- **Manifest ijrosi:** `sync-openapi` skripti endi imzolanganda ishlamay qoladi
  `manifest.json` yo'q, noto'g'ri tuzilgan yoki yangi yaratilganiga mos kelmaydi
  spec, shuning uchun imzosiz suratlarni sukut bo'yicha chop etib bo'lmaydi. Qayta ishga tushirish
  Kanonik manifestni yangilash uchun `cargo xtask openapi --sign <key>` va
  sinxronlashni qayta ishga tushiring, shunda versiyali oniy tasvirlar imzolangan metadatani oladi.
  `--allow-unsigned`-ni faqat mahalliy oldindan ko'rish uchun o'tkazing (CI hali ham ishlaydi
  `ci/check_openapi_spec.sh`, spetsifikatsiyani qayta tiklaydi va tekshiradi
  majburiyatlarni birlashtirishga ruxsat berishdan oldin manifest).

## Tuzilishi

```
docs/portal/
├── docs/                 # Markdown/MDX content for the portal
├── i18n/                 # Locale overrides generated by sync-i18n
├── src/                  # React pages/components (placeholder scaffolding)
├── static/               # Static assets served verbatim (includes OpenAPI JSON)
├── scripts/              # Helper scripts (OpenAPI synchronisation)
├── docusaurus.config.js  # Core site configuration
└── sidebars.js           # Sidebar / navigation model
```

### Proksi-server konfiguratsiyasini sinab ko'ring

“Sinab ko‘ring” sinov maydoni so‘rovlarni `scripts/tryit-proxy.mjs` orqali yo‘naltiradi. Sozlang
uni ishga tushirishdan oldin muhit o'zgaruvchilari bilan proksi:

```bash
export TRYIT_PROXY_TARGET="https://torii.staging.sora"
export TRYIT_PROXY_ALLOWED_ORIGINS="http://localhost:3000"
export TRYIT_PROXY_BEARER="sora-dev-token"          # optional
export TRYIT_PROXY_PUBLIC_URL="http://localhost:8787"
npm run tryit-proxy
```

- `TRYIT_PROXY_LISTEN` (standart `127.0.0.1:8787`) ulanish manzilini boshqaradi.
- `TRYIT_PROXY_RATE_LIMIT` / `TRYIT_PROXY_RATE_WINDOW_MS` ichki xotirani sozlash
  tezlikni cheklovchi (sukut bo'yicha 60 soniyada 60 ta so'rov).
- Qo'ng'iroq qiluvchining `Authorization` raqamini yo'naltirish uchun `TRYIT_PROXY_ALLOW_CLIENT_AUTH=1` ni o'rnating
  standart tashuvchi tokenga tayanish o'rniga sarlavha.
- Proksi-server `static/openapi/torii.json` dan chetga chiqsa, ishga tushirishni rad etadi.
  `static/openapi/manifest.json` da imzolangan manifest. Yugurish
  Spetsifikatsiyani yangilash uchun `npm run sync-openapi -- --latest`; eksport
  `TRYIT_PROXY_ALLOW_STALE_SPEC=1` faqat favqulodda vaziyatlarni bekor qilish uchun (ogohlantirish
  tizimga kirgan va proksi-server baribir ishga tushadi).
- Sting proksi-serverini cheklash va simni ulash uchun `npm run probe:tryit-proxy` ni ishga tushiring.
  monitoring ishlariga buyruq berish; `npm run manage:tryit-proxy -- update` soddalashtiradi
  orqaga qaytarish uchun `.env` zaxira nusxasini saqlagan holda Torii so'nggi nuqtalarini aylantiring.
- `TRYIT_PROXY_PROBE_METRICS_FILE` va `TRYIT_PROXY_PROBE_LABELS` probga ruxsat bering
  `probe_success`/`probe_duration_seconds`ni Prometheus matn fayli formatida chiqaring
  sog'liqni saqlash tekshiruvini node_exporter yoki istalgan partiya kollektoriga ulashingiz mumkin.
- Prometheus qirib tashlaydi `tryit_proxy_request_duration_ms_bucket`/`_count`/`_sum`
  gistogramma seriyasi; docs portal Grafana asboblar paneli ularni p95/p99 kechikishi uchun sarflaydi
  SLO kuzatuvi (`dashboards/grafana/docs_portal.json`).

### OAuth qurilma kodiga kirish

Portal ishonchli tarmoqlardan tashqarida bo'lsa, OAuth qurilmasini sozlang
avtorizatsiya, shuning uchun sharhlovchilar uzoq umr ko'radigan Torii tokenlariga hech qachon tegmaydilar. ni eksport qiling
`npm run start` yoki `npm run build` ishga tushirishdan oldin quyidagi o'zgaruvchilar:

| O'zgaruvchi | Eslatmalar |
|----------|-------|
| `DOCS_OAUTH_DEVICE_CODE_URL` / `DOCS_OAUTH_TOKEN_URL` | Qurilmani avtorizatsiya qilish grantini amalga oshiradigan OAuth oxirgi nuqtalari. |
| `DOCS_OAUTH_CLIENT_ID` | Hujjatlarni oldindan ko'rish uchun ro'yxatdan o'tgan mijoz identifikatori. |
| `DOCS_OAUTH_SCOPE` / `DOCS_OAUTH_AUDIENCE` | Berilgan tokenni cheklash uchun ixtiyoriy doira/auditoriya qatorlari. |
| `DOCS_OAUTH_POLL_INTERVAL_MS` | Portal vidjeti tomonidan qo'llaniladigan minimal so'rov oralig'i (standart 5000ms; pastroq qiymatlar rad etiladi). |
| `DOCS_OAUTH_DEVICE_CODE_TTL_SECONDS` / `DOCS_OAUTH_TOKEN_TTL_SECONDS` | Server `expires_in`ni o'tkazib yuborganida foydalaniladigan zaxira muddati tugagan oynalar. |
| `DOCS_OAUTH_ALLOW_INSECURE` | `1` ni faqat mahalliy rivojlanish uchun o'rnating, bu qurilish vaqtini himoya qilishni chetlab o'tish. Agar talab qilinadigan o'zgaruvchilar etishmayotgan bo'lsa yoki TTLlar majburiy diapazondan tashqariga chiqsa, ishlab chiqarish tuzilmalari muvaffaqiyatsiz tugadi. |

Misol:

```bash
export DOCS_OAUTH_DEVICE_CODE_URL="https://auth.dev.sora.example/oauth/device/code"
export DOCS_OAUTH_TOKEN_URL="https://auth.dev.sora.example/oauth/token"
export DOCS_OAUTH_CLIENT_ID="docs-preview"
export DOCS_OAUTH_SCOPE="torii openid offline_access"
```

Qurilish ushbu qiymatlarni olgandan so'ng, Sinab ko'ring konsoli tizimga kirishni ko'rsatadi
qurilma kodini ko'rsatadigan, tokenning so'nggi nuqtasini so'roq qiladigan, Bearerni to'ldiradigan panel
maydon avtomatik ravishda o'chiriladi va muddati tugashi bilan tokenni tozalaydi. Portal rad etadi
OAuth konfiguratsiyasi toʻliq boʻlmaganda yoki token/qurilma TTLlari tushib qolganda boshlanadi
xavfsizlik byudjetidan tashqarida; faqat `DOCS_OAUTH_ALLOW_INSECURE=1`ni eksport qiling
anonim kirish mumkin bo'lgan bir martalik mahalliy oldindan ko'rishlar. Proksi hali ham
`X-TryIt-Auth` qo'llanmasini bekor qiladi, shuning uchun OAuth o'zgaruvchilarini olib tashlashingiz mumkin
har doim maxsus tokenlarni joylashtirishni xohlasangiz.

[`docs/devportal/security-hardening.md`](docs/devportal/security-hardening.md) uchun qarang.
to'liq tahdid modeli va pen-test eshiklari.

### Xavfsizlik sarlavhalari

`docusaurus.config.js` endi deterministik xavfsizlik sarlavhalarini chiqaradi - kontent xavfsizligi
siyosat, Ishonchli turlar, Ruxsatlar-Siyosat va Yoʻnaltiruvchi-siyosat — shunday statik xostlar
dev server bilan bir xil himoya panjaralarini meros qilib oladi. Standartlar faqat skriptlarga ruxsat beradi
portal manbasidan xizmat ko'rsatadi va o'zaro kelib chiqishi `connect-src` trafikini cheklaydi
sozlangan analitik soʻnggi nuqtasi va proksi-serverni sinab koʻring. Ishlab chiqarishni rad etish
HTTPS bo'lmagan tahlillar/uni sinab ko'rish so'nggi nuqtalari; `http://` ga qarshi mahalliy oldindan ko'rish uchun
maqsadlar, `DOCS_SECURITY_ALLOW_INSECURE=1` to'g'ridan-to'g'ri to'siqni o'rnating
pasaytirishni tan oladi.

### Mahalliylashtirish ish jarayoni

Portal manba tili sifatida ingliz va yapon, ibroniy,
Ispan, portugal, frantsuz, rus, arab, urdu, birma, gruzin,
Arman, ozarbayjon, qozoq, boshqird, amxar, dzonqa, oʻzbek, moʻgʻul,
An'anaviy xitoy va soddalashtirilgan xitoy tillari. Yangi hujjatlar kelganda,
yugurish:

```bash
npm run sync-i18n
```

Skript ombori bo'ylab `scripts/sync_docs_i18n.py` xatti-harakatlarini aks ettiradi.
`i18n/<lang>/docusaurus-plugin-content-docs/current/...` ostida stub tarjimalarini yaratish.
Keyin muharrirlar to'ldiruvchi matnni almashtirishi va oldingi masalani yangilashi mumkin
metama'lumotlar (`status`, `translation_last_reviewed` va boshqalar).

Odatiy bo'lib, ishlab chiqarish tuzilmalari faqat ro'yxatda keltirilgan tillarni o'z ichiga oladi
`docs/i18n/published_locales.json` (hozirda faqat ingliz tilida) stub to'ldiruvchilari
yuborilmaydi. Oldindan ko'rish kerak bo'lganda `DOCS_I18N_INCLUDE_STUBS=1` ni o'rnating
mahalliy miqyosda amalga oshirilayotgan joylar.

### Kontent xaritasi (MVP maqsadlari)| Bo'lim | Holati | Eslatmalar |
|---------|--------|-------|
| Norito Tez boshlash (`docs/norito/quickstart.md`) | 🢓 Chop etilgan | End-to-end Docker + Kotodama + Norito foydali yukini yuborish uchun CLI ko'rsatmasi. |
| Norito Ledger Walkthrough (`docs/norito/ledger-walkthrough.md`) | 🢓 Chop etilgan | CLI tomonidan boshqariladigan registr → mint → tranzaksiya/status tekshiruvi va SDK pariteti havolalari bilan o‘tkazma oqimi. |
| SDK retseptlari (`docs/sdk/recipes/`) | 🟡 Chiqarilmoqda | Rust/Python/JS/Swift/Java daftar oqimi retseptlari chop etildi; qolgan SDKlar naqshni aks ettirishi kerak. |
| API havolasi | 🢠 Avtomatlashtirilgan | `yarn sync-openapi` nashr etadi `static/openapi/torii.json`; Har bir nashrdan keyin tekshirish uchun DevRel. |
| Streaming yo'l xaritasi | 🢠 Qo'yilgan | Orqaga integratsiyalashuv uchun `docs/norito-streaming-roadmap.md` ga qarang. |
| SoraFS ko'p manbali rejalashtirish (`docs/sorafs/provider-advert-multisource.md`) | 🢓 Chop etilgan | Ko'p provayderlarni qabul qilish uchun TLV diapazonlari, boshqaruvni tekshirish, CLI moslamalari va telemetriya ma'lumotnomalarini umumlashtiradi. |

> 📌 2026-03-05 nazorat punkti: tezkor boshlash va kamida bitta retsept nashr etilgandan so‘ng, ushbu jadvalni olib tashlang va uning o‘rniga portal ishtirokchilari qo‘llanmasiga havola qiling.

## CI va joylashtirish

- `ci/check_docs_portal.sh` avval `node scripts/check-sdk-recipes.mjs` ni SDK daftarlari retseptlarini ularning kanonik manba fayllariga mos kelishini tekshirish uchun ishga tushiradi, so'ngra `npm ci|install`, `npm run build` va asosiy bo'limlarni tasdiqlaydi (bosh sahifa, I0NT02, I0108) Norito, nashriyot roʻyxati) yaratilgan HTMLda mavjud.
- `ci/check_openapi_spec.sh` `cargo xtask openapi` orqali Torii spetsifikatsiyasini qayta tiklaydi,
  uni `static/openapi/torii.json` va `versions/current/torii.json` bilan taqqoslaydi,
  va nazorat summasi manifestini tekshiradi, shuning uchun kuzatilgan spetsifikatsiya yoki xeshda PRlar muvaffaqiyatsiz bo'ladi
  eskirgan.
- `.github/workflows/check-docs.yml` kontent regressiyalarini ushlash uchun har bir PR bo'yicha portal tuzilmasini ishga tushiradi.
- `.github/workflows/docs-portal-preview.yml` tortishish so'rovlari uchun sayt yaratadi, `build/checksums.sha256` deb yozadi, uni `sha256sum -c` orqali tasdiqlaydi, `artifacts/preview-site.tar.gz` sifatida chiqishni paketlaydi, `artifacts/preview-site.tar.gz` orqali tavsiflovchini yaratadi, `scripts/generate-preview-descriptor.mjs`, ham saytni, ham metasta va metastani yuklaydi. sharhlovchilar qurilishni qayta ishga tushirmasdan aniq suratni tekshirishlari mumkin. Ish jarayoni, shuningdek, manifest/arxiv dayjestlarini (va agar mavjud bo'lsa, SoraFS to'plamini) umumlashtiruvchi tortishish so'rovi sharhini joylashtiradi, shuning uchun sharhlovchilar Harakatlar jurnalini ochmasdan tekshirish natijasini ko'rishadi.
- `.github/workflows/docs-portal-deploy.yml` statik tuzilmani GitHub sahifalarida `main`/`master` manziliga surish orqali nashr etadi, bu esa `github-pages` muhit URL manzilida oldindan ko‘rish imkonini beradi. Ish jarayoni joylashtirishdan oldin ochilish sahifasidagi tutunni tekshirishni amalga oshiradi.
- `scripts/sorafs-package-preview.sh` oldindan koʻrish saytini deterministik SoraFS toʻplamiga (CAR, reja, manifest) oʻzgartiradi va hisobga olish maʼlumotlari JSON konfiguratsiyasi orqali taʼminlanganda (qarang: `docs/examples/sorafs_preview_publish.json`), manifestni I1800.04.0.2. endga yuborishi mumkin.
- `scripts/preview_verify.sh` tekshiruvchilar oldindan qo'lda bajargan nazorat summasi + deskriptorni tekshirish oqimini bajaradi. Oldindan ko'rish artefaktlari o'zgartirilmaganligini tasdiqlash uchun uni chiqarilgan `build/` katalogiga (va ixtiyoriy deskriptor/arxiv yo'llariga) yo'naltiring.
- `scripts/sorafs-pin-release.sh` plus `.github/workflows/docs-portal-sorafs-pin.yml` ishlab chiqarish SoraFS quvur liniyasini avtomatlashtiradi: qurish/sinov, CAR + manifest yaratish, Sigstore imzolash, tekshirish, ixtiyoriy taxallusni ulash va boshqaruvni tekshirish uchun artefakt yuklash. Chiqarishni targ'ib qilishda `workflow_dispatch` orqali ish jarayonini ishga tushiring.
- `cargo xtask soradns-verify-gar --gar <path> --name <fqdn> [...]` shlyuz avtorizatsiyasi yozuvlarini imzolanishi yoki operatsiyaga jo'natilishidan oldin tasdiqlaydi. Yordamchi kanonik/chiroyli xostlar, manifest metamaʼlumotlari va telemetriya yorliqlari deterministik siyosatga mos kelishini taʼminlaydi va `--json-out` orqali DG-3 dalillari uchun JSON xulosasini chiqarishi mumkin.
- Yuborilganda, pin yordamchisi `artifacts/sorafs/portal.dns-cutover.json` ishlab chiqarish uchun `scripts/generate-dns-cutover-plan.mjs` ni ham chaqiradi. `DNS_CHANGE_TICKET`, `DNS_CUTOVER_WINDOW`, `DNS_HOSTNAME`, `DNS_ZONE` va `DNS_OPS_CONTACT` (yoki tegishli `--dns-*` bayroqlarini o'tkazing) o'rnating, shuning uchun DNS ishlab chiqarish uchun metadatalarni kesish kerak. Keshni bekor qilish va orqaga qaytarish ilgaklari kerak bo'lganda, `DNS_CACHE_PURGE_ENDPOINT`, `DNS_CACHE_PURGE_AUTH_ENV` va `DNS_PREVIOUS_PLAN` (yoki `--cache-purge-*` / `--previous-dns-plan` bayroqlari) qo'shing, shuning uchun API yozuvi yoki deskriptini yozing. qaytishlar. Deskriptor endi shtapelli `Sora-Route-Binding` (xost, CID, sarlavha/bog'lash yo'llari, tekshirish buyruqlari) ni ham hujjatlashtiradi, shuning uchun GAR targ'iboti va zaxira rejasini ko'rib chiqishlar chetida xizmat ko'rsatgan aniq sarlavhalarga havola qiladi.
- Xuddi shu ish jarayoni `scripts/sns_zonefile_skeleton.py` orqali SNS zonasi fayli skeleti/resolver parchasini chiqarishi mumkin. IPv4/IPv6/CNAME/SPKI/TXT meta-ma'lumotlarini taqdim eting (`DNS_ZONEFILE_IPV4` kabi env varyasyonlari yoki `--dns-zonefile-ipv4` kabi CLI bayroqlari kabi), GAR dayjestini `DNS_GAR_DIGEST` bilan o'tkazing va yordamchi I08NI02 (I08NI02) ni hal qiladi. snippet) avtomatik ravishda SN-7 dalillari kesish deskriptori bilan birga tushadi.
- DNS identifikatori endi yuqoridagi artefaktlarga (yo'llar, kontent CID, isbot holati va so'zma-so'z sarlavha shabloniga) havola qiluvchi `gateway_binding` bo'limini o'rnatadi, shuning uchun DG-3 o'zgarishlarini tasdiqlash shlyuzda kutilayotgan aniq `Sora-Name/Sora-Proof/CSP/HSTS` to'plamini o'z ichiga oladi.

Har bir `npm run build` chaqiruvi `postbuild` kancasini ishga tushiradi.
`build/checksums.sha256`. Qurgandan so'ng (yoki CI artefaktlarini yuklab oling), ishga tushiring
`./docs/portal/scripts/preview_verify.sh --build-dir docs/portal/build` gacha
manifestni tasdiqlang. `--descriptor <path>`/`--archive <path>` qachon o'ting
CI dan metadata to'plamini tekshirish, shuning uchun skript yozilganlarni o'zaro tekshirishi mumkin
digests va fayl nomlari.
- Mahalliy oldindan ko'rishni xavfsiz almashish uchun `npm run serve` ni ishga tushiring; buyruq endi o'raladi
  `scripts/serve-verified-preview.mjs`, `preview_verify.sh`ni bajaradi
  `docusaurus serve` ishga tushirishdan oldin va agar manifest tekshiruvi bajarilmasa yoki to'xtatiladi
  yo'qolgan, hatto CI dan tashqarida ham oldindan ko'rishlar nazorat summasini himoya qiladi. The
  `npm run serve:verified` taxallus aniq qo'ng'iroqlar uchun mavjud bo'lib qoladi.

### URL manzillari va nashr qaydlarini oldindan ko'rish

- Umumiy beta-versiyasi: `https://docs.iroha.tech/`
- GitHub shuningdek, har bir joylashtirish uchun **github-pages** muhiti ostida tuzilishni ochib beradi.
- Portal mazmuniga teguvchi tortish soʻrovlari oʻz ichiga oʻrnatilgan sayt, nazorat summasi manifesti, siqilgan arxiv va deskriptorni oʻz ichiga olgan Actions artefaktlarini (`docs-portal-preview`, `docs-portal-preview-metadata`) oʻz ichiga oladi; sharhlovchilar `index.html` ni mahalliy sifatida yuklab olishlari va ochishlari va oldindan ko'rishlarni almashishdan oldin nazorat summalarini tekshirishlari mumkin. Ish jarayoni har bir PRda xulosa sharhini (manifest/arxiv xeshlari va SoraFS holati) tushiradi, shuning uchun sharhlovchilar tekshirishdan o'tganligi haqida tezkor signalga ega bo'ladilar.
- Havolani tashqaridan ulashishdan oldin artefaktlar CI ishlab chiqargan narsaga mos kelishini tasdiqlash uchun oldindan ko'rish to'plamini yuklab olgandan so'ng `./docs/portal/scripts/preview_verify.sh --build-dir <extracted build> --descriptor <descriptor> --archive <archive>` dan foydalaning.
- Chiqarish eslatmalari yoki holat yangilanishlarini tayyorlashda tashqi sharhlovchilar omborni klonlashsiz eng soʻnggi portal suratini koʻrib chiqishlari uchun oldindan koʻrish URL manziliga murojaat qiling.
- `docs/portal/docs/devportal/preview-invite-flow.md` orqali oldindan ko'rish to'lqinlarini muvofiqlashtiring
  va uni har bir `docs/portal/docs/devportal/reviewer-onboarding.md` bilan bog'lang
  taklif qilish, telemetriyani eksport qilish va tashqariga chiqish bosqichi bir xil dalillar izidan qayta foydalanadi.