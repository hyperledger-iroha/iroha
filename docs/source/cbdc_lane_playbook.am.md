---
lang: am
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

# ሲቢሲሲ የግል ሌይን መጫወቻ መጽሐፍ (NX-6)

> ** የመንገድ ካርታ ትስስር፡** NX-6 (CBDC የግል ሌይን አብነት እና የተፈቀደላቸው ዝርዝር ፍሰት) እና NX-14 (Nexus runbooks)።  
> ** ባለቤቶች፡** የፋይናንስ አገልግሎቶች WG፣ Nexus Core WG፣ Compliance WG።  
> ** ሁኔታ፡** መቅረጽ — የትግበራ መንጠቆዎች በ`crates/iroha_data_model::nexus`፣ `crates/iroha_core::governance::manifest`፣ እና `integration_tests/tests/nexus/lane_registry.rs` ላይ አሉ፣ ነገር ግን የCBC-ተኮር መግለጫዎች፣ የተፈቀደላቸው ዝርዝር እና ኦፕሬተር runbooks ጠፍተዋል። ይህ የመጫወቻ መጽሐፍ የማጣቀሻ ውቅረትን እና የመሳፈሪያ የስራ ፍሰትን ይመዘግባል ስለዚህ የCBC ማሰማራቶች በቆራጥነት ሊቀጥሉ ይችላሉ።

## ወሰን እና ሚናዎች

- **የማእከላዊ ባንክ መስመር ("CBDC ሌይን")፡** የተፈቀዱ አረጋጋጮች፣ የጥበቃ ማቋቋሚያ ቋት እና ፕሮግራም-ሊደረግ የሚችል የገንዘብ ፖሊሲዎች። እንደ የተገደበ የውሂብ ቦታ + የሌይን ጥንድ ከራሱ የአስተዳደር መግለጫ ጋር ይሰራል።
- ** የጅምላ/የችርቻሮ ባንክ መረጃ ቦታዎች፡** ተሳታፊ DS UAIDsን የሚይዝ፣ የችሎታ መግለጫዎችን የሚቀበል እና በCBC መስመር ላይ ለአቶሚክ AXT በተፈቀደላቸው ዝርዝር ውስጥ ሊገባ ይችላል።
- **ፕሮግራም-ገንዘብ dApps:** CBDCን የሚፈጅ ውጫዊ ዲኤስ አንድ ጊዜ በተፈቀደላቸው ዝርዝር ውስጥ በ`ComposabilityGroup` ማዞሪያ በኩል ይፈስሳል።
- ** አስተዳደር እና ተገዢነት፡** ፓርላማ (ወይም ተመጣጣኝ ሞጁል) የሌይን መግለጫዎችን፣ ችሎታዎችን ያሳያል እና የተፈቀደላቸው ዝርዝር ለውጦችን ያጸድቃል። ተገዢነት ከNorito መገለጫዎች ጎን ለጎን ማስረጃዎችን ያከማቻል።

** ጥገኛዎች ***

1. የሌይን ካታሎግ + የውሂብ ቦታ ካታሎግ የወልና (`docs/source/nexus_lanes.md`፣ `defaults/nexus/config.toml`)።
2. የሌይን አንጸባራቂ ማስፈጸሚያ (`crates/iroha_core/src/governance/manifest.rs`፣ ወረፋ በ`crates/iroha_core/src/queue.rs`)።
3. አቅምን ያሳያል + UAIDs (`crates/iroha_data_model/src/nexus/manifest.rs`)።
4. የጊዜ መርሐግብር አዘጋጅ TEU ኮታዎች + መለኪያዎች (`integration_tests/tests/scheduler_teu.rs`, `docs/source/telemetry.md`).

## 1. የማጣቀሻ ሌይን አቀማመጥ

### 1.1 ሌይን ካታሎግ እና የውሂብ ቦታ ግቤቶች

ለ`[[nexus.lane_catalog]]` እና `[[nexus.dataspace_catalog]]` የወሰኑ ግቤቶችን ያክሉ። ከታች ያለው ምሳሌ `defaults/nexus/config.toml` በሲቢሲሲ ሌይን በ 1500 TEU የሚይዝ እና ረሃብን እስከ ስድስት ቦታዎች የሚይዝ እና ለጅምላ ባንኮች እና የችርቻሮ ቦርሳዎች ተዛማጅ የዳታ ቦታ ተለዋጭ ስሞችን ያራዝመዋል።

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

