---
id: nexus-elastic-lane
lang: uz
direction: ltr
source: docs/portal/docs/nexus/nexus-elastic-lane.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: Elastic lane provisioning (NX-7)
sidebar_label: Elastic Lane Provisioning
description: Bootstrap workflow for creating Nexus lane manifests, catalog entries, and rollout evidence.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

::: Eslatma Kanonik manba
Bu sahifa `docs/source/nexus_elastic_lane.md`ni aks ettiradi. Tarjima portalga tushmaguncha ikkala nusxani ham bir xilda saqlang.
:::

# Elastik yo'lakni tayyorlash asboblar to'plami (NX-7)

> **Yo‘l xaritasi bandi:** NX-7 — Elastik yo‘lni ta’minlash uchun asboblar  
> **Holat:** Asboblar tugallandi — manifestlar, katalog parchalari, Norito foydali yuklari, tutun sinovlari,
> va yuklash-test paketi yordamchisi endi slotning kechikishini tiklaydi + dalil shunday tekshiradi
> yuklamalar maxsus skriptsiz chop etilishi mumkin.

Ushbu qo'llanma operatorlarni avtomatlashtiradigan yangi `scripts/nexus_lane_bootstrap.sh` yordamchisi orqali yo'naltiradi.
qatorli manifest generatsiyasi, qator/maʼlumotlar fazosi katalogi parchalari va tarqatish dalillari. Maqsad qilishdir
yangi Nexus qatorlarini (ommaviy yoki xususiy) bir nechta fayllarni qo'lda tahrirlashsiz yoki
katalog geometriyasini qo'lda qaytadan chiqarish.

## 1. Old shartlar

1. Yoʻlak taxalluslari, maʼlumotlar maydoni, validatorlar toʻplami, nosozliklarga chidamlilik (`f`) va hisob-kitob siyosati uchun boshqaruvni tasdiqlash.
2. Yakuniy tasdiqlovchi ro'yxati (hisob identifikatorlari) va himoyalangan nomlar maydoni ro'yxati.
3. Yaratilgan parchalarni qo'shishingiz uchun tugun konfiguratsiyasi omboriga kirish.
4. Lane manifest registrining yo'llari (qarang: `nexus.registry.manifest_directory` va
   `cache_directory`).
5. Yo‘lak uchun telemetriya kontaktlari/PagerDuty tutqichlari, shuning uchun ogohlantirishlar yo‘lga chiqishi bilanoq ulanadi.
   onlayn keladi.

## 2. Chiziqli artefaktlarni yarating

Yordamchini ombor ildizidan ishga tushiring:

```bash
scripts/nexus_lane_bootstrap.sh \
  --lane-alias "Payments Lane" \
  --lane-id 3 \
  --dataspace-alias payments \
  --governance-module parliament \
  --settlement-handle xor_global \
  --validator <katakana-i105-account-id> \
  --validator <katakana-i105-account-id> \
  --validator <katakana-i105-account-id> \
  --protected-namespace payments \
  --description "High-throughput interbank payments lane" \
  --dataspace-description "Payments dataspace" \
  --route-instruction finance::pacs008 \
  --encode-space-directory \
  --space-directory-out artifacts/nexus/payments_lane/payments.manifest.to \
  --telemetry-contact payments-ops@sora.org \
  --output-dir artifacts/nexus/payments_lane
```

Asosiy bayroqlar:

- `--lane-id` yangi yozuvning `nexus.lane_catalog` indeksiga mos kelishi kerak.
- `--dataspace-alias` va `--dataspace-id/hash` ma'lumotlar maydoni katalogiga kirishni nazorat qiladi (standart
  o'tkazib yuborilganda chiziq identifikatori).
- `--validator` takrorlanishi yoki `--validators-file` dan olinishi mumkin.
- `--route-instruction` / `--route-account` joylashtirishga tayyor marshrutlash qoidalarini chiqaradi.
- `--metadata key=value` (yoki `--telemetry-contact/channel/runbook`) runbook kontaktlarini shunday suratga oladi
  asboblar paneli darhol to'g'ri egalarni ro'yxatga oladi.
