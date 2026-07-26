---
lang: uz
direction: ltr
source: docs/portal/docs/soranet/pq-rollout-plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 9774dca76eff9ff13fcad9bf1fa7f084b95a987c392727cf0e6a74a4844e2b8e
source_last_modified: "2026-01-22T14:45:01.375247+00:00"
translation_last_reviewed: 2026-02-07
id: pq-rollout-plan
title: SNNet-16G Post-Quantum Rollout Playbook
sidebar_label: PQ Rollout Plan
description: Operational guide for promoting the SoraNet hybrid X25519+ML-KEM handshake from canary to default across relays, clients, and SDKs.
translator: machine-google-reviewed
---

::: Eslatma Kanonik manba
:::

SNNet-16G SoraNet transporti uchun kvantdan keyingi ishga tushirishni yakunlaydi. `rollout_phase` tugmalari operatorlarga mavjud bo'lgan A bosqich qo'riqlash talabidan B bosqich ko'pchilik qamroviga va C bosqich qat'iy PQ holatiga qadar har bir sirt uchun xom JSON/TOMLni tahrir qilmasdan deterministik reklamani muvofiqlashtirish imkonini beradi.

Ushbu o'yin kitobi quyidagilarni o'z ichiga oladi:

- Faza ta'riflari va yangi konfiguratsiya tugmalari (`sorafs.gateway.rollout_phase`, `sorafs.rollout_phase`) kod bazasiga ulangan (`crates/iroha_config/src/parameters/actual.rs:2230`, `crates/iroha/src/config/user.rs:251`).
- SDK va CLI bayrog'ini xaritalash, shuning uchun har bir mijoz tarqatishni kuzatishi mumkin.
- Relay/mijozning kanareykalarni rejalashtirish kutilmalari, shuningdek, rag'batlantirishni ta'minlaydigan boshqaruv panellari (`dashboards/grafana/soranet_pq_ratchet.json`).
- Qaytish ilgaklari va yong'inga qarshi burg'ulash kitobiga havolalar ([PQ ratchet runbook](./pq-ratchet-runbook.md)).

## Bosqich xaritasi

| `rollout_phase` | Samarali anonimlik bosqichi | Standart effekt | Oddiy foydalanish |
|----------------|--------------------------|----------------|---------------|
| `canary` | `anon-guard-pq` (A bosqich) | Filo isinayotganda har bir pallada kamida bitta PQ qo'riqchisini talab qiling. | Asosiy va erta kanareyka haftalari. |
| `ramp` | `anon-majority-pq` (B bosqich) | >= uchdan ikki qismi qamrab olish uchun PQ o'rni tomon yo'naltirilgan tanlov; Klassik relelar zaxira sifatida qolmoqda. | Mintaqaviy releyli kanareykalar; SDK oldindan koʻrish oʻchiriladi. |
| `default` | `anon-strict-pq` (S bosqich) | Faqat PQ sxemalarini qo'llash va pasaytirish signallarini kuchaytirish. | Telemetriya va boshqaruvni imzolash tugagach, yakuniy reklama. |

Agar sirt aniq `anonymity_policy` ni o'rnatsa, u ushbu komponent uchun fazani bekor qiladi. Aniq bosqichni o'tkazib yuborish endi `rollout_phase` qiymatiga to'g'ri keladi, shuning uchun operatorlar fazani har bir muhit uchun bir marta aylantirishi va mijozlarga uni meros qilib olishiga ruxsat berishlari mumkin.

## Konfiguratsiya ma'lumotnomasi

### Orkestr (`sorafs_gateway`)

```toml
[sorafs.gateway]
# Promote to Stage B (majority-PQ) canary
rollout_phase = "ramp"
# Optional: force a specific stage independent of the phase
# anonymity_policy = "anon-majority-pq"
```

Orkestr yuklagichi qayta ishlash bosqichini ish vaqtida (`crates/sorafs_orchestrator/src/lib.rs:2229`) hal qiladi va uni `sorafs_orchestrator_policy_events_total` va `sorafs_orchestrator_pq_ratio_*` orqali yuzaga chiqaradi. Qo‘llashga tayyor bo‘laklar uchun `docs/examples/sorafs_rollout_stage_b.toml` va `docs/examples/sorafs_rollout_stage_c.toml` ga qarang.

### Rust mijozi / `iroha_cli`

```toml
[sorafs]
# Keep clients aligned with orchestrator promotion cadence
rollout_phase = "default"
# anonymity_policy = "anon-strict-pq"  # optional explicit override
```

