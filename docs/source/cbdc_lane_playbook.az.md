---
lang: az
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

> **Yol xəritəsi bağlantısı:** NX-6 (CBDC şəxsi zolaq şablonu və ağ siyahı axını) və NX-14 (Nexus runbooks).  
> **Sahiblər:** Financial Services WG, Nexus Core WG, Compliance WG.  
> **Status:** Tərtib edilməsi — tətbiq qarmaqları `crates/iroha_data_model::nexus`, `crates/iroha_core::governance::manifest` və `integration_tests/tests/nexus/lane_registry.rs`-də mövcuddur, lakin CBDC-ə xas manifestlər, ağ siyahılar və operator runbook-ları yox idi. Bu oyun kitabı istinad konfiqurasiyasını və işə qəbul işini sənədləşdirir, beləliklə CBDC yerləşdirmələri qəti şəkildə davam edə bilər.

## Əhatə və Rollar

- **Mərkəzi bank zolağı (“CBDC zolağı”):** İcazəli validatorlar, saxlama hesablaşma buferləri və proqramlaşdırıla bilən pul siyasətləri. Öz idarəetmə manifestinə malik məhdud məlumat məkanı + zolaq cütü kimi işləyir.
- **Topdan/pərakəndə bank məlumat məkanları:** UAID-ləri saxlayan, qabiliyyət manifestlərini alan və CBDC zolağı ilə atomik AXT üçün ağ siyahıya salına bilən iştirakçı DS.
- **Proqramlaşdırıla bilən pul dApps:** CBDC axınlarını istehlak edən xarici DS ağ siyahıya salındıqdan sonra `ComposabilityGroup` marşrutlaşdırma vasitəsilə keçir.
- **İdarəetmə və uyğunluq:** Parlament (və ya ekvivalent modul) zolaq manifestlərini, qabiliyyət manifestlərini və ağ siyahı dəyişikliklərini təsdiq edir; uyğunluq Norito manifestləri ilə yanaşı sübut paketlərini saxlayır.

**Asılılıqlar**

1. Zolaq kataloqu + məlumat məkanı kataloqu naqilləri (`docs/source/nexus_lanes.md`, `defaults/nexus/config.toml`).
2. Zolaq manifestinin tətbiqi (`crates/iroha_core/src/governance/manifest.rs`, `crates/iroha_core/src/queue.rs`-də növbə qapısı).
3. Bacarıq manifestləri + UAID-lər (`crates/iroha_data_model/src/nexus/manifest.rs`).
4. Planlayıcı TEU kvotaları + ölçülər (`integration_tests/tests/scheduler_teu.rs`, `docs/source/telemetry.md`).

## 1. İstinad zolağının düzülüşü

### 1.1 Lane kataloqu və məlumat məkanı girişləri

`[[nexus.lane_catalog]]` və `[[nexus.dataspace_catalog]]`-ə xüsusi qeydlər əlavə edin. Aşağıdakı nümunə `defaults/nexus/config.toml`-i hər slot üçün 1500 TEU ehtiyatı saxlayan və aclığı altı yuvaya endirən CBDC zolağı ilə, üstəgəl topdansatış banklar və pərakəndə pul kisələri üçün uyğun məlumat məkanı ləqəblərini genişləndirir.

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
metadata.settlement.buffer_asset = "xor#sora"
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

**Qeydlər**

- `metadata.scheduler.teu_capacity` və `metadata.scheduler.starvation_bound_slots` `integration_tests/tests/scheduler_teu.rs` tərəfindən həyata keçirilən TEU ölçülərini qidalandırır. Operatorlar onları qəbul nəticələri ilə sinxronlaşdırmalıdırlar ki, `nexus_scheduler_lane_teu_capacity` şablona uyğun olsun.
- Yuxarıdakı hər bir məlumat məkanı ləqəbi idarəetmə manifestlərində və qabiliyyət manifestlərində (aşağıya bax) görünməlidir ki, qəbul avtomatik olaraq sürüşməni rədd etsin.

### 1.2 Zolaqlı manifest skeleti

Zolaq manifestləri `nexus.registry.manifest_directory` vasitəsilə konfiqurasiya edilmiş qovluq altında yaşayır (bax: `crates/iroha_config/src/parameters/actual.rs`). Fayl adları zolaq ləqəblərinə uyğun olmalıdır (`cbdc.manifest.json`). Sxem `integration_tests/tests/nexus/lane_registry.rs`-də idarəetmə manifest testlərini əks etdirir.