**ማስታወሻዎች**

- `metadata.scheduler.teu_capacity` እና `metadata.scheduler.starvation_bound_slots` በ `integration_tests/tests/scheduler_teu.rs` የተለማመዱትን የ TEU መለኪያዎች ይመገባሉ። `nexus_scheduler_lane_teu_capacity` ከአብነት ጋር እንዲዛመድ ኦፕሬተሮች ከመቀበል ውጤቶች ጋር እንዲመሳሰሉ ማድረግ አለባቸው።
- ከላይ ያለው እያንዳንዱ የዳታ ስፔስ ተለዋጭ ስም በአስተዳደር መግለጫዎች እና ችሎታዎች ውስጥ መታየት አለበት (ከዚህ በታች ይመልከቱ) ስለዚህ መግባት በራስ-ሰር መንሸራተትን ውድቅ ያደርጋል።

### 1.2 ሌይን አንጸባራቂ አጽም

ሌይን በ`nexus.registry.manifest_directory` በኩል በተዋቀረው ማውጫ ስር በቀጥታ ያሳያል (`crates/iroha_config/src/parameters/actual.rs` ይመልከቱ)። የፋይል ስሞች ከሌይን ተለዋጭ ስሞች (`cbdc.manifest.json`) ጋር መመሳሰል አለባቸው። መርሃግብሩ በ`integration_tests/tests/nexus/lane_registry.rs` ውስጥ ያሉትን የአስተዳደር አንጸባራቂ ሙከራዎችን ያንጸባርቃል።

