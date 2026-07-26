---
id: orchestrator-config
lang: uz
direction: ltr
source: docs/portal/docs/sorafs/orchestrator-config.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: SoraFS Orchestrator Configuration
sidebar_label: Orchestrator Configuration
description: Configure the multi-source fetch orchestrator, interpret failures, and debug telemetry output.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

::: Eslatma Kanonik manba
:::

# Ko'p manbali olish orkestratori qo'llanmasi

SoraFS ko'p manbali olish orkestratori deterministik, parallel drayvlar
boshqaruv tomonidan qo'llab-quvvatlanadigan reklamalarda chop etilgan provayder to'plamidan yuklab olishlar. Bu
qo'llanma orkestrni qanday sozlashni, qanday xato signallarini kutish kerakligini tushuntiradi
ishga tushirish paytida va qaysi telemetriya oqimlari sog'liq ko'rsatkichlarini ochib beradi.

## 1. Konfiguratsiyaga umumiy nuqtai

Orkestr uchta konfiguratsiya manbasini birlashtiradi:

| Manba | Maqsad | Eslatmalar |
|--------|---------|-------|
| `OrchestratorConfig.scoreboard` | Provayderning vaznini normallashtiradi, telemetriyaning yangiligini tasdiqlaydi va auditlar uchun ishlatiladigan JSON reyting jadvalini davom ettiradi. | `crates/sorafs_car::scoreboard::ScoreboardConfig` tomonidan qo'llab-quvvatlanadi. |
| `OrchestratorConfig.fetch` | Ish vaqti chegaralarini qo'llaydi (qayta urinish byudjetlari, parallellik chegaralari, tekshirish o'tish tugmalari). | `crates/sorafs_car::multi_fetch` da `FetchOptions` xaritalari. |
| CLI / SDK parametrlari | Tengdoshlar sonini cheklang, telemetriya hududlarini biriktiring va rad etish/ko'paytirish siyosatlarini kiriting. | `sorafs_cli fetch` bu bayroqlarni bevosita ochib beradi; SDK ularni `OrchestratorConfig` orqali o'tkazadi. |

`crates/sorafs_orchestrator::bindings`-dagi JSON yordamchilari to'liq seriyali
konfiguratsiyani Norito JSON-ga o'rnatib, uni SDK bog'lashlari orqali ko'chma qilish va
avtomatlashtirish.

### 1.1 JSON konfiguratsiyasi namunasi

```json
{
  "scoreboard": {
    "latency_cap_ms": 6000,
    "weight_scale": 12000,
    "telemetry_grace_secs": 900,
    "persist_path": "/var/lib/sorafs/scoreboards/latest.json"
  },
  "fetch": {
    "verify_lengths": true,
    "verify_digests": true,
    "retry_budget": 4,
    "provider_failure_threshold": 3,
    "global_parallel_limit": 8
  },
  "telemetry_region": "iad-prod",
  "max_providers": 6,
  "transport_policy": "soranet_first"
}
```

Faylni odatiy `iroha_config` qatlamlari orqali saqlash (`defaults/`, foydalanuvchi,
haqiqiy) shuning uchun deterministik joylashtirishlar tugunlar bo'ylab bir xil chegaralarni meros qilib oladi.
SNNet-5a tarqatish bilan mos keladigan faqat to'g'ridan-to'g'ri zaxira profili uchun,
`docs/examples/sorafs_direct_mode_policy.json` va hamroh bilan maslahatlashing
`docs/source/sorafs/direct_mode_pack.md` da ko'rsatma.

### 1.2 Muvofiqlikni bekor qilish

SNNet-9 boshqaruvga asoslangan muvofiqlikni orkestrga kiritadi. Yangi
Norito JSON konfiguratsiyasidagi `compliance` ob'ekti kesilgan qismlarni suratga oladi
bu qabul qilish quvurini faqat to'g'ridan-to'g'ri rejimga majbur qiladi:

```json
"compliance": {
  "operator_jurisdictions": ["US", "JP"],
  "jurisdiction_opt_outs": ["US"],
  "blinded_cid_opt_outs": [
    "C6B434E5F23ABD318F01FEDB834B34BD16B46E0CC44CD70536233A632DFA3828"
  ],
  "audit_contacts": ["mailto:compliance@example.org"]
}
```

- `operator_jurisdictions` ISO‑3166 alfa‑2 kodlarini e'lon qiladi, bu erda
  orkestr instansiyasi ishlaydi. Kodlar davomida katta harflar bilan normallashtiriladi
  tahlil qilish.
- `jurisdiction_opt_outs` boshqaruv registrini aks ettiradi. Har qanday operator qachon
  yurisdiktsiya ro'yxatda paydo bo'lsa, orkestr ijro etadi
  `transport_policy=direct-only` va siyosatning qaytarilish sababini chiqaradi
  `compliance_jurisdiction_opt_out`.
- `blinded_cid_opt_outs` manifest dayjestlari ro'yxatini beradi (ko'r qilingan CIDlar, shunday kodlangan
  bosh hex). Mos keladigan foydali yuklar ham to'g'ridan-to'g'ri rejalashtirishni va
  telemetriyada `compliance_blinded_cid_opt_out` zaxirasini yuzaga keltiring.
- `audit_contacts` URI boshqaruvi operatorlar e'lon qilishni kutayotganini qayd etadi.
  ularning GAR o'yin kitoblari.
- `attestations` siyosatni qo'llab-quvvatlovchi imzolangan muvofiqlik paketlarini oladi.
  Har bir yozuv ixtiyoriy `jurisdiction` (ISO-3166 alfa-2 kodi), a
  `document_uri`, kanonik 64 belgili `digest_hex`, emissiya
  vaqt tamg'asi `issued_at_ms` va ixtiyoriy `expires_at_ms`. Bu artefaktlar
  boshqaruv vositalarini bog'lashi uchun orkestrning tekshiruv ro'yxatiga o'ting
  imzolangan hujjatlarni bekor qiladi.