`iroha::Client` endi tahlil qilingan fazani (`crates/iroha/src/client.rs:2315`) qayd qiladi, shuning uchun yordamchi buyruqlar (masalan, `iroha_cli app sorafs fetch`) standart anonimlik siyosati bilan birga joriy bosqich haqida xabar berishi mumkin.

## Avtomatlashtirish

Ikkita `cargo xtask` yordamchisi jadval yaratish va artefaktni suratga olishni avtomatlashtiradi.

1. **Hududiy jadvalni yarating**

   ```bash
   cargo xtask soranet-rollout-plan \
     --regions us-east,eu-west,apac \
     --start 2026-04-01T00:00:00Z \
     --window 6h \
     --spacing 24h \
     --client-offset 8h \
     --phase ramp \
     --environment production
   ```

   Davomiylik `s`, `m`, `h` yoki `d` qo'shimchalarini qabul qiladi. Buyruq o'zgartirish so'rovi bilan yuborilishi mumkin bo'lgan `artifacts/soranet_pq_rollout_plan.json` va Markdown xulosasini (`artifacts/soranet_pq_rollout_plan.md`) chiqaradi.

2. **Matkap artefaktlarini imzolar bilan suratga oling**

   ```bash
   cargo xtask soranet-rollout-capture \
     --log logs/pq_fire_drill.log \
     --artifact kind=scoreboard,path=artifacts/canary.scoreboard.json \
     --artifact kind=fetch-summary,path=artifacts/canary.fetch.json \
     --key secrets/pq_rollout_signing_ed25519.hex \
     --phase ramp \
     --label "beta-canary" \
     --note "Relay canary - APAC first"
   ```

   Buyruq taqdim etilgan fayllarni `artifacts/soranet_pq_rollout/<timestamp>_<label>/` ga ko'chiradi, har bir artefakt uchun BLAKE3 dayjestlarini hisoblaydi va metama'lumotlarni o'z ichiga olgan `rollout_capture.json` va foydali yuk ustida Ed25519 imzosini yozadi. Yong'inga qarshi mashg'ulot daqiqalarini imzolaydigan bir xil shaxsiy kalitdan foydalaning, shunda boshqaruv tezda suratga olishni tasdiqlashi mumkin.

## SDK va CLI bayroq matritsasi

| Yuzaki | Kanareyka (A bosqich) | Rampa (B bosqich) | Standart (S bosqich) |
|---------|------------------|----------------|-------------------|
| `sorafs_cli` olish | `--anonymity-policy stage-a` yoki fazaga tayaning | `--anonymity-policy stage-b` | `--anonymity-policy stage-c` |
| JSON orkestr konfiguratsiyasi (`sorafs.gateway.rollout_phase`) | `canary` | `ramp` | `default` |
| Rust mijoz konfiguratsiyasi (`iroha.toml`) | `rollout_phase = "canary"` (standart) | `rollout_phase = "ramp"` | `rollout_phase = "default"` |
| `iroha_cli` imzolangan buyruqlar | `--anonymity-policy stage-a` | `--anonymity-policy stage-b` | `--anonymity-policy stage-c` |
| Java/Android `GatewayFetchOptions` | `setRolloutPhase("canary")`, ixtiyoriy ravishda `setAnonymityPolicy(AnonymityPolicy.ANON_GUARD_PQ)` | `setRolloutPhase("ramp")`, ixtiyoriy ravishda `.ANON_MAJORIY_PQ` | `setRolloutPhase("default")`, ixtiyoriy ravishda `.ANON_STRICT_PQ` |
| JavaScript orkestr yordamchilari | `rolloutPhase: "canary"` yoki `anonymityPolicy: "anon-guard-pq"` | `"ramp"` / `"anon-majority-pq"` | `"default"` / `"anon-strict-pq"` |
| Python `fetch_manifest` | `rollout_phase="canary"` | `"ramp"` | `"default"` |
| Swift `SorafsGatewayFetchOptions` | `anonymityPolicy: "anon-guard-pq"` | `"anon-majority-pq"` | `"anon-strict-pq"` |

Barcha SDK xaritani orkestr tomonidan foydalaniladigan bir xil bosqich tahlilchisiga o‘zgartiradi (`crates/sorafs_orchestrator/src/lib.rs:365`), shuning uchun aralash tilli o‘rnatishlar sozlangan faza bilan qulflangan bosqichda qoladi.

## Kanareykalarni rejalashtirish nazorat ro'yxati

1. **Parvozdan oldin (T minus 2 hafta)**