```json
{
  "lane": "cbdc",
  "dataspace": "cbdc.core",
  "version": 1,
  "governance": "central_bank_multisig",
  "validators": [
    "soraカタカナ...",
    "soraカタカナ...",
    "soraカタカナ...",
    "soraカタカナ..."
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

ቁልፍ መስፈርቶች፡- አረጋጋጮች ** አለባቸው** ቀኖናዊ የI105 መለያ መታወቂያዎች (አይ18NI00000043X የለም፤ ​​`@domain` እንደ ግልጽ የማዞሪያ ፍንጭ ብቻ አባሪ) በካታሎግ ውስጥ አለ። `quorum` ወደ ባለብዙ ሲግ ገደብ (≥2) አዘጋጅ።
- የተጠበቁ የስም ቦታዎች በ `Queue::push` (`crates/iroha_core/src/queue.rs` ይመልከቱ) ተፈጻሚዎች ናቸው, ስለዚህ ሁሉም የ CBDC ኮንትራቶች `gov_namespace` + `gov_contract_id` መግለጽ አለባቸው.
- `composability_group` መስኮች በ `docs/source/nexus.md` §8.6 ውስጥ የተገለጸውን እቅድ ይከተላሉ; ባለቤቱ (CBDC ሌን) የተፈቀደላቸው ዝርዝር እና ኮታዎችን ያቀርባል። የተፈቀደላቸው DS አንጸባራቂዎች `group_id_hex` + `activation_epoch` ብቻ ይጠቅሳሉ።
- አንጸባራቂውን ከገለበጡ በኋላ `LaneManifestRegistry::from_config` መጫኑን ለማረጋገጥ `cargo test -p integration_tests nexus::lane_registry -- --nocapture` ን ያሂዱ።

### 1.3 የአቅም መግለጫዎች (UAID ፖሊሲዎች)

አቅምን ያሳያል (`crates/iroha_data_model/src/nexus/manifest.rs` ውስጥ `crates/iroha_data_model/src/nexus/manifest.rs`) `UniversalAccountId` ከመወሰኛ አበል ጋር ያስራል። ባንኮች እና dApps የተፈረሙ ፖሊሲዎችን ማምጣት እንዲችሉ በጠፈር ማውጫ በኩል ያትሟቸው።

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

- የፈቃድ ህግ (`ManifestVerdict::Denied`) ቢሆንም ህጎችን መካድ ያሸንፋል፣ ስለዚህ አግባብነት ያለው ከፈቀደ በኋላ ሁሉንም ግልጽ ክዶ ያስቀምጡ።
- ለአቶሚክ ክፍያ መያዣዎች `AllowanceWindow::PerSlot` እና `PerMinute`/`PerDay` የደንበኛ ገደቦችን ይጠቀሙ።
- በ UAID/ዳታ ቦታ አንድ ነጠላ መግለጫ በቂ ነው; ማነቃቂያዎች እና የአገልግሎት ማብቂያዎች የፖሊሲ ማሽከርከር ችሎታን ያስገድዳሉ።
- የሌይኑ አሂድ ጊዜ `expiry_epoch` እንደደረሰ በራስ-ሰር ይገለጣል፣
  ስለዚህ የኦፕሬሽን ቡድኖች `SpaceDirectoryEvent::ManifestExpired` በቀላሉ ይቆጣጠራሉ ፣
  የ`nexus_space_directory_revision_total` ዴልታ በማህደር ያስቀምጡ እና የTorii ትርዒቶችን ያረጋግጡ
  `status = "Expired"`. የCLI `manifest expire` ትእዛዝ እንዳለ ይቆያል
  በእጅ መሻር ወይም ማስረጃ መልሶ መሙላት።

## 2. የባንክ ቦርዲንግ እና የተፈቀደላቸው የስራ ፍሰት

| ደረጃ | ባለቤት(ዎች) | ድርጊቶች | ማስረጃ |
|-------------|--------|
| 0. ቅበላ | CBDC PMO | የKYC ዶሴ፣ ቴክኒካል DS መግለጫ፣ አረጋጋጭ ዝርዝር፣ የUAID ካርታ ስራ ይሰብስቡ። | የመግቢያ ትኬት፣ የተፈረመ የዲኤስ አንጸባራቂ ረቂቅ። |
| 1. አስተዳደር ማጽደቅ | ፓርላማ / ተገዢነት | የግምገማ ማስገቢያ ጥቅል፣ አጸፋዊ ምልክት `cbdc.manifest.json`፣ `AssetPermissionManifest` አጽድቋል። | የተፈረመ የአስተዳደር ደቂቃዎች፣ የሰነድ ማስረጃ ሰነድ ሃሽ። |
| 2. የአቅም መስጠት | CBDC መስመር ኦፕስ | ኢንኮድ በ`norito::json::to_string_pretty` በኩል ይገለጣል፣ በጠፈር ማውጫ ስር ያከማቹ፣ ኦፕሬተሮችን ያሳውቁ። | አንጸባራቂ JSON + norito `.to` ፋይል፣ BLAKE3 መፍጨት። |
| 3. የተፈቀደላቸው ዝርዝር ማግበር | CBDC መስመር ኦፕስ | DSIDን ወደ `composability_group.whitelist` አክል፣ ባምፕ `activation_epoch`፣ አንጸባራቂ አሰራጭ፤ አስፈላጊ ከሆነ የውሂብ ቦታ ማዘዋወርን ያዘምኑ። | የአንጸባራቂ ልዩነት፣ `kagami config diff` ውፅዓት፣ የአስተዳደር ማረጋገጫ መታወቂያ። |
| 4. የልቀት ማረጋገጫ | QA Guild / Ops | የውህደት ፈተናዎችን፣ የTEU ጭነት ሙከራዎችን እና ፕሮግራም-ገንዘብን እንደገና ማጫወት (ከዚህ በታች ይመልከቱ) ያሂዱ። | `cargo test` ምዝግብ ማስታወሻዎች፣ TEU ዳሽቦርዶች፣ በፕሮግራም ሊደረጉ የሚችሉ-ገንዘብ ቋሚ ውጤቶች። |
| 5. የማስረጃ መዝገብ | ተገዢነት WG | ቅርቅብ መግለጫዎች፣ ማጽደቆች፣ የችሎታ መፍጨት፣ የሙከራ ውጤቶች፣ እና Prometheus ቧጨራዎች በ`artifacts/nexus/cbdc_<stamp>/`። | የማስረጃ ታርቦል፣ የቼክሰም ፋይል፣ የምክር ቤት ማቋረጥ። |

### የኦዲት ጥቅል አጋዥየ`iroha app space-directory manifest audit-bundle` ረዳትን ከስፔስ ማውጫ ተጠቀም
የማስረጃ ፓኬጁን ከማቅረቡ በፊት እያንዳንዱን የችሎታ መግለጫ ለማንፀባረቅ playbook።
አንጸባራቂ JSON (ወይም `.to` ክፍያ) እና የውሂብ ቦታ መገለጫ ያቅርቡ፡

```bash
iroha app space-directory manifest audit-bundle \
  --manifest-json fixtures/space_directory/capability/cbdc_wholesale.manifest.json \
  --profile fixtures/space_directory/profile/cbdc_lane_profile.json \
  --out-dir artifacts/nexus/cbdc/2026-02-01T00-00Z/cbdc_wholesale \
  --notes "CBDC wholesale refresh"
