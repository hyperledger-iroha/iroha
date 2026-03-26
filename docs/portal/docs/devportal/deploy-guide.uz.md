---
lang: uz
direction: ltr
source: docs/portal/docs/devportal/deploy-guide.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 2ee0a1d26e0c9f3c1ab8908f7eb0dc73049d452e451aff9d2d892c19d733557e
source_last_modified: "2026-01-22T16:26:46.492940+00:00"
translation_last_reviewed: 2026-02-07
id: deploy-guide
title: SoraFS Deployment Guide
sidebar_label: Deployment Guide
description: Promote the developer portal through the SoraFS pipeline with deterministic builds, Sigstore signing, and rollback drills.
translator: machine-google-reviewed
---

## Umumiy ko'rinish

Bu kitob **DOCS-7** (SoraFS nashriyot) va **DOCS-8** yoʻl xaritasi elementlarini oʻzgartiradi.
(CI/CD pin avtomatizatsiyasi) ishlab chiquvchi portali uchun amaldagi protseduraga aylantiring.
U qurish/lint bosqichini, SoraFS qadoqlashni, Sigstore tomonidan qo'llab-quvvatlangan manifestni qamrab oladi.
imzolash, taxallusni ilgari surish, tekshirish va orqaga qaytarish mashqlari, shuning uchun har bir oldindan ko'rish va
reliz artefakti takrorlanishi va tekshirilishi mumkin.

Oqim sizda `sorafs_cli` ikkilik faylga ega ekanligingizni taxmin qiladi (o'rnatilgan).
`--features cli`), Torii so'nggi nuqtasiga pin-ro'yxatga olish ruxsati bilan kirish va
OIDC Sigstore uchun hisob ma'lumotlari. Uzoq muddatli sirlarni saqlang (`IROHA_PRIVATE_KEY`,
`SIGSTORE_ID_TOKEN`, Torii tokenlari) sizning CI omboringizda; mahalliy ishga tushirish ularni manba qilishi mumkin
qobiq eksportidan.

## Old shartlar

- `npm` yoki `pnpm` bilan 18.18+ tugun.
- `cargo run -p sorafs_car --features cli --bin sorafs_cli` dan `sorafs_cli`.
- Torii URL manzili, `/v1/sorafs/*` va vakolatli hisob/maxfiy kalit
  manifest va taxalluslarni yuborishi mumkin.
- OIDC emitenti (GitHub Actions, GitLab, ish yukining identifikatori va boshqalar)
  `SIGSTORE_ID_TOKEN`.
- Majburiy emas: quruq yugurish uchun `examples/sorafs_cli_quickstart.sh` va
  GitHub/GitLab ish oqimi iskala uchun `docs/source/sorafs_ci_templates.md`.
- Try it OAuth parametrlarini (`DOCS_OAUTH_*`) sozlang va ishga tushiring.
  Qurilishni targ'ib qilishdan oldin [xavfsizlikni kuchaytirish bo'yicha nazorat ro'yxati](./security-hardening.md)
  laboratoriya tashqarisida. Ushbu o'zgaruvchilar yo'q bo'lganda portal qurilishi endi ishlamay qoladi
  yoki TTL/saylov tugmalari majburiy oynalardan tashqariga tushganda; eksport
  `DOCS_OAUTH_ALLOW_INSECURE=1` faqat bir martalik mahalliy oldindan koʻrish uchun. ni biriktiring
  ozodlik chiptasiga pen-test dalil.

## 0-qadam — Sinab ko'ring proksi to'plamini oling

Netlify yoki shlyuzga oldindan ko‘rishni targ‘ib qilishdan oldin, sinab ko‘ring proksi-serverga muhr qo‘ying
manbalar va imzolangan OpenAPI manifest dayjesti deterministik to'plamga aylanadi:

```bash
cd docs/portal
npm run release:tryit-proxy -- \
  --out ../../artifacts/tryit-proxy/$(date -u +%Y%m%dT%H%M%SZ) \
  --target https://torii.dev.sora \
  --label preview-2026-02-14
```

`scripts/tryit-proxy-release.mjs` proksi/prob/orqaga qaytarish yordamchilaridan nusxa oladi,
OpenAPI imzosini tekshiradi va `release.json` plus yozadi
`checksums.sha256`. Ushbu toʻplamni Netlify/SoraFS shlyuzi reklamasiga biriktiring
chipta, shuning uchun sharhlovchilar aniq proksi-manbalarni va Torii maqsadli maslahatlarni takrorlashlari mumkin
qayta qurmasdan. To'plam, shuningdek, mijoz tomonidan taqdim etilgan tashuvchilar bor-yo'qligini ham qayd etadi
ishga tushirish rejasi va CSP qoidalarini sinxronlashtirish uchun yoqilgan (`allow_client_auth`).

## 1-qadam - Portalni yarating va to'ldiring

```bash
cd docs/portal
npm ci
npm run sync-openapi
npm run sync-norito-snippets
npm run test:norito-snippets
npm run test:widgets
npm run check:links
npm run build
```

`npm run build` avtomatik ravishda `scripts/write-checksums.mjs` ni ishga tushirib, quyidagilarni ishlab chiqaradi:

- `build/checksums.sha256` - `sha256sum -c` uchun mos SHA256 manifest.
- `build/release.json` — metamaʼlumotlar (`tag`, `generated_at`, `source`)
  har bir CAR/manifest.

Har ikkala faylni ham CAR xulosasi bilan birga arxivlang, shunda sharhlovchilar oldindan koʻrishni farq qilishi mumkin
qayta tiklanmasdan artefaktlar.

## 2-qadam - Statik aktivlarni to'plang

CAR paketini Docusaurus chiqish katalogiga qarshi ishga tushiring. Quyidagi misol
`artifacts/devportal/` ostida barcha artefaktlarni yozadi.

```bash
OUT=artifacts/devportal
mkdir -p "$OUT"

sorafs_cli car pack \
  --input build \
  --car-out "$OUT"/portal.car \
  --plan-out "$OUT"/portal.plan.json \
  --summary-out "$OUT"/portal.car.json \
  --chunker-handle sorafs.sf1@1.0.0
```

Xulosa JSON parchalarni hisoblash, hazm qilish va isbotlashni rejalashtirish bo'yicha maslahatlarni oladi.
`manifest build` va CI asboblar paneli keyinroq qayta ishlatiladi.

## 2b-qadam — OpenAPI toʻplami va SBOM hamrohlari

DOCS-7 portal sayti, OpenAPI surati va SBOM foydali yuklarini nashr qilishni talab qiladi.
aniq namoyon bo'lganidek, shlyuzlar `Sora-Proof`/`Sora-Content-CID` ni mahkamlashi mumkin
har bir artefakt uchun sarlavhalar. Chiqarish yordamchisi
(`scripts/sorafs-pin-release.sh`) allaqachon OpenAPI katalogini paketlaydi
(`static/openapi/`) va `syft` orqali chiqarilgan SBOMlar alohida
`openapi.*`/`*-sbom.*` CARs va metama'lumotlarni yozib oladi
`artifacts/sorafs/portal.additional_assets.json`. Qo'lda oqimni ishga tushirganda,
Har bir foydali yuk uchun 2–4-bosqichlarni oʻz prefikslari va metamaʼlumotlar yorliqlari bilan takrorlang
(masalan, `--car-out "$OUT"/openapi.car` plus
`--metadata alias_label=docs.sora.link/openapi`). Har bir manifest/taxallusni ro'yxatdan o'tkazing
DNS-ni almashtirishdan oldin Torii (sayt, OpenAPI, portal SBOM, OpenAPI SBOM) bilan bog'lang.
shlyuz barcha nashr etilgan artefaktlar uchun zımbalangan dalillarga xizmat qilishi mumkin.

## 3-qadam - Manifestni yarating

```bash
sorafs_cli manifest build \
  --summary "$OUT"/portal.car.json \
  --manifest-out "$OUT"/portal.manifest.to \
  --manifest-json-out "$OUT"/portal.manifest.json \
  --pin-min-replicas 5 \
  --pin-storage-class warm \
  --pin-retention-epoch 14 \
  --metadata alias_label=docs.sora.link
```

Chiqarish oynasiga pin siyosati bayroqlarini sozlang (masalan, `--pin-storage-class)
kanareykalar uchun issiq`). JSON varianti ixtiyoriy, lekin kodni tekshirish uchun qulay.

## 4-qadam — Sigstore bilan imzolang