- A bosqichini tasdiqlang, avvalgi ikki haftada to'xtash darajasi <1% va PQ qamrovi har bir mintaqa uchun >=70% (`sorafs_orchestrator_pq_candidate_ratio`).
   - Kanareyka oynasini tasdiqlovchi boshqaruvni ko'rib chiqish joyini rejalashtiring.
   - Sahnalashtirishda `sorafs.gateway.rollout_phase = "ramp"` ni yangilang (orkestr JSON-ni tahrirlang va qayta joylashtiring) va reklama quvurini quriting.

2. **Estafeta kanareykasi (T kun)**

   - `rollout_phase = "ramp"` orkestr va ishtirokchi reley manifestlarida bir vaqtning o'zida bitta mintaqani targ'ib qiling.
   - PQ Ratchet asboblar panelida (hozirda ishga tushirish paneli mavjud) "Natijalar bo'yicha siyosat hodisalari" va "Brownout tezligi" ni TTL qo'riqlash keshini ikki baravar oshiring.
   - Audit saqlash uchun ishga tushirishdan oldin va keyin `sorafs_cli guard-directory fetch` suratlarini kesib oling.

3. **Mijoz/SDK kanareykasi (T plus 1 hafta)**

   - Mijoz konfiguratsiyasida `rollout_phase = "ramp"` ni aylantiring yoki belgilangan SDK kogortalari uchun `stage-b` bekor qilishni o'tkazing.
   - Telemetriya farqlarini yozib oling (`sorafs_orchestrator_policy_events_total` `client_id` va `region` tomonidan guruhlangan) va ularni tarqatish hodisalari jurnaliga biriktiring.

4. **Birlamchi reklama (T plus 3 hafta)**

   - Boshqaruv imzolangandan so'ng, orkestr va mijoz konfiguratsiyasini `rollout_phase = "default"` ga o'tkazing va imzolangan tayyorlikni tekshirish ro'yxatini chiqarish artefaktlariga aylantiring.

## Boshqaruv va dalillarni tekshirish ro'yxati