```json
{
  "lane": "cbdc",
  "dataspace": "cbdc.core",
  "version": 1,
  "governance": "central_bank_multisig",
  "validators": [
    "i105...",
    "i105...",
    "i105...",
    "i105..."
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

Əsas tələblər:- Təsdiqləyicilər **qataloqda mövcud olan kanonik I105 hesab identifikatorları olmalıdır (`@domain` yoxdur; `@domain`-i yalnız açıq marşrut göstərişi kimi əlavə edin). `quorum`-i multisig həddinə təyin edin (≥2).
- Qorunan ad məkanları `Queue::push` (bax: `crates/iroha_core/src/queue.rs`) tərəfindən tətbiq edilir, buna görə də bütün CBDC müqavilələrində `gov_namespace` + `gov_contract_id` göstərilməlidir.
- `composability_group` sahələri `docs/source/nexus.md` §8.6-da təsvir edilən sxemə uyğundur; sahibi (CBDC zolağı) ağ siyahı və kvotaları təmin edir. Ağ siyahıya alınmış DS manifestləri yalnız `group_id_hex` + `activation_epoch`-i təyin edir.
- Manifesti kopyaladıqdan sonra `LaneManifestRegistry::from_config`-in onu yüklədiyini təsdiqləmək üçün `cargo test -p integration_tests nexus::lane_registry -- --nocapture`-i işə salın.

### 1.3 Bacarıq manifestləri (UAID siyasətləri)

Qabiliyyət təzahürləri (`AssetPermissionManifest`, `crates/iroha_data_model/src/nexus/manifest.rs`) `UniversalAccountId`-i deterministik ehtiyatlara bağlayır. Onları Space Directory vasitəsilə dərc edin ki, banklar və dApps imzalanmış siyasətləri əldə edə bilsin.

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

- İcazə verən qayda (`ManifestVerdict::Denied`) uyğun gəlsə belə, inkar qaydaları qalib gəlir, ona görə də bütün açıq inkarları müvafiq icazədən sonra yerləşdirin.
- Atom ödəniş tutacaqları üçün `AllowanceWindow::PerSlot` və müştəri limitlərini artırmaq üçün `PerMinute`/`PerDay` istifadə edin.
- UAID/dataspace üçün tək manifest kifayətdir; aktivləşdirmələr və bitmə müddətləri siyasətin fırlanma kadansını tətbiq edir.
- `expiry_epoch`-ə çatdıqdan sonra zolaqlı işləmə vaxtı avtomatik olaraq başa çatır,
  ona görə də əməliyyat qrupları sadəcə `SpaceDirectoryEvent::ManifestExpired`-ə nəzarət edir,
  `nexus_space_directory_revision_total` deltasını arxivləşdirin və Torii şoularını yoxlayın
  `status = "Expired"`. CLI `manifest expire` əmri üçün əlçatan qalır
  əl ilə ləğvetmə və ya sübut doldurma.

## 2. Bankın işə qəbulu və Ağ siyahı iş axını

| Faza | Sahib(lər) | Fəaliyyətlər | Sübut |
|-------|----------|---------|----------|
| 0. Qəbul | CBDC PMO | KYC dosyesini, texniki DS manifestini, validator siyahısını, UAID xəritəsini toplayın. | Qəbul bileti, imzalanmış DS manifest layihəsi. |
| 1. İdarəetmənin təsdiqi | Parlament / Uyğunluq | Qəbul paketini nəzərdən keçirin, `cbdc.manifest.json` işarəsi, `AssetPermissionManifest` təsdiq edin. | İmzalanmış idarəetmə protokolları, manifest commit hash. |
| 2. Bacarıqların verilməsi | CBDC zolaqlı əməliyyatlar | `norito::json::to_string_pretty` vasitəsilə manifestləri kodlayın, Space Directory altında saxlayın, operatorları xəbərdar edin. | Manifest JSON + norito `.to` faylı, BLAKE3 həzm. |
| 3. Ağ siyahının aktivləşdirilməsi | CBDC zolaqlı əməliyyatlar | DSID-i `composability_group.whitelist`-ə əlavə edin, `activation_epoch`-ə vurun, manifest paylayın; lazım gələrsə məlumat məkanının marşrutunu yeniləyin. | Manifest fərqi, `kagami config diff` çıxışı, idarəetmə təsdiqi ID. |
| 4. Yayımlamanın doğrulanması | QA Gildiyası / Əməliyyatlar | İnteqrasiya testlərini, TEU yükləmə testlərini və proqramlaşdırıla bilən pulun təkrarını həyata keçirin (aşağıya baxın). | `cargo test` qeydləri, TEU panelləri, proqramlaşdırıla bilən pul qurğusunun nəticələri. |
| 5. Sübut arxivi | Uyğunluq WG | Paket manifestləri, təsdiqləri, qabiliyyət həzmləri, test nəticələri və `artifacts/nexus/cbdc_<stamp>/` altında Prometheus qırıntıları. | Sübut tarball, yoxlama faylı, şuranın imzalanması. |

### Audit paketinin köməkçisiSpace Directory-dən `iroha app space-directory manifest audit-bundle` köməkçisindən istifadə edin
Sübut paketini təqdim etməzdən əvvəl hər bir qabiliyyətin təzahürünü çəkmək üçün oyun kitabı.
Manifest JSON (və ya `.to` yükü) və məlumat məkanı profilini təmin edin:

```bash
iroha app space-directory manifest audit-bundle \
  --manifest-json fixtures/space_directory/capability/cbdc_wholesale.manifest.json \
  --profile fixtures/space_directory/profile/cbdc_lane_profile.json \
  --out-dir artifacts/nexus/cbdc/2026-02-01T00-00Z/cbdc_wholesale \
  --notes "CBDC wholesale refresh"
