---
lang: uz
direction: ltr
source: docs/source/cbdc_lane_playbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: b97d0dd24de65ba37cba0fa4d404235b63a771f49ac6ee8f07e4814d2a6db814
source_last_modified: "2026-01-22T16:26:46.564417+00:00"
translation_last_reviewed: 2026-02-07
title: CBDC Lane Playbook
sidebar_label: CBDC Lane Playbook
description: Reference configuration, whitelist flow, and compliance evidence for permissioned CBDC lanes on SORA Nexus.
translator: machine-google-reviewed
---

# CBDC Private Lane Playbook (NX-6)

> **Yo‘l xaritasi aloqasi:** NX-6 (CBDC shaxsiy yo‘lak shablonlari va oq ro‘yxatlar oqimi) va NX-14 (Nexus runbooks).  
> **Egalari:** Financial Services WG, Nexus Core WG, Compliance WG.  
> **Holat:** Loyihalash — amalga oshirish ilgaklari `crates/iroha_data_model::nexus`, `crates/iroha_core::governance::manifest` va `integration_tests/tests/nexus/lane_registry.rs` da mavjud, biroq CBDC-ga xos manifestlar, oq roʻyxatlar va operator ish kitoblari yoʻq edi. Ushbu o'yin kitobi ma'lumotnoma konfiguratsiyasi va ishga tushirish ish jarayonini hujjatlashtiradi, shuning uchun CBDC o'rnatilishi aniq davom etishi mumkin.

## Qamrov va rollar

- **Markaziy bank yoʻli (“CBDC yoʻlagi”):** Ruxsat berilgan validatorlar, kastodial hisob-kitoblar buferlari va dasturlashtiriladigan pul siyosatlari. O'zining boshqaruv manifestiga ega cheklangan ma'lumotlar maydoni + qator juftligi sifatida ishlaydi.
- **Ulgurji/chakana bank ma'lumotlar maydonlari:** UAID-larga ega, qobiliyat manifestlarini qabul qiluvchi va CBDC qatorli atomik AXT uchun oq ro'yxatga kiritilishi mumkin bo'lgan DS ishtirokchisi.
- **Dasturlashtiriladigan pulli dApps:** CBDC oqimlarini iste'mol qiladigan tashqi DS oq ro'yxatga kiritilgandan so'ng `ComposabilityGroup` marshrutlash orqali o'tadi.
- **Boshqaruv va muvofiqlik:** Parlament (yoki ekvivalent modul) yoʻlak manifestlarini, qobiliyat manifestlarini va oq roʻyxat oʻzgarishlarini tasdiqlaydi; muvofiqlik Norito manifestlari bilan birga dalillar to'plamlarini saqlaydi.

**Tobeliklar**

1. Lane katalogi + ma'lumotlar maydoni katalogining simlari (`docs/source/nexus_lanes.md`, `defaults/nexus/config.toml`).
2. Lane manifest ijrosi (`crates/iroha_core/src/governance/manifest.rs`, `crates/iroha_core/src/queue.rs` da navbat darvozasi).
3. Qobiliyat manifestlari + UAIDlar (`crates/iroha_data_model/src/nexus/manifest.rs`).
4. Rejalashtiruvchi TEU kvotalari + ko'rsatkichlari (`integration_tests/tests/scheduler_teu.rs`, `docs/source/telemetry.md`).

## 1. Yo'naltiruvchi qator tartibi

### 1.1 Lane katalogi va maʼlumotlar fazosi yozuvlari

`[[nexus.lane_catalog]]` va `[[nexus.dataspace_catalog]]` ga bag'ishlangan yozuvlarni qo'shing. Quyidagi misol `defaults/nexus/config.toml` ni CBDC liniyasi bilan kengaytiradi, u har bir slot uchun 1500 TEU saqlaydi va ochlikni olti slotga kamaytiradi, shuningdek, ulgurji banklar va chakana hamyonlar uchun mos keladigan maʼlumotlar maydoni taxalluslari.

