---
lang: az
direction: ltr
source: docs/source/confidential_assets_rotation.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: fd1e43316c492cc96ed107f6318841ad8db160735d4698c4f05562ff6127fda9
source_last_modified: "2026-01-22T14:35:37.492932+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

//! `roadmap.md:M3` tərəfindən istinad edilən məxfi aktivlərin rotasiyası kitabçası.

# Məxfi Aktivlərin Rotasiyası Runbook

Bu dərslik operatorların məxfi aktivi necə planlaşdırdığını və icra etdiyini izah edir
fırlanmalar (parametr dəstləri, yoxlama düymələri və siyasət keçidləri).
pul kisələri, Torii müştəriləri və mempool mühafizəçilərinin determinist qalmasını təmin etmək.

## Həyat dövrü və Statuslar

Məxfi parametr dəstləri (`PoseidonParams`, `PedersenParams`, doğrulama açarları)
müəyyən bir hündürlükdə effektiv statusu əldə etmək üçün istifadə edilən qəfəs və köməkçi yaşayır
`crates/iroha_core/src/state.rs:7540`–`7561`. İş vaxtı köməkçiləri süpürmə işlərini gözləyir
hədəf hündürlüyə çatan kimi keçidlər və daha sonra uğursuzluqları qeyd edin
təkrar yayımlar (`crates/iroha_core/src/state.rs:6725`–`6765`).

Aktiv siyasətləri daxil edin
`pending_transition { transition_id, new_mode, effective_height, conversion_window }`
beləliklə idarəetmə vasitəsilə yeniləmələri planlaşdıra bilər
`ScheduleConfidentialPolicyTransition` və tələb olunarsa onları ləğv edin. Bax
`crates/iroha_data_model/src/asset/definition.rs:320` və Torii DTO güzgüləri
(`crates/iroha_torii/src/routing.rs:1539`–`1580`).

## Fırlanma iş axını

1. **Yeni parametr paketlərini dərc edin.** Operatorlar təqdim edir
   `PublishPedersenParams`/`PublishPoseidonParams` təlimatları (CLI)
   `iroha app zk params publish ...`) metadata ilə yeni generator dəstləri hazırlamaq,
   aktivləşdirmə / köhnəlmə pəncərələri və status markerləri. İcraçı rədd edir
   dublikat ID-lər, artan olmayan versiyalar və ya pis status keçidləri
   `crates/iroha_core/src/smartcontracts/isi/world.rs:2499`–`2635` və
   reyestr testləri uğursuzluq rejimlərini əhatə edir (`crates/iroha_core/tests/confidential_params_registry.rs:93`–`226`).
2. **Qeydiyyatdan keçin/əsas yeniləmələri yoxlayın.** `RegisterVerifyingKey` arxa planı tətbiq edir,
   öhdəliyi və açar daxil ola bilməmişdən əvvəl dövrə/versiya məhdudiyyətləri
   reyestr (`crates/iroha_core/src/smartcontracts/isi/world.rs:2067`–`2137`).
   Açarı yeniləmək avtomatik olaraq köhnə girişi ləğv edir və daxili baytları silir,
   `crates/iroha_core/tests/zk_vk_deprecate_marks_status.rs:1` tərəfindən həyata keçirildiyi kimi.
3. **Aktiv-siyasət keçidlərini planlaşdırın.** Yeni parametr ID-ləri aktiv olduqdan sonra,
   idarəetmə istənilən ilə `ScheduleConfidentialPolicyTransition` çağırır
   rejimi, keçid pəncərəsi və audit hash. İcraçı ziddiyyətdən imtina edir
   keçidlər və ya mükəmməl şəffaf təchizatı olan aktivlər. kimi testlər
   `crates/iroha_core/tests/confidential_policy_gates.rs:300`–`384` bunu yoxlayın
   dayandırılmış keçidlər aydın `pending_transition`, isə
   `confidential_policy_transition_reaches_shielded_only_on_schedule`
   lines385–433 planlaşdırılmış təkmilləşdirmələrin tam olaraq `ShieldedOnly`-ə çevrildiyini təsdiqləyir
   effektiv hündürlük.
4. **Siyasət tətbiqi və yaddaş qoruyucusu.** Blok icraçısı bütün gözlənilənləri süpürür
   hər blokun başlanğıcında keçidlər (`apply_policy_if_due`) və emissiyalar
   keçid uğursuz olarsa, operatorlar yenidən planlaşdıra bilsinlər. Qəbul zamanı
   mempool effektiv siyasəti blokun ortasında dəyişəcək əməliyyatlardan imtina edir,
   keçid pəncərəsi boyunca deterministik daxil olmağın təmin edilməsi
   (`docs/source/confidential_assets.md:60`).

## Pul kisəsi və SDK Tələbləri- Swift və digər mobil SDK-lar aktiv siyasəti əldə etmək üçün Torii köməkçilərini ifşa edir.
  üstəgəl hər hansı gözlənilən keçid, beləliklə, pul kisələri imzalamadan əvvəl istifadəçiləri xəbərdar edə bilər. Bax
  `IrohaSwift/Sources/IrohaSwift/ToriiClient.swift:309` (DTO) və əlaqəli
  `IrohaSwift/Tests/IrohaSwiftTests/ToriiClientTests.swift:591`-də testlər.
- CLI eyni metaməlumatları `iroha ledger assets data-policy get` vasitəsilə əks etdirir (köməkçi
  `crates/iroha_cli/src/main.rs:1497`–`1670`), operatorlara yoxlama aparmağa imkan verir.
  siyasət/parametr identifikatorları aktiv tərifinə daxil edilir
  blok mağaza.

## Test və Telemetriya Əhatəsi

- `crates/iroha_core/tests/zk_ledger_scaffold.rs:288`–`345` həmin siyasəti təsdiqləyir
  keçidlər metadata snapshotlarına yayılır və tətbiq edildikdən sonra silinir.
- `crates/iroha_core/tests/zk_dedup.rs:1` sübut edir ki, `Preverify` önbelleği
  fırlanma ssenariləri daxil olmaqla, ikiqat xərcləmələri/ikiqat sübutları rədd edir
  öhdəliklər fərqlənir.
- `crates/iroha_core/tests/zk_confidential_events.rs` və
  `zk_shield_transfer_audit.rs` uçdan-uca qalxan örtüyü → köçürmə → ekrandan çıxarın
  axınlar, parametr fırlanmaları arasında audit yolunun sağ qalmasını təmin edir.
- `dashboards/grafana/confidential_assets.json` və
  `docs/source/confidential_assets.md:401` CommitmentTree və sənədini
  hər bir kalibrləmə/fırlanma qaçışını müşayiət edən doğrulayıcı-keş ölçü cihazları.

## Runbook Sahibliyi

- **DevRel / Pulqabı SDK Rəhbərləri:** SDK parçalarını + göstərən sürətli başlanğıcları qoruyun
  Gözləyən keçidləri necə üzə çıxarmaq və nanəni təkrar oynamaq → transfer → aşkar etmək
  yerli testlər (`docs/source/project_tracker/confidential_assets_phase_c.md:M3.2` altında izlənilir).
- **Program Mgmt / Confidential Assets TL:** keçid sorğularını təsdiqləyin, saxlayın
  `status.md` qarşıdan gələn fırlanmalarla yeniləndi və imtinaların (əgər varsa) olmasını təmin edin.
  kalibrləmə kitabçasının yanında qeyd olunur.