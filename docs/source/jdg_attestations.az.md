---
lang: az
direction: ltr
source: docs/source/jdg_attestations.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 459e8ed4612da7cfa68053e4e299b2f68e7620d4f3b98a8a721ebf8327829ea1
source_last_modified: "2026-01-08T21:57:18.412403+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# JDG Attestasiyaları: Mühafizə, Rotasiya və Saxlama

Bu qeyd indi `iroha_core`-də göndərilən v1 JDG attestasiya mühafizəsini sənədləşdirir.

- **Komitə göstərir:** Norito kodlu `JdgCommitteeManifest` paketləri hər bir məlumat məkanında fırlanma daşıyır
  cədvəllər (`committee_id`, sifarişli üzvlər, hədd, `activation_height`, `retire_height`).
  Manifestlər `JdgCommitteeSchedule::from_path` ilə yüklənir və ciddi şəkildə artırılır
  istefaya/aktivləşdirmə arasında isteğe bağlı lütf üst-üstə düşməsi (`grace_blocks`) ilə aktivasiya yüksəklikləri
  komitələr.
- **Atestasiya mühafizəsi:** `JdgAttestationGuard` məlumat məkanının bağlanmasını, istifadə müddətini başa vurmasını, köhnəlmiş sərhədlərini tətbiq edir,
  komitə id/ərəfəsində uyğunluq, imzalayan üzvlük, dəstəklənən imza sxemləri və isteğe bağlı
  `JdgSdnEnforcer` vasitəsilə SDN təsdiqi. Ölçü qapaqları, maksimum gecikmə və icazə verilən imza sxemləri bunlardır
  konstruktor parametrləri; `validate(attestation, dataspace, current_height)` aktivi qaytarır
  komitə və ya strukturlaşdırılmış səhv.
  - `scheme_id = 1` (`simple_threshold`): hər imzalayan imza, isteğe bağlı imzalayan bitmap.
  - `scheme_id = 2` (`bls_normal_aggregate`): üzərində tək əvvəlcədən yığılmış BLS-normal imza
    attestasiya hash; imzalayan bitmap isteğe bağlıdır, attestasiyada bütün imzalayanlar üçün defoltdur. BLS
    ümumi təsdiqləmə manifestdə hər bir komitə üzvü üçün etibarlı PoP tələb edir; itkin və ya
    etibarsız PP-lər attestasiyadan imtina edirlər.
  `governance.jdg_signature_schemes` vasitəsilə icazə siyahısını konfiqurasiya edin.
- **Saxlama anbarı:** `JdgAttestationStore` konfiqurasiya edilə bilən bir məlumat məkanı üçün attestasiyaları izləyir
  hər məlumat məkanı qapağı, əlavədəki ən köhnə girişləri budamaq. `for_dataspace` və ya zəng edin
  Audit/replay paketlərini əldə etmək üçün `for_dataspace_and_epoch`.
- **Testlər:** Vahid əhatə dairəsi indi etibarlı komitə seçimini, imzalayanın naməlum rəddini, köhnəlməsini həyata keçirir
  attestasiyadan imtina, dəstəklənməyən sxem idləri və saxlama budaması. Bax
  `crates/iroha_core/src/jurisdiction.rs`.

Mühafizəçi konfiqurasiya edilmiş icazə siyahısından kənar sxemləri rədd edir.