```toml
lane_count = 5

[[nexus.lane_catalog]]
index = 3
alias = "cbdc"
description = "Central bank CBDC lane"
dataspace = "cbdc.core"
visibility = "restricted"
lane_type = "cbdc_private"
governance = "central_bank_multisig"
settlement = "xor_dual_fund"
metadata.scheduler.teu_capacity = "1500"
metadata.scheduler.starvation_bound_slots = "6"
metadata.settlement.buffer_account = "buffer::cbdc_treasury"
metadata.settlement.buffer_asset = "61CtjvNd9T3THAR65GsMVHr82Bjc"
metadata.settlement.buffer_capacity_micro = "1500000000"
metadata.telemetry.contact = "ops@cb.example"

[[nexus.dataspace_catalog]]
alias = "cbdc.core"
id = 10
description = "CBDC issuance dataspace"
fault_tolerance = 1

[[nexus.dataspace_catalog]]
alias = "cbdc.bank.wholesale"
id = 11
description = "Wholesale bank onboarding lane"
fault_tolerance = 1

[[nexus.dataspace_catalog]]
alias = "cbdc.dapp.retail"
id = 12
description = "Retail wallets and programmable-money dApps"
fault_tolerance = 1

[nexus.routing_policy]
default_lane = 0
default_dataspace = "universal"

[[nexus.routing_policy.rules]]
lane = 3
dataspace = "cbdc.core"
[nexus.routing_policy.rules.matcher]
instruction = "cbdc::*"
description = "Route CBDC contracts to the restricted lane"
```

**Eslatmalar**

- `metadata.scheduler.teu_capacity` va `metadata.scheduler.starvation_bound_slots` `integration_tests/tests/scheduler_teu.rs` tomonidan amalga oshirilgan TEU o'lchagichlarini oziqlantiradi. `nexus_scheduler_lane_teu_capacity` shablonga mos kelishi uchun operatorlar ularni qabul natijalari bilan sinxronlashtirishi kerak.
- Yuqoridagi har bir maʼlumot maydoni taxalluslari boshqaruv manifestlarida va qobiliyat manifestlarida paydo boʻlishi kerak (pastga qarang), shuning uchun qabul qilish driftni avtomatik ravishda rad etadi.

### 1.2 Lane manifest skeleti

Lane manifestlari `nexus.registry.manifest_directory` orqali sozlangan katalog ostida yashaydi (qarang: `crates/iroha_config/src/parameters/actual.rs`). Fayl nomlari qator taxalluslariga mos kelishi kerak (`cbdc.manifest.json`). Sxema `integration_tests/tests/nexus/lane_registry.rs` da boshqaruv manifest testlarini aks ettiradi.

```json
{
  "lane": "cbdc",
  "dataspace": "cbdc.core",
  "version": 1,
  "governance": "central_bank_multisig",
  "validators": [
    { "validator": "<i105-account-id>", "peer_id": "<peer-id>" },
    { "validator": "<i105-account-id>", "peer_id": "<peer-id>" },
    { "validator": "<i105-account-id>", "peer_id": "<peer-id>" },
    { "validator": "<i105-account-id>", "peer_id": "<peer-id>" }
  ],
  "quorum": 3,
  "protected_namespaces": [
    "cbdc.core",
    "cbdc.policy",
    "cbdc.settlement"
  ],
  "hooks": {
    "runtime_upgrade": {
      "allow": true,
      "require_metadata": true,
      "metadata_key": "cbdc_upgrade_id",
      "allowed_ids": [
        "upgrade-issuance-v1",
        "upgrade-pilot-retail"
      ]
    }
  },
  "composability_group": {
    "group_id_hex": "7ab3f9b3b2777e9f8b3d6fae50264a1e0ffab7c74542ff10d67fbdd073d55710",
    "activation_epoch": 2048,
    "whitelist": [
      "ds::cbdc.bank.wholesale",
      "ds::cbdc.dapp.retail"
    ],
    "quotas": {
      "group_teu_share_max": 500,
      "per_ds_teu_max": 250
    }
  }
}
```

