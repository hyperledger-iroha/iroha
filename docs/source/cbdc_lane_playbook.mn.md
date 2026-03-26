---
lang: mn
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

> **Замын зургийн холболт:** NX-6 (CBDC хувийн эгнээний загвар ба цагаан жагсаалтын урсгал) болон NX-14 (Nexus runbooks).  
> **Эзэмшигчид:** Санхүүгийн үйлчилгээний АЖ, Nexus Үндсэн АЖ, Дагаж мөрдөх АЖ.  
> ** Статус:** Төсөл боловсруулах — хэрэгжүүлэх дэгээ `crates/iroha_data_model::nexus`, `crates/iroha_core::governance::manifest`, `integration_tests/tests/nexus/lane_registry.rs` дээр байгаа боловч CBDC-д зориулсан манифестууд, цагаан жагсаалтууд болон операторын runbook-ууд дутуу байсан. Энэхүү тоглоомын дэвтэр нь лавлагааны тохиргоо болон залгах ажлын урсгалыг баримтжуулсан тул CBDC-ийн байршуулалтыг тодорхой үргэлжлүүлэх боломжтой.

## Хамрах хүрээ ба үүрэг

- **Төв банкны эгнээ ("CBDC lane"):** Зөвшөөрөгдсөн баталгаажуулагч, кастодиал төлбөр тооцооны буфер болон програмчлагдсан мөнгөний бодлого. Хязгаарлагдмал өгөгдлийн орон зай + өөрийн удирдлагын манифест бүхий эгнээний хос хэлбэрээр ажилладаг.
- **Бөөний/жижиглэн худалдааны банкны мэдээллийн орон зай:** UAID эзэмшдэг, чадварын манифест хүлээн авдаг, CBDC эгнээ бүхий атомын AXT-д зөвшөөрөгдсөн жагсаалтад орсон оролцогч DS.
- **Програмчлагдах мөнгөний dApps:** CBDC-г ашигладаг гадаад DS нь цагаан жагсаалтад орсны дараа `ComposabilityGroup` чиглүүлэлтээр дамждаг.
- **Засаглал ба дагаж мөрдөх:** Парламент (эсвэл түүнтэй адилтгах модуль) эгнээний манифест, чадварын манифест, зөвшөөрөгдсөн жагсаалтын өөрчлөлтийг баталдаг; нийцэл нь Norito манифестийн зэрэгцээ нотлох баримтуудыг хадгалдаг.

** хамаарал**

1. Lane каталог + өгөгдлийн орон зайн каталогийн утас (`docs/source/nexus_lanes.md`, `defaults/nexus/config.toml`).
2. Эгнээний манифестийн хэрэгжилт (`crates/iroha_core/src/governance/manifest.rs`, `crates/iroha_core/src/queue.rs` дахь дарааллын хаалга).
3. Чадамжийн илрэл + UAIDs (`crates/iroha_data_model/src/nexus/manifest.rs`).
4. Төлөөлөгчийн TEU квот + хэмжүүр (`integration_tests/tests/scheduler_teu.rs`, `docs/source/telemetry.md`).

## 1. Лавлах эгнээний зохион байгуулалт

### 1.1 Lane каталог ба өгөгдлийн орон зайн оруулгууд

`[[nexus.lane_catalog]]` болон `[[nexus.dataspace_catalog]]`-д зориулалтын оруулгуудыг нэмнэ үү. Доорх жишээнд `defaults/nexus/config.toml`-ийг CBDC эгнээгээр өргөтгөж, нэг слот бүрт 1500 TEU нөөцөлж, өлсгөлөнг зургаан слот хүртэл бууруулж, бөөний банкууд болон жижиглэн худалдааны хэтэвчүүдэд тохирох дата зайны өөр нэрүүдийг оруулсан болно.

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

**Тэмдэглэл**