```bash
sorafs_cli manifest sign \
  --manifest "$OUT"/portal.manifest.to \
  --chunk-plan "$OUT"/portal.plan.json \
  --bundle-out "$OUT"/portal.manifest.bundle.json \
  --signature-out "$OUT"/portal.manifest.sig \
  --identity-token-provider github-actions \
  --identity-token-audience sorafs-devportal
```

To'plam manifest dayjest, parcha dayjestlar va BLAKE3 xeshini qayd qiladi.
OIDC tokeni JWTni davom ettirmasdan. To'plamni ham, ajratilgan holda ham saqlang
imzo; ishlab chiqarish reklamalari iste'foga chiqish o'rniga bir xil artefaktlarni qayta ishlatishi mumkin.
Mahalliy ishga tushirishlar provayder bayroqlarini `--identity-token-env` (yoki o'rnatish) bilan almashtirishi mumkin
`SIGSTORE_ID_TOKEN` muhitda) tashqi OIDC yordamchisi
token.

## 5-qadam - PIN reestriga yuboring

Imzolangan manifestni (va bo'lak rejasini) Torii manziliga yuboring. Har doim xulosa so'rang
natijada ro'yxatga olish kitobi yozuvi/taxallus isboti tekshirilishi mumkin.

```bash
sorafs_cli manifest submit \
  --manifest "$OUT"/portal.manifest.to \
  --chunk-plan "$OUT"/portal.plan.json \
  --torii-url "$TORII_URL" \
  --authority <katakana-i105-account-id> \
  --private-key "$IROHA_PRIVATE_KEY" \
  --submitted-epoch 20260101 \
  --alias-namespace docs \
  --alias-name sora.link \
  --alias-proof "$OUT"/docs.alias.proof \
  --summary-out "$OUT"/portal.submit.json \
  --response-out "$OUT"/portal.submit.response.json
```

Oldindan koʻrish yoki kanareyka taxallusi (`docs-preview.sora`) chiqarilayotganda
noyob taxallus bilan taqdim etish, shuning uchun QA ishlab chiqarishdan oldin tarkibni tekshirishi mumkin
rag'batlantirish.

Taxallusni ulash uchta maydonni talab qiladi: `--alias-namespace`, `--alias-name` va
`--alias-proof`. Boshqaruv isbot to'plamini ishlab chiqaradi (base64 yoki Norito bayt)
taxallus so'rovi tasdiqlanganda; uni CI sirlarida saqlang va uni a sifatida ko'rsating
`manifest submit` ni chaqirishdan oldin fayl. Taxallus bayroqlarini o'rnatmagan holda qoldiring
faqat manifestni DNS-ga tegmasdan mahkamlash niyatida.

## 5b-qadam — Boshqaruv taklifini ishlab chiqish

Har bir manifest Parlament tayyor taklifi bilan sayohat qilishi kerak, shunda har qanday Sora
Fuqaro o'zgartirishni imtiyozli guvohnoma olmasdan kiritishi mumkin.
Yuborish/imzolash bosqichlaridan so‘ng quyidagini bajaring:

```bash
sorafs_cli manifest proposal \
  --manifest "$OUT"/portal.manifest.to \
  --chunk-plan "$OUT"/portal.plan.json \
  --submitted-epoch 20260101 \
  --alias-hint docs.sora.link \
  --proposal-out "$OUT"/portal.pin.proposal.json
```

`portal.pin.proposal.json` kanonik `RegisterPinManifest` ni suratga oladi
yo'riqnoma, parcha dayjesti, siyosat va taxallusga ishora. Uni boshqaruvga biriktiring
chipta yoki Parlament portali, shuning uchun delegatlar yukni qayta tiklamasdan farq qilishi mumkin
artefaktlar. Chunki buyruq hech qachon Torii vakolat kalitiga tegmaydi, har qanday
Fuqaro o'z taklifini mahalliy darajada ishlab chiqishi mumkin.

## 6-qadam - Dalillar va telemetriyani tekshiring

Qattiqlashdan so'ng, deterministik tekshirish bosqichlarini bajaring:

```bash
sorafs_cli proof verify \
  --manifest "$OUT"/portal.manifest.to \
  --car "$OUT"/portal.car \
  --summary-out "$OUT"/portal.proof.json

sorafs_cli manifest verify-signature \
  --manifest "$OUT"/portal.manifest.to \
  --bundle "$OUT"/portal.manifest.bundle.json \
  --chunk-plan "$OUT"/portal.plan.json
```

- `torii_sorafs_gateway_refusals_total` va tekshiring
  Anomaliyalar uchun `torii_sorafs_replication_sla_total{outcome="missed"}`.
- Try-It proksi-serverini va yozib olingan havolalarni ishlatish uchun `npm run probe:portal` ni ishga tushiring
  yangi mahkamlangan kontentga qarshi.
- da tasvirlangan monitoring dalillarini oling
  [Nashr qilish va monitoring](./publishing-monitoring.md) shuning uchun DOCS-3c
  Nashriyot bosqichlari bilan bir qatorda kuzatuvchanlik eshigi qondiriladi. Yordamchi
  endi bir nechta `bindings` yozuvlarini qabul qiladi (sayt, OpenAPI, SBOM portali, OpenAPI
  SBOM) va `Sora-Name`/`Sora-Proof`/`Sora-Content-CID` ni maqsadga muvofiq amalga oshiradi
  ixtiyoriy `hostname` qo'riqchisi orqali xost. Quyidagi chaqiruv ikkala a ni ham yozadi
  yagona JSON xulosasi va dalillar toʻplami (`portal.json`, `tryit.json`,
  `binding.json` va `checksums.sha256`) chiqarish katalogi ostida:

  ```bash
  npm run monitor:publishing -- \
    --config ../../configs/docs_monitor.json \
    --json-out ../../artifacts/sorafs/preview-2026-02-14/monitoring/summary.json \
    --evidence-dir ../../artifacts/sorafs/preview-2026-02-14/monitoring
  ```

## 6a-qadam - Gateway sertifikatlarini rejalashtirish

GAR paketlarini yaratishdan oldin shlyuz sifatida TLS SAN/challenge rejasini tuzing
jamoa va DNS tasdiqlovchilari bir xil dalillarni ko'rib chiqadilar. Yangi yordamchi ni aks ettiradi
Kanonik joker belgilar xostlarini sanash orqali DG-3 avtomatlashtirish kiritishlari,
yoqimli xost SAN'lari, DNS-01 teglari va tavsiya etilgan ACME muammolari:

```bash
cargo xtask soradns-acme-plan \
  --name docs.sora \
  --json-out artifacts/sorafs/portal.acme-plan.json
```

JSON-ni chiqarish to'plami bilan birga joylashtiring (yoki uni o'zgartirish bilan birga yuklang
chipta) operatorlar SAN qiymatlarini Torii-ga joylashtirishlari mumkin
`torii.sorafs_gateway.acme` konfiguratsiyasi va GAR tekshiruvchilari buni tasdiqlashlari mumkin
xost hosilalarini qayta ishga tushirmasdan kanonik/chiroyli xaritalashlar. Qo'shimcha qo'shing
Xuddi shu nashrda targ'ib qilingan har bir qo'shimcha uchun `--name` argumentlari.

## 6b-qadam - Kanonik xost xaritalarini oling

GAR foydali yuklarini shablon qilishdan oldin, har biri uchun deterministik xost xaritasini yozib oling
taxallus. `cargo xtask soradns-hosts` har bir `--name` xeshlarini o'zining kanonikiga aylantiradi
label (`<base32>.gw.sora.id`), kerakli joker belgini chiqaradi
(`*.gw.sora.id`) va chiroyli xostni keltirib chiqaradi (`<alias>.gw.sora.name`). Chidamli
reliz artefaktlaridagi chiqish, shuning uchun DG-3 sharhlovchilari xaritalashdan farq qilishi mumkin
GAR taqdimnomasi bilan bir qatorda:

```bash
cargo xtask soradns-hosts \
  --name docs.sora \
  --json-out artifacts/sorafs/portal.canonical-hosts.json
```

GAR yoki shlyuzda tez ishlamay qolish uchun `--verify-host-patterns <file>` dan foydalaning
majburiy JSON kerakli xostlardan birini o'tkazib yuboradi. Yordamchi bir nechtasini qabul qiladi
tekshirish fayllari, bu GAR shablonini ham, shablonini ham osonlashtirdi
Xuddi shu chaqiruvda `portal.gateway.binding.json` shtapelli:

```bash
cargo xtask soradns-hosts \
  --name docs.sora \
  --json-out artifacts/sorafs/portal.canonical-hosts.json \
  --verify-host-patterns artifacts/sorafs/portal.gar.json \
  --verify-host-patterns artifacts/sorafs/portal.gateway.binding.json
```

Xulosa JSON va tekshirish jurnalini DNS/shlyuzni o'zgartirish chiptasiga ilova qiling
auditorlar kanonik, joker belgilar va yoqimli xostlarni qayta ishga tushirmasdan tasdiqlashlari mumkin
hosila skriptlari. Har safar yangi taxalluslar qo'shilsa, buyruqni qayta ishga tushiring
to'plami, shuning uchun keyingi GAR yangilanishlari bir xil dalillar izini meros qilib oladi.

## 7-qadam - DNS kesish deskriptorini yarating

Ishlab chiqarishni qisqartirish uchun tekshiriladigan o'zgartirish paketi talab qilinadi. Muvaffaqiyatli so'ng
topshirish (taxallus bog'lovchi), yordamchi chiqaradi
`artifacts/sorafs/portal.dns-cutover.json`, suratga olish:- taxallusni bog'laydigan metama'lumotlar (nom maydoni/nom/dalil, manifest dayjesti, Torii URL,
  topshirilgan davr, hokimiyat);
- chiqarish konteksti (teg, taxallus yorlig'i, manifest/CAR yo'llari, parcha rejasi, Sigstore
  to'plam);
- tekshirish ko'rsatkichlari (prob buyrug'i, taxallus + Torii oxirgi nuqtasi); va
- ixtiyoriy o'zgartirishni boshqarish maydonlari (chipta identifikatori, kesish oynasi, operatsiya aloqasi,
  ishlab chiqarish xost nomi/zonasi);
- `Sora-Route-Binding` shtapelidan olingan marshrutni ilgari surish metamaʼlumotlari
  sarlavha (kanonik xost/CID, sarlavha + bog'lash yo'llari, tekshirish buyruqlari),
  GAR rag'batlantirish va qayta tiklash mashqlari bir xil dalillarga murojaat qilishni ta'minlash;
- yaratilgan marshrut rejasi artefaktlari (`gateway.route_plan.json`,
  sarlavha shablonlari va ixtiyoriy orqaga qaytarish sarlavhalari) shuning uchun chiptalar va CI ni o'zgartiring
  Lint ilgaklar har bir DG-3 paketi kanonik paketga havola qilishini tekshirishi mumkin
  tasdiqlashdan oldin rag'batlantirish/orqaga qaytarish rejalari;
- ixtiyoriy keshni bekor qilish metamaʼlumotlari (tozalash soʻnggi nuqtasi, auth oʻzgaruvchisi, JSON
  foydali yuk va misol `curl` buyrug'i); va
- oldingi identifikatorga ishora qiluvchi orqaga qaytarish bo'yicha maslahatlar (yordam yorlig'i va manifest
  digest) shuning uchun o'zgartirish chiptalari deterministik qayta yo'lni oladi.

Chiqarish keshni tozalashni talab qilganda, bilan birga kanonik rejani yarating
kesish tavsifi:

```bash
cargo xtask soradns-cache-plan \
  --name docs.sora \
  --path / \
  --path /gateway/manifest.json \
  --auth-header Authorization \
  --auth-env CACHE_PURGE_TOKEN \
  --json-out artifacts/sorafs/portal.cache_plan.json
```

Olingan `portal.cache_plan.json` ni DG-3 paketiga biriktiring, shunda operatorlar
chiqarishda deterministik xostlar/yo'llar (va mos keladigan autentifikatsiya ko'rsatmalari) mavjud
`PURGE` so'rovlari. Deskriptorning ixtiyoriy kesh metadata bo'limiga murojaat qilishi mumkin
to'g'ridan-to'g'ri ushbu fayl, o'zgarishlarni nazorat qiluvchi tekshiruvchilarni aynan qaysi biri bo'yicha hizalanadi
so'nggi nuqtalar kesish paytida yuviladi.

Har bir DG-3 paketiga reklama va orqaga qaytarish ro'yxati ham kerak. orqali yarating
`cargo xtask soradns-route-plan`, shuning uchun o'zgarishlarni nazorat qiluvchi sharhlovchilar aniq kuzatishlari mumkin
taxallus bo'yicha oldindan parvoz, kesish va orqaga qaytarish bosqichlari:

```bash
cargo xtask soradns-route-plan \
  --name docs.sora \
  --json-out artifacts/sorafs/gateway.route_plan.json
```

Chiqarilgan `gateway.route_plan.json` kanonik/chiroyli xostlarni ushlaydi, sahnalashtirilgan
sog'lig'ini tekshirish eslatmalari, GAR majburiy yangilanishlari, keshni tozalash va orqaga qaytarish amallari.
O'zgartirishni yuborishdan oldin uni GAR/bog'lash/kesish artefaktlari bilan to'plang
Ops bir xil skriptlangan qadamlarni mashq qilishi va imzolashi uchun chipta.

`scripts/generate-dns-cutover-plan.mjs` bu deskriptorga quvvat beradi va ishlaydi
avtomatik ravishda `sorafs-pin-release.sh` dan. Uni qayta tiklash yoki sozlash uchun
qo'lda:

```bash
node scripts/generate-dns-cutover-plan.mjs \
  --pin-report artifacts/sorafs/portal.pin.report.json \
  --out artifacts/sorafs/portal.dns-cutover.json \
  --change-ticket OPS-4821 \
  --dns-hostname docs.sora.link \
  --dns-zone sora.link \
  --ops-contact docs-oncall@sora.link \
  --cache-purge-endpoint https://cache.api/purge \
  --cache-purge-auth-env CACHE_PURGE_TOKEN \
  --previous-dns-plan artifacts/sorafs/previous.dns-cutover.json
```

PIN-kodni ishga tushirishdan oldin atrof-muhit o'zgaruvchilari orqali ixtiyoriy metama'lumotlarni to'ldiring
yordamchi:

| O'zgaruvchi | Maqsad |
|----------|---------|
| `DNS_CHANGE_TICKET` | Chipta identifikatori deskriptorda saqlanadi. |
| `DNS_CUTOVER_WINDOW` | ISO8601 kesish oynasi (masalan, `2026-03-21T15:00Z/2026-03-21T15:30Z`). |
| `DNS_HOSTNAME`, `DNS_ZONE` | Ishlab chiqarish xost nomi + vakolatli zona. |
| `DNS_OPS_CONTACT` | Chaqiruvdagi taxallus yoki eskalatsiya kontakti. |
| `DNS_CACHE_PURGE_ENDPOINT` | Deskriptorda qayd etilgan keshni tozalashning so'nggi nuqtasi. |
| `DNS_CACHE_PURGE_AUTH_ENV` | Tozalash tokenini oʻz ichiga olgan Env var (birlamchi `CACHE_PURGE_TOKEN`). |
| `DNS_PREVIOUS_PLAN` | Orqaga qaytarish meta-ma'lumotlari uchun oldingi kesish deskriptoriga yo'l. |

Tasdiqlovchilar manifestni tekshirishlari uchun JSON-ni DNS o'zgarishlarini tekshirishga biriktiring
CI jurnallarini qirib tashlamasdan hazm qilish, taxallus bilan bog'lash va tekshirish buyruqlari.
CLI bayroqlari `--dns-change-ticket`, `--dns-cutover-window`, `--dns-hostname`,
`--dns-zone`, `--ops-contact`, `--cache-purge-endpoint`,
`--cache-purge-auth-env` va `--previous-dns-plan` bir xil bekor qilishni ta'minlaydi
yordamchini CI tashqarisida ishga tushirganda.

## 8-qadam - Rezolyutsiya zonasi skeletini chiqaring (ixtiyoriy)

Ishlab chiqarishni kesish oynasi ma'lum bo'lganda, reliz skripti chiqaradi
SNS zonasi faylining skeleti va rezolyutsiyasi avtomatik ravishda. Kerakli DNS-ni o'tkazing
muhit o'zgaruvchilari yoki CLI opsiyalari orqali yozuvlar va metama'lumotlar; yordamchi
kesilgandan so'ng darhol `scripts/sns_zonefile_skeleton.py` ga qo'ng'iroq qiladi
deskriptor hosil bo‘ladi. Kamida bitta A/AAAA/CNAME qiymati va GARni taqdim eting
dayjest (imzolangan GAR foydali yukining BLAKE3-256). Agar zona/xost nomi ma'lum bo'lsa
va `--dns-zonefile-out` qoldirilsa, yordamchi unga yozadi
`artifacts/sns/zonefiles/<zone>/<hostname>.json` va aholi soni
`ops/soradns/static_zones.<hostname>.json` hal qiluvchi parcha sifatida.