Asosiy talablar:-

- Validators **must** be declared as explicit bindings with a canonical I105
  authority account plus a concrete `peer_id`. Legacy string-only validator
  arrays are rejected.
- Each manifest `peer_id` must resolve to a registered runtime peer with a live
  consensus key that is present in the current commit topology; Torii routes
  only to those authoritative peer bindings and fails closed when the runtime
  truth disagrees with the manifest.
- Validator accounts should remain stable governance identities even if the
  underlying host or peer keys rotate; update the manifest `peer_id` binding
  when the serving peer changes. Set `quorum` to the multisig threshold (≥2).
- Himoyalangan nom maydonlari `Queue::push` tomonidan amalga oshiriladi (qarang: `crates/iroha_core/src/queue.rs`), shuning uchun barcha CBDC shartnomalarida `gov_namespace` + `gov_contract_id` belgilanishi kerak.
- `composability_group` maydonlari `docs/source/nexus.md` §8.6 da tasvirlangan sxemaga amal qiladi; egasi (CBDC yo'li) oq ro'yxat va kvotalar bilan ta'minlaydi. Oq ro'yxatga kiritilgan DS manifestlari faqat `group_id_hex` + `activation_epoch` ni belgilaydi.
- Manifestdan nusxa olgandan so'ng, `LaneManifestRegistry::from_config` uni yuklashini tasdiqlash uchun `cargo test -p integration_tests nexus::lane_registry -- --nocapture` ni ishga tushiring.

### 1.3 Imkoniyatlar manifestlari (UAID siyosatlari)

Qobiliyat manifestlari (`AssetPermissionManifest` da `crates/iroha_data_model/src/nexus/manifest.rs`) `UniversalAccountId` ni deterministik imtiyozlarga bog'laydi. Banklar va dApps imzolangan siyosatlarni olishi uchun ularni Space Directory orqali nashr eting.

```json
{
  "version": 1,
  "uaid": "uaid:5f77b4fcb89cb03a0ab8f46d98a72d585e3b115a55b6bdb2e893d3f49d9342f1",
  "dataspace": 11,
  "issued_ms": 1762723200000,
  "activation_epoch": 2050,
  "expiry_epoch": 2300,
  "entries": [
    {
      "scope": {
        "dataspace": 11,
        "program": "cbdc.transfer",
        "method": "transfer",
        "asset": "CBDC#centralbank",
        "role": "Initiator"
      },
      "effect": {
        "Allow": {
          "max_amount": "1000000000",
          "window": "PerDay"
        }
      },
      "notes": "Wholesale transfer allowance (per UAID)"
    },
    {
      "scope": {
        "dataspace": 11,
        "program": "cbdc.kit",
        "method": "withdraw"
      },
      "effect": {
        "Deny": {
          "reason": "Withdrawals disabled for this UAID"
        }
      }
    }
  ]
}
```

- Ruxsat berilgan qoida (`ManifestVerdict::Denied`) mos kelganda ham rad etish qoidalari g'alaba qozonadi, shuning uchun barcha ochiq rad etishlarni tegishli ruxsatnomalardan keyin joylashtiring.
- Atom to'lovlari uchun `AllowanceWindow::PerSlot` va mijozlar chegaralarini o'zgartirish uchun `PerMinute`/`PerDay` dan foydalaning.
- UAID/ma'lumotlar maydoni uchun bitta manifest kifoya qiladi; faollashtirishlar va amal qilish muddati siyosatning aylanish kadensiyasini amalga oshiradi.
- `expiry_epoch` ga erishilgandan so'ng, chiziqning ishlash muddati endi avtomatik ravishda tugaydi,
  shuning uchun operatsion guruhlar oddiygina `SpaceDirectoryEvent::ManifestExpired`ni kuzatib boradi,
  `nexus_space_directory_revision_total` deltasini arxivlang va Torii ko'rsatuvlarini tekshiring
  `status = "Expired"`. CLI `manifest expire` buyrug'i mavjud bo'lib qoladi
  qo'lda bekor qilish yoki dalillarni to'ldirish.

## 2. Bankni ishga tushirish va oq roʻyxat ish jarayoni

| Bosqich | Ega(lar)i | Amallar | Dalil |
|-------|----------|---------|----------|
| 0. Qabul qilish | CBDC PMO | KYC ma'lumotlarini, texnik DS manifestini, validator ro'yxatini, UAID xaritasini to'plang. | Qabul qilish chiptasi, imzolangan DS manifest loyihasi. |
| 1. Boshqaruvni tasdiqlash | Parlament / Muvofiqlik | Qabul qilish paketini ko'rib chiqing, `cbdc.manifest.json` belgisini ko'ring, `AssetPermissionManifest`ni tasdiqlang. | Imzolangan boshqaruv protokoli, manifest commit hash. |
| 2. Imkoniyatlarni berish | CBDC yo'lak operatsiyalari | `norito::json::to_string_pretty` orqali manifestlarni kodlash, Space Directory ostida saqlash, operatorlarni xabardor qilish. | Manifest JSON + norito `.to` fayli, BLAKE3 dayjesti. |
| 3. Oq ro'yxatni faollashtirish | CBDC yo'lak operatsiyalari | DSID-ni `composability_group.whitelist` ga qo'shing, `activation_epoch`-ga teging, manifestni tarqating; agar kerak bo'lsa, ma'lumotlar maydoni marshrutini yangilang. | Manifest farqi, `kagami config diff` chiqishi, boshqaruvni tasdiqlash identifikatori. |
| 4. Chiqarishni tekshirish | QA gildiyasi / Ops | Integratsiya testlarini, TEU yuk testlarini va dasturlashtiriladigan pulni takrorlashni o'tkazing (pastga qarang). | `cargo test` jurnallari, TEU asboblar paneli, dasturlashtiriladigan pul moslamalari natijalari. |
| 5. Dalillar arxivi | Muvofiqlik WG | `artifacts/nexus/cbdc_<stamp>/` ostida toʻplam manifestlari, tasdiqlashlari, qobiliyat dayjestlari, sinov natijalari va Prometheus parchalari. | Dalil tarball, checksum fayli, kengash imzosi. |

### Audit paketi yordamchisiKosmik katalogdagi `iroha app space-directory manifest audit-bundle` yordamchisidan foydalaning
Dalillar to'plamini topshirishdan oldin har bir qobiliyatning manifestini suratga olish uchun o'yin kitobi.
Manifest JSON (yoki `.to` foydali yuk) va maʼlumotlar maydoni profilini taqdim eting:

```bash
iroha app space-directory manifest audit-bundle \
  --manifest-json fixtures/space_directory/capability/cbdc_wholesale.manifest.json \
  --profile fixtures/space_directory/profile/cbdc_lane_profile.json \
  --out-dir artifacts/nexus/cbdc/2026-02-01T00-00Z/cbdc_wholesale \
  --notes "CBDC wholesale refresh"
```

Buyruq kanonik JSON/Norito/xesh nusxalarini chiqaradi.
`audit_bundle.json`, u UAID, ma'lumotlar maydoni identifikatori, faollashtirish/muddatni qayd etadi
epochs, manifest xesh va profil audit ilgaklari talab qilinganda
`SpaceDirectoryEvent` obunalari. To'plamni dalil ichiga tashlang
auditorlar va regulyatorlar aniq baytlarni keyinroq takrorlashlari uchun katalog.

### 2.1 Buyruqlar va tekshirishlar

1. **Line manifestlari:** `cargo test -p integration_tests nexus::lane_registry -- --nocapture lane_manifest_registry_loads_fixture_manifests`.
2. **Rejalashtiruvchi kvotalar:** `cargo test -p integration_tests scheduler_teu -- queue_teu_backlog_matches_metering queue_routes_transactions_across_configured_lanes`.
3. **Manuel tutun:** CBDC fayllariga ishora qiluvchi manifest katalogi bilan `irohad --sora --config configs/soranexus/nexus/config.toml --chain 0000…`, keyin `/v1/sumeragi/status` tugmasini bosing va CBDC qatori uchun `lane_governance.manifest_ready=true` ni tekshiring.
4. **Oq roʻyxat muvofiqligi testi:** `cargo test -p integration_tests nexus::cbdc_whitelist -- --nocapture` mashqlari `integration_tests/tests/nexus/cbdc_whitelist.rs`, tahlil qilish `fixtures/space_directory/profile/cbdc_lane_profile.json` va havola qilingan qobiliyatlar oq roʻyxatga kiritilgan har bir kirishning UAID, maʼlumotlar maydoni, faollashtirish davri va I18000000 roʻyxat ruxsatnomalariga mos kelishini taʼminlaydi. `fixtures/space_directory/capability/` ostida. Oq ro'yxat yoki manifestlar o'zgarganda sinov jurnalini NX-6 dalillar to'plamiga biriktiring.

### 2.2 CLI parchalari

- `cargo run -p iroha_cli -- manifest cbdc --uaid <hash> --dataspace 11 --template cbdc_wholesale` orqali UAID + manifest skeletini yarating.
- `iroha app space-directory manifest publish --manifest cbdc_wholesale.manifest.to` (yoki `--manifest-json cbdc_wholesale.manifest.json`) yordamida Torii (Space Directory) uchun imkoniyatlar manifestini nashr qilish; yuboruvchi hisobda CBDC ma'lumotlar maydoni uchun `CanPublishSpaceDirectoryManifest` bo'lishi kerak.
- Agar operatsiya stoli masofaviy avtomatlashtirishni ishga tushirsa, HTTP orqali nashr qiling:

  ```bash
  curl -X POST https://torii.soranexus/v1/space-directory/manifests \
       -H 'Content-Type: application/json' \
       -d '{
            "authority": "<i105-account-id>",
            "private_key": "ed25519:CiC7…",
            "manifest": '"'"'$(cat fixtures/space_directory/capability/cbdc_wholesale.manifest.json)'"'"',
            "reason": "CBDC onboarding wave 4"
          }'
  ```

  Torii nashr qilish tranzaktsiyasi navbatga qo'yilgandan so'ng `202 Accepted`ni qaytaradi; xuddi shunday
  CIDR/API-token shlyuzlari qo'llaniladi va zanjirdagi ruxsat talabi mos keladi
  CLI ish jarayoni.
- Favqulodda bekor qilish Torii manziliga POST yuborish orqali masofadan turib berilishi mumkin:

  ```bash
  curl -X POST https://torii.soranexus/v1/space-directory/manifests/revoke \
       -H 'Content-Type: application/json' \
       -d '{
            "authority": "<i105-account-id>",
            "private_key": "ed25519:CiC7…",
            "uaid": "uaid:0f4d…ab11",
            "dataspace": 11,
            "revoked_epoch": 9216,
            "reason": "Fraud investigation #NX-16-R05"
          }'
  ```

  Torii bekor qilish tranzaktsiyasi navbatga qo'yilgandan so'ng `202 Accepted` qaytaradi; xuddi shunday
  CIDR/API-token shlyuzlari boshqa ilova soʻnggi nuqtalari sifatida qoʻllaniladi va `CanPublishSpaceDirectoryManifest`
  hali ham zanjirda talab qilinadi.
- Oq roʻyxatga aʼzolikni aylantiring: `cbdc.manifest.json` tahrirlang, `activation_epoch` ga teging va xavfsiz nusxasi orqali barcha validatorlarga qayta joylang; `LaneManifestRegistry` sozlangan soʻrov oraligʻida qayta yuklanadi.

## 3. Muvofiqlik dalillari to'plami

Artefaktlarni `artifacts/nexus/cbdc_rollouts/<YYYYMMDDThhmmZ>/` ostida saqlang va dayjestni boshqaruv chiptasiga ilova qiling.

| Fayl | Tavsif |
|------|-------------|
| `cbdc.manifest.json` | Oq ro'yxat farqi bilan imzolangan chiziq manifest (oldin/keyin). |
| `capability/<uaid>.manifest.json` & `.to` | Norito + JSON qobiliyati har bir UAID uchun namoyon bo'ladi. |
| `compliance/kyc_<bank>.pdf` | Regulyatorga tegishli KYC attestatsiyasi. |
| `metrics/nexus_scheduler_lane_teu_capacity.prom` | Prometheus qirib tashlash TEU bo'sh joyini isbotlaydi. |
| `tests/cargo_test_nexus_lane_registry.log` | Manifest sinovidan tizimga kiring. |
| `tests/cargo_test_scheduler_teu.log` | TEU marshrutini tasdiqlovchi jurnal. |
| `programmable_money/axt_replay.json` | Dasturlashtiriladigan pul bilan o'zaro muvofiqligini ko'rsatadigan transkriptni takrorlang (4-bo'limga qarang). |
| `approvals/governance_minutes.md` | Manifest xesh + faollashtirish davriga oid imzolangan tasdiqlash bayonnomalari. |**Tasdiqlash skripti:** `ci/check_cbdc_rollout.sh` ni ishga tushiring va boshqaruv chiptasiga biriktirishdan oldin dalillar to‘plami tugallanganligini tasdiqlang. Yordamchi har bir `artifacts/nexus/cbdc_rollouts/` (yoki `CBDC_ROLLOUT_BUNDLE=<path>`) ni har bir `cbdc.manifest.json` uchun skanerlaydi, manifest/kompozitsiya guruhini tahlil qiladi, har bir qobiliyat manifestida mos keladigan `.to` fayli borligini tekshiradi, `.to` faylini tekshiradi, Prometheus testlarini tekshiradi metrics scrape plus `cargo test` jurnallari, dasturlashtiriladigan pulli qayta ijro JSONni tasdiqlaydi va tasdiqlash daqiqalarida faollashtirish davri va manifest xesh koʻrsatilganligini taʼminlaydi. Eng so'nggi versiya NX-6 da ko'rsatilgan xavfsizlik relslarini ham qo'llaydi: kvorum e'lon qilingan validatorlar to'plamidan oshmasligi kerak, himoyalangan nomlar bo'shliqlari bo'sh bo'lmagan satrlar bo'lishi kerak va qobiliyat manifestlari monoton ravishda o'sib borayotgan `activation_epoch`/`expiry_epoch` shaklga ega bo'lishi kerak. `Allow`/`Deny` effektlari. `fixtures/nexus/cbdc_rollouts/` ostida namunaviy dalillar to'plami `integration_tests/tests/nexus/cbdc_rollout_bundle.rs` integratsiya testi orqali amalga oshiriladi, bunda validator CI ga ulangan.

## 4. Dasturlashtiriladigan-pul o'zaro muvofiqligi

Bank (11-ma'lumotlar maydoni) va chakana dApp (12-ma'lumotlar maydoni) ikkalasi ham bir xil `ComposabilityGroupId` ro'yxatiga kiritilgandan so'ng, dasturlashtiriladigan pul oqimlari `docs/source/nexus.md` §8.6 dan AXT sxemasiga amal qiladi:

1. Chakana savdo dApp o'zining UAID + AXT dayjestiga bog'langan aktivlar boshqaruvini so'raydi. CBDC yo'li tutqichni `AssetPermissionManifest::evaluate` orqali tekshiradi (g'alabalarni rad etish, nafaqalar majburiy).
2. Ikkala DS ham bir xil birikma guruhini e'lon qiladi, shuning uchun marshrutlash ularni atomik qo'shilish uchun CBDC qatoriga tushiradi (`LaneRoutingPolicy` o'zaro oq ro'yxatga kiritilganda `group_id` dan foydalanadi).
3. Amalga oshirish jarayonida CBDC DS o'z sxemasi ichida AML/KYC isbotlarini (`use_asset_handle` psevdokodi `nexus.md` da) amalga oshiradi, dApp DS esa CBDC fragmenti muvaffaqiyatli bo'lgandan keyingina mahalliy biznes holatini yangilaydi.
4. Tasdiqlovchi material (FASTPQ + DA majburiyatlari) CBDC chizig'i bilan chegaralanib qoladi; birlashma kitobi yozuvlari shaxsiy ma'lumotlarni sizdirmasdan global holatni deterministik saqlaydi.

Dasturlashtiriladigan pulni takrorlash arxivi quyidagilarni o'z ichiga olishi kerak:

- AXT deskriptori + so'rov/javoblarni boshqarish.
- Norito kodli tranzaksiya konverti.
- Olingan tushumlar (baxtli yo'l, yo'lni rad etish).
- `telemetry::fastpq.execution_mode`, `nexus_scheduler_lane_teu_slot_committed` va `lane_commitments` uchun telemetriya parchalari.

## 5. Observability & Runbooks

- **Metrikalar:** Monitor `nexus_scheduler_lane_teu_capacity`, `nexus_scheduler_lane_teu_slot_committed`, `nexus_scheduler_lane_teu_deferral_total{reason}`, `governance_manifest_admission_total` va `lane_governance_sealed_total` (qarang: `docs/source/telemetry.md`).
- **Boshqaruv paneli:** `docs/source/grafana_scheduler_teu.json` ni CBDC qatori bilan kengaytiring; oq ro'yxatni buzish (har bir faollashtirish davrida izohlar) va imkoniyatlarning amal qilish muddati tugashi uchun panellarni qo'shing.
- **Ogohlantirishlar:** `rate(nexus_scheduler_lane_teu_deferral_total{reason="cap_exceeded"}[5m]) > 0` 15 daqiqa davomida yoki `lane_governance.manifest_ready=false` bir so‘rov oralig‘idan ortiq davom etganda ishga tushadi.
- **Runbook koʻrsatkichlari:** `docs/source/governance_api.md` da himoyalangan nomlar maydoni boʻyicha koʻrsatmalarga havola va `docs/source/nexus.md` da dasturlashtiriladigan pul muammolarini bartaraf etish.

## 6. Qabul qilish bo'yicha nazorat ro'yxati- [ ] CBDC liniyasi `nexus.lane_catalog` da e'lon qilingan, TEU testlariga mos keladigan TEU metama'lumotlari bilan.
- [ ] Imzolangan `cbdc.manifest.json` manifest katalogida mavjud, `cargo test -p integration_tests nexus::lane_registry` orqali tasdiqlangan.
- [ ] Har bir UAID uchun berilgan va kosmik katalogda saqlanadigan qobiliyat manifestlari; birlik testlari (`crates/iroha_data_model/src/nexus/manifest.rs`) orqali tasdiqlangan ustunlikni rad etish/ruxsat berish.
- [ ] Oq roʻyxat faollashuvi boshqaruv tasdiqlash identifikatori, `activation_epoch` va Prometheus dalillari bilan qayd etilgan.
- [ ] Dasturlashtiriladigan pulni qayta o'ynash arxivlangan, dastani chiqarish, rad etish va oqimlarga ruxsat berishni ko'rsatadi.
- [ ] NX-6 ni 🈯 dan 🈺 gacha tugatgandan soʻng boshqaruv chiptasi va `status.md` dan bogʻlangan kriptografik dayjest bilan yuklangan dalillar toʻplami.

Ushbu oʻyin kitobidan soʻng NX-6 uchun yetkazib beriladigan hujjatlarga javob beradi va CBDC yoʻlak konfiguratsiyasi, oq roʻyxatga kirish va dasturlashtiriladigan pul bilan oʻzaro ishlash uchun deterministik shablonni taqdim etish orqali kelajakdagi yoʻl xaritasi elementlarini (NX-12/NX-15) blokdan chiqaradi.