- `--allow-runtime-upgrades` + `--runtime-upgrade-*` manifestga ish vaqtini yangilash kancasini qo'shing
  bo'lak kengaytirilgan operator boshqaruvlarini talab qilganda.
- `--encode-space-directory` avtomatik ravishda `cargo xtask space-directory encode` ni chaqiradi. U bilan bog'lang
  `--space-directory-out` kodlangan `.to` faylini sukut bo'yicha boshqa joyda xohlaganingizda.

Skript `--output-dir` ichida uchta artefakt ishlab chiqaradi (joriy katalog uchun birlamchi),
va kodlash yoqilganda ixtiyoriy toʻrtinchisi:

1. `<slug>.manifest.json` — validator kvorumini, himoyalangan nom maydonlarini va oʻz ichiga olgan chiziqli manifest
   ixtiyoriy ish vaqtini yangilash kancasi metadata.
2. `<slug>.catalog.toml` — TOML parchasi, `[[nexus.lane_catalog]]`, `[[nexus.dataspace_catalog]]`,
   va har qanday so'ralgan marshrutlash qoidalari. `fault_tolerance` maʼlumotlar maydoniga oʻlchamga oʻrnatilganligiga ishonch hosil qiling
   yo'l o'rni qo'mitasi (`3f+1`).
3. `<slug>.summary.json` - geometriyani tavsiflovchi audit xulosasi (slug, segmentlar, metama'lumotlar) va
   kerakli ishlab chiqarish bosqichlari va aniq `cargo xtask space-directory encode` buyrug'i (
   `space_directory_encode.command`). Dalil uchun ushbu JSONni bort chiptasiga biriktiring.
4. `<slug>.manifest.to` - `--encode-space-directory` o'rnatilganda chiqariladi; Torii uchun tayyor
   `iroha app space-directory manifest publish` oqimi.

JSON/snippetlarni fayllarni yozmasdan oldindan ko'rish uchun `--dry-run` va ustiga yozish uchun `--force` dan foydalaning.
mavjud artefaktlar.

## 3. O'zgarishlarni qo'llang

1. Manifest JSON faylini sozlangan `nexus.registry.manifest_directory` (va keshga) nusxalang
   katalog, agar ro'yxatga olish kitobi masofaviy to'plamlarni aks ettirsa). Agar manifestlar versiyada bo'lsa, faylni topshiring
   konfiguratsiya repo.
2. Katalog parchasini `config/config.toml` (yoki tegishli `config.d/*.toml`) ga qo'shing. Ta'minlash
   `nexus.lane_count` kamida `lane_id + 1` va `nexus.routing_policy.rules`ni yangilang.
   yangi chiziqqa ishora qilishi kerak.
