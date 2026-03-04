---
lang: az
direction: ltr
source: docs/examples/finance/repo_custodian_ack_template.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: c52d7f2c5ec9dc4cda81895561bc1261659935c94bf3f7febb0867f4981fe616
source_last_modified: "2026-01-22T16:26:46.472177+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Repo Qəyyumluq Təsdiq Şablonu

Repo (ikitərəfli və ya üçtərəfli) qəyyuma istinad etdikdə bu şablondan istifadə edin
`RepoAgreement::custodian` vasitəsilə. Məqsəd qəyyumluq SLA-nı, marşrutlaşdırmanı qeyd etməkdir
hesablar və aktivlər hərəkət etməzdən əvvəl kontaktları qazın. Şablonu özünüzə kopyalayın
sübut kataloqu (məsələn
`artifacts/finance/repo/<slug>/custodian_ack_<custodian>.md`), doldurun
yertutanları və faylı bölməsində təsvir edilən idarəetmə paketinin bir hissəsi kimi hash edin
`docs/source/finance/repo_ops.md` §2.8.

## 1. Metadata

| Sahə | Dəyər |
|-------|-------|
| Müqavilə identifikatoru | `<repo-yyMMdd-XX>` |
| Saxlayan hesab id | `<ih58...>` |
| / tarix tərəfindən hazırlanmışdır | `<custodian ops lead>` |
| Masa kontaktları qəbul edildi | `<desk lead + counterparty>` |
| Sübut kataloqu | ``artifacts/finance/repo/<slug>/`` |

## 2. Qəyyumluq sahəsi

- **Girov tərifləri alındı:** `<list of asset definition ids>`
- **Nağd pul vahidi / hesablaşma dəmir yolu:** `<xor#sora / other>`
- **Qeydiyyat pəncərəsi:** `<start/end timestamps or SLA summary>`
- **Daimi təlimatlar:** `<hash + path to standing instruction document>`
- **Avtomatlaşdırma ilkin şərtləri:** `<scripts, configs, or runbooks custodian will invoke>`

## 3. Marşrutlaşdırma və Monitorinq

| Maddə | Dəyər |
|------|-------|
| Qəyyum pul kisəsi / kitab hesabı | `<asset ids or ledger path>` |
| Monitorinq kanalı | `<Slack/phone/on-call rotation>` |
| Qazma ilə əlaqə | `<primary + backup>` |
| Tələb olunan xəbərdarlıqlar | `<PagerDuty service, Grafana board, etc.>` |

## 4. Bəyanatlar

1. *Qeydiyyata hazırlıq:* “Biz mərhələli `repo initiate` faydalı yükünü nəzərdən keçirdik.
   yuxarıda göstərilən identifikatorlar və sadalanan SLA çərçivəsində girov qəbul etməyə hazırdırlar
   §2-də.”
2. *Geri qaytarma öhdəliyi:* “Yuxarıda adı çəkilən geri qaytarma kitabını yerinə yetirəcəyik.
   hadisə komandiri tərəfindən idarə olunur və CLI qeydlərini və hashləri təmin edəcəkdir
   `governance/drills/<timestamp>.log`.”
3. *Dəlillərin saxlanması:* “Biz etirafı ayaqda saxlayacağıq
   ən azı `<duration>` üçün təlimatlar və CLI qeydləri və onları
   sorğu əsasında maliyyə şurası.”

Aşağıda imzalayın (idarəetmə vasitəsilə ötürülən elektron imzalar məqbuldur
izləyici).

| Adı | Rol | İmza / tarix |
|------|------|------------------|
| `<custodian ops lead>` | Saxlama operatoru | `<signature>` |
| `<desk lead>` | Masa | `<signature>` |
| `<counterparty>` | Qarşı tərəf | `<signature>` |

> İmzalanandan sonra faylı hash edin (məsələn: `sha256sum custodian_ack_<cust>.md`) və
> Rəyçilər təsdiq edə bilməsi üçün idarəetmə paketi cədvəlində həzm yazın
> səsvermə zamanı istinad edilən təsdiq baytları.