| O'zgaruvchi / bayroq | Maqsad |
|----------------|---------|
| `DNS_ZONEFILE_OUT`, `--dns-zonefile-out` | Yaratilgan zonali skelet uchun yo'l. |
| `DNS_ZONEFILE_RESOLVER_SNIPPET`, `--dns-zonefile-resolver-snippet` | Resolver snippet yo'li (o'tkazib yuborilganda birlamchi `ops/soradns/static_zones.<hostname>.json` hisoblanadi). |
| `DNS_ZONEFILE_TTL`, `--dns-zonefile-ttl` | TTL yaratilgan yozuvlarga qo'llaniladi (standart: 600 soniya). |
| `DNS_ZONEFILE_IPV4`, `--dns-zonefile-ipv4` | IPv4 manzillari (vergul bilan ajratilgan env yoki takrorlanadigan CLI bayrog'i). |
| `DNS_ZONEFILE_IPV6`, `--dns-zonefile-ipv6` | IPv6 manzillari. |
| `DNS_ZONEFILE_CNAME`, `--dns-zonefile-cname` | Ixtiyoriy CNAME maqsadi. |
| `DNS_ZONEFILE_SPKI`, `--dns-zonefile-spki-pin` | SHA-256 SPKI pinlari (base64). |
| `DNS_ZONEFILE_TXT`, `--dns-zonefile-txt` | Qo'shimcha TXT yozuvlari (`key=value`). |
| `DNS_ZONEFILE_VERSION`, `--dns-zonefile-version` | Hisoblangan zona fayli versiyasi yorlig'ini bekor qiling. |
| `DNS_ZONEFILE_EFFECTIVE_AT`, `--dns-zonefile-effective-at` | Kesish oynasini boshlash o'rniga `effective_at` vaqt tamg'asini (RFC3339) majburlang. |
| `DNS_ZONEFILE_PROOF`, `--dns-zonefile-proof` | Meta-ma'lumotlarda qayd etilgan dalilni bekor qiling. |
| `DNS_ZONEFILE_CID`, `--dns-zonefile-cid` | Metadatada qayd etilgan CIDni bekor qiling. |
| `DNS_ZONEFILE_FREEZE_STATE`, `--dns-zonefile-freeze-state` | Guardian muzlatish holati (yumshoq, qattiq, eritish, monitoring, favqulodda). |
| `DNS_ZONEFILE_FREEZE_TICKET`, `--dns-zonefile-freeze-ticket` | Muzlatish uchun qo'riqchi/kengash chiptasi ma'lumotnomasi. |
| `DNS_ZONEFILE_FREEZE_EXPIRES_AT`, `--dns-zonefile-freeze-expires-at` | RFC3339 eritish uchun vaqt tamg'asi. |
| `DNS_ZONEFILE_FREEZE_NOTES`, `--dns-zonefile-freeze-note` | Qo'shimcha muzlatish qaydlari (vergul bilan ajratilgan env yoki takrorlanadigan bayroq). |
| `DNS_GAR_DIGEST`, `--dns-gar-digest` | Imzolangan GAR foydali yukining BLAKE3-256 dayjesti (hex). Shlyuz ulanishlari mavjud bo'lganda talab qilinadi. |

GitHub Actions ish jarayoni ushbu qiymatlarni ombor sirlaridan o'qiydi, shuning uchun har bir ishlab chiqarish pin avtomatik ravishda zona fayl artefaktlarini chiqaradi. Quyidagi sirlarni sozlang (satrlarda koʻp qiymatli maydonlar uchun vergul bilan ajratilgan roʻyxatlar boʻlishi mumkin):

| Yashirin | Maqsad |
|--------|---------|
| `DOCS_SORAFS_DNS_HOSTNAME`, `DOCS_SORAFS_DNS_ZONE` | Ishlab chiqarish xost nomi/zonasi yordamchiga uzatildi. |
| `DOCS_SORAFS_DNS_OPS_CONTACT` | Deskriptorda saqlangan qo'ng'iroq bo'yicha taxallus. |
| `DOCS_SORAFS_ZONEFILE_IPV4`, `DOCS_SORAFS_ZONEFILE_IPV6` | nashr qilish uchun IPv4/IPv6 yozuvlari. |
| `DOCS_SORAFS_ZONEFILE_CNAME` | Ixtiyoriy CNAME maqsadi. |
| `DOCS_SORAFS_ZONEFILE_SPKI` | Base64 SPKI pinlari. |
| `DOCS_SORAFS_ZONEFILE_TXT` | Qo'shimcha TXT yozuvlari. |
| `DOCS_SORAFS_ZONEFILE_FREEZE_STATE/TICKET/EXPIRES_AT/NOTES` | Skeletda yozilgan metamaʼlumotlarni muzlatish. |
| `DOCS_SORAFS_GAR_DIGEST` | Imzolangan GAR foydali yukining hex-kodlangan BLAKE3 dayjesti. |

`.github/workflows/docs-portal-sorafs-pin.yml` ishga tushirilganda, `dns_change_ticket` va `dns_cutover_window` kirishlarini kiriting, shunda deskriptor/zona fayli toʻgʻri oʻzgartirish oynasi metamaʼlumotlarini meros qilib oladi. Faqat quruq yugurishda ularni bo'sh qoldiring.

Oddiy chaqiruv (SN-7 egasining ish kitobiga mos keladi):

```bash
./docs/portal/scripts/sorafs-pin-release.sh \
  --dns-zonefile-out artifacts/sns/zonefiles/sora.link/20250303.docs.sora.json \
  --dns-zonefile-resolver-snippet ops/soradns/static_zones.docs.json \
  --dns-zonefile-ipv4 198.51.100.4 \
  --dns-zonefile-ttl 600 \
  --dns-zonefile-freeze-state soft \
  --dns-zonefile-freeze-ticket SNS-DF-XXXX \
  --dns-zonefile-freeze-expires-at 2025-03-10T12:00Z \
  --dns-gar-digest <gar-digest-hex> \
  …other flags…
```

Yordamchi avtomatik ravishda o'zgartirish chiptasini TXT yozuvi sifatida o'tkazadi va
agar kesilgan oyna boshlanishini `effective_at` vaqt tamg'asi sifatida meros qilib oladi
bekor qilingan. To'liq operatsion ish jarayoni uchun qarang
`docs/source/sorafs_gateway_dns_owner_runbook.md`.

### Umumiy DNS delegatsiyasi eslatmasi

Zonafayl skeleti faqat zona uchun vakolatli yozuvlarni belgilaydi. Siz
hali ham registrator yoki DNS da ota-zonasi NS/DS delegatsiyasini sozlashingiz kerak
provayder, shuning uchun oddiy internet nom serverlarini topishi mumkin.

- Apex/TLD kesishlari uchun ALIAS/ANAME (provayderga xos) dan foydalaning yoki A/AAAA-ni nashr qiling.
  shlyuzga ishora qiluvchi yozuvlar anycast IP-lar.
- Subdomenlar uchun CNAME-ni olingan yoqimli xostga nashr qiling
  (`<fqdn>.gw.sora.name`).
- Kanonik xost (`<hash>.gw.sora.id`) shlyuz domeni ostida qoladi va
  umumiy hududingizda chop etilmagan.

### Gateway sarlavhasi shabloni

O'rnatish yordamchisi ham `portal.gateway.headers.txt` va chiqaradi
`portal.gateway.binding.json`, DG-3 talablarini qondiradigan ikkita artefakt
shlyuz-kontentni bog'lash talabi:

- `portal.gateway.headers.txt` to'liq HTTP sarlavha blokini o'z ichiga oladi (shu jumladan
  `Sora-Name`, `Sora-Content-CID`, `Sora-Proof`, CSP, HSTS va
  `Sora-Route-Binding` deskriptori) chekka shlyuzlar har bir shlyuzga mahkamlanishi kerak.
  javob.
- `portal.gateway.binding.json` xuddi shu ma'lumotni mashinada o'qilishi mumkin bo'lgan shaklda yozadi
  formasini o'zgartiring, shuning uchun chiptalarni o'zgartiring va avtomatlashtirish xost/sid bog'lanishlarisiz farq qilishi mumkin
  qirib tashlash qobiq chiqishi.

Ular orqali avtomatik ravishda yaratiladi
`cargo xtask soradns-binding-template`
va taqdim etilgan taxallus, manifest dayjesti va shlyuz xost nomini yozib oling
`sorafs-pin-release.sh` ga. Sarlavha blokini qayta tiklash yoki sozlash uchun quyidagilarni bajaring:

```bash
cargo xtask soradns-binding-template \
  --manifest artifacts/sorafs/portal.manifest.json \
  --alias docs.sora \
  --hostname docs.sora.link \
  --route-label production \
  --json-out artifacts/sorafs/portal.gateway.binding.json \
  --headers-out artifacts/sorafs/portal.gateway.headers.txt
```

Bekor qilish uchun `--csp-template`, `--permissions-template` yoki `--hsts-template` dan oʻting
ma'lum bir joylashtirishga qo'shimcha kerak bo'lganda standart sarlavha shablonlari
direktivalar; sarlavhani tushirish uchun ularni mavjud `--no-*` kalitlari bilan birlashtiring
butunlay.

Sarlavha snippetini CDN o'zgartirish so'roviga biriktiring va JSON hujjatini yuboring
shlyuzni avtomatlashtirish quvuriga kiriting, shuning uchun haqiqiy xost reklamasi mos keladi
dalillarni ozod qilish.

Chiqarish skripti DG-3 chiptalarini avtomatik ravishda tekshirish yordamchisini ishga tushiradi
har doim oxirgi dalillarni o'z ichiga oladi. Har safar o'zgartirganingizda uni qo'lda qayta ishga tushiring
JSONni qo'lda ulash:

```bash
cargo xtask soradns-verify-binding \
  --binding artifacts/sorafs/portal.gateway.binding.json \
  --alias docs.sora.link \
  --hostname docs.sora.link \
  --proof-status ok \
  --manifest-json artifacts/sorafs/portal.manifest.json
```

Buyruq bog'lash to'plamida olingan `Sora-Proof` foydali yukini tasdiqlaydi,
`Sora-Route-Binding` metadata manifest CID + xost nomiga mos kelishini ta'minlaydi,
va har qanday sarlavha drifts bo'lsa tez muvaffaqiyatsiz. yonidagi konsol chiqishini arxivlang
DG-3 dan tashqarida buyruqni ishga tushirganingizda boshqa joylashtirish artefaktlari
sharhlovchilar bog'lash kesishdan oldin tasdiqlanganligini tasdiqlovchi dalillarga ega.> **DNS deskriptor integratsiyasi:** `portal.dns-cutover.json` endi
> Ushbu artefaktlarga ishora qiluvchi `gateway_binding` bo'limi (yo'llar, kontent CID,
> isbot holati va tom maʼnodagi sarlavha shablonlari) **va** `route_plan` bandi
> `gateway.route_plan.json` va asosiy + orqaga qaytarish sarlavhasiga havola
> andozalar. Ushbu bloklarni har bir DG-3 o'zgartirish chiptasiga qo'shing, shunda sharhlovchilar mumkin
> aniq `Sora-Name/Sora-Proof/CSP` qiymatlarini farqlang va marshrutni tasdiqlang
> rag'batlantirish/orqaga qaytarish rejalari qurilishni ochmasdan dalillar to'plamiga mos keladi
> arxiv.