Muvofiqlik blokini operatorlar uchun odatiy konfiguratsiya qatlamlari orqali taqdim eting
deterministik bekor qilishlarni qabul qilish. Orkestr _after_ muvofiqlikni qo'llaydi
yozish rejimi boʻyicha maslahatlar: hatto SDK `upload-pq-only`, yurisdiktsiya yoki
manifestdan voz kechish hali ham faqat to'g'ridan-to'g'ri tashishga qaytadi va yo'q bo'lganda tezda ishlamay qoladi
muvofiq provayderlar mavjud.

Kanonik rad etish kataloglari ostida yashaydi
`governance/compliance/soranet_opt_outs.json`; Boshqaruv kengashi e'lon qiladi
yorliqli nashrlar orqali yangilanishlar. Konfiguratsiyaning to'liq namunasi (shu jumladan
attestations) mavjud `docs/examples/sorafs_compliance_policy.json`, va
operativ jarayon tasvirlangan
[GAR muvofiqligi kitobi](../../../source/soranet/gar_compliance_playbook.md).

### 1.3 CLI va SDK tugmalari

| Bayroq / Maydon | Effekt |
|-------------|--------|
| `--max-peers` / `OrchestratorConfig::with_max_providers` | Hisoblar paneli filtridan qancha provayder omon qolishini cheklaydi. Har bir mos provayderdan foydalanish uchun `None` ga sozlang. |
| `--retry-budget` / `FetchOptions::per_chunk_retry_limit` | Har bir bo‘lak uchun bosh harflar takrorlanadi. Cheklovdan oshib ketish `MultiSourceError::ExhaustedRetries` ni oshiradi. |
| `--telemetry-json` | Kechikish/muvaffaqiyatsizlik oniy suratlarini hisoblar paneli yaratuvchisiga kiritadi. `telemetry_grace_secs` dan ortiq eski telemetriya provayderlarni nomaqbul deb belgilaydi. |
| `--scoreboard-out` | Ishdan keyingi tekshirish uchun hisoblangan ko'rsatkichlar jadvalini (mavqul + nomaqbul provayderlar) saqlaydi. |
| `--scoreboard-now` | Tablo vaqt tamg'asini (Unix soniya) bekor qiladi, shuning uchun fikstür tasvirlari deterministik bo'lib qoladi. |
| `--deny-provider` / ball siyosati kancasi | Reklamalarni o'chirmasdan provayderlarni rejalashtirishdan qat'iy ravishda chiqarib tashlang. Tez javob beruvchi qora ro'yxatga olish uchun foydalidir. |
| `--boost-provider=name:delta` | Boshqaruv og'irliklariga tegmasdan turib, provayder uchun vaznli raund-robin kreditlarini sozlang. |
| `--telemetry-region` / `OrchestratorConfig::with_telemetry_region` | Yorliqlar koʻrsatkichlar va tuzilgan jurnallarni chiqaradi, shuning uchun asboblar paneli geografiya yoki tarqatish toʻlqini boʻyicha aylantira oladi. |
| `--transport-policy` / `OrchestratorConfig::with_transport_policy` | Ko‘p manbali orkestr asosiy bo‘lganligi sababli birlamchi `soranet-first`. `direct-only` ni pasaytirish yoki muvofiqlik direktivasiga rioya qilishda foydalaning va faqat PQ uchuvchilari uchun `soranet-strict` ni zaxiralang; muvofiqlikni bekor qilish hali ham qattiq shift vazifasini bajaradi. |

SoraNet-first endi standart yuk tashish hisoblanadi va orqaga qaytarishlar tegishli SNNet blokerini keltirishi kerak. SNNet-4/5/5a/5b/6a/7/8/12/13 bitiruvchisidan so'ng, boshqaruv talab qilinadigan pozitsiyani oldinga siljitadi (`soranet-strict` tomon); shu vaqtgacha faqat hodisaga asoslangan bekor qilishlar `direct-only` ga ustunlik berishi kerak va ular ishga tushirish jurnalida qayd etilishi kerak.

Yuqoridagi barcha bayroqlar ikkala `sorafs_cli fetch` va `--` uslubidagi sintaksisni qabul qiladi.
ishlab chiquvchiga qaragan `sorafs_fetch` ikkilik. SDK'lar terish orqali bir xil variantlarni ko'rsatadi
quruvchilar.

### 1.4 Guard keshini boshqarish

Endi CLI SoraNet qo'riqchi selektoriga ulanadi, shuning uchun operatorlar kirishni pin qilishlari mumkin
o'rni to'liq SNNet-5 transportini ishga tushirishdan qat'iy oldinroq. Uch
yangi bayroqlar ish jarayonini boshqaradi:

| bayroq | Maqsad |
|------|---------|
| `--guard-directory <PATH>` | Oxirgi relay konsensusini tavsiflovchi JSON fayliga ishora qiladi (quyida ko'rsatilgan). Katalogdan o'tish, olishni amalga oshirishdan oldin himoya keshini yangilaydi. |
| `--guard-cache <PATH>` | Norito kodli `GuardSet` davom etadi. Keyingi ishga tushirishlar yangi katalog berilmagan taqdirda ham keshdan qayta foydalanadi. |
| `--guard-target <COUNT>` / `--guard-retention-days <DAYS>` | Kirish qo'riqchilari soni (standart 3) va saqlash oynasi (standart 30 kun) uchun ixtiyoriy bekor qilish. |
| `--guard-cache-key <HEX>` | Ixtiyoriy 32 baytli kalit Blake3 MAC bilan himoyalangan keshlarni belgilash uchun ishlatiladi, shuning uchun faylni qayta ishlatishdan oldin tekshirish mumkin. |

Guard katalogining foydali yuklari ixcham sxemadan foydalanadi:

`--guard-directory` bayrog'i endi Norito kodlanganligini kutmoqda
`GuardDirectorySnapshotV2` foydali yuk. Ikkilik oniy tasvir quyidagilarni o'z ichiga oladi:

- `version` — sxema versiyasi (hozirda `2`).
- `directory_hash`, `published_at_unix`, `valid_after_unix`, `valid_until_unix` - konsensus
  har bir o'rnatilgan sertifikatga mos kelishi kerak bo'lgan metama'lumotlar.
- `validation_phase` - sertifikat siyosati eshigi (`1` = bitta Ed25519 imzosiga ruxsat berish,
  `2` = qo'sh imzoni afzal ko'ring, `3` = ikki tomonlama imzoni talab qiladi).
- `issuers` — `fingerprint`, `ed25519_public` va `mldsa65_public` bilan boshqaruv emitentlari.
  Barmoq izlari sifatida hisoblanadi
  `BLAKE3("soranet.src.v2.issuer" || ed25519 || u32(len(ml-dsa)) || ml-dsa)`.
- `relays` — SRCv2 to'plamlari ro'yxati (`RelayCertificateBundleV2::to_cbor()` chiqishi). Har bir to'plam
  rele deskriptorini, qobiliyat bayroqlarini, ML-KEM siyosatini va ikkita Ed25519/ML-DSA-65ni olib yuradi
  imzolar.

CLI katalogni birlashtirishdan oldin e'lon qilingan emitent kalitlari bilan har bir to'plamni tekshiradi

Oxirgi konsensusni `--guard-directory` bilan birlashtirish uchun CLI-ni chaqiring.
mavjud kesh. Selektor hali ham ichida bo'lgan mahkamlangan qo'riqchilarni saqlaydi
saqlash oynasi va katalogda mos; yangi o'rni o'rniga muddati o'tgan
yozuvlar. Muvaffaqiyatli yuklangandan so'ng, yangilangan kesh qayta yo'lga yoziladi
`--guard-cache` orqali ta'minlanadi, keyingi seanslarni deterministik saqlaydi. SDK'lar
qo'ng'iroq qilish orqali bir xil xatti-harakatni takrorlashi mumkin
`GuardSelector::select(&RelayDirectory, existing_guard_set, now_unix_secs)` va
natijada olingan `GuardSet` ni `SorafsGatewayFetchOptions` orqali ulash.

`ml_kem_public_hex` selektorga PQ qo'llab-quvvatlaydigan qo'riqchilarni birinchi o'ringa qo'yish imkonini beradi.
SNNet-5 ning chiqarilishi. Bosqichni almashtirish (`anon-guard-pq`, `anon-majority-pq`,
`anon-strict-pq`) endi klassik o'rni avtomatik ravishda pasaytiradi: PQ qo'riqchisi yoqilganda
mavjud bo'lsa, selektor ortiqcha klassik pinlarni tushiradi, shuning uchun keyingi seanslar yoqadi
gibrid qo'l siqish. CLI/SDK xulosalari orqali hosil bo'lgan aralash yuzaga chiqadi
`anonymity_status`/`anonymity_reason`, `anonymity_effective_policy`,
`anonymity_pq_selected`,
`anonymity_classical_selected`, `anonymity_pq_ratio`,
`anonymity_classical_ratio` va hamroh nomzod/defitsit/ta'minot deltasi
dalalar, jigarrang va klassik orqaga qaytishlarni aniq qiladi.

Guard kataloglari endi orqali to'liq SRCv2 to'plamini joylashtirishi mumkin
`certificate_base64`. Orkestrator har bir to'plamni dekodlaydi, qayta tasdiqlaydi
Ed25519/ML-DSA imzolari va tahlil qilingan sertifikat bilan birga saqlaydi
himoya keshi. Sertifikat mavjud bo'lganda, u uchun kanonik manba bo'ladi
PQ kalitlari, qo'l siqish to'plamining afzalliklari va vazni; muddati o'tgan sertifikatlar
zanjirning hayot aylanishini boshqarish orqali tarqaladi va orqali yuzaga keladi
qayd qiluvchi `telemetry::sorafs.guard` va `telemetry::sorafs.circuit`
amal qilish oynasi, qoʻl siqish toʻplamlari va ikkitomonlama imzolar kuzatilganmi yoki yoʻqmi
har bir qo'riqchi.

Snapshotlarni noshirlar bilan sinxronlashtirish uchun CLI yordamchilaridan foydalaning:```bash
sorafs_cli guard-directory fetch \
  --url https://directory.soranet.dev/mainnet_snapshot.norito \
  --output ./state/guard_directory.norito \
  --expected-directory-hash <directory-hash-hex>

sorafs_cli guard-directory verify \
  --path ./state/guard_directory.norito \
  --expected-directory-hash <directory-hash-hex>
```

`fetch` SRCv2 oniy rasmini diskka yozishdan oldin yuklab oladi va tekshiradi,
`verify` esa boshqa manbalardan olingan artefaktlar uchun tekshirish quvurini takrorlaydi.
jamoalar, CLI/SDK himoya selektori chiqishini aks ettiruvchi JSON xulosasini chiqaradi.

### 1.5 O'chirish davri boshqaruvchisi

O'rni katalogi va himoya keshi taqdim etilganda, orkestr
SoraNet sxemalarini oldindan qurish va yangilash uchun elektron hayot aylanishi boshqaruvchisini faollashtiradi
har bir olib kelishdan oldin. Konfiguratsiya `OrchestratorConfig` da ishlaydi
(`crates/sorafs_orchestrator/src/lib.rs:305`) ikkita yangi maydon orqali:

- `relay_directory`: SNNet-3 katalogining suratini olib yuradi, shuning uchun o'rta/chiqish sakrab tushadi
  deterministik tarzda tanlanishi mumkin.
- `circuit_manager`: ixtiyoriy konfiguratsiya (sukut bo'yicha yoqilgan)
  TTL sxemasi.

Norito JSON endi `circuit_manager` blokini qabul qiladi:

```json
"circuit_manager": {
  "enabled": true,
  "circuit_ttl_secs": 900
}
```

SDK katalog ma'lumotlarini yo'naltiradi
`SorafsGatewayFetchOptions::relay_directory`
(`crates/iroha/src/client.rs:320`) va CLI istalgan vaqtda uni avtomatik ravishda o'tkazadi
`--guard-directory` yetkazib beriladi (`crates/iroha_cli/src/commands/sorafs.rs:365`).

Qo'riqchi metama'lumotlar o'zgarganda boshqaruvchi sxemalarni yangilaydi (oxirgi nuqta, PQ kaliti,
yoki qadalgan vaqt tamg'asi) yoki TTL tugaydi. Yordamchi `refresh_circuits`
har bir qabul qilishdan oldin chaqiriladi (`crates/sorafs_orchestrator/src/lib.rs:1346`)
Operatorlar hayot tsikli qarorlarini kuzatishi uchun `CircuitEvent` jurnallarini chiqaradi. Cho'milish
test `circuit_manager_latency_soak_remains_stable_across_rotations`
(`crates/sorafs_orchestrator/src/soranet.rs:1479`) barqaror kechikishni namoyish etadi
uchta qo'riqchi aylanishi bo'ylab; ilova qilingan hisobotga qarang
`docs/source/soranet/reports/circuit_stability.md:1`.

### 1.6 Mahalliy QUIC proksi

Orkestrator ixtiyoriy ravishda mahalliy QUIC proksi-serverini yaratishi mumkin, shuning uchun brauzer kengaytmalari
va SDK adapterlari sertifikatlarni boshqarishi yoki kesh kalitlarini himoya qilishi shart emas. The
proksi-server qaytariladigan manzilga ulanadi, QUIC ulanishlarini tugatadi va a ni qaytaradi
Norito manifestida sertifikat va ixtiyoriy himoya kesh kaliti tavsiflanadi.
mijoz. Proksi tomonidan chiqarilgan transport hodisalari orqali hisoblanadi
`sorafs_orchestrator_transport_events_total`.

JSON orkestridagi yangi `local_proxy` bloki orqali proksi-serverni yoqing:

```json
"local_proxy": {
  "bind_addr": "127.0.0.1:9443",
  "telemetry_label": "dev-proxy",
  "guard_cache_key_hex": "0123456789ABCDEF0123456789ABCDEF0123456789ABCDEF0123456789ABCDEF",
  "emit_browser_manifest": true,
  "proxy_mode": "bridge",
  "prewarm_circuits": true,
  "max_streams_per_circuit": 64,
  "circuit_ttl_hint_secs": 300,
  "norito_bridge": {
    "spool_dir": "./storage/streaming/soranet_routes",
    "extension": "norito"
  },
  "kaigi_bridge": {
    "spool_dir": "./storage/streaming/soranet_routes",
    "extension": "norito",
    "room_policy": "public"
  }
}
```

- `bind_addr` proksi-server qayerda tinglashini nazorat qiladi (so'rash uchun `0` portidan foydalaning
  vaqtinchalik port).
- `telemetry_label` ko'rsatkichlarga tarqaladi, shuning uchun asboblar paneli farqlay oladi
  olish seanslaridan proksi-serverlar.
- `guard_cache_key_hex` (ixtiyoriy) proksi-serverga bir xil kalitli himoyani ko'rsatishga imkon beradi
  CLI/SDK-larga tayanadigan kesh, brauzer kengaytmalarini sinxronlashtirish.
- `emit_browser_manifest` qo'l siqish manifestni qaytaradimi yoki yo'qmi
  kengaytmalar saqlashi va tasdiqlashi mumkin.
- `proxy_mode` proksi-server mahalliy trafikni (`bridge`) yoki yo'qligini tanlaydi.
  faqat metadata chiqaradi, shuning uchun SDK'lar SoraNet sxemalarini o'zlari ochishi mumkin
  (`metadata-only`). Proksi standart sifatida `bridge`; qachon `metadata-only` o'rnating a
  ish stantsiyasi oqimlarni uzatmasdan manifestni ochishi kerak.
- `prewarm_circuits`, `max_streams_per_circuit` va `circuit_ttl_hint_secs`
  brauzerga qo'shimcha maslahatlar beradi, shuning uchun u parallel oqimlarni byudjetlashtirishi va
  proksi-server sxemalardan qanchalik agressiv foydalanishini tushuning.
- `car_bridge` (ixtiyoriy) mahalliy CAR arxiv keshida nuqtalar. `extension`
  oqim maqsadi `*.car` qoldirilsa, maydon qo'shilgan qo'shimchani boshqaradi; o'rnatish
  `allow_zst = true`, oldindan siqilgan `*.car.zst` foydali yuklarga bevosita xizmat ko'rsatish uchun.
- `kaigi_bridge` (ixtiyoriy) proksi-serverga biriktirilgan Kaigi marshrutlarini ochib beradi. The
  `room_policy` maydoni ko'prikning `public` yoki ishlamasligi haqida e'lon qiladi.
  `authenticated` rejimi, shuning uchun brauzer mijozlari to'g'ri GAR yorliqlarini oldindan tanlashlari mumkin.
- `sorafs_cli fetch` `--local-proxy-mode=bridge|metadata-only` va
  `--local-proxy-norito-spool=PATH` bekor qiladi, bu esa operatorlarga tugmani almashtirishga imkon beradi
  ish vaqti rejimi yoki JSON siyosatini o'zgartirmasdan muqobil spoullarga ishora qiling.
- `downgrade_remediation` (ixtiyoriy) avtomatik pasaytirish kancasini sozlaydi.
  Yoqilganda, orkestr telemetriyani pasaytirish portlashlari uchun kuzatib boradi
  va `window_secs` ichida sozlangan `threshold`dan so'ng mahalliy
  proksi-serverni `target_mode` (standart `metadata-only`) ga kiriting. Bir marta pasaytirishlar to'xtaydi
  proksi `cooldown_secs` dan keyin `resume_mode` ga qaytadi. `modes` dan foydalaning
  triggerni ma'lum o'rni rollariga qamrab olish uchun massiv (kirish o'rni uchun standart).

Proksi-server ko'prik rejimida ishlaganda, u ikkita dastur xizmatiga xizmat qiladi:

- **`norito`** – mijozning oqim maqsadiga nisbatan hal qilinadi
  `norito_bridge.spool_dir`. Maqsadlar sanitarizatsiya qilinadi (kesish yo'q, mutlaq emas
  yo'llar) va faylda kengaytma bo'lmasa, sozlangan qo'shimcha qo'llaniladi
  foydali yuk brauzerga so'zma-so'z uzatilishidan oldin.
- **`car`** – oqim maqsadlari `car_bridge.cache_dir` ichida hal qilinadi, meros qilib oladi
  konfiguratsiya qilingan standart kengaytma va siqilgan foydali yuklarni rad eting
  `allow_zst` o'rnatildi. Muvaffaqiyatli ko'priklar oldin `STREAM_ACK_OK` bilan javob beradi
  mijozlar quvurlarni tekshirishlari uchun arxiv baytlarini uzatish.

Ikkala holatda ham proksi-server HMAC kesh-tegini ta'minlaydi (kesh-kesh kaliti himoyalanganda
qo'l siqish paytida mavjud) va yozuvlar `norito_*` / `car_*` telemetriya sababi
Kodlar, shuning uchun asboblar paneli muvaffaqiyatlarni, etishmayotgan fayllarni va tozalashni farqlashi mumkin
bir qarashda muvaffaqiyatsizliklar.

`Orchestrator::local_proxy().await` ishlaydigan tutqichni ochadi, shuning uchun qo'ng'iroq qiluvchilar mumkin
PEM sertifikatini o'qing, brauzer manifestini oling yoki oqlangan so'rovni so'rang
ilova chiqqanda o'chirish.

Yoqilganda proksi endi **manifest v2** yozuvlariga xizmat qiladi. Mavjuddan tashqari
sertifikat va himoya kesh kaliti, v2 quyidagilarni qo'shadi:

- `alpn` (`"sorafs-proxy/1"`) va `capabilities` qatori mijozlar tasdiqlashi uchun
  ular gapirishlari kerak bo'lgan oqim protokoli.
- Qo'l siqish uchun `session_id` va kesh-yorliqlash tuzi (`cache_tagging` bloki)
  har bir seans uchun qo'riqchi yaqinliklari va HMAC teglarini oling.
- O'chirish va qo'riqchini tanlash bo'yicha maslahatlar (`circuit`, `guard_selection`,
  `route_hints`) shuning uchun brauzer integratsiyasi oqimlar paydo bo'lishidan oldin yanada boy foydalanuvchi interfeysini ko'rsatishi mumkin.
  ochildi.
- Mahalliy asboblar uchun namuna olish va maxfiylik tugmalari bilan `telemetry_v2`.
- Har bir `STREAM_ACK_OK` `cache_tag_hex` ni o'z ichiga oladi. Mijozlar qiymatni aks ettiradi
  Keshlangan HTTP yoki TCP so'rovlarini chiqarishda `x-sorafs-cache-tag` sarlavhasi
  qo'riqchi tanlovlari dam olishda shifrlangan bo'lib qoladi.

v1 kichik to'plamiga tayanishni davom eting.

## 2. Muvaffaqiyatsizlik semantikasi

Orkestr bittadan oldin qat'iy qobiliyat va byudjetni tekshiradi
bayt uzatiladi. Muvaffaqiyatsizliklar uch toifaga bo'linadi:

1. **Muvofiqlikdagi nosozliklar (parvoz oldidan).** Provayderlarning masofani o‘tkazish qobiliyati yo‘qligi,
   muddati o'tgan reklamalar yoki eskirgan telemetriya skorbord artefaktiga kiritilgan va
   rejalashtirishdan chiqarib tashlangan. CLI xulosalari `ineligible_providers` ni to'ldiradi
   sabablar bilan massiv, shuning uchun operatorlar boshqaruvning siljishini qirib tashlamasdan tekshirishlari mumkin
   jurnallar.
2. **Ish vaqtining charchashi.** Har bir provayder ketma-ket nosozliklarni kuzatib boradi. Bir marta
   konfiguratsiya qilingan `provider_failure_threshold` ga yetdi, provayder belgilangan
   Seansning qolgan qismi uchun `disabled`. Agar har bir provayder ga o'tsa
   `disabled`, orkestr qaytadi
   `MultiSourceError::NoHealthyProviders { last_error, chunk_index }`.
3. **Deterministik abortlar.** Strukturaviy xatolar sifatida qattiq chegaralar sirti:
   - `MultiSourceError::NoCompatibleProviders` - manifest parchani talab qiladi
     span yoki hizalama qolgan provayderlar hurmat qila olmaydi.
   - `MultiSourceError::ExhaustedRetries` - har bir parcha uchun qayta urinish byudjeti edi
     iste'mol qilingan.
   - `MultiSourceError::ObserverFailed` - quyi oqim kuzatuvchilari (oqimli ilgaklar)
     tasdiqlangan bo'lakni rad etdi.

Har bir xato noto'g'ri bo'lak indeksini va agar mavjud bo'lsa, yakuniy indeksni o'rnatadi
provayderning ishlamay qolishi sababi. Ularni bo'shatish blokerlari sifatida ko'ring - xuddi shu tarzda qayta urinib ko'ring
kiritish asosiy e'lon, telemetriya, yoki qadar muvaffaqiyatsizlikni takrorlaydi
provayderning sog'lig'idagi o'zgarishlar.

### 2.1 Scoreboard barqarorligi

`persist_path` sozlanganda, orkestr yakuniy jadvalni yozadi.
har bir yugurishdan keyin. JSON hujjati quyidagilarni o'z ichiga oladi:

- `eligibility` (`eligible` yoki `ineligible::<reason>`).
- `weight` (ushbu yugurish uchun belgilangan normallashtirilgan vazn).
- `provider` metama'lumotlari (identifikator, so'nggi nuqtalar, parallellik byudjeti).

Chiqarilgan artefaktlar bilan bir qatorda skorbord snapshotlarini arxivlang, shuning uchun qora ro'yxatga kiriting va
chiqarish qarorlari tekshirilishi mumkin.

## 3. Telemetriya va disk raskadrovka

### 3.1 Prometheus ko'rsatkichlari

Orkestr `iroha_telemetry` orqali quyidagi ko'rsatkichlarni chiqaradi:| Metrik | Yorliqlar | Tavsif |
|--------|--------|-------------|
| `sorafs_orchestrator_active_fetches` | `manifest_id`, `region` | Parvozda o'rnatilgan olib kelinish o'lchovi. |
| `sorafs_orchestrator_fetch_duration_ms` | `manifest_id`, `region` | Gistogramma yozishning oxirigacha kechikish vaqti. |
| `sorafs_orchestrator_fetch_failures_total` | `manifest_id`, `region`, `reason` | Terminal nosozliklari hisoblagichi (qayta urinishlar tugadi, provayderlar yo'q, kuzatuvchi nosozliklari). |
| `sorafs_orchestrator_retries_total` | `manifest_id`, `provider`, `reason` | Har bir provayder uchun qayta urinishlar soni. |
| `sorafs_orchestrator_provider_failures_total` | `manifest_id`, `provider`, `reason` | O'chirishga olib keladigan seans darajasidagi provayder xatolarining hisoblagichi. |
| `sorafs_orchestrator_policy_events_total` | `region`, `stage`, `outcome`, `reason` | Anonimlik siyosati qarorlari soni (toʻgʻrilash va ajralish holatlari) ishga tushirish bosqichi va qayta tiklash sababi boʻyicha guruhlangan. |
| `sorafs_orchestrator_pq_ratio` | `region`, `stage` | Tanlangan SoraNet to'plami orasida PQ relay ulushining gistogrammasi. |
| `sorafs_orchestrator_pq_candidate_ratio` | `region`, `stage` | Tablo snapshotidagi PQ reley ta'minoti nisbatlarining gistogrammasi. |
| `sorafs_orchestrator_pq_deficit_ratio` | `region`, `stage` | Siyosat tanqisligi gistogrammasi (maqsadli va haqiqiy PQ ulushi o'rtasidagi bo'shliq). |
| `sorafs_orchestrator_classical_ratio` | `region`, `stage` | Har bir seansda ishlatiladigan klassik reley ulushining gistogrammasi. |
| `sorafs_orchestrator_classical_selected` | `region`, `stage` | Har bir seans uchun tanlangan klassik rele sonining gistogrammasi. |

Ishlab chiqarish tugmachalarini aylantirishdan oldin ko'rsatkichlarni bosqichma-bosqich asboblar paneliga birlashtiring.
Tavsiya etilgan tartib SF-6 kuzatuv rejasini aks ettiradi:

1. **Faol yig'ish** — agar o'lchagich mos kelmasdan ko'tarilsa, ogohlantiradi.
2. **Qayta urinish nisbati** — `retry` hisoblagichlari tarixiy bazaviy qiymatlardan oshib ketganda ogohlantiradi.
3. **Provayder nosozliklari** — har qanday provayder kesib o'tganda peyjer ogohlantirishlarini ishga tushiradi
   `session_failure > 0` 15 daqiqa ichida.

### 3.2 Strukturaviy jurnal maqsadlari

Orkestr deterministik maqsadlar uchun tuzilgan voqealarni nashr etadi:

- `telemetry::sorafs.fetch.lifecycle` — `start` va `complete` hayot aylanishi
  bo'laklar soni, qayta urinishlar va umumiy davomiylik bilan markerlar.
- `telemetry::sorafs.fetch.retry` — qayta urinish hodisalari (`provider`, `reason`,
  `attempts`) qo'lda triajga oziqlantirish uchun.
- `telemetry::sorafs.fetch.provider_failure` - provayderlar tufayli o'chirilgan
  takroriy xatolar.
- `telemetry::sorafs.fetch.error` - terminaldagi nosozliklar bilan umumlashtiriladi
  `reason` va ixtiyoriy provayder metama'lumotlari.

Ushbu oqimlarni mavjud Norito log quvuriga yo'naltiring, shuning uchun hodisaga javob bering
haqiqatning yagona manbasiga ega. Hayotiy tsikl hodisalari orqali PQ/klassik aralashmani ochib beradi
`anonymity_effective_policy`, `anonymity_pq_ratio`,
`anonymity_classical_ratio` va ularning hamroh hisoblagichlari,
ko'rsatkichlarni qirib tashlamasdan asboblar panelini o'rnatishni osonlashtiradi. davomida
GA tarqatish, hayot aylanishi/qayta urinish hodisalari uchun jurnal darajasini `info` ga mahkamlang va
Terminal xatolar uchun `warn`.

### 3.3 JSON xulosalari

`sorafs_cli fetch` ham, Rust SDK ham quyidagilarni o'z ichiga olgan tuzilgan xulosani qaytaradi:

- `provider_reports` muvaffaqiyat/muvaffaqiyatsizliklar soni va provayder bo'lganmi?
  nogiron.
- `chunk_receipts` qaysi provayder har bir qismni qanoatlantirgani haqida batafsil ma'lumot beradi.
- `retry_stats` va `ineligible_providers` massivlari.

Noto'g'ri ishlaydigan provayderlarni tuzatishda xulosa faylini arxivlang - kvitansiyalar xaritasi
to'g'ridan-to'g'ri yuqoridagi jurnal metama'lumotlariga.

## 4. Operatsion nazorat ro'yxati

1. **CIda bosqich konfiguratsiyasi.** `sorafs_fetch` ni maqsad bilan ishga tushiring
   konfiguratsiya, muvofiqlik ko'rinishini olish uchun `--scoreboard-out` dan o'ting va
   oldingi versiyadan farq qiladi. Har qanday kutilmagan nomaqbul provayder to'xtatiladi
   reklama.
2. **Telemetriyani tasdiqlang.** `sorafs.fetch.*` oʻrnatish eksportini taʼminlash
   foydalanuvchilar uchun ko'p manbali olishni yoqishdan oldin o'lchovlar va tuzilgan jurnallar.
   Ko'rsatkichlarning yo'qligi odatda orkestrning jabhasi yo'qligini ko'rsatadi
   chaqirilgan.
3. **Hujjat bekor qilinadi.** Favqulodda `--deny-provider` yoki
   `--boost-provider` sozlamalari, JSON (yoki CLI chaqiruvi) ni
   o'zgartirish jurnali. Orqaga qaytarishlar bekor qilishni qaytarishi va yangi tabloni egallashi kerak
   surat.
4. **Tut sinovlarini qaytadan o‘tkazing.** Qayta urinish byudjetlari yoki provayder chegaralarini o‘zgartirgandan so‘ng,
   kanonik moslamani qaytadan oling (`fixtures/sorafs_manifest/ci_sample/`) va
   parcha tushumlari deterministik bo'lib qolayotganini tekshiring.

Yuqoridagi amallarga rioya qilish orkestrning harakatini takrorlash imkonini beradi
bosqichma-bosqich tarqatish va hodisaga javob berish uchun zarur bo'lgan telemetriyani ta'minlaydi.

### 4.1 Siyosatni bekor qilish

Operatorlar faol transport/anonimlik bosqichini tahrir qilmasdan belgilashlari mumkin
`policy_override.transport_policy` va sozlash orqali asosiy konfiguratsiya
`policy_override.anonymity_policy` o'zlarining `orchestrator` JSON (yoki etkazib berish)
`--transport-policy-override=` / `--anonymity-policy-override=` gacha
`sorafs_cli fetch`). Agar bekor qilish mavjud bo'lsa, orkestr o'tkazib yuboradi
odatiy qo'ng'iroqni qaytarish: agar so'ralgan PQ darajasi qondirilmasa,
olish jimgina pasaytirish o'rniga `no providers` bilan bajarilmaydi. ga qaytish
standart xatti-harakatlar bekor qilish maydonlarini tozalash kabi oddiy.

Standart `iroha_cli app sorafs fetch` buyrug'i bir xil bekor qilish bayroqlarini ko'rsatadi,
ularni shlyuz mijoziga yo'naltirish, shuning uchun ad-hoc olish va avtomatlashtirish skriptlari
bir xil bosqichli pinning xatti-harakatlarini baham ko'ring.

Cross-SDK moslamalari `fixtures/sorafs_gateway/policy_override/` ostida yashaydi. The
CLI, Rust mijozi, JavaScript ulanishlari va Swift jabduqlar kodini dekodlash
`override.json` o'zlarining paritet to'plamlarida, shuning uchun foydali yuklarni bekor qilishda har qanday o'zgarishlar
bu armatura yangilanishi va `cargo test -p iroha`, `npm test` va qayta ishga tushirilishi kerak va
SDK larni bir xilda saqlash uchun `swift test`. Har doim qayta tiklangan moslamani ulang
ko'rib chiqishni o'zgartiring, shuning uchun quyi oqim iste'molchilari bekor qilish shartnomasini farqlashi mumkin.

Boshqaruv har bir bekor qilish uchun runbook yozuvini talab qiladi. Sababini yozib oling,
kutilgan davomiylik va o'zgarishlar jurnalida orqaga qaytarish tetikleyicisi haqida PQga xabar bering
ratchet aylanish kanali va imzolangan tasdiqni bir xil artefaktga qo'shing
Tablo snapshotini saqlaydigan to'plam. Overrides qisqa muddatga mo'ljallangan
favqulodda vaziyatlar (masalan, PQ qo'riqchisi jigarrang); uzoq muddatli siyosat o'zgarishi kerak
oddiy kengash ovozi orqali tugunlar yangi sukut bo'yicha birlashadi.

### 4.2 PQ Ratchet Fire Drill

- **Runbook:** uchun `docs/source/soranet/pq_ratchet_runbook.md` ga rioya qiling
  lavozimga ko'tarilish/pasaytirish mashqlari, shu jumladan qo'riqchi-katalog bilan ishlash va orqaga qaytarish.
- **Boshqaruv paneli:** Monitoring uchun `dashboards/grafana/soranet_pq_ratchet.json` ni import qiling
  `sorafs_orchestrator_policy_events_total`, to'xtash tezligi va PQ nisbati o'rtacha
  mashq paytida.
- **Avtomatlashtirish:** `cargo test -p sorafs_orchestrator pq_ratchet_fire_drill_records_metrics`
  bir xil o'tishlarni amalga oshiradi va ko'rsatkichlar o'sishini tekshiradi
  operatorlar jonli matkapni ishga tushirishdan oldin kutilgan.

## 5. Rollout Playbooks

SNNet-5 transport vositasi yangi qo'riqchilar tanlovini, boshqaruvni taqdim etadi
attestatsiyalar va siyosatning kamchiliklari. Quyidagi o'yin kitoblari ketma-ketlikni kodlaydi
oxirgi foydalanuvchilar uchun ko‘p manbali yuklarni yoqishdan oldin amal qiling, shuningdek, pasaytirish
to'g'ridan-to'g'ri rejimga qaytish yo'li.

### 5.1 Ishlab chiquvchining parvozdan oldin (CI / Staging)

1. **CIda ko‘rsatkichlar jadvalini qayta yarating.** `sorafs_cli fetch` (yoki SDK) ishga tushiring
   ekvivalenti) bilan `fixtures/sorafs_manifest/ci_sample/` manifestiga qarshi
   nomzod konfiguratsiyasi. Tabloni davom ettiring
   `--scoreboard-out=artifacts/sorafs/scoreboard.json` va tasdiqlang:
   - `anonymity_status=="met"` va `anonymity_pq_ratio` belgilangan talablarga javob beradi
     bosqich (`anon-guard-pq`, `anon-majority-pq` yoki `anon-strict-pq`).
   - Deterministik bo'lak kvitansiyalari hali ham belgilangan oltin to'plamga mos keladi
     ombori.
2. **Manifest boshqaruvini tasdiqlang.** CLI/SDK xulosasini tekshiring va
   yangi paydo bo'lgan `manifest_governance.council_signatures` massivi o'z ichiga oladi
   kutilgan Kengashning barmoq izlari. Bu shlyuz javoblarining GARga yuborilishini tasdiqlaydi
   konvert va `validate_manifest` uni qabul qildi.
3. **Muvofiqlikni bekor qilishni mashq qiling.** Har bir yurisdiksiya profilini quyidagidan yuklang
   `docs/examples/sorafs_compliance_policy.json` va orkestrni tasdiqlang
   to'g'ri siyosat qaytarilishini chiqaradi (`compliance_jurisdiction_opt_out` yoki
   `compliance_blinded_cid_opt_out`). Agar yo'q bo'lsa, natijada olib kelish xatosini yozing
   muvofiq transport vositalari mavjud.
4. **Ochishga taqlid qiling.** `transport_policy` ni `direct-only` ga aylantiring.
   konfiguratsiyani sinovdan o'tkazing va orkestrni ta'minlash uchun olishni qayta ishga tushiring
   SoraNet relelariga tegmasdan Torii/QUIC ga qaytadi. Ushbu JSON-ni saqlang
   Variant versiya nazorati ostida, shuning uchun uni bir vaqt ichida tez targ'ib qilish mumkin
   voqea.

### 5.2 Operatorni ishga tushirish (ishlab chiqarish to'lqinlari)1. **`iroha_config` orqali konfiguratsiyani bosqichma-bosqich o'tkazing.** Ishlatilgan aniq JSONni nashr qiling
   CIda `actual` qatlamni bekor qilish sifatida. Orkestr pod/binarni tasdiqlang
   ishga tushirilganda yangi konfiguratsiya xeshini qayd qiladi.
2. **Asosiy himoya keshlari.** `--guard-directory` orqali o‘rni katalogini yangilang
   va Norito himoya keshini `--guard-cache` bilan saqlang. Kesh mavjudligini tekshiring
   imzolangan (agar `--guard-cache-key` sozlangan bo'lsa) va versiya ostida saqlanadi
   boshqaruvni o'zgartirish.
3. **Telemetriya asboblar panelini yoqing.** Foydalanuvchi trafigiga xizmat ko‘rsatishdan oldin, quyidagilarga ishonch hosil qiling
   muhit `sorafs.fetch.*`, `sorafs_orchestrator_policy_events_total` va
   proksi ko'rsatkichlari (mahalliy QUIC proksi-serverdan foydalanganda). Signallar bog'langan bo'lishi kerak
   `anonymity_brownout_effective` va moslik zaxira hisoblagichlari.
4. **Tuman sinovlarini o‘tkazing.** Har birida boshqaruv tomonidan tasdiqlangan manifestni oling.
   provayderlar guruhi (PQ, klassik va to'g'ridan-to'g'ri) va to'liq tushumlarni tasdiqlang,
   CAR dayjestlari va kengash imzolari CI bazasiga mos keladi.
5. **Muloqotni faollashtirish.** Ro'yxatdan o'tish trekerini yangilang
   `scoreboard.json` artefakt, himoya keshi barmoq izi va havola
   birinchi ishlab chiqarishni olish uchun manifest boshqaruv tekshiruvini ko'rsatadigan jurnallar.

### 5.3 Pastki versiya / Orqaga qaytarish tartibi

Hodisalar, PQ taqchilligi yoki tartibga solish talablari orqaga qaytishga majbur qilganda, amal qiling
Bu deterministik ketma-ketlik:

1. **Tashish siyosatini o‘zgartirish.** `transport_policy=direct-only` (va agar bo‘lsa) amal qiling
   yangi SoraNet sxemasi qurilishini darhol to'xtatadi.
2. **Qo'riqchi holatini tozalash.** Murojaat qilgan himoya kesh faylini o'chiring yoki arxivlang
   `--guard-cache`, shuning uchun keyingi ishga tushirishlar mahkamlangan relelarni qayta ishlatishga urinmaydi.
   Tez qayta yoqish rejalashtirilganda va kesh saqlanib qolganda, bu qadamni o'tkazib yuboring
   amal qiladi.
3. **Mahalliy proksi-serverlarni o‘chirib qo‘ying.** Agar mahalliy QUIC proksi-server `bridge` rejimida bo‘lsa,
   `proxy_mode="metadata-only"` bilan orkestrni qayta ishga tushiring yoki olib tashlang
   `local_proxy` butunlay bloklanadi. Port reliz shunday ish stantsiyani hujjatlashtirish va
   brauzer integratsiyasi to'g'ridan-to'g'ri Torii kirishiga qaytadi.
4. **Muvofiqlik bekor qilinishini aniqlang.** Yurisdiksiyadan voz kechish yozuvini qo‘shing (yoki
   Blinded-CID entry) ta'sirlangan foydali yuklar uchun muvofiqlik siyosatiga
   avtomatlashtirish va asboblar paneli qasddan to'g'ridan-to'g'ri rejimda ishlashni aks ettiradi.
5. **Auditorlik dalillarini qo'lga kiriting.** `--scoreboard-out` bilan o'zgartirilgandan so'ng yuklashni bajaring
   va CLI JSON xulosasini (jumladan, `manifest_governance`) yonida saqlang
   voqea chiptasi.

### 5.4 Tartibga solinadigan joylashtirishni tekshirish ro'yxati

| Tekshirish punkti | Maqsad | Tavsiya etilgan dalillar |
|------------|---------|----------------------|
| Muvofiqlik siyosati bosqichma-bosqich | Yurisdiksiyani ajratish GAR arizalari bilan mos kelishini tasdiqlaydi. | Imzolangan `soranet_opt_outs.json` oniy rasm + orkestr konfiguratsiyasi farqi. |
| Manifest boshqaruvi qayd etildi | Proves Kengash imzolari har bir shlyuz manifestiga hamroh bo'ladi. | `sorafs_cli fetch ... --output /dev/null --summary out.json`, `manifest_governance.council_signatures` arxivlangan. |
| Attestatsiya inventarizatsiyasi | `compliance.attestations` da havola qilingan hujjatlarni kuzatib boradi. | PDF/JSON artefaktlarini attestatsiya dayjesti va amal qilish muddati bilan birga saqlang. |
| Pastki darajali matkap qayd etildi | Orqaga qaytarish deterministik bo'lib qolishini ta'minlaydi. | Faqat toʻgʻridan-toʻgʻri siyosat qoʻllangani va himoya keshi tozalanganini koʻrsatadigan choraklik quruq ish rekordi. |
| Telemetriyani saqlash | Regulyatorlar uchun sud-tibbiy ma'lumotlarni taqdim etadi. | Boshqaruv paneli eksporti yoki `sorafs.fetch.*` ni tasdiqlovchi OTEL snapshoti va muvofiqlik zaxiralari siyosat boʻyicha saqlanib qolmoqda. |

Operatorlar har bir chiqish oynasidan oldin nazorat ro'yxatini ko'rib chiqishlari va jihozlashlari kerak
so'rov bo'yicha boshqaruv yoki tartibga soluvchi organlarga dalillar to'plami. Ishlab chiquvchilar qayta foydalanishlari mumkin
o'limdan keyingi paketlar uchun bir xil artefaktlar, agar qorayishlar yoki muvofiqlik bekor qilinganda
sinov paytida ishga tushiriladi.