3. Kodlash (agar siz `--encode-space-directory` ni o'tkazib yuborsangiz) va manifestni Space Directoryga nashr qiling
   xulosada olingan buyruq yordamida (`space_directory_encode.command`). Bu hosil qiladi
   `.manifest.to` foydali yuk Torii auditorlar uchun dalillarni kutadi va qayd etadi; orqali yuboring
   `iroha app space-directory manifest publish`.
4. `irohad --sora --config path/to/config.toml --trace-config` ni ishga tushiring va kuzatuv chiqishini arxivlang
   chiqish chiptasi. Bu yangi geometriyaning yaratilgan slug/kura segmentlariga mos kelishini isbotlaydi.
5. Manifest/katalog o'zgarishlari kiritilgandan so'ng, chiziqqa tayinlangan validatorlarni qayta ishga tushiring. Saqlash
   kelajakdagi auditlar uchun chiptadagi JSON xulosasi.

## 4. Ro'yxatga olish kitobini tarqatish to'plamini yarating

Yaratilgan manifest va ustki qatlamni toʻplang, shunda operatorlar tarmoqni boshqarish maʼlumotlarini ularsiz tarqatishi mumkin
har bir xostda konfiguratsiyalarni tahrirlash. Bundler yordamchisi manifestlarni kanonik tartibga nusxalaydi,
`nexus.registry.cache_directory` uchun ixtiyoriy boshqaruv katalogi qoplamasini ishlab chiqaradi va
oflayn transferlar uchun tarball:

```bash
scripts/nexus_lane_registry_bundle.sh \
  --manifest artifacts/nexus/payments_lane/payments.manifest.json \
  --output-dir artifacts/nexus/payments_lane/registry_bundle \
  --default-module parliament \
  --module name=parliament,module_type=parliament,param.quorum=2 \
  --bundle-out artifacts/nexus/payments_lane/registry_bundle.tar.gz
```

Chiqishlar:

1. `manifests/<slug>.manifest.json` — ularni sozlanganlarga nusxalash
   `nexus.registry.manifest_directory`.
2. `cache/governance_catalog.json` - `nexus.registry.cache_directory` ga tushish. Har `--module`
   kirish ulanish moduli ta'rifiga aylanadi, bu boshqaruv modulini almashtirishga (NX-2) imkon beradi.
   `config.toml` tahrirlash o'rniga kesh qoplamasini yangilash.
3. `summary.json` — xeshlar, metamaʼlumotlarning ustki qismi va operator koʻrsatmalarini oʻz ichiga oladi.
4. Majburiy emas `registry_bundle.tar.*` — SCP, S3 yoki artefakt kuzatuvchilari uchun tayyor.

Butun katalogni (yoki arxivni) har bir validator bilan sinxronlashtiring, havo bo'shlig'i bo'lgan xostlardan chiqarib oling va nusxa oling
Torii ni qayta ishga tushirishdan oldin manifestlar + kesh qoplamasi ularning ro'yxatga olish kitobi yo'llarida.

## 5. Validator tutun sinovlari

Torii qayta ishga tushirilgandan so'ng, `manifest_ready=true` qator hisobotlarini tekshirish uchun yangi tutun yordamchisini ishga tushiring,
ko'rsatkichlar kutilgan qatorlar sonini ko'rsatadi va muhrlangan o'lchov aniq. Manifestlarni talab qiladigan chiziqlar
bo'sh bo'lmagan `manifest_path` ni ochishi kerak; Yo'l yo'qolganda yordamchi endi darhol muvaffaqiyatsiz bo'ladi
Har bir NX-7 o'rnatish yozuvi imzolangan manifest dalillarni o'z ichiga oladi:

```bash
scripts/nexus_lane_smoke.py \
  --status-url https://torii.example.com/v1/sumeragi/status \
  --metrics-url https://torii.example.com/metrics \
  --lane-alias payments \
  --expected-lane-count 3 \
  --min-da-quorum 0.95 \
  --max-oracle-staleness 75 \
  --expected-oracle-twap 60 \
  --oracle-twap-tolerance 5 \
  --max-oracle-haircut-bps 75 \
  --min-settlement-buffer 0.25 \
  --min-block-height 1000 \
  --max-finality-lag 4 \
  --max-settlement-backlog 0.5 \
  --max-headroom-events 0 \
  --max-slot-p95 1000 \
  --max-slot-p99 1100 \
  --min-slot-samples 10
```

O'z-o'zidan imzolangan muhitlarni sinab ko'rishda `--insecure` qo'shing. Agar chiziq bo'lsa, skript noldan farq qiladi
etishmayotgan, muhrlangan yoki ko'rsatkichlar/temetriya kutilgan qiymatlardan siljish. dan foydalaning
`--min-block-height`, `--max-finality-lag`, `--max-settlement-backlog` va
`--max-headroom-events` tugmalari har bir chiziqli blok balandligi/yakuniylik/ortiqchalik/bo'sh joy telemetriyasini saqlash uchun
operatsion konvertlar ichida va ularni `--max-slot-p95` / `--max-slot-p99` bilan bog'lang
(plyus `--min-slot-samples`) NX‑18 slot-davomiylik maqsadlarini yordamchidan chiqmasdan amalga oshirish uchun.

Havo boʻshligʻi tekshiruvlari (yoki CI) uchun siz jonli efirni bosish oʻrniga olingan Torii javobini takrorlashingiz mumkin.
yakuniy nuqta:

```bash
scripts/nexus_lane_smoke.py \
  --status-file fixtures/nexus/lanes/status_ready.json \
  --metrics-file fixtures/nexus/lanes/metrics_ready.prom \
  --lane-alias core \
  --lane-alias payments \
  --expected-lane-count 3 \
  --min-da-quorum 0.95 \
  --max-oracle-staleness 75 \
  --expected-oracle-twap 60 \
  --oracle-twap-tolerance 5 \
  --max-oracle-haircut-bps 75 \
  --min-settlement-buffer 0.25 \
  --min-block-height 1000 \
  --max-finality-lag 4 \
  --max-settlement-backlog 0.5 \
  --max-headroom-events 0 \
  --max-slot-p95 1000 \
  --max-slot-p99 1100 \
  --min-slot-samples 10
```

`fixtures/nexus/lanes/` ostida yozilgan moslamalar yuklash apparati tomonidan ishlab chiqarilgan artefaktlarni aks ettiradi.
yordamchi, shuning uchun yangi manifestlarni maxsus skriptsiz liniyalash mumkin. CI bir xil oqim orqali mashq qiladi
`ci/check_nexus_lane_smoke.sh` va `ci/check_nexus_lane_registry_bundle.sh`
(taxallus: `make check-nexus-lanes`) NX-7 tutun yordamchisi nashr etilganlarga mos kelishini isbotlash uchun
foydali yuk formati va to'plamdagi dayjestlar/qoplamalar takrorlanishini ta'minlash uchun.

Yoʻlak nomi oʻzgartirilganda, `nexus.lane.topology` telemetriya hodisalarini yozib oling (masalan,
`journalctl -u irohad -o json | jq 'select(.msg=="nexus.lane.topology")'`) va ularni qayta kiriting
tutun yordamchisi. `--telemetry-file/--from-telemetry` bayrog'i yangi qator bilan ajratilgan jurnalni qabul qiladi va
`--require-alias-migration old:new`, `alias_migrated` hodisasi nomini o'zgartirishni qayd etganini ta'kidlaydi:

```bash
scripts/nexus_lane_smoke.py \
  --status-file fixtures/nexus/lanes/status_ready.json \
  --metrics-file fixtures/nexus/lanes/metrics_ready.prom \
  --telemetry-file fixtures/nexus/lanes/telemetry_alias_migrated.ndjson \
  --lane-alias payments \
  --lane-alias core \
  --expected-lane-count 3 \
  --min-da-quorum 0.95 \
  --max-oracle-staleness 75 \
  --expected-oracle-twap 60 \
  --oracle-twap-tolerance 5 \
  --max-oracle-haircut-bps 75 \
  --min-settlement-buffer 0.25 \
  --min-block-height 1000 \
  --max-finality-lag 4 \
  --max-settlement-backlog 0.5 \
  --max-headroom-events 0 \
  --max-slot-p95 1000 \
  --max-slot-p99 1100 \
  --min-slot-samples 10 \
  --require-alias-migration core:payments
```

`telemetry_alias_migrated.ndjson` moslamasi nomini o'zgartirishning kanonik namunasini birlashtiradi, shuning uchun CI tekshirishi mumkin
jonli tugun bilan aloqa qilmasdan telemetriyani tahlil qilish yo'li.

## Validator yuk testlari (NX-7 dalillari)

Yo‘l xaritasi **NX-7** har bir yangi qatorda takrorlanuvchi validator yukini yuklashni talab qiladi. Foydalanish
`scripts/nexus_lane_load_test.py` tutun tekshiruvlari, uyalar davomiyligi eshiklari va uyalar to'plamini tikish uchun
boshqaruv takrorlanishi mumkin bo'lgan yagona artefakt to'plamida namoyon bo'ladi:

```bash
scripts/nexus_lane_load_test.py \
  --status-file artifacts/nexus/load/payments-2026q2/torii_status.json \
  --metrics-file artifacts/nexus/load/payments-2026q2/metrics.prom \
  --telemetry-file artifacts/nexus/load/payments-2026q2/nexus.lane.topology.ndjson \
  --lane-alias payments \
  --lane-alias core \
  --expected-lane-count 3 \
  --slot-range 81200-81600 \
  --workload-seed NX7-PAYMENTS-2026Q2 \
  --require-alias-migration core:payments \
  --out-dir artifacts/nexus/load/payments-2026q2
```

Yordamchi bir xil DA kvorum, oracle, hisob-kitob buferi, TEU va slot-davomiylik eshiklarini qo'llaydi.
tutun yordamchisi tomonidan va `smoke.log`, `slot_summary.json`, slot manifestini yozadi va
`load_test_manifest.json` tanlangan `--out-dir` ga yuklanadi, shuning uchun yuklarni to'g'ridan-to'g'ri ulash mumkin
buyurtma skriptsiz tarqatish chiptalari.

## 6. Telemetriya va boshqaruvni kuzatish

- Yo'lak asboblar panelini (`dashboards/grafana/nexus_lanes.json` va tegishli qoplamalar) yangilang
  yangi qator identifikatori va metadata. Yaratilgan metama'lumotlar kalitlari (`contact`, `channel`, `runbook` va boshqalar)
  yorliqlarni oldindan to'ldirish oddiy.
- Qabul qilishni yoqishdan oldin Wire PagerDuty/Alertmanager yangi qator uchun qoidalari. `summary.json`
  keyingi qadamlar qatori [Nexus operatsiyalari](./nexus-operations) da nazorat roʻyxatini aks ettiradi.
- Validator to'plami ishga tushgandan so'ng, manifest to'plamini kosmik katalogda ro'yxatdan o'tkazing. Xuddi shunday foydalaning
  manifest JSON yordamchi tomonidan yaratilgan, boshqaruv ish kitobiga muvofiq imzolangan.
- Tutun sinovlari (FindNetworkStatus, Torii) uchun [Sora Nexus operatorni ishga tushirish](./nexus-operator-onboarding) ga rioya qiling
  erishish imkoniyati) va yuqorida ishlab chiqarilgan artefaktlar to'plami bilan dalillarni qo'lga kiriting.

## 7. Quruq yugurish misoli

Fayllarni yozmasdan artefaktlarni oldindan ko'rish uchun:

```bash
scripts/nexus_lane_bootstrap.sh \
  --lane-alias "Payments Lane" \
  --lane-id 3 \
  --dataspace-alias payments \
  --governance-module parliament \
  --settlement-handle xor_global \
  --validator <katakana-i105-account-id> \
  --validator <katakana-i105-account-id> \
  --dry-run
```

Buyruq JSON xulosasi va TOML snippetini stdout-ga chop etadi va bu jarayon davomida tez takrorlashga imkon beradi.
rejalashtirish.

---

Qo'shimcha kontekst uchun qarang:- [Nexus operatsiyalari](./nexus-operations) — operatsion nazorat roʻyxati va telemetriya talablari.
- [Sora Nexus operatorni ishga tushirish](./nexus-operator-onboarding) — bortga kirishning batafsil oqimi
  yangi yordamchi.
- [Nexus chiziqli modeli](./nexus-lane-model) — asbob tomonidan ishlatiladigan chiziqli geometriya, slugs va saqlash tartibi.