## 9-qadam - nashriyot monitorlarini ishga tushirish

Yo‘l xaritasi vazifasi **DOCS-3c** portaldan doimiy dalillarni talab qiladi, Sinab ko‘ring
proksi va shlyuz ulanishlari chiqarilgandan keyin sog'lom bo'lib qoladi. Birlashtirilganni ishga tushiring
7–8-qadamlardan so'ng darhol kuzatib boring va uni rejalashtirilgan zondlaringizga ulang:

```bash
cd docs/portal
npm run monitor:publishing -- \
  --config ../../configs/docs_monitor.json \
  --json-out ../../artifacts/sorafs/${RELEASE_TAG}/monitoring/summary-$(date -u +%Y%m%dT%H%M%SZ).json \
  --evidence-dir ../../artifacts/sorafs/${RELEASE_TAG}/monitoring
```

- `scripts/monitor-publishing.mjs` konfiguratsiya faylini yuklaydi (qarang
  Sxema uchun `docs/portal/docs/devportal/publishing-monitoring.md`) va
  uchta tekshiruvni amalga oshiradi: portal yo'li problari + CSP/ruxsatlar - siyosatni tekshirish,
  Proksi-zondlarni sinab ko'ring (ixtiyoriy ravishda `/metrics` so'nggi nuqtasini bosing) va
  tekshiradigan shlyuzni ulash tekshiruvi (`cargo xtask soradns-verify-binding`).
  olingan bog'lash to'plami kutilgan taxallusga, xostga, isbot holatiga,
  va manifest JSON.
- Har qanday prob muvaffaqiyatsiz bo'lsa, buyruq noldan farq qiladi, shuning uchun CI, cron jobs yoki
  runbook operatorlari taxalluslarni targ'ib qilishdan oldin nashrni to'xtatishi mumkin.
- `--json-out` dan o'tish har bir maqsad bilan bitta umumiy JSON foydali yukini yozadi
  holat; `--evidence-dir` chiqaradi `summary.json`, `portal.json`, `tryit.json`,
  `binding.json` va `checksums.sha256`, shuning uchun boshqaruv sharhlovchilari farq qilishi mumkin
  monitorlarni qayta ishga tushirmasdan natijalar beradi. Ushbu katalogni ostida arxivlang
  `artifacts/sorafs/<tag>/monitoring/` Sigstore to'plami va DNS bilan birga
  kesish tavsifi.
- Monitor chiqishini qo'shing, Grafana eksporti (`dashboards/grafana/docs_portal.json`),
  va Alertmanager matkap ID reliz chipta shunday DOCS-3c SLO bo'lishi mumkin
  keyinroq tekshiriladi. Maxsus nashriyot monitor o'yin kitobi shu manzilda yashaydi
  `docs/portal/docs/devportal/publishing-monitoring.md`.

Portal tekshiruvlari HTTPS ni talab qiladi va `http://` asosiy URL manzillarini rad etadi.
`allowInsecureHttp` monitor konfiguratsiyasida o'rnatilgan; ishlab chiqarish/sahnalashtirishni davom ettirish
TLS-dagi maqsadlar va faqat mahalliy oldindan ko'rish uchun bekor qilishni yoqing.

Bir marta Buildkite/cron da `npm run monitor:publishing` orqali monitorni avtomatlashtiring.
portal jonli. Ishlab chiqarish URL manzillariga ishora qilingan xuddi shu buyruq davom etayotganlarni ta'minlaydi
relizlar orasida SRE/Docs tayanadigan sog'liq tekshiruvlari.

## `sorafs-pin-release.sh` bilan avtomatlashtirish

`docs/portal/scripts/sorafs-pin-release.sh` 2–6-qadamlarni qamrab oladi. Bu:

1. `build/` arxivlari deterministik tarballga,
2. ishlaydi `car pack`, `manifest build`, `manifest sign`, `manifest verify-signature`,
   va `proof verify`,
3. Torii bo‘lganda ixtiyoriy ravishda `manifest submit` (shu jumladan taxallusni bog‘lash) ni bajaradi
   hisobga olish ma'lumotlari mavjud va
4. yozadi `artifacts/sorafs/portal.pin.report.json`, ixtiyoriy
  `portal.pin.proposal.json`, DNS kesish deskriptori (yuborishdan keyin),
  va shlyuzni ulash to'plami (`portal.gateway.binding.json` plus
  matn sarlavhasi bloki) shuning uchun boshqaruv, tarmoq va operatsiyalar guruhlari farq qilishi mumkin
  CI jurnallarini qirib tashlamasdan dalillar to'plami.

`PIN_ALIAS`, `PIN_ALIAS_NAMESPACE`, `PIN_ALIAS_NAME` va (ixtiyoriy) oʻrnating
Skriptni chaqirishdan oldin `PIN_ALIAS_PROOF_PATH`. Quritish uchun `--skip-submit` dan foydalaning
yugurish; Quyida tavsiflangan GitHub ish jarayoni buni `perform_submit` orqali almashtiradi
kiritish.

## 8-qadam — OpenAPI xususiyatlari va SBOM toʻplamlarini nashr qilish