```

ትዕዛዙ ቀኖናዊ JSON/Norito/hash ቅጂዎችን ከጎኑ ያወጣል።
`audit_bundle.json`፣ UAIDን፣ የውሂብ ቦታ መታወቂያን፣ ማግበር/ማብቂያውን ይመዘግባል
የሚፈለጉትን በሚያስፈጽሙበት ጊዜ ዘመን፣ የፕሮፋይል ሃሽ እና የመገለጫ ኦዲት መንጠቆዎች
`SpaceDirectoryEvent` የደንበኝነት ምዝገባዎች. ጥቅሉን በማስረጃው ውስጥ ይጣሉት
ማውጫ ስለዚህ ኦዲተሮች እና ተቆጣጣሪዎች ትክክለኛውን ባይት በኋላ እንደገና ማጫወት ይችላሉ።

### 2.1 ትዕዛዞች እና ማረጋገጫዎች

1. ** ሌይን ይገለጻል: ** `cargo test -p integration_tests nexus::lane_registry -- --nocapture lane_manifest_registry_loads_fixture_manifests`.
2. ** የጊዜ መርሐግብር ኮታዎች: ** `cargo test -p integration_tests scheduler_teu -- queue_teu_backlog_matches_metering queue_routes_transactions_across_configured_lanes`.
3. **የእጅ ጭስ፡** `irohad --sora --config configs/soranexus/nexus/config.toml --chain 0000…` ከማንፀባረቂያው ማውጫ ጋር ወደ CBDC ፋይሎች ይጠቁማል፣ከዚያ `/v1/sumeragi/status` ይምቱ እና `lane_governance.manifest_ready=true`ን ለCBDC መስመር ያረጋግጡ።
4. **የነጮች ዝርዝር ወጥነት ፈተና፡** `cargo test -p integration_tests nexus::cbdc_whitelist -- --nocapture` ልምምዶች `integration_tests/tests/nexus/cbdc_whitelist.rs`፣ `fixtures/space_directory/profile/cbdc_lane_profile.json` ን መተንተን እና የተጠቀሰው አቅም እያንዳንዱ የተፈቀደላቸው የመግቢያ UAID፣ የመረጃ ቦታ፣ የማግበር ዘመን እና የአበል ዝርዝር ከማንፀባረቂያው I18005 ኤንቲኤክስ0005 ጋር የሚዛመዱ መሆናቸውን ያረጋግጣል። `fixtures/space_directory/capability/`. የተፈቀደላቸው ዝርዝር ውስጥ ወይም በሚገለጥበት ጊዜ ሁሉ የሙከራ ምዝግብ ማስታወሻውን ከNX-6 ማስረጃ ጥቅል ጋር ያያይዙት።

### 2.2 CLI ቅንጥቦች

- UAID + አንጸባራቂ አጽም በ`cargo run -p iroha_cli -- manifest cbdc --uaid <hash> --dataspace 11 --template cbdc_wholesale` ይፍጠሩ።
- `iroha app space-directory manifest publish --manifest cbdc_wholesale.manifest.to` (ወይም `--manifest-json cbdc_wholesale.manifest.json`) በመጠቀም የችሎታ መግለጫን ለTorii (የጠፈር ማውጫ) ያትሙ። የማስረከቢያ መለያው ለCBC የመረጃ ቦታ `CanPublishSpaceDirectoryManifest` መያዝ አለበት።
- የ ops ዴስክ የርቀት አውቶማቲክን እያሄደ ከሆነ በኤችቲቲፒ ያትሙ፡-

  ```bash
  curl -X POST https://torii.soranexus/v1/space-directory/manifests \
       -H 'Content-Type: application/json' \
       -d '{
            "authority": "soraカタカナ...",
            "private_key": "ed25519:CiC7…",
            "manifest": '"'"'$(cat fixtures/space_directory/capability/cbdc_wholesale.manifest.json)'"'"',
            "reason": "CBDC onboarding wave 4"
          }'
  ```

  Torii የማተም ግብይቱ ከተሰለፈ `202 Accepted` ይመልሳል። ተመሳሳይ ነው።
  CIDR/API-token በሮች ይተገበራሉ እና በሰንሰለት ላይ ያለው የፍቃድ መስፈርት ከ ጋር ይዛመዳል
  CLI የስራ ፍሰት.
- የአደጋ መሻር በርቀት ወደ Torii በመለጠፍ ሊሰጥ ይችላል፡-

  ```bash
  curl -X POST https://torii.soranexus/v1/space-directory/manifests/revoke \
       -H 'Content-Type: application/json' \
       -d '{
            "authority": "soraカタカナ...",
            "private_key": "ed25519:CiC7…",
            "uaid": "uaid:0f4d…ab11",
            "dataspace": 11,
            "revoked_epoch": 9216,
            "reason": "Fraud investigation #NX-16-R05"
          }'
  ```

  Torii የመሻር ግብይቱ ከተሰለፈ `202 Accepted` ይመልሳል። ተመሳሳይ ነው።
  CIDR/API-token በሮች እንደ ሌሎች የመተግበሪያ የመጨረሻ ነጥቦች ይተገበራሉ፣ እና `CanPublishSpaceDirectoryManifest`
  አሁንም በሰንሰለት ላይ ያስፈልጋል።
- የተፈቀደላቸው መዝገብ አባልነቶችን አዙር፡ `cbdc.manifest.json` አርትዕ፣ `activation_epoch` አሽከርክር እና ደህንነቱ በተጠበቀ ቅጂ ለሁሉም አረጋጋጮች እንደገና ማሰማራት። `LaneManifestRegistry` ትኩስ-እንደገና በተዋቀረው የምርጫ ክፍተት ላይ።

## 3. የማክበር ማስረጃ ቅርቅብ

ቅርሶችን በ`artifacts/nexus/cbdc_rollouts/<YYYYMMDDThhmmZ>/` ስር ያከማቹ እና የምግብ መፍጫውን ከአስተዳደር ትኬት ጋር ያያይዙ።

| ፋይል | መግለጫ |
|-------------|
| `cbdc.manifest.json` | የተፈረመበት የሌይን አንጸባራቂ ከተፈቀደላቸው ዝርዝር ልዩነት (በፊት/በኋላ)። |
| `capability/<uaid>.manifest.json` & `.to` | Norito + JSON ችሎታ ለእያንዳንዱ UAID ይገለጣል። |
| `compliance/kyc_<bank>.pdf` | ተቆጣጣሪ ፊት ለፊት ያለው የKYC ማረጋገጫ። |
| `metrics/nexus_scheduler_lane_teu_capacity.prom` | Prometheus መቧጨር የTEU ዋና ክፍልን ያረጋግጣል። |
| `tests/cargo_test_nexus_lane_registry.log` | ከአንጸባራቂ የሙከራ ሩጫ ይመዝገቡ። |
| `tests/cargo_test_scheduler_teu.log` | የTEU ማዘዋወር ማለፉን የሚያረጋግጥ ምዝግብ ማስታወሻ። |
| `programmable_money/axt_replay.json` | የፕሮግራም-ገንዘብ መስተጋብርን የሚያሳይ ግልባጭ እንደገና ያጫውቱ (ክፍል 4 ይመልከቱ)። |
| `approvals/governance_minutes.md` | አንጸባራቂ hash + ገቢር ዘመንን የሚያመለክት የተፈረመ የማጽደቅ ደቂቃዎች። |** የማረጋገጫ ስክሪፕት፡** የማስረጃ ጥቅሉን ከአስተዳደር ትኬት ጋር ከማያያዝዎ በፊት መጠናቀቁን ለማረጋገጥ `ci/check_cbdc_rollout.sh` ያሂዱ። ረዳቱ `artifacts/nexus/cbdc_rollouts/` (ወይም `CBDC_ROLLOUT_BUNDLE=<path>`) ለእያንዳንዱ `cbdc.manifest.json` ይቃኛል፣ የማኒፌክት/የማጠናቀር ችሎታ ቡድኑን ይተነትናል፣ እያንዳንዱ የችሎታ ደብተር ተዛማጅ `.to` ፋይል፣ የPrometheus ማረጋገጫ ዩኤንአይኤንአይ000000tes ያረጋግጣል scrape plus `cargo test` ምዝግብ ማስታወሻዎች፣ የፕሮግራም-ገንዘብ መልሶ ማጫወት JSONን ያረጋግጣል፣ እና የፀደቁ ደቂቃዎች የማግበር ዘመንን እና የገለጻ ሃሽን መጠቀማቸውን ያረጋግጣል። የቅርብ ጊዜ ሪቪው በNX-6 የተጠሩትን የደህንነት ሀዲዶችም ያስፈጽማል፡ ምልአተ ጉባኤው ከተገለጸው አረጋጋጭ ስብስብ መብለጥ አይችልም፣ የተጠበቁ የስም ቦታዎች ባዶ ያልሆኑ ሕብረቁምፊዎች መሆን አለባቸው፣ እና የችሎታ መገለጫዎች በብቸኝነት የሚጨምሩ `activation_epoch`/`expiry_epoch` ጥንዶች በጥሩ ሁኔታ የተፈጠሩ መሆን አለባቸው። `Allow`/`Deny` ተጽዕኖዎች። በ `fixtures/nexus/cbdc_rollouts/` ስር ያለው የናሙና ማስረጃ ጥቅል የሚተገበረው በውህደት ፈተና `integration_tests/tests/nexus/cbdc_rollout_bundle.rs` ሲሆን አረጋጋጩን ወደ CI እንዲይዝ ያደርጋል።

## 4. የፕሮግራም-ገንዘብ መስተጋብር

አንዴ ባንክ (ዳታ ስፔስ 11) እና የችርቻሮ dApp (ዳታ ስፔስ 12) ሁለቱም በተመሳሳይ `ComposabilityGroupId` ውስጥ ከተፈቀዱ በፕሮግራም የሚቻሉ የገንዘብ ፍሰቶች ከ`docs/source/nexus.md` §8.6 ያለውን የ AXT ጥለት ይከተላሉ፡-

1. የችርቻሮ dApp ከ UAID + AXT መፈጨት ጋር የተያያዘ የንብረት መያዣ ይጠይቃል። የ CBDC ሌይን እጀታውን በ`AssetPermissionManifest::evaluate` በኩል ያረጋግጣል (ድሎችን መካድ፣ አበሎች ተፈጻሚ ይሆናሉ)።
2. ሁለቱም DS አንድ አይነት የተዋሃደ ቡድን ያውጃሉ ስለዚህ ማዘዋወር ወደ ሲቢሲሲ መስመር ለአቶሚክ ማካተት (`LaneRoutingPolicy` እርስ በርስ በተፈቀዱ መዝገብ ውስጥ ሲገባ `group_id` ይጠቀማል)።
3. በአፈፃፀም ወቅት፣ CBDC DS በወረዳው ውስጥ የAML/KYC ማረጋገጫዎችን (`use_asset_handle` pseudocode in `nexus.md`) ያስፈጽማል፣ dApp DS ደግሞ የአካባቢያዊ የንግድ ሁኔታን የሚያዘምነው CBDC ቁርጥራጭ ከተሳካ በኋላ ነው።
4. የማረጋገጫ ቁሳቁስ (FASTPQ + DA ቁርጠኝነት) በ CBDC መስመር ላይ ብቻ ይቆያል; የውህደት-መጽሔት ግቤቶች የግል መረጃን ሳያወጡ የአለምን ሁኔታ መወሰኛ ያቆያሉ።

ፕሮግራም-ገንዘብ መልሶ ማጫወት መዝገብ የሚከተሉትን ማካተት አለበት:

- AXT ገላጭ + ጥያቄ/ምላሾችን ይይዛል።
- Norito የተመሰጠረ የግብይት ፖስታ።
- የውጤት ደረሰኞች (ደስተኛ መንገድ, መከልከል).
- የቴሌሜትሪ ቅንጥቦች ለ`telemetry::fastpq.execution_mode`፣ `nexus_scheduler_lane_teu_slot_committed` እና `lane_commitments`።

## 5. ታዛቢነት እና Runbooks

- ** መለኪያዎች፡** ክትትል `nexus_scheduler_lane_teu_capacity`፣ `nexus_scheduler_lane_teu_slot_committed`፣ `nexus_scheduler_lane_teu_deferral_total{reason}`፣ `governance_manifest_admission_total`፣ እና `lane_governance_sealed_total` (`docs/source/telemetry.md` ይመልከቱ)።
- ** ዳሽቦርዶች: *** `docs/source/grafana_scheduler_teu.json` በ CBDC መስመር ረድፍ ያራዝሙ; ለተፈቀደላቸው ዝርዝር ጩኸት (በእያንዳንዱ የነቃ ጊዜ ማብራሪያዎች) እና የአገልግሎት ጊዜ ማብቂያ ቧንቧዎችን ፓነሎች ይጨምሩ።
- ** ማንቂያዎች፡** `rate(nexus_scheduler_lane_teu_deferral_total{reason="cap_exceeded"}[5m]) > 0` ለ15ደቂቃዎች ወይም `lane_governance.manifest_ready=false` ከአንድ የምርጫ ልዩነት በላይ ሲቆይ ቀስቅሴ።
- ** Runbook ጠቋሚዎች፡** በ`docs/source/governance_api.md` ውስጥ ወደ የተጠበቀ የስም ቦታ መመሪያ እና በ `docs/source/nexus.md` ውስጥ በፕሮግራም ሊደረግ የሚችል ገንዘብ መላ መፈለግ።

## 6. ተቀባይነት ማረጋገጫ ዝርዝር- [] CBDC ሌይን በ`nexus.lane_catalog` ከTEU ሜታዳታ ከTEU ሙከራዎች ጋር ተገለጸ።
- [ ] በ`cargo test -p integration_tests nexus::lane_registry` በኩል የተረጋገጠ `cbdc.manifest.json` የተፈረመ በአንጸባራቂ ማውጫ ውስጥ ይገኛል።
- [ ] ለእያንዳንዱ UAID የተሰጠ እና በጠፈር ማውጫ ውስጥ የተከማቸ ችሎታ ያሳያል። በክፍል ሙከራዎች (`crates/iroha_data_model/src/nexus/manifest.rs`) የተረጋገጠውን ቅድሚያ መከልከል/ፍቀድ።
- [ ] የተፈቀደላቸው ዝርዝር ማግበር ከአስተዳደር ፈቃድ መታወቂያ፣ `activation_epoch` እና Prometheus ማስረጃ ጋር ተመዝግቧል።
- [] በፕሮግራም ሊሰራ የሚችል ገንዘብ መልሶ ማጫወት በማህደር ተቀምጧል፣ እጀታ መስጠትን፣ መከልከልን እና ፍሰቶችን መፍቀድን ያሳያል።
- [ ] የማስረጃ ቅርቅብ ከክሪፕቶግራፊክ ዳይጀስት ጋር የተሰቀለ፣ ከአስተዳደር ትኬት የተገናኘ እና `status.md` አንዴ NX-6 ከ🈯 እስከ 🈺 ተመርቋል።

ይህንን የመጫወቻ መጽሐፍ መከተል ለNX-6 የሚቀርበውን ሰነድ ያረካል እና የወደፊት የመንገድ ካርታ እቃዎችን (NX-12/NX-15) ለCBC ሌይን ውቅር፣ የተፈቀደላቸው ቦርዲንግ እና በፕሮግራም ሊረዳ የሚችል የገንዘብ መስተጋብር አብነት በማቅረብ እንዳይታገድ ያደርጋል።