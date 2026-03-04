---
id: orchestrator-tuning
lang: uz
direction: ltr
source: docs/portal/docs/sorafs/orchestrator-tuning.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: Orchestrator Rollout & Tuning
sidebar_label: Orchestrator Tuning
description: Practical defaults, tuning guidance, and audit checkpoints for taking the multi-source orchestrator to GA.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

::: Eslatma Kanonik manba
:::

# Orkestrni ishga tushirish va sozlash bo'yicha qo'llanma

Ushbu qoʻllanma [konfiguratsiya maʼlumotnomasi](orchestrator-config.md) va
[ko'p manbali rollout runbook](multi-source-rollout.md). Bu tushuntiradi
Har bir ishlab chiqarish bosqichi uchun orkestrni qanday sozlash kerak, qanday talqin qilish kerak
skorbord artefaktlari va qaysi telemetriya signallari oldin bo'lishi kerak
trafikni kengaytirish. Tavsiyalarni doimiy ravishda CLI, SDK va boshqalarda qo'llang
avtomatlashtirish, shuning uchun har bir tugun bir xil deterministik olib kelish siyosatiga amal qiladi.

## 1. Asosiy parametrlar to'plami

Umumiy konfiguratsiya shablonidan boshlang va kichik tugmalar to'plamini shunday sozlang
tarqatish davom etmoqda. Quyidagi jadvalda tavsiya etilgan qiymatlar keltirilgan
eng keng tarqalgan bosqichlar; ro'yxatda bo'lmagan qiymatlar standart sozlamalarga qaytadi
`OrchestratorConfig::default()` va `FetchOptions::default()`.

| Bosqich | `max_providers` | `fetch.per_chunk_retry_limit` | `fetch.provider_failure_threshold` | `scoreboard.latency_cap_ms` | `scoreboard.telemetry_grace_secs` | Eslatmalar |
|-------|-----------------|----------------------------------------|-----------------------------------|---------------------------------------|------------------------------------|-------|
| **Laboratoriya / CI** | `3` | `2` | `2` | `2500` | `300` | Qattiq kechikish qopqog'i va qulay oyna yuzasi shovqinli telemetriyani tezda. Yaroqsiz manifestlarni tezroq ochish uchun takroriy urinishlarni kamaytiring. |
| **Sahnalashtirish** | `4` | `3` | `3` | `4000` | `600` | Ishlab chiqarish standartlarini aks ettiradi, shu bilan birga tadqiqotchi tengdoshlar uchun bo'sh joy qoldiradi. |
| **Kanarya** | `6` | `3` | `3` | `5000` | `900` | Standartlarga mos keladi; `telemetry_region` sozlang, shuning uchun asboblar paneli kanareykalar trafigida aylana oladi. |
| **Umumiy mavjudlik** | `None` (barcha muvofiqlaridan foydalaning) | `4` | `4` | `5000` | `900` | Tekshirishlar determinizmni qo'llashda davom etar ekan, vaqtinchalik nosozliklarni o'zlashtirish uchun qayta urinish va muvaffaqiyatsizlik chegaralarini oshiring. |

- Agar quyi oqim bo'lmasa, `scoreboard.weight_scale` standart `10_000` da qoladi.
  tizim boshqa butun son ruxsatini talab qiladi. O'lchovni oshirib bo'lmaydi
  provayder buyurtmasini o'zgartirish; u faqat zichroq kredit taqsimotini chiqaradi.
- Fazalar o'rtasida o'tishda JSON to'plamini davom ettiring va foydalaning
  `--scoreboard-out`, shuning uchun audit izi aniq parametrlar to'plamini qayd qiladi.

## 2. Skorbord gigienasi

Tablo manifest talablari, provayder reklamalari va telemetriyani birlashtiradi.
Oldinga siljishdan oldin:

1. **Telemetriyaning yangiligini tasdiqlang.** Murojaat qilgan oniy tasvirlarga ishonch hosil qiling
   `--telemetry-json` sozlangan imtiyozli oynada suratga olindi. Yozuvlar
   konfiguratsiya qilingan `telemetry_grace_secs` dan eski bilan muvaffaqiyatsiz
   `TelemetryStale { last_updated }`. Buni qiyin to'xtash sifatida qabul qiling va uni yangilang
   davom ettirishdan oldin telemetriya eksporti.
2. **Muvofiqlik sabablarini tekshiring.** Artefaktlarni davom ettirish orqali
   `--scoreboard-out=/var/lib/sorafs/scoreboards/preflight.json`. Har bir kirish
   aniq nosozlik sababi bilan `eligibility` blokini olib yuradi. Bekor qilmang
   qobiliyatlarning mos kelmasligi yoki muddati o'tgan reklamalar; yuqori oqim yukini tuzatish.
3. **Og'irlik deltalarini ko'rib chiqing.** `normalised_weight` maydonini bilan solishtiring
   oldingi nashr. Og'irlikning >10% o'zgarishi atayin reklama bilan bog'liq bo'lishi kerak
   yoki telemetriya o'zgaradi va ular tarqatish jurnalida tan olinishi kerak.
4. **Artifaktlarni arxivlash.** `scoreboard.persist_path` ni sozlang, shunda har bir ishga tushirish nurlari chiqaradi
   yakuniy jadval lavhasi. Artefaktni chiqarish yozuviga biriktiring
   manifest va telemetriya to'plami bilan birga.
5. **Provayder aralash dalillarini yozib oling.** `scoreboard.json` metamaʼlumotlari _va_
   mos keladigan `summary.json` `provider_count` ni ochishi kerak,
   `gateway_provider_count` va olingan `provider_mix` yorlig'i, shuning uchun sharhlovchilar
   yugurish `direct-only`, `gateway-only` yoki `mixed` ekanligini isbotlay oladi.
   Gateway ushlaydi, shuning uchun `provider_count=0` plus hisoboti
   `provider_mix="gateway-only"`, aralash yugurishlar esa noldan farqli hisoblarni talab qiladi.
   ikkala manba. `cargo xtask sorafs-adoption-check` bu maydonlarni (va
   Hisoblar/yorliqlar mos kelmasa, bajarilmaydi), shuning uchun uni har doim yonma-yon ishlating
   `ci/check_sorafs_orchestrator_adoption.sh` yoki sizning buyurtmangiz bo'yicha suratga olish skripti
   `adoption_report.json` dalillar to'plamini ishlab chiqaring. Torii shlyuzlari bo'lganda
   ishtirok etsa, `gateway_manifest_id`/`gateway_manifest_cid` ni tabloda saqlang
   metadata, shuning uchun qabul qilish eshigi manifest konvertni bilan bog'lashi mumkin
   qo'lga olingan provayder aralashmasi.

Batafsil maydon ta'riflari uchun qarang
`crates/sorafs_car/src/scoreboard.rs` va CLI xulosa tuzilishi
`sorafs_cli fetch --json-out`.

## CLI va SDK bayroq ma'lumotnomasi

`sorafs_cli fetch` (qarang: `crates/sorafs_car/src/bin/sorafs_cli.rs`) va
`iroha_cli app sorafs fetch` o'rami (`crates/iroha_cli/src/commands/sorafs.rs`)
bir xil orkestr konfiguratsiya yuzasini baham ko'ring. Quyidagi bayroqlardan foydalaning
tarqatish dalillarini olish yoki kanonik moslamalarni takrorlash:

Umumiy koʻp manbali bayroq maʼlumotnomasi (faqat ushbu faylni tahrirlash orqali CLI yordami va hujjatlarni sinxronlashtiring):

- `--max-peers=<count>` qancha mos provayderlar tablo filtridan omon qolishini cheklaydi. Har bir mos provayderdan translatsiya qilish uchun sozlanmagan holda qoldiring, `1` ni faqat bitta manbali qayta tiklashni ataylab ishlatganda o'rnating. `maxPeers` tugmachasini SDK larda aks ettiradi (`SorafsGatewayFetchOptions.maxPeers`, `SorafsGatewayFetchOptions.max_peers`).
- `--retry-budget=<count>` `FetchOptions` tomonidan qo'llaniladigan har bir parcha qayta urinish chegarasiga yo'naltiriladi. Tavsiya etilgan qiymatlar uchun sozlash qo'llanmasidagi tarqatish jadvalidan foydalaning; Dalillarni to'playdigan CLI ishlanmalari paritetni saqlash uchun SDK standartiga mos kelishi kerak.
- `--telemetry-region=<label>` teglari `sorafs_orchestrator_*` Prometheus seriyali (va OTLP relelari) mintaqa/env yorlig'i bilan, shuning uchun asboblar paneli laboratoriya, sahnalashtirish, kanareyka va GA trafigini ajratishi mumkin.
- `--telemetry-json=<path>` skorbordda havola qilingan oniy tasvirni kiritadi. Auditorlar ishga tushirishni takrorlashi uchun (shuning uchun `cargo xtask sorafs-adoption-check --require-telemetry` qaysi OTLP oqimi tasvirga olinganligini isbotlashi mumkin) hisoblar paneli yonidagi JSON-ni saqlang.
- `--local-proxy-*` (`--local-proxy-mode`, `--local-proxy-norito-spool`, `--local-proxy-kaigi-spool`, `--local-proxy-kaigi-policy`) ko'prik kuzatuvchi ilgaklarini yoqadi. O'rnatilganda, orkestrator qismlarni mahalliy Norito/Kaigi proksi-server orqali uzatadi, shuning uchun brauzer mijozlari, himoya keshlari va Kaigi xonalari Rust tomonidan chiqarilgan bir xil kvitansiyalarni oladi.
- `--scoreboard-out=<path>` (ixtiyoriy ravishda `--scoreboard-now=<unix_secs>` bilan bog'langan) auditorlar uchun moslik suratini saqlab qoladi. Doim doimiy JSONni reliz chiptasida ko'rsatilgan telemetriya va manifest artefaktlari bilan bog'lang.
- `--deny-provider name=ALIAS` / `--boost-provider name=ALIAS:delta` reklama metama'lumotlari ustiga deterministik tuzatishlarni qo'llaydi. Ushbu bayroqlardan faqat mashqlar uchun foydalaning; ishlab chiqarishni pasaytirish boshqaruv artefaktlari orqali o'tishi kerak, shuning uchun har bir tugun bir xil siyosat to'plamini qo'llaydi.
- `--provider-metrics-out` / `--chunk-receipts-out` tarqatish nazorat ro'yxatida ko'rsatilgan har bir provayderning sog'lig'i ko'rsatkichlarini va parcha kvitansiyalarini saqlab qoladi; farzandlikka olish to‘g‘risidagi guvohnomani taqdim etishda ikkala artefaktni ham ilova qiling.

Misol (nashr qilingan armatura yordamida):

```bash
sorafs_cli fetch \
  --plan fixtures/sorafs_orchestrator/multi_peer_parity_v1/plan.json \
  --gateway-provider gw-alpha=... \
  --telemetry-source-label otlp::staging \
  --scoreboard-out artifacts/sorafs_orchestrator/latest/scoreboard.json \
  --json-out artifacts/sorafs_orchestrator/latest/summary.json \
  --provider-metrics-out artifacts/sorafs_orchestrator/latest/provider_metrics.json \
  --chunk-receipts-out artifacts/sorafs_orchestrator/latest/chunk_receipts.json

cargo xtask sorafs-adoption-check \
  --scoreboard artifacts/sorafs_orchestrator/latest/scoreboard.json \
  --summary artifacts/sorafs_orchestrator/latest/summary.json
```

SDK'lar Rust'da `SorafsGatewayFetchOptions` orqali bir xil konfiguratsiyani iste'mol qiladi.
mijoz (`crates/iroha/src/client.rs`), JS ulanishlari
(`javascript/iroha_js/src/sorafs.js`) va Swift SDK
(`IrohaSwift/Sources/IrohaSwift/SorafsOptions.swift`). O'sha yordamchilarni ichkarida saqlang
Operatorlar siyosatlarni avtomatlashtirish bo'ylab nusxalashlari uchun CLI standart sozlamalari bilan qulflash bosqichi
buyurtma qilingan tarjima qatlamlarisiz.

## 3. Siyosatni sozlash

`FetchOptions` qayta urinish xatti-harakatlari, parallellik va tekshirishni nazorat qiladi. Qachon
sozlash:

- **Qayta urinishlar:** `per_chunk_retry_limit` ni `4` dan ortiq oshirish tiklanishni oshiradi
  vaqt, lekin provayderning xatolarini maskalash xavfi. Shift sifatida `4` ni saqlashni afzal ko'ring va
  kambag'al ijrochilarni yuzaga chiqarish uchun provayderning aylanishiga tayanib.
- **Muvaffaqiyatsizlik chegarasi:** `provider_failure_threshold`
  provayder seansning qolgan qismida o'chirib qo'yilgan. Ushbu qiymat bilan moslang
  Qayta urinish siyosati: qayta urinish byudjetidan pastroq chegara orkestrni majburlaydi
  barcha qayta urinishlar tugashidan oldin tengdoshni chiqarib tashlash.
- **Mos vaqt:** `global_parallel_limit` (`None`) oʻrnatilmagan holda qoldiring.
  Muayyan muhit e'lon qilingan diapazonlarni to'ydira olmaydi. O'rnatilganda, ishonch hosil qiling
  qiymati ≤ ochlikdan qochish uchun provayder oqimi byudjetlarining yig'indisi.
- **Tasdiqlash tugmalari:** `verify_lengths` va `verify_digests` qolishi kerak
  ishlab chiqarishda faollashtirilgan. Ular provayderlar parkini aralashtirganda determinizmni kafolatlaydi
  o'yinda; ularni faqat izolyatsiya qilingan fuzzing muhitlarida o'chirib qo'ying.

## 4. Transport va anonimlik bosqichi

Buning uchun `rollout_phase`, `anonymity_policy` va `transport_policy` maydonlaridan foydalaning.
maxfiylik pozitsiyasini ifodalaydi:- `rollout_phase="snnet-5"` ni afzal ko'ring va standart anonimlik siyosatiga ruxsat bering
  SNNet-5 bosqichlarini kuzatib boring. Faqat `anonymity_policy_override` orqali bekor qilish
  boshqaruv imzolangan direktivani chiqarganda.
- SNNet-4/5/5a/5b/6a/7/8/12/13 🈺 bo'lganda `transport_policy="soranet-first"` ni asosiy chiziq sifatida saqlang
  (qarang: `roadmap.md`). `transport_policy="direct-only"` faqat hujjatlashtirilgan uchun foydalaning
  pasaytirish/muvofiqlik mashqlari va oldin PQ qamrovini tekshirishni kuting
  `transport_policy="soranet-strict"` ga ko'tarilish - agar bu daraja tezda muvaffaqiyatsiz bo'ladi
  faqat klassik relelar qoladi.
- `write_mode="pq-only"` faqat har bir yozish yo'lida (SDK,
  orkestr, boshqaruv vositalari) PQ talablarini qondira oladi. davomida
  rollouts `write_mode="allow-downgrade"` ni saqlaydi, shuning uchun favqulodda vaziyatlarga javob berish mumkin
  to'g'ridan-to'g'ri yo'nalishlarda, telemetriya esa pasayishni belgilaydi.
- Qo'riqchilarni tanlash va sxemalar SoraNet katalogiga tayanadi. ta'minlang
  imzolangan `relay_directory` surati va `guard_set` keshini saqlang, shuning uchun saqlang
  charchoq kelishilgan saqlash oynasi ichida qoladi. Kesh barmoq izi qayd etildi
  tomonidan `sorafs_cli fetch` tarqatish dalillarining bir qismini tashkil qiladi.

## 5. Downgrade & Compliance Hooks

Ikkita orkestr quyi tizimi siyosatni qo'lda aralashuvisiz amalga oshirishga yordam beradi:

- **Yangi versiyani tuzatish** (`downgrade_remediation`): monitorlar
  `handshake_downgrade_total` hodisalari va sozlangandan keyin `threshold`
  `window_secs` doirasidan oshib ketdi, mahalliy proksi-serverni `target_mode` ga majburlaydi
  (faqat metadata sukut bo'yicha). Standartlarni saqlang (`threshold=3`, `window=300`,
  `cooldown=900`). Har qanday hujjat
  chiqarish jurnalida bekor qiling va asboblar paneli kuzatilishini ta'minlang
  `sorafs_proxy_downgrade_state`.
- **Muvofiqlik siyosati** (`compliance`): yurisdiktsiya va manifest ajralishlar
  boshqaruv tomonidan boshqariladigan rad etish ro'yxatlari orqali o'tadi. Hech qachon inline ad-hoc bekor qilmang
  konfiguratsiya to'plamida; o'rniga imzolangan yangilanishni so'rang
  `governance/compliance/soranet_opt_outs.json` va yaratilgan JSONni qayta joylashtiring.

Ikkala tizim uchun ham hosil bo'lgan konfiguratsiya to'plamini saqlang va uni qo'shing
dalillarni e'lon qiling, shunda auditorlar pasaytirishlar qanday boshlanganligini kuzatishlari mumkin.

## 6. Telemetriya va asboblar paneli

Chiqarishni kengaytirishdan oldin, quyidagi signallar jonli ekanligini tasdiqlang
maqsadli muhit:

- `sorafs_orchestrator_fetch_failures_total{reason="no_healthy_providers"}` -
  kanareyka tugagandan so'ng nolga teng bo'lishi kerak.
- `sorafs_orchestrator_retries_total` va
  `sorafs_orchestrator_retry_ratio` - davomida 10% dan pastda barqarorlashishi kerak
  kanareyka va GA dan keyin 5% ostida qoling.
- `sorafs_orchestrator_policy_events_total` - kutilganini tasdiqlaydi
  ishlab chiqarish bosqichi faol (`stage` yorlig'i) va `outcome` orqali o'chirishlarni qayd qiladi.
- `sorafs_orchestrator_pq_candidate_ratio` /
  `sorafs_orchestrator_pq_deficit_ratio` - PQ o'rni ta'minotiga qarshi kuzatuv
  siyosat umidlari.
- `telemetry::sorafs.fetch.*` jurnal maqsadlari - umumiy jurnalga uzatilishi kerak
  `status=failed` uchun saqlangan qidiruvlar bilan agregator.

dan kanonik Grafana asboblar panelini yuklang
`dashboards/grafana/sorafs_fetch_observability.json` (portalda eksport qilingan
**SoraFS → Fetch Observability**) ostida mintaqa/manifest selektorlari,
provayder issiqlik xaritasini qayta sinab ko'rish, kechikish gistogrammalari va to'xtash hisoblagichlari mos keladi
kuyish paytida qanday SRE ko'rib chiqadi. Alertmanager qoidalarini ulang
`dashboards/alerts/sorafs_fetch_rules.yml` va Prometheus sintaksisini tasdiqlang
`scripts/telemetry/test_sorafs_fetch_alerts.sh` bilan (yordamchi avtomatik ravishda
`promtool test rules` mahalliy yoki Docker da ishlaydi). Ogohlantirishni topshirish talab qiladi
skript bosib chiqaradigan bir xil marshrutlash bloki, shuning uchun operatorlar dalillarni bog'lashlari mumkin
chiqish chiptasi.

### Telemetriyani yoqish ish jarayoni

“Yo‘l xaritasi” bandi **SF-6e** o‘girishdan oldin 30 kunlik telemetriyani yoqishni talab qiladi.
GA standartlariga ko'p manbali orkestr. Buning uchun ombor skriptlaridan foydalaning
oynada har bir kun uchun takrorlanadigan artefakt to'plamini oling:

1. `ci/check_sorafs_orchestrator_adoption.sh` ni kuyish muhiti bilan ishga tushiring
   tugmalar o'rnatilgan. Misol:

   ```bash
   SORAFS_BURN_IN_LABEL=canary-week-1 \
   SORAFS_BURN_IN_REGION=us-east-1 \
   SORAFS_BURN_IN_MANIFEST=manifest-v4 \
   SORAFS_BURN_IN_DAY=7 \
   SORAFS_BURN_IN_WINDOW_DAYS=30 \
   ci/check_sorafs_orchestrator_adoption.sh
   ```

   Yordamchi `fixtures/sorafs_orchestrator/multi_peer_parity_v1` ni takrorlaydi,
   yozadi `scoreboard.json`, `summary.json`, `provider_metrics.json`,
   `chunk_receipts.json` va `adoption_report.json` ostida
   `artifacts/sorafs_orchestrator/<timestamp>/` va minimal sonni talab qiladi
   `cargo xtask sorafs-adoption-check` orqali tegishli provayderlar.
2. Yonish o'zgaruvchilari mavjud bo'lganda, skript ham chiqaradi
   `burn_in_note.json`, teg, kun indeksi, manifest identifikatori, telemetriyani olish
   manba va artefakt hazm qilish. Ushbu JSONni ishlab chiqarish jurnaliga biriktiring
   30 kunlik oynada har kuni qanoatlantirilgan suratlar aniq.
3. Yangilangan Grafana platasini import qiling (`dashboards/grafana/sorafs_fetch_observability.json`)
   sahnalashtirish/ishlab chiqarish ish maydoniga kiriting, uni kuyish yorlig'i bilan belgilang va
   Har bir panelda manifest/mintaqa uchun namunalar ko'rsatilishini tasdiqlang.
4. `scripts/telemetry/test_sorafs_fetch_alerts.sh` (yoki `promtool test rules …`) ni ishga tushiring
   `dashboards/alerts/sorafs_fetch_rules.yml` buni hujjatlash uchun o'zgartirilganda
   ogohlantirish marshruti yonish paytida eksport qilingan ko'rsatkichlarga mos keladi.
5. Olingan asboblar paneli snapshotini, ogohlantirish sinovi chiqishini va jurnalning dumini arxivlang
   orkestr bilan birga `telemetry::sorafs.fetch.*` qidiruvlaridan
   artefaktlar, shuning uchun boshqaruv o'lchovlarni olmasdan dalillarni takrorlashi mumkin
   jonli tizimlar.

## 7. Chiqarish nazorat ro'yxati

1. Nomzod konfiguratsiyasi va suratga olishdan foydalangan holda CIda reyting jadvallarini qayta yarating
   versiya nazorati ostidagi artefaktlar.
2. Har bir muhitda (laboratoriya, staging,
   kanareyka, ishlab chiqarish) va `--scoreboard-out` va `--json-out` ni biriktiring.
   ishlab chiqarish yozuviga artefaktlar.
3. Barcha o'lchovlarni ta'minlab, chaqiruv bo'yicha muhandis bilan telemetriya asboblar panelini ko'rib chiqing
   yuqorida jonli namunalar mavjud.
4. Yakuniy konfiguratsiya yo'lini yozib oling (odatda `iroha_config` orqali) va
   Reklamalar va muvofiqlik uchun foydalaniladigan boshqaruv reestrining git commit.
5. Chiqarish kuzatuvchisini yangilang va mijozga yangi standart sozlamalar haqida SDK guruhlarini xabardor qiling
   integratsiya bir xilda qoladi.

Ushbu qo'llanmaga rioya qilish orkestrni joylashtirishni aniq va tekshirilishi mumkin bo'ladi
qayta urinish byudjetlarini sozlash uchun aniq fikr-mulohazalarni taqdim etayotganda, provayder
qobiliyat va maxfiylik holati.