DOCS-7 sayohat qilish uchun portal tuzilishi, OpenAPI spetsifikatsiyasi va SBOM artefaktlarini talab qiladi.
bir xil deterministik quvur liniyasi orqali. Mavjud yordamchilar uchtasini ham qamrab oladi:

1. **Tekshiruvni qayta tiklang va imzolang.**

   ```bash
   npm run sync-openapi -- --version=2025-q3 --mirror=current --latest
   cargo xtask openapi --sign docs/portal/static/openapi/manifest.json
   ```

   Agar saqlamoqchi bo'lsangiz, `--version=<label>` orqali chiqarish yorlig'ini o'tkazing
   tarixiy surat (masalan, `2025-q3`). Yordamchi suratni yozadi
   `static/openapi/versions/<label>/torii.json` ga, uni aks ettiradi
   `versions/current` va metama'lumotlarni yozib oladi (SHA-256, manifest holati va
   yangilangan vaqt tamg'asi) `static/openapi/versions.json` da. Ishlab chiquvchi portali
   Swagger/RapiDoc panellari versiyani tanlash vositasini taqdim etishi uchun ushbu indeksni o'qiydi
   va tegishli dayjest/imzo ma'lumotlarini inline ko'rsatish. O'tkazib yuborish
   `--version` oldingi nashr yorliqlarini saqlab qoladi va faqat yangilaydi
   `current` + `latest` ko'rsatkichlari.

   Manifest SHA-256/BLAKE3 hazm bo'lishini oladi, shunda shlyuz shtapelni oladi.
   `Sora-Proof` sarlavhalari `/reference/torii-swagger` uchun.

2. **CycloneDX SBOM-larini chiqaring.** Chiqaruvchi quvur liniyasi allaqachon syft-ga asoslanganini kutmoqda.
   `docs/source/sorafs_release_pipeline_plan.md` uchun SBOMlar. Chiqishni saqlang
   qurilish artefaktlari yonida:

   ```bash
   syft dir:build -o json > "$OUT"/portal.sbom.json
   syft file:docs/portal/static/openapi/torii.json -o json > "$OUT"/openapi.sbom.json
   ```