- `metadata.scheduler.teu_capacity` болон `metadata.scheduler.starvation_bound_slots` нь `integration_tests/tests/scheduler_teu.rs`-ийн хэрэгжүүлсэн TEU хэмжигчийг тэжээдэг. `nexus_scheduler_lane_teu_capacity` загвартай таарч байхын тулд операторууд тэдгээрийг хүлээн авах үр дүнтэй синхрончлох ёстой.
- Дээрх өгөгдлийн орон зайн бусад нэр бүр засаглалын манифест болон чадамжийн манифест (доороос харна уу) гарч ирэх ёстой тул элсэлт автоматаар дрифтээс татгалздаг.

### 1.2 Замын манифест араг яс

Lane манифестууд нь `nexus.registry.manifest_directory` (`crates/iroha_config/src/parameters/actual.rs`-г үзнэ үү)-ээр тохируулсан лавлах дор амьдардаг. Файлын нэр нь эгнээний өөр нэртэй тохирч байх ёстой (`cbdc.manifest.json`). Уг схем нь `integration_tests/tests/nexus/lane_registry.rs` дээрх засаглалын манифест тестийг тусгадаг.

```json
{
  "lane": "cbdc",
  "dataspace": "cbdc.core",
  "version": 1,
  "governance": "central_bank_multisig",
  "validators": [
    "<i105-account-id>",
    "<i105-account-id>",
    "<i105-account-id>",
    "<i105-account-id>"
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

Гол шаардлага:- Баталгаажуулагчид **заавал** нь каталогт байгаа i105 дансны стандарт ID (`@domain` байхгүй; `@domain`-г зөвхөн тодорхой чиглүүлэлтийн сануулга болгон нэмнэ үү) байх ёстой. `quorum`-г multisig босго (≥2) болгож тохируулна уу.
- Хамгаалагдсан нэрийн орон зайг `Queue::push` (`crates/iroha_core/src/queue.rs`-ыг үзнэ үү) хэрэгжүүлдэг тул бүх CBDC гэрээнд `gov_namespace` + `gov_contract_id` заах ёстой.
- `composability_group` талбарууд нь `docs/source/nexus.md` §8.6-д тодорхойлсон схемийн дагуу байна; эзэмшигч (CBDC lane) нь цагаан жагсаалт болон квотыг нийлүүлдэг. Зөвшөөрөгдсөн жагсаалтад орсон DS манифест нь зөвхөн `group_id_hex` + `activation_epoch`-г зааж өгдөг.
- Манифестыг хуулж авсны дараа `LaneManifestRegistry::from_config` ачаалж байгааг баталгаажуулахын тулд `cargo test -p integration_tests nexus::lane_registry -- --nocapture` ажиллуулна уу.

### 1.3 Чадамжийн илрэл (UAID бодлого)

Чадамжийн манифестууд (`crates/iroha_data_model/src/nexus/manifest.rs`-д `AssetPermissionManifest`) `UniversalAccountId`-ийг детерминистик нэмэлтүүдтэй холбодог. Банкууд болон dApp-ууд гарын үсэг зурсан бодлогыг татаж авах боломжтой болохын тулд тэдгээрийг Сансрын лавлахаар дамжуулан нийтлээрэй.

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

- Зөвшөөрөгдсөн дүрмүүд (`ManifestVerdict::Denied`) таарч байсан ч үгүйсгэх дүрэм ялна, тиймээс бүх тодорхой үгүйсгэлийг холбогдох зөвшөөрлийн дараа байрлуулна уу.
- Атомын төлбөрийн бариулын хувьд `AllowanceWindow::PerSlot`, хэрэглэгчийн хязгаарлалтын хувьд `PerMinute`/`PerDay` ашиглана уу.
- UAID/өгөгдлийн орон зайд нэг манифест хангалттай; идэвхжүүлэлт болон хугацаа нь бодлогын эргэлтийн хэмжүүрийг мөрддөг.
- Одоо `expiry_epoch`-д хүрмэгц эгнээний ажиллах хугацаа автоматаар дуусна.
  Тиймээс үйл ажиллагааны багууд зүгээр л `SpaceDirectoryEvent::ManifestExpired`-г хянадаг.
  `nexus_space_directory_revision_total` дельтаг архивлаж, Torii шоунуудыг шалгана уу
  `status = "Expired"`. CLI `manifest expire` командыг ашиглах боломжтой хэвээр байна
  гараар хүчингүй болгох эсвэл нотлох баримтыг дүүргэх.

## 2. Банкны элсэлт ба цагаан жагсаалтын ажлын урсгал

| Үе шат | Эзэмшигч(үүд) | Үйлдлүүд | Нотлох баримт |
|-------|----------|---------|----------|
| 0. Хэрэглэх | CBDC PMO | KYC файл, техникийн DS манифест, баталгаажуулагчийн жагсаалт, UAID зураглалыг цуглуул. | Хүлээн авах тасалбар, гарын үсэг зурсан DS манифест ноорог. |
| 1. Засаглалын батлах | Парламент / Дагаж мөрдөх | Хэрэглээний багцыг хянаж, `cbdc.manifest.json` тэмдэг, `AssetPermissionManifest`-г баталгаажуулна уу. | Гарын үсэг зурсан засаглалын протокол, манифест commit hash. |
| 2. Чадвар олгох | CBDC lane ops | `norito::json::to_string_pretty`-ээр дамжуулан манифестуудыг кодлох, Сансрын лавлах дор хадгалах, операторуудад мэдэгдэнэ. | Манифест JSON + norito `.to` файл, BLAKE3 дижест. |
| 3. Цагаан жагсаалтыг идэвхжүүлэх | CBDC lane ops | DSID-г `composability_group.whitelist`-д хавсаргаж, `activation_epoch`-ийг цохиж, манифестийг тараах; шаардлагатай бол өгөгдлийн орон зайн чиглүүлэлтийн шинэчлэлт. | Манифестын ялгаа, `kagami config diff` гаралт, засаглалын зөвшөөрлийн ID. |
| 4. Өргөдлийг баталгаажуулах | QA Guild / Ops | Интеграцийн туршилт, TEU ачааллын тест, программчлагдах мөнгөний дахин тоглуулах (доороос үзнэ үү). | `cargo test` бүртгэлүүд, TEU хяналтын самбарууд, програмчлагдсан мөнгөний бэхэлгээний үр дүн. |
| 5. Нотлох баримтын архив | Дагаж мөрдөх АХ | `artifacts/nexus/cbdc_<stamp>/`-ийн дагуу багцын манифест, зөвшөөрөл, чадавхи, туршилтын гаралт, Prometheus хусалт. | Нотлох баримт tarball, шалгах файл, зөвлөлийн гарын үсэг. |

### Аудитын багцын туслахСансрын лавлахаас `iroha app space-directory manifest audit-bundle` туслагчийг ашиглана уу
нотлох баримтыг илгээхээс өмнө боломж бүрийг агшин зуурын зураг авах тоглоомын ном.
Манифест JSON (эсвэл `.to` ачаалал) болон дата зайны профайлыг өгнө үү:

```bash
iroha app space-directory manifest audit-bundle \
  --manifest-json fixtures/space_directory/capability/cbdc_wholesale.manifest.json \
  --profile fixtures/space_directory/profile/cbdc_lane_profile.json \
  --out-dir artifacts/nexus/cbdc/2026-02-01T00-00Z/cbdc_wholesale \
  --notes "CBDC wholesale refresh"