| Faza o'zgarishi | Reklama darvozasi | Dalillar to'plami | Boshqaruv paneli va ogohlantirishlar |
|-------------|----------------|-----------------|---------------------|
| Kanareyka → Rampa *(B bosqichni oldindan ko'rish)* | Bosqich - keyingi 14 kun ichida o'chirish darajasi <1%, targ'ib qilingan mintaqa uchun `sorafs_orchestrator_pq_candidate_ratio` ≥ 0,7, Argon2 chiptasi p95 < 50 ms va bron qilingan reklama uchun boshqaruv sloti. | `cargo xtask soranet-rollout-plan` JSON/Markdown juftligi, bogʻlangan `sorafs_cli guard-directory fetch` suratlari (oldin/keyin), imzolangan `cargo xtask soranet-rollout-capture --label canary` toʻplami va kanareyka daqiqalari [PQ ratchet runbook](I180000000000000000000000000000) | `dashboards/grafana/soranet_pq_ratchet.json` (Siyosat hodisalari + Brownout Rate), `dashboards/grafana/soranet_privacy_metrics.json` (SN16 pasaytirish nisbati), `docs/source/soranet/snnet16_telemetry_plan.md` da telemetriya ma'lumotnomalari. |
| Rampa → Standart *(S bosqichini amalga oshirish)* | 30 kunlik SN16 telemetriya yonishi kuzatildi, `sn16_handshake_downgrade_total` bazaviy tekis, `sorafs_orchestrator_brownouts_total` mijoz kanareykasida nolga teng va proksi-serverni almashtirish repetisiyasi qayd etildi. | `sorafs_cli proxy set-mode --mode gateway|direct` transkripti, `promtool test rules dashboards/alerts/soranet_handshake_rules.yml` chiqishi, `sorafs_cli guard-directory verify` jurnali va imzolangan `cargo xtask soranet-rollout-capture --label default` toʻplami. | Xuddi shu PQ Ratchet taxtasi va `docs/source/sorafs_orchestrator_rollout.md` va `dashboards/grafana/soranet_privacy_metrics.json` da hujjatlashtirilgan SN16 pasaytirish panellari. |
| Favqulodda pasayish / orqaga qaytishga tayyorlik | Hisoblagichlar pasayganida, qo‘riqlash katalogi tekshiruvi bajarilmaganda yoki `/policy/proxy-toggle` buferi doimiy pasaytirish hodisalarini qayd qilganda ishga tushadi. | `docs/source/ops/soranet_transport_rollback.md`, `sorafs_cli guard-directory import` / `guard-cache prune` jurnallari, `cargo xtask soranet-rollout-capture --label rollback`, voqea chiptalari va bildirishnoma shablonlarining nazorat roʻyxati. | `dashboards/grafana/soranet_pq_ratchet.json`, `dashboards/grafana/soranet_privacy_metrics.json` va ikkala ogohlantirish paketlari (`dashboards/alerts/soranet_handshake_rules.yml`, `dashboards/alerts/soranet_privacy_rules.yml`). |

- Har bir artefaktni `artifacts/soranet_pq_rollout/<timestamp>_<label>/` ostida yaratilgan `rollout_capture.json` bilan saqlang, shuning uchun boshqaruv paketlarida skorbord, promtool izlari va dayjestlar mavjud.
- Yuklangan dalillarning SHA256 dayjestlarini (PDF daqiqalari, suratga olish toʻplami, qoʻriqchi snapshotlari) reklama daqiqalariga ilova qiling, shunda parlament maʼqullashlari sahnalashtirish klasteriga kirmasdan takrorlanadi.
- `docs/source/soranet/snnet16_telemetry_plan.md` lug'at darajasini pasaytirish va ogohlantirish chegaralari uchun kanonik manba bo'lib qolishini isbotlash uchun reklama chiptasidagi telemetriya rejasiga murojaat qiling.

## Boshqaruv paneli va telemetriya yangilanishlari

`dashboards/grafana/soranet_pq_ratchet.json` endi ushbu oʻyin kitobiga havola qiluvchi va joriy bosqichni koʻrsatadigan “Oʻchirish rejasi” izoh paneli bilan birga yetkazib beriladi, shuning uchun boshqaruv tekshiruvlari qaysi bosqich faolligini tasdiqlashi mumkin. Panel tavsifini konfiguratsiya tugmalaridagi kelajakdagi o'zgarishlar bilan sinxronlashtiring.

Ogohlantirish uchun mavjud qoidalar `stage` yorlig'idan foydalanishiga ishonch hosil qiling, shunda kanareyka va standart bosqichlar alohida siyosat chegaralarini (`dashboards/alerts/soranet_handshake_rules.yml`) ishga tushiradi.

## Orqaga qaytarish ilgaklari

### Standart → Rampa (S bosqich → B bosqich)

1. `sorafs_cli config set --config orchestrator.json sorafs.gateway.rollout_phase ramp` bilan orkestratorni pasaytiring (va bir xil fazani SDK konfiguratsiyalarida aks ettiring), B bosqichi butun flot bo'ylab davom etadi.
2. `/policy/proxy-toggle` tuzatish ish jarayoni tekshirilishi mumkin bo'lishi uchun transkriptni yozib, `sorafs_cli proxy set-mode --mode direct --note "sn16 rollback"` orqali mijozlarni xavfsiz transport profiliga majburlang.
3. `artifacts/soranet_pq_rollout/` ostida qorovul-katalog farqlari, promtool chiqishi va asboblar paneli skrinshotlarini arxivlash uchun `cargo xtask soranet-rollout-capture --label rollback-default`-ni ishga tushiring.

### Rampa → Kanareyka (B bosqich → A bosqich)

1. `sorafs_cli guard-directory import --guard-directory guards.json` bilan ko'tarilishdan oldin olingan qo'riqchi katalogining snapshotini import qiling va `sorafs_cli guard-directory verify` ni qayta ishga tushiring, shunda pasaytirish paketi xeshlarni o'z ichiga oladi.
2. Orkestr va mijoz konfiguratsiyalarida `rollout_phase = "canary"` (yoki `anonymity_policy stage-a` bilan bekor qilish) ni o‘rnating, so‘ng pasaytirilgan quvur liniyasini isbotlash uchun [PQ ratchet runbook](./pq-ratchet-runbook.md) dan PQ ratchet matkapini qayta o‘ynang.
3. Boshqaruvni xabardor qilishdan oldin yangilangan PQ Ratchet va SN16 telemetriya skrinshotlarini hamda ogohlantirish natijalarini voqea jurnaliga ilova qiling.

### Guardrail eslatmalari- Har safar pasaytirish sodir bo'lganda `docs/source/ops/soranet_transport_rollback.md` ma'lumotnomasiga murojaat qiling va keyingi ish uchun har qanday vaqtinchalik yumshatishni `TODO:` elementi sifatida ro'yxatga olish trekeriga yozib oling.
- `dashboards/alerts/soranet_handshake_rules.yml` va `dashboards/alerts/soranet_privacy_rules.yml` ni orqaga qaytarishdan oldin va keyin `promtool test rules` qamrovi ostida saqlang, shuning uchun ogohlantirish to'plami suratga olish to'plami bilan birga hujjatlashtiriladi.