3. **Har bir foydali yukni avtomobilga joylashtiring.**

   ```bash
   sorafs_cli car pack \
     --input docs/portal/static/openapi \
     --car-out "$OUT"/openapi.car \
     --plan-out "$OUT"/openapi.plan.json \
     --summary-out "$OUT"/openapi.car.json

   sorafs_cli car pack \
     --input "$OUT"/portal.sbom.json \
     --car-out "$OUT"/portal.sbom.car \
     --plan-out "$OUT"/portal.sbom.plan.json \
     --summary-out "$OUT"/portal.sbom.car.json
   ```

   Asosiy sayt bilan bir xil `manifest build` / `manifest sign` qadamlarini bajaring,
   har bir aktiv uchun taxalluslarni sozlash (masalan, spetsifikatsiyalar uchun `docs-openapi.sora` va
   Imzolangan SBOM to'plami uchun `docs-sbom.sora`). Alohida taxalluslarni saqlash
   SoraDNS dalillarini, GARlarni va qaytarib olish chiptalarini aniq foydali yuk bilan ta'minlaydi.

4. **Yuborish va bog‘lash.** Mavjud vakolat + Sigstore to‘plamidan qayta foydalaning, lekin
   relizlar ro'yxatiga taxallus kortejini yozib oling, shunda auditorlar qaysi birini kuzatishi mumkin
   Manifest hazm bo'ladigan Sora nomi xaritalar.

Portal qurilishi bilan birga spetsifikatsiya/SBOM manifestlarini arxivlash har bir narsani ta'minlaydi
Chipta paketni qayta ishga tushirmasdan to'liq artefakt to'plamini o'z ichiga oladi.

### Avtomatlashtirish yordamchisi (CI/paket skripti)

`./ci/package_docs_portal_sorafs.sh` 1–8-qadamlarni kodlaydi, shuning uchun yoʻl xaritasi bandi
**DOCS‑7** bitta buyruq bilan bajarilishi mumkin. Yordamchi:

- kerakli portalni tayyorlashni amalga oshiradi (`npm ci`, OpenAPI/norito sinxronlash, vidjet testlari);
- `sorafs_cli` orqali portal, OpenAPI va SBOM CARs + manifest juftlarini chiqaradi;
- ixtiyoriy ravishda `sorafs_cli proof verify` (`--proof`) va Sigstore imzosini ishga tushiradi
  (`--sign`, `--sigstore-provider`, `--sigstore-audience`);
- har bir artefaktni `artifacts/devportal/sorafs/<timestamp>/` ostida tushiradi va
  `package_summary.json` deb yozadi, shuning uchun CI/release asboblari to'plamni qabul qilishi mumkin; va
- eng so'nggi ishga tushirishni ko'rsatish uchun `artifacts/devportal/sorafs/latest` ni yangilaydi.

Misol (Sigstore + PoR bilan to'liq quvur liniyasi):

```bash
./ci/package_docs_portal_sorafs.sh \
  --proof \
  --sign \
  --sigstore-provider=github-actions \
  --sigstore-audience=sorafs-devportal
```

Bilishga arziydigan bayroqlar:

- `--out <dir>` - artefakt ildizini bekor qilish (standart vaqt tamg'asi bilan belgilangan papkalarni saqlaydi).
- `--skip-build` - mavjud `docs/portal/build` dan qayta foydalanish (CI imkoni bo'lmaganda qulay
  oflayn oynalar tufayli qayta qurish).
- `--skip-sync-openapi` - `cargo xtask openapi` bo'lganda `npm run sync-openapi` ni o'tkazib yuboring
  crates.io-ga kira olmaydi.
- `--skip-sbom` - ikkilik o'rnatilmaganda `syft` ga qo'ng'iroq qilishdan saqlaning (
  skript o'rniga ogohlantirishni chop etadi).
- `--proof` - har bir CAR/manifest juftligi uchun `sorafs_cli proof verify`-ni ishga tushiring. ko'p
  fayl yuklamalari hali ham CLI-da chunk-planni qo'llab-quvvatlashni talab qiladi, shuning uchun bu bayroqni qoldiring
  Agar siz `plan chunk count` xatolarini topsangiz va bir marta qo'lda tekshirsangiz, o'rnatilmagan.
  yuqori oqim darvozasi erlari.
- `--sign` - `sorafs_cli manifest sign` ni chaqiring. Tokenni taqdim eting
  `SIGSTORE_ID_TOKEN` (yoki `--sigstore-token-env`) yoki CLI ga uni olishiga ruxsat bering.
  `--sigstore-provider/--sigstore-audience`.

Ishlab chiqarish artefaktlarini jo'natishda `docs/portal/scripts/sorafs-pin-release.sh` dan foydalaning.
Endi u portalni, OpenAPI va SBOM foydali yuklarini paketlaydi, har bir manifestni imzolaydi va
`portal.additional_assets.json` da qoʻshimcha aktiv metamaʼlumotlarini qayd qiladi. Yordamchi
CI paketlovchisi va yangisi tomonidan ishlatiladigan bir xil ixtiyoriy tugmalarni tushunadi
`--openapi-*`, `--portal-sbom-*` va `--openapi-sbom-*` kalitlari shu sababli
artefakt uchun taxallus kortejlarini tayinlash, orqali SBOM manbasini bekor qilish
`--openapi-sbom-source`, ba'zi foydali yuklarni o'tkazib yuborish (`--skip-openapi`/`--skip-sbom`),
va `--syft-bin` bilan standart bo'lmagan `syft` binarga ishora qiling.

Skript o'zi bajaradigan har bir buyruqni ko'rsatadi; jurnalni chiqish chiptasiga nusxalash
`package_summary.json` bilan bir qatorda, sharhlovchilar CAR dayjestlarini farqlashlari mumkin, rejalashtirish
metama'lumotlar va Sigstore to'plam xeshlarini maxsus qobiq chiqishisiz.

## 9-qadam - Gateway + SoraDNS tekshiruvi

Kesishni e'lon qilishdan oldin, yangi taxallus SoraDNS orqali hal qilinishini isbotlang
shlyuzlar shtapel yangi dalillar:

1. **Zand eshigini ishga tushiring.** `ci/check_sorafs_gateway_probe.sh` mashqlari
   demo qurilmalariga qarshi `cargo xtask sorafs-gateway-probe`
   `fixtures/sorafs_gateway/probe_demo/`. Haqiqiy joylashtirish uchun probni yo'naltiring
   maqsadli xost nomida:

   ```bash
   ./ci/check_sorafs_gateway_probe.sh -- \
     --gateway "https://docs.sora/.well-known/sorafs/manifest" \
     --header "Accept: application/json" \
     --gar fixtures/sorafs_gateway/probe_demo/demo.gar.jws \
     --gar-key "demo-gar=$(<fixtures/sorafs_gateway/probe_demo/gar_pub.hex>)" \
     --host "docs.sora" \
     --report-json artifacts/sorafs_gateway_probe/ci/docs.json
   ```

   Prob `Sora-Name`, `Sora-Proof` va `Sora-Proof-Status` kodini dekodlaydi.
   `docs/source/sorafs_alias_policy.md` va manifest hazm bo'lganda muvaffaqiyatsizlikka uchraydi,
   TTLlar yoki GAR bog'lanishlari drift.

   Engil vaznli nuqta tekshiruvlari uchun (masalan, faqat bog'lovchi to'plam bo'lganda
   o'zgartirildi), `cargo xtask soradns-verify-binding --binding <portal.gateway.binding.json> --alias "<alias>" --hostname "<gateway-host>" --proof-status ok --manifest-json <portal.manifest.json>` ni ishga tushiring.
   Yordamchi qo'lga kiritilgan bog'lash to'plamini tasdiqlaydi va ozod qilish uchun qulaydir
   to'liq prob matkap o'rniga faqat majburiy tasdiqlash kerak chiptalar.

2. **Matkap dalillarini oling.** Operator matkaplari yoki PagerDuty quruq yugurishlar uchun
   `scripts/telemetry/run_sorafs_gateway_probe.sh --ssenariy bilan tekshirish
   devportal-rollout -- …`. Oʻram sarlavhalar/jurnallarni ostida saqlaydi
   `artifacts/sorafs_gateway_probe/<stamp>/`, yangilanishlar `ops/drill-log.md` va
   (ixtiyoriy) orqaga qaytarish ilgaklarini yoki PagerDuty foydali yuklarini ishga tushiradi. Oʻrnatish
   IP-ni qattiq kodlash o'rniga SoraDNS yo'lini tekshirish uchun `--host docs.sora`.3. **DNS ulanishlarini tekshiring.** Boshqaruv taxallus isbotini nashr qilganda, yozib oling
   probda havola qilingan GAR fayli (`--gar`) va uni nashrga biriktiring
   dalil. Resolver egalari bir xil kirishni aks ettirishi mumkin
   Keshlangan yozuvlar yangi manifestga mos kelishini ta'minlash uchun `tools/soradns-resolver`.
   JSONni biriktirishdan oldin ishga tushiring
   `cargo xtask soradns-verify-gar --gar <path> --name <alias> [--manifest-cid <cid>] [--telemetry-label <label>]`
   Shunday qilib, deterministik xost xaritasi, manifest metama'lumotlari va telemetriya belgilari
   oflayn rejimda tasdiqlangan. Yordamchi bilan birga `--json-out` xulosasini chiqarishi mumkin
   imzolangan GAR, shuning uchun sharhlovchilar ikkilik faylni ochmasdan tekshirilishi mumkin bo'lgan dalillarga ega.
  Yangi GAR loyihasini tuzayotganda afzal qiling
  `cargo xtask soradns-gar-template --name <alias> --manifest <portal.manifest.json> --telemetry-label <label> ...`
  (faqat manifest fayli bo'lmasa, `--manifest-cid <cid>` ga qayting
  mavjud). Endi yordamchi CID **va** BLAKE3 dayjestini to'g'ridan-to'g'ri dan oladi
  manifest JSON, bo'sh joyni qisqartiradi, takroriy `--telemetry-label` nusxalarini chiqaradi
  bayroqlar, teglarni saralaydi va standart CSP/HSTS/Permissions-Policy-ni chiqaradi
  JSON yozishdan oldin shablonlarni o'rnating, shuning uchun foydali yuk hatto qachon ham deterministik bo'lib qoladi
  operatorlar turli qobiqlardan teglarni oladi.

4. **Taxallus koʻrsatkichlarini koʻring.** `torii_sorafs_alias_cache_refresh_duration_ms` saqlang
   va ekranda `torii_sorafs_gateway_refusals_total{profile="docs"}` esa
   prob ishlayapti; ikkala seriya ham chizilgan
   `dashboards/grafana/docs_portal.json`.

## 10-qadam - Monitoring va dalillarni to'plash

- **Boshqaruv paneli.** `dashboards/grafana/docs_portal.json` eksporti (portal SLO),
  `dashboards/grafana/sorafs_gateway_observability.json` (shlyuzning kechikishi +
  sog'lig'ini isbotlash) va `dashboards/grafana/sorafs_fetch_observability.json`
  (orkestrator salomatligi) har bir nashr uchun. JSON eksportlarini biriktiring
  Chiptani chiqaring, shunda ko'rib chiquvchilar Prometheus so'rovlarini takrorlashlari mumkin.
- **Arxivlarni tekshirish.** `artifacts/sorafs_gateway_probe/<stamp>/` ni git-ilovada saqlang
  yoki sizning dalil paqiringiz. Tekshiruv xulosasi, sarlavhalar va PagerDuty-ni qo'shing
  telemetriya skripti tomonidan olingan foydali yuk.
- **Chiqarish paketi.** Portal/SBOM/OpenAPI CAR xulosalari, manifestni saqlang
  to'plamlar, Sigstore imzolari, `portal.pin.report.json`, Try-It tekshiruv jurnallari va
  bitta vaqt tamg'asi bo'lgan jild ostidagi hisobotlarni havola-tekshirish (masalan,
  `artifacts/sorafs/devportal/20260212T1103Z/`).
- **Burg'ulash jurnali.** Zondlar matkapning bir qismi bo'lsa, ruxsat bering
  `scripts/telemetry/run_sorafs_gateway_probe.sh` `ops/drill-log.md` ga qo'shing
  shuning uchun xuddi shu dalil SNNet-5 xaos talabini qondiradi.
- **Chipta havolalari.** Grafana panel identifikatorlariga yoki biriktirilgan PNG eksportiga havola qiling.
  o'zgartirish chiptasi, tekshirish hisoboti yo'li bilan birga, shuning uchun o'zgartirish-reviewers
  qobiq kirishisiz SLO-larni o'zaro tekshirishi mumkin.

## 11-qadam - Ko'p manbali matkap va skorbord dalillarini olish

SoraFS da nashr qilish uchun endi koʻp manbali olish dalillari kerak (DOCS-7/SF-6)
yuqoridagi DNS/shlyuz dalillari bilan birga. Manifestni mahkamlagandan so'ng:

1. **`sorafs_fetch` ni jonli manifestga qarshi ishga tushiring.** Xuddi shu reja/manifestdan foydalaning
   2-3-bosqichlarda ishlab chiqarilgan artefaktlar va har biri uchun berilgan shlyuz hisob ma'lumotlari
   provayder. Auditorlar orkestrni takrorlashi uchun har bir chiqishni davom ettiring
   qaror yo'li:

   ```bash
   OUT=artifacts/sorafs/devportal
   FETCH_OUT="$OUT/fetch/$(date -u +%Y%m%dT%H%M%SZ)"
   mkdir -p "$FETCH_OUT"

   cargo run -p sorafs_car --bin sorafs_fetch -- \
     --plan "$OUT/portal.plan.json" \
     --manifest-json "$OUT/portal.manifest.json" \
     --gateway-provider name=docs-us,provider-id="$DOCS_US_PROVIDER_ID",base-url="$DOCS_US_GATEWAY",stream-token="$DOCS_US_STREAM_TOKEN" \
     --gateway-provider name=docs-eu,provider-id="$DOCS_EU_PROVIDER_ID",base-url="$DOCS_EU_GATEWAY",stream-token="$DOCS_EU_STREAM_TOKEN" \
     --scoreboard-out "$FETCH_OUT/scoreboard.json" \
     --provider-metrics-out "$FETCH_OUT/providers.ndjson" \
     --json-out "$FETCH_OUT/fetch.json" \
     --chunk-receipts-out "$FETCH_OUT/chunk_receipts.ndjson" \
     --telemetry-json artifacts/sorafs/provider_telemetry.json \
     --max-peers=3 \
     --retry-budget=4
   ```

   - Avval manifest tomonidan havola qilingan provayder reklamalarini oling (masalan
     `sorafs_cli manifest describe --provider-adverts-out artifacts/sorafs/provider_adverts/`)
     va ularni `--provider-advert name=path` orqali o'tkazing, shunda skorbord mumkin
     qobiliyat oynalarini aniq baholang. Foydalanish
     `--allow-implicit-provider-metadata` **faqat** oʻyinlarni qayta oʻynatishda
     CI; ishlab chiqarish matkaplari bilan qo'ndi imzolangan e'lonlar iqtibos keltirish kerak
     pin.
   - Manifest qo'shimcha hududlarga murojaat qilganda, buyruqni bilan takrorlang
     mos keladigan provayder kortejlar qiladi, shuning uchun har bir kesh/taxallus mos keladi
     artefakt olib kelish.

2. **Chiqishlarni arxivlash.** `scoreboard.json` doʻkonida,
   `providers.ndjson`, `fetch.json` va `chunk_receipts.ndjson` ostida
   dalillar papkasini chiqaring. Ushbu fayllar tengdoshlarning vaznini oladi, qayta urinib ko'ring
   byudjet, kechikish EWMA va boshqaruv paketi bo'lishi kerak bo'lgan har bir qism uchun tushumlar
   SF-7 uchun saqlang.

3. **Telemetriyani yangilang.** Qabul qilish natijalarini **SoraFS Fetch-ga import qiling
   Kuzatish mumkinligi** asboblar paneli (`dashboards/grafana/sorafs_fetch_observability.json`),
   `torii_sorafs_fetch_duration_ms`/`_failures_total` va
   anomaliyalar uchun provayder oralig'idagi panellar. Grafana panel suratlarini ulang
   Chiptani tablo yo'li bilan birga qoldiring.

4. **Ogohlantirish qoidalarini cheklang.** `scripts/telemetry/test_sorafs_fetch_alerts.sh` ni ishga tushiring
   chiqarishni yopishdan oldin Prometheus ogohlantirish paketini tekshirish uchun. Biriktiring
   chiptaga promtool chiqishi, shuning uchun DOCS-7 sharhlovchilari stendni tasdiqlashlari mumkin
   va sekin-provayder ogohlantirishlar qurolli qoladi.

5. **CI ga ulang.** Portal pinining ish jarayoni `sorafs_fetch` qadamini ortda qoldiradi
   `perform_fetch_probe` kirish; sahnalashtirish/ishlab chiqarish uchun uni yoqing, shuning uchun
   Manifest to'plami bilan bir qatorda qo'llanmasiz olib kelish dalillari ishlab chiqariladi
   aralashuv. Mahalliy matkaplar bir xil skriptni eksport qilish orqali qayta ishlatishi mumkin
   shlyuz tokenlari va vergul bilan ajratilgan `PIN_FETCH_PROVIDERS` ni o'rnatish
   provayderlar ro'yxati.

## Rag'batlantirish, kuzatuvchanlik va orqaga qaytarish

1. **Aksiya:** alohida sahnalashtirish va ishlab chiqarish taxalluslarini saqlang. tomonidan targ'ib qiling
   `manifest submit` ni bir xil manifest/to'plam bilan qayta ishga tushirish, almashtirish
   Ishlab chiqarish taxallusiga ishora qilish uchun `--alias-namespace/--alias-name`. Bu
   QA staging pinini tasdiqlaganidan keyin qayta qurish yoki iste'foga chiqishdan qochadi.
2. **Monitoring:** pin-registr boshqaruv panelini import qiling
   (`docs/source/grafana_sorafs_pin_registry.json`) va portalga xos
   zondlar (qarang: `docs/portal/docs/devportal/observability.md`). Tekshirish summasi haqida ogohlantirish
   drift, muvaffaqiyatsiz zondlar yoki qayta urinib ko'ring.
3. **Orqaga qaytarish:** orqaga qaytarish, oldingi manifestni qayta yuborish (yoki manifestni bekor qilish)
   joriy taxallus) `sorafs_cli manifest submit --alias ... --retire` yordamida.
   Har doim oxirgi yaxshi ma'lum bo'lgan to'plamni va CAR xulosasini saqlang, shunda orqaga qaytarish isbotlari mumkin
   CI jurnallari aylansa, qayta yaratiladi.

## CI ish jarayoni shabloni

Sizning quvuringiz kamida:

1. Build + lint (`npm ci`, `npm run build`, nazorat summasini yaratish).
2. Paket (`car pack`) va manifestlarni hisoblash.
3. Ishga oid OIDC tokeni (`manifest sign`) yordamida imzolang.
4. Audit uchun artefaktlarni (CAR, manifest, bundle, plan, summaries) yuklang.
5. PIN reyestriga yuboring:
   - So'rovlarni olish → `docs-preview.sora`.
   - Teglar / himoyalangan filiallar → ishlab chiqarish taxallusni reklama qilish.
6. Chiqishdan oldin problar + isbot tekshirish eshiklarini ishga tushiring.

`.github/workflows/docs-portal-sorafs-pin.yml` ushbu bosqichlarning barchasini birlashtiradi
qo'lda nashrlar uchun. Ish jarayoni:

- portalni quradi/sinov qiladi,
- qurilishni `scripts/sorafs-pin-release.sh` orqali paketlaydi,
- GitHub OIDC yordamida manifest to'plamini imzolaydi/tasdiqlaydi,
- CAR/manifest/to'plam/reja/isbot xulosalarini artefakt sifatida yuklaydi va
- (ixtiyoriy) sirlar mavjud bo'lganda manifest + taxallusni majburiy yuboradi.

Ishni boshlashdan oldin quyidagi ombor sirlarini/o'zgaruvchilarini sozlang:

| Ism | Maqsad |
|------|---------|
| `DOCS_SORAFS_TORII_URL` | `/v1/sorafs/pin/register`ni ochib beruvchi Torii xost. |
| `DOCS_SORAFS_SUBMITTED_EPOCH` | Epoch identifikatori taqdimotlar bilan yozib olingan. |
| `DOCS_SORAFS_AUTHORITY` / `DOCS_SORAFS_PRIVATE_KEY` | Manifestni topshirish uchun imzolash vakolati. |
| `DOCS_SORAFS_ALIAS_NAMESPACE` / `DOCS_SORAFS_ALIAS_NAME` | `perform_submit` `true` bo'lganda manifestga bog'langan taxallus korteji. |
| `DOCS_SORAFS_ALIAS_PROOF_B64` | Base64 bilan kodlangan taxallusni tasdiqlovchi toʻplam (ixtiyoriy; taxallusni bogʻlashni oʻtkazib yuborishni unutmang). |
| `DOCS_ANALYTICS_*` | Boshqa ish oqimlari tomonidan qayta ishlatiladigan mavjud tahlil/tekshirish so‘nggi nuqtalari. |

Action UI orqali ish jarayonini ishga tushiring:

1. `alias_label` (masalan, `docs.sora.link`), ixtiyoriy `proposal_alias`,
   va ixtiyoriy `release_tag` bekor qilish.
2. Torii ga tegmasdan artefakt yaratish uchun `perform_submit` ni belgisiz qoldiring.
   (quruq yugurish uchun foydali) yoki uni to'g'ridan-to'g'ri sozlanganlarga nashr qilishni yoqing
   taxallus.

`docs/source/sorafs_ci_templates.md` hali ham umumiy CI yordamchilarini hujjatlashtiradi
Ushbu ombordan tashqaridagi loyihalar, lekin portal ish oqimiga ustunlik berish kerak
kundalik nashrlar uchun.

## Tekshirish ro'yxati

- [ ] `npm run build`, `npm run test:*` va `npm run check:links` yashil rangda.
- [ ] Artefaktlarda olingan `build/checksums.sha256` va `build/release.json`.
- [ ] `artifacts/` ostida yaratilgan CAR, reja, manifest va xulosa.
- [ ] Sigstore to'plami + jurnallar bilan saqlangan alohida imzo.
- [ ] `portal.manifest.submit.summary.json` va `portal.manifest.submit.response.json`
      taqdimotlar sodir bo'lganda qo'lga kiritiladi.
- [ ] `portal.pin.report.json` (va ixtiyoriy `portal.pin.proposal.json`)
      CAR/manifest artefaktlari bilan birga arxivlangan.
- [ ] `proof verify` va `manifest verify-signature` jurnallari arxivlandi.
- [ ] Grafana asboblar paneli yangilandi + Sinab ko'rish sinovlari muvaffaqiyatli.
- [ ] Qayta tiklash qaydlari (oldingi manifest ID + taxallus dayjest) ilovasiga biriktirilgan
      chiqish chiptasi.