```

Энэ тушаал нь каноник JSON/Norito/хэш хуулбарыг хажуугаар нь гаргадаг.
`audit_bundle.json`, UAID, өгөгдлийн орон зайны ID, идэвхжүүлэлт/хугацаа дуусах хугацааг бүртгэдэг
эрин үе, манифест хэш, профайлын аудитын дэгээг ашиглан шаардлагатайг хэрэгжүүлэхийн зэрэгцээ
`SpaceDirectoryEvent` захиалга. Багцыг нотлох баримт дотор хая
аудиторууд болон зохицуулагчид дараа нь яг байтыг дахин тоглуулах боломжтой.

### 2.1 Команд ба баталгаажуулалт

1. **Лайн манифестууд:** `cargo test -p integration_tests nexus::lane_registry -- --nocapture lane_manifest_registry_loads_fixture_manifests`.
2. **Хуваарьлагчийн квот:** `cargo test -p integration_tests scheduler_teu -- queue_teu_backlog_matches_metering queue_routes_transactions_across_configured_lanes`.
3. **Гараар утаа:** CBDC файлууд руу чиглэсэн манифест лавлах `irohad --sora --config configs/soranexus/nexus/config.toml --chain 0000…`, дараа нь `/v1/sumeragi/status` дарж, CBDC эгнээний `lane_governance.manifest_ready=true`-г шалгана уу.
4. **Цагаан жагсаалтын тууштай байдлын тест:** `cargo test -p integration_tests nexus::cbdc_whitelist -- --nocapture` `integration_tests/tests/nexus/cbdc_whitelist.rs` дасгалууд, `fixtures/space_directory/profile/cbdc_lane_profile.json`-г задлан шинжилж, лавлагаалагдсан чадавхи нь цагаан жагсаалтын оролт бүрийн UAID, өгөгдлийн зай, идэвхжүүлэх үе болон I180000000000000000000000000000000000000000 зөвшөөрөгдөх жагсаалттай тохирч байгаа эсэхийг шалгах боломжтой. `fixtures/space_directory/capability/` доор. Цагаан жагсаалт эсвэл манифест өөрчлөгдөх бүрт туршилтын бүртгэлийг NX-6 нотлох баримтын багцад хавсаргана уу.

### 2.2 CLI хэсэг

- `cargo run -p iroha_cli -- manifest cbdc --uaid <hash> --dataspace 11 --template cbdc_wholesale`-ээр дамжуулан UAID + манифест араг ясыг үүсгэнэ.
- `iroha app space-directory manifest publish --manifest cbdc_wholesale.manifest.to` (эсвэл `--manifest-json cbdc_wholesale.manifest.json`) ашиглан Torii (Space Directory) дээр боломжийн манифест нийтлэх; илгээж буй данс нь CBDC дата зайнд `CanPublishSpaceDirectoryManifest`-тэй байх ёстой.
- Үйл ажиллагааны ширээ алсын автоматжуулалтыг ажиллуулж байгаа бол HTTP-ээр нийтлэх:

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

  Torii нь нийтлэх гүйлгээг дараалалд оруулсны дараа `202 Accepted`-г буцаана; адилхан
  CIDR/API токен хаалганууд хамаарах ба гинжин хэлхээний зөвшөөрлийн шаардлага нь дараахтай тохирч байна
  CLI ажлын урсгал.
- Яаралтай цуцлалтыг Torii дугаарт POST хийж алсаас гаргаж болно:

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

  Torii нь хүчингүй болгох гүйлгээг дараалалд оруулсны дараа `202 Accepted`-г буцаана; адилхан
  CIDR/API токен хаалганууд нь бусад програмын төгсгөлийн цэгүүдийн нэгэн адил хэрэгждэг ба `CanPublishSpaceDirectoryManifest`
  гинжин хэлхээнд шаардлагатай хэвээр байна.
- Зөвшөөрөгдсөн жагсаалтын гишүүнчлэлийг эргүүлэх: `cbdc.manifest.json`-г засварлаж, `activation_epoch`-ийг дарж, аюулгүй хуулбараар бүх баталгаажуулагч руу дахин байршуулах; `LaneManifestRegistry` нь тохируулсан санал асуулгын интервал дээр дахин ачаалагдана.

## 3. Нийцлийн нотлох баримтын багц

Олдворуудыг `artifacts/nexus/cbdc_rollouts/<YYYYMMDDThhmmZ>/`-ийн доор хадгалж, удирдлагын тасалбарт хураангуйг хавсаргана уу.

| Файл | Тодорхойлолт |
|------|-------------|
| `cbdc.manifest.json` | Цагаан жагсаалтын ялгаа бүхий гарын үсэг зурсан эгнээний манифест (өмнө/дараа). |
| `capability/<uaid>.manifest.json` & `.to` | UAID бүрийн хувьд Norito + JSON чадвар илэрдэг. |
| `compliance/kyc_<bank>.pdf` | Зохицуулагчтай холбоотой KYC гэрчилгээ. |
| `metrics/nexus_scheduler_lane_teu_capacity.prom` | Prometheus TEU өндөр зайг нотлох хусах. |
| `tests/cargo_test_nexus_lane_registry.log` | Манифест туршилтын гүйлтээс нэвтэрч орох. |
| `tests/cargo_test_scheduler_teu.log` | TEU чиглүүлэлтийн дамжуулалтыг нотлох бүртгэл. |
| `programmable_money/axt_replay.json` | Программчлагдах боломжтой мөнгөний харилцан үйлчлэлийг харуулсан хуулбарыг дахин тоглуулах (4-р хэсгийг үзнэ үү). |
| `approvals/governance_minutes.md` | Манифест хэш + идэвхжүүлэх үеийг харуулсан гарын үсэгтэй зөвшөөрлийн протокол. |**Баталгаажуулалтын скрипт:** засаглалын тасалбарт хавсаргахаасаа өмнө нотлох баримтыг бүрэн гүйцэд гэж баталгаажуулахын тулд `ci/check_cbdc_rollout.sh`-г ажиллуул. Туслагч нь `artifacts/nexus/cbdc_rollouts/` (эсвэл `CBDC_ROLLOUT_BUNDLE=<path>`) `cbdc.manifest.json` бүрийг сканнердаж, манифест/композиторын бүлгийг задлан шинжилж, чадварын манифест бүрд тохирох `.to` файл байгаа эсэхийг шалгаж, `.to` файл байгаа эсэхийг шалгаж, `.to` дээр Prometheus тестийг шалгана. metrics scrape дээр нэмэх нь `cargo test` логууд, программчлагдах мөнгөний дахин тоглуулах JSON-г баталгаажуулж, зөвшөөрлийн минутанд идэвхжүүлэх үе болон манифест хэшийг иш татдаг. Хамгийн сүүлийн үеийн хувилбар нь NX-6-д дурдсан аюулгүй байдлын шугамыг мөн хэрэгжүүлдэг: чуулга нь зарласан баталгаажуулагчийн багцаас хэтэрч болохгүй, хамгаалагдсан нэрийн орон зай нь хоосон биш мөр байх ёстой бөгөөд чадварын манифестууд нь нэг хэвийн байдлаар нэмэгдэж буй `activation_epoch`/`expiry_epoch` хэлбэрийг тунхаглах ёстой. `Allow`/`Deny` нөлөө. `fixtures/nexus/cbdc_rollouts/`-ийн дагуу нотлох баримтын дээжийг `integration_tests/tests/nexus/cbdc_rollout_bundle.rs` интеграцийн тестээр гүйцэтгэж, баталгаажуулагчийг CI-д холбосон хэвээр байна.

## 4. Програмчлагдах-Мөнгөний харилцан ажиллах чадвар

Банк (өгөгдлийн орон зай 11) болон жижиглэн худалдааны dApp (өгөгдлийн орон зай 12) хоёулаа ижил `ComposabilityGroupId` жагсаалтад орсон бол программчлагдах мөнгөний урсгал нь `docs/source/nexus.md` §8.6-ийн AXT загварыг дагана:

1. Жижиглэнгийн dApp нь өөрийн UAID + AXT хураангуйд холбогдсон хөрөнгийн зохицуулалтыг хүсдэг. CBDC эгнээ нь бариулыг `AssetPermissionManifest::evaluate`-ээр баталгаажуулдаг (хожлыг үгүйсгэх, тэтгэмжийг хэрэгжүүлэх).
2. DS хоёулаа нэгтгэх чадварын ижил бүлгийг зарладаг тул чиглүүлэлт нь тэдгээрийг атом оруулах зорилгоор CBDC эгнээнд буулгадаг (`LaneRoutingPolicy` нь харилцан зөвшөөрөгдсөн жагсаалтад орсон үед `group_id`-г ашигладаг).
3. Гүйцэтгэх явцад CBDC DS нь хэлхээн дотроо AML/KYC нотолгоог хэрэгжүүлдэг (`use_asset_handle` псевдокод `nexus.md`), харин dApp DS нь зөвхөн CBDC хэсэг амжилттай болсны дараа л орон нутгийн бизнесийн төлөвийг шинэчилдэг.
4. Баталгаажуулах материал (FASTPQ + DA амлалтууд) нь CBDC эгнээнд хязгаарлагдах болно; merge-ledger оруулгууд нь хувийн мэдээллийг алдагдуулахгүйгээр дэлхийн төлөв байдлыг тодорхойлогч хэвээр байлгадаг.

Програмчлагдах мөнгөний дахин тоглуулах архив нь дараахь зүйлийг агуулна.

- AXT тодорхойлогч + хүсэлт/хариултуудыг зохицуулах.
- Norito кодлогдсон гүйлгээний дугтуй.
- Үр дүнгийн баримтууд (аз жаргалтай зам, замыг үгүйсгэх).
- `telemetry::fastpq.execution_mode`, `nexus_scheduler_lane_teu_slot_committed`, `lane_commitments`-д зориулсан телеметрийн хэсгүүд.

## 5. Ажиглах боломжтой байдал ба Runbooks

- **Хэмжүүр:** Хяналт `nexus_scheduler_lane_teu_capacity`, `nexus_scheduler_lane_teu_slot_committed`, `nexus_scheduler_lane_teu_deferral_total{reason}`, `governance_manifest_admission_total`, `lane_governance_sealed_total` (`docs/source/telemetry.md`-ийг үзнэ үү).
- **Хяналтын самбар:** `docs/source/grafana_scheduler_teu.json`-г CBDC эгнээний эгнээгээр сунгах; Зөвшөөрөгдсөн жагсаалтын гацалт (идэвхжүүлэлтийн үе болгонд тэмдэглэгээ хийх) болон хүчин чадал дуусах шугам хоолойнуудыг нэмэх.
- **Сэрэмжлүүлэг:** `rate(nexus_scheduler_lane_teu_deferral_total{reason="cap_exceeded"}[5m]) > 0` 15 минутын турш эсвэл `lane_governance.manifest_ready=false` санал асуулгын нэг интервалаас хэтэрсэн үед гох.
- **Runbook заагч:** `docs/source/governance_api.md` дээрх хамгаалагдсан нэрийн зайны заавар, `docs/source/nexus.md` дээрх програмчлагдсан мөнгөний алдааг олж засварлах холбоос.

## 6. Хүлээн авах хяналтын хуудас- [ ] CBDC зурвасыг `nexus.lane_catalog`-д TEU-ийн туршилтад нийцэх TEU мета өгөгдлийн хамт зарласан.
- [ ] `cargo test -p integration_tests nexus::lane_registry`-ээр баталгаажуулсан, манифестын лавлахад `cbdc.manifest.json` гарын үсэг зурсан.
- [ ] UAID бүрт гаргаж, Сансрын лавлахад хадгалагдсан чадварын манифест; Нэгжийн туршилтаар баталгаажуулсан давуу эрхийг үгүйсгэх/зөвшөөрөх (`crates/iroha_data_model/src/nexus/manifest.rs`).
- [ ] Цагаан жагсаалтын идэвхжүүлэлтийг засаглалын зөвшөөрлийн ID, `activation_epoch`, Prometheus нотлох баримтаар бүртгэсэн.
- [ ] Программчлагдах мөнгөний давталтыг архивласан, бариул гаргах, үгүйсгэх, урсгалыг зөвшөөрөх зэргийг харуулсан.
- [ ] NX-6-г 🈯-с 🈺 хүртэл төгсөхөд засаглалын тасалбар болон `status.md`-аас холбосон криптограф тоймтой хамт байршуулсан нотлох баримтын багц.

Энэхүү тоглоомын дэвтрийг дагаснаар NX-6-д хүргэх баримт бичигт нийцэж, CBDC-ийн эгнээний тохиргоо, цагаан жагсаалтад бүртгүүлэх, программчлагдах мөнгө-мөнгөний харилцан үйлчлэлд зориулсан тодорхой загвараар хангаснаар ирээдүйн замын зураглалын зүйлсийн (NX-12/NX-15) блокуудыг арилгана.