```

Komanda kanonik JSON/Norito/hash nüsxələrini yayır.
`audit_bundle.json`, UAID, məlumat məkanı identifikatoru, aktivasiya/saxlama müddətini qeyd edir
tələb olunanları tətbiq edərkən dövrlər, manifest hash və profil audit qarmaqları
`SpaceDirectoryEvent` abunələri. Paketi dəlilin içərisinə atın
auditorlar və tənzimləyicilər daha sonra dəqiq baytları təkrarlaya bilsinlər.

### 2.1 Əmrlər və doğrulamalar

1. **Line manifestləri:** `cargo test -p integration_tests nexus::lane_registry -- --nocapture lane_manifest_registry_loads_fixture_manifests`.
2. **Cədvəlləndirici kvotaları:** `cargo test -p integration_tests scheduler_teu -- queue_teu_backlog_matches_metering queue_routes_transactions_across_configured_lanes`.
3. **Manuel tüstü:** CBDC fayllarına işarə edən manifest kataloqu ilə `irohad --sora --config configs/soranexus/nexus/config.toml --chain 0000…`, sonra `/v1/sumeragi/status` düyməsini basın və CBDC zolağı üçün `lane_governance.manifest_ready=true`-i yoxlayın.
4. **Ağ siyahıya uyğunluq testi:** `cargo test -p integration_tests nexus::cbdc_whitelist -- --nocapture` `integration_tests/tests/nexus/cbdc_whitelist.rs` məşqləri, `fixtures/space_directory/profile/cbdc_lane_profile.json` təhlili və istinad edilən imkanlar hər bir ağ siyahı girişinin UAID, data məkanı, aktivləşdirmə dövrünün və man10X0NT500 siyahısına uyğun olmasını təmin etmək üçün təzahür edir. `fixtures/space_directory/capability/` altında. Ağ siyahı və ya manifestlər dəyişdikdə test jurnalını NX-6 sübut paketinə əlavə edin.

### 2.2 CLI fraqmentləri

- `cargo run -p iroha_cli -- manifest cbdc --uaid <hash> --dataspace 11 --template cbdc_wholesale` vasitəsilə UAID + manifest skeleti yaradın.
- `iroha app space-directory manifest publish --manifest cbdc_wholesale.manifest.to` (və ya `--manifest-json cbdc_wholesale.manifest.json`) istifadə edərək Torii (Space Directory) üçün imkan manifestini dərc edin; təqdim edən hesabda CBDC məlumat məkanı üçün `CanPublishSpaceDirectoryManifest` olmalıdır.
- Əməliyyat masası uzaqdan avtomatlaşdırma ilə işləyirsə, HTTP vasitəsilə dərc edin:

  ```bash
  curl -X POST https://torii.soranexus/v1/space-directory/manifests \
       -H 'Content-Type: application/json' \
       -d '{
            "authority": "i105...",
            "private_key": "ed25519:CiC7…",
            "manifest": '"'"'$(cat fixtures/space_directory/capability/cbdc_wholesale.manifest.json)'"'"',
            "reason": "CBDC onboarding wave 4"
          }'
  ```

  Torii nəşr əməliyyatı növbəyə qoyulduqdan sonra `202 Accepted` qaytarır; eyni
  CIDR/API-token qapıları tətbiq edilir və zəncir üzərində icazə tələbi ilə uyğun gəlir
  CLI iş axını.
- Fövqəladə ləğvetmə Torii nömrəsinə POST göndərməklə uzaqdan verilə bilər:

  ```bash
  curl -X POST https://torii.soranexus/v1/space-directory/manifests/revoke \
       -H 'Content-Type: application/json' \
       -d '{
            "authority": "i105...",
            "private_key": "ed25519:CiC7…",
            "uaid": "uaid:0f4d…ab11",
            "dataspace": 11,
            "revoked_epoch": 9216,
            "reason": "Fraud investigation #NX-16-R05"
          }'
  ```

  Ləğv əməliyyatı növbəyə qoyulduqdan sonra Torii `202 Accepted` qaytarır; eyni
  CIDR/API-token qapıları digər proqram son nöqtələri kimi tətbiq edilir və `CanPublishSpaceDirectoryManifest`
  hələ də zəncirdə tələb olunur.
- Ağ siyahı üzvlüyünü döndərin: `cbdc.manifest.json`-i redaktə edin, `activation_epoch`-i döyün və təhlükəsiz surətdə bütün təsdiqləyicilərə yenidən yerləşdirin; `LaneManifestRegistry` konfiqurasiya edilmiş sorğu intervalında yenidən yüklənir.

## 3. Uyğunluq Sübut Paketi

Artefaktları `artifacts/nexus/cbdc_rollouts/<YYYYMMDDThhmmZ>/` altında saxlayın və həzmi idarəetmə biletinə əlavə edin.

| Fayl | Təsvir |
|------|-------------|
| `cbdc.manifest.json` | Ağ siyahı fərqi ilə imzalanmış zolaq manifest (əvvəl/sonra). |
| `capability/<uaid>.manifest.json` & `.to` | Norito + JSON qabiliyyəti hər UAID üçün təzahür edir. |
| `compliance/kyc_<bank>.pdf` | Tənzimləyici ilə üzləşən KYC sertifikatı. |
| `metrics/nexus_scheduler_lane_teu_capacity.prom` | Prometheus TEU boşluğunu sübut edən qırış. |
| `tests/cargo_test_nexus_lane_registry.log` | Manifest testindən daxil olun. |
| `tests/cargo_test_scheduler_teu.log` | TEU marşrutlaşdırma keçidlərini sübut edən jurnal. |
| `programmable_money/axt_replay.json` | Proqramlaşdırıla bilən pul ilə qarşılıqlı əlaqəni nümayiş etdirən stenoqramı təkrar oxuyun (4-cü bölməyə baxın). |
| `approvals/governance_minutes.md` | Manifest hash + aktivləşdirmə dövrünə istinad edən imzalanmış təsdiq protokolu. |**Təsdiqləmə skripti:** İdarə biletinə əlavə etməzdən əvvəl sübut paketinin tamamlandığını təsdiqləmək üçün `ci/check_cbdc_rollout.sh`-i işə salın. Köməkçi hər `artifacts/nexus/cbdc_rollouts/` (və ya `CBDC_ROLLOUT_BUNDLE=<path>`) hər `cbdc.manifest.json` üçün skan edir, manifest/kompozisiya qrupunu təhlil edir, hər bir qabiliyyət manifestində uyğun `.to` faylı olduğunu yoxlayır, `.to` faylını yoxlayır, `.to`-də testləri yoxlayır metrics scrape plus `cargo test` qeydləri, proqramlaşdırıla bilən pulla təkrar oynatılan JSON-u təsdiqləyir və təsdiq dəqiqələrində aktivasiya dövrünə və manifest hashına istinad edir. Ən son rev, həmçinin NX-6-da çağırılan təhlükəsizlik relslərini tətbiq edir: kvorum elan edilmiş təsdiqləyici dəstini keçə bilməz, qorunan ad məkanları boş olmayan sətirlər olmalıdır və qabiliyyət manifestləri monoton şəkildə artan `activation_epoch`/`expiry_epoch` formada elan edilməlidir. `Allow`/`Deny` effektləri. `fixtures/nexus/cbdc_rollouts/` altında nümunə sübut paketi `integration_tests/tests/nexus/cbdc_rollout_bundle.rs` inteqrasiya testi ilə həyata keçirilir və validator CI-yə qoşulur.

## 4. Proqramlaşdırıla bilən-Pulla qarşılıqlı əlaqə

Bank (məlumat məkanı 11) və pərakəndə dApp (məlumat məkanı 12) hər ikisi eyni `ComposabilityGroupId`-də ağ siyahıya salındıqdan sonra proqramlaşdırıla bilən pul axınları `docs/source/nexus.md` §8.6-dan AXT nümunəsini izləyir:

1. Pərakəndə dApp öz UAID + AXT həzminə bağlı aktiv idarəsini tələb edir. CBDC zolağı sapı `AssetPermissionManifest::evaluate` vasitəsilə yoxlayır (qalibləri rədd edin, müavinətlər tətbiq edilir).
2. Hər iki DS eyni birləşmə qrupunu elan edir, ona görə də marşrutlaşdırma onları atomik daxiletmə üçün CBDC zolağına yığır (`LaneRoutingPolicy`, qarşılıqlı ağ siyahıya salındıqda `group_id` istifadə edir).
3. İcra zamanı CBDC DS öz dövrəsində AML/KYC sübutlarını tətbiq edir (`use_asset_handle` psevdokod `nexus.md`), dApp DS isə yalnız CBDC fraqmenti uğur qazandıqdan sonra yerli biznes vəziyyətini yeniləyir.
4. Sübut materialı (FASTPQ + DA öhdəlikləri) CBDC zolağında qalır; birləşmə kitabçası qeydləri şəxsi məlumatları sızdırmadan qlobal vəziyyəti deterministik saxlayır.

Proqramlaşdırıla bilən pul təkrarı arxivinə aşağıdakılar daxil olmalıdır:

- AXT deskriptoru + sorğu/cavabları idarə edir.
- Norito kodlu əməliyyat zərfi.
- Nəticə qəbzləri (xoşbəxt yol, inkar yolu).
- `telemetry::fastpq.execution_mode`, `nexus_scheduler_lane_teu_slot_committed` və `lane_commitments` üçün telemetriya fraqmentləri.

## 5. Müşahidə oluna bilənlik və Runbooks

- **Metriklər:** Monitor `nexus_scheduler_lane_teu_capacity`, `nexus_scheduler_lane_teu_slot_committed`, `nexus_scheduler_lane_teu_deferral_total{reason}`, `governance_manifest_admission_total` və `lane_governance_sealed_total` (bax: `docs/source/telemetry.md`).
- ** İdarə panelləri:** `docs/source/grafana_scheduler_teu.json`-i CBDC zolağı ilə genişləndirin; ağ siyahının boşaldılması (hər aktivləşdirmə dövründə qeydlər) və qabiliyyətin müddəti bitən boru kəmərləri üçün panellər əlavə edin.
- **Xəbərdarlıqlar:** `rate(nexus_scheduler_lane_teu_deferral_total{reason="cap_exceeded"}[5m]) > 0` 15 dəqiqə ərzində və ya `lane_governance.manifest_ready=false` bir sorğu intervalından sonra davam etdikdə işə salın.
- **Runbook göstəriciləri:** `docs/source/governance_api.md`-də qorunan ad məkanı təlimatı və `docs/source/nexus.md`-də proqramlaşdırıla bilən pul problemlərinin aradan qaldırılması ilə əlaqə.

## 6. Qəbul yoxlama siyahısı- [ ] CBDC zolağı `nexus.lane_catalog`-də TEU testlərinə uyğun TEU metadatası ilə elan edilmişdir.
- [ ] İmzalanmış `cbdc.manifest.json` manifest kataloqunda mövcuddur, `cargo test -p integration_tests nexus::lane_registry` vasitəsilə təsdiq edilmişdir.
- [ ] Hər bir UAID üçün buraxılmış və Kosmik Kataloqda saxlanılan qabiliyyət manifestləri; Vahid testləri (`crates/iroha_data_model/src/nexus/manifest.rs`) vasitəsilə təsdiq edilmiş üstünlüyü rədd et/icazə ver.
- [ ] Ağ siyahının aktivləşdirilməsi idarəetmə təsdiqi ID, `activation_epoch` və Prometheus sübutu ilə qeydə alınıb.
- [ ] Proqramlaşdırıla bilən pulun təkrarı arxivləşdirilib, tutacaqların buraxılması, rədd edilməsi və axınlara icazə verilməsini nümayiş etdirir.
- [ ] NX-6 🈯-dan 🈺-ə qədər bitirdikdən sonra idarəetmə bileti və `status.md` ilə əlaqələndirilmiş kriptoqrafik həzm ilə yüklənmiş sübut paketi.

Bu dərslikdən sonra NX-6 üçün təqdim edilə bilən sənədləri təmin edir və CBDC zolağının konfiqurasiyası, ağ siyahıya qoşulma və proqramlaşdırıla bilən pul ilə qarşılıqlı əlaqə üçün deterministik şablon təmin etməklə gələcək yol xəritəsi elementlərini (NX-12/NX-15) blokdan çıxarır.