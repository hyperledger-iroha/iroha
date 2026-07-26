---
lang: az
direction: ltr
source: docs/source/compliance/android/eu/legal_signoff_memo.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 8bb3e19ca5eb661d202b5e3b9cd118207ded277e8ff717e16a342b71e7a67857
source_last_modified: "2025-12-29T18:16:35.926037+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# AND6 AB Hüquqi Qeydiyyat Memo Şablonu

Bu memo yol xəritəsi bəndinin **AND6** tələb etdiyi hüquqi baxışı qeyd edir
Aİ (ETSI/GDPR) artefakt paketi tənzimləyicilərə təqdim olunur. Məsləhətçi klonlamalıdır
hər buraxılış üçün bu şablon, aşağıdakı sahələri doldurun və imzalanmış nüsxəni saxlayın
memoda istinad edilən dəyişməz artefaktlarla yanaşı.

## Xülasə

- ** Buraxılış / Qatar:** `<e.g., 2026.1 GA>`
- **İzləmə tarixi:** `<YYYY-MM-DD>`
- **Məsləhətçi / Rəyçi:** `<name + organisation>`
- **Əhatə dairəsi:** `ETSI EN 319 401 security target, GDPR DPIA summary, SBOM attestation`
- **Əlaqədar biletlər:** `<governance or legal issue IDs>`

## Artefakt Yoxlama Siyahısı

| Artefakt | SHA-256 | Məkan / Link | Qeydlər |
|----------|---------|-----------------|-------|
| `security_target.md` | `<hash>` | `docs/source/compliance/android/eu/security_target.md` + idarəetmə arxivi | Buraxılış identifikatorlarını və təhlükə modeli düzəlişlərini təsdiqləyin. |
| `gdpr_dpia_summary.md` | `<hash>` | Eyni kataloq / lokalizasiya güzgüləri | Redaksiya siyasəti istinadlarının `sdk/android/telemetry_redaction.md`-ə uyğun olduğundan əmin olun. |
| `sbom_attestation.md` | `<hash>` | Sübut paketində eyni kataloq + kosign paketi | CycloneDX + mənşə imzalarını yoxlayın. |
| Sübut jurnalı sıra | `<hash>` | `docs/source/compliance/android/evidence_log.csv` | Sətir nömrəsi `<n>` |
| Cihaz laboratoriyası ehtiyat paketi | `<hash>` | `artifacts/android/device_lab_contingency/<YYYYMMDD>/*.tgz` | Bu buraxılışla əlaqəli uğursuzluq məşqini təsdiqləyir. |

> Paketdə daha çox fayl varsa, əlavə sətirlər əlavə edin (məsələn, məxfilik
> əlavələr və ya DPIA tərcümələri). Hər artefakt onun dəyişməzliyinə istinad etməlidir
> hədəfi və onu yaradan Buildkite işini yükləyin.

## Tapıntılar və İstisnalar

- `None.` *(qalıq riskləri əhatə edən, kompensasiya edən güllə siyahısı ilə əvəz edin
  nəzarətlər və ya tələb olunan təqib tədbirləri.)*

## Təsdiq

- **Qərar:** `<Approved / Approved with conditions / Blocked>`
- **İmza / Vaxt damğası:** `<digital signature or email reference>`
- **İzləyici sahiblər:** `<team + due date for any conditions>`

Son yaddaşı idarəetmə sübutları qutusuna yükləyin, SHA-256-nı kopyalayın
`docs/source/compliance/android/evidence_log.csv` və yükləmə yolunu bağlayın
`status.md`. Qərar "Bloklanıb" olarsa, AND6 sükanına keçin
komitə və sənədlərin düzəldilməsi addımları həm yol xəritəsi isti siyahısında, həm də
cihaz-laboratoriya fövqəladə hallar jurnalı.