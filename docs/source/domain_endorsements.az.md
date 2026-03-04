---
lang: az
direction: ltr
source: docs/source/domain_endorsements.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 7c337150e6de1efa9f9480ba8126ecd5ada4ed8ee7ee8b70a95fd7f6348f9016
source_last_modified: "2025-12-29T18:16:35.952418+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Domen Təsdiqləri

Domen təsdiqləri operatorlara komitə tərəfindən imzalanmış bəyanat əsasında domen yaratmağa və yenidən istifadə etməyə imkan verir. Təsdiq yükü zəncirdə qeydə alınmış Norito obyektidir ki, müştərilər kimin hansı domeni və nə vaxt sertifikat verdiyini yoxlaya bilsinlər.

## Faydalı yük forması

- `version`: `DOMAIN_ENDORSEMENT_VERSION_V1`
- `domain_id`: kanonik domen identifikatoru
- `committee_id`: insan tərəfindən oxuna bilən komitə etiketi
- `statement_hash`: `Hash::new(domain_id.to_string().as_bytes())`
- `issued_at_height` / `expires_at_height`: blok hündürlüyünü məhdudlaşdıran etibarlılıq
- `scope`: isteğe bağlı məlumat məkanı və əlavə olaraq qəbul edilən blokun hündürlüyünü **ötəməli** olan `[block_start, block_end]` pəncərəsi (daxil olmaqla)
- `signatures`: `body_hash()` üzərində imzalar (`signatures = []` ilə təsdiq)
- `metadata`: isteğe bağlı Norito metadata (təklif identifikatorları, audit keçidləri və s.)

## İcra

- Nexus aktivləşdirildikdə və `nexus.endorsement.quorum > 0` olduqda və ya domen üzrə siyasət tələb olunduğu kimi domeni işarələdikdə təsdiqlər tələb olunur.
- Təsdiqləmə domen/bəyanat hash bağlamasını, versiyanı, blok pəncərəsini, məlumat məkanına üzvlüyünü, istifadə müddətini/yaşını və komitə kvorumunu tətbiq edir. İmzalayanların `Endorsement` rolu ilə canlı konsensus açarları olmalıdır. Təkrarlar `body_hash` tərəfindən rədd edilir.
- Domen qeydiyyatına əlavə edilmiş təsdiqlər `endorsement` metadata açarından istifadə edir. Eyni doğrulama yolundan yeni domeni qeydiyyatdan keçirmədən audit üçün təsdiqləri qeyd edən `SubmitDomainEndorsement` təlimatı istifadə edir.

## Komitələr və siyasətlər

- Komitələr zəncirdə qeydiyyata alına bilər (`RegisterDomainCommittee`) və ya konfiqurasiya defoltlarından əldə edilə bilər (`nexus.endorsement.committee_keys` + `nexus.endorsement.quorum`, id = `default`).
- Domen üzrə siyasətlər `SetDomainEndorsementPolicy` (komitə id, `max_endorsement_age`, `required` bayrağı) vasitəsilə konfiqurasiya edilir. Yox olduqda, Nexus defoltları istifadə olunur.

## CLI köməkçiləri

- Təsdiq yaradın/imzalayın (stdout-a Norito JSON çıxarır):

  ```
  iroha endorsement prepare \
    --domain wonderland \
    --committee-id default \
    --issued-at-height 5 \
    --expires-at-height 25 \
    --block-start 5 \
    --block-end 15 \
    --signer-key <PRIVATE_KEY> --signer-key <PRIVATE_KEY>
  ```

- Təsdiq təqdim edin:

  ```
  iroha endorsement submit --file endorsement.json
  # or: cat endorsement.json | iroha endorsement submit
  ```

- İdarəetməni idarə edin:
  - `iroha endorsement register-committee --committee-id jdga --quorum 2 --member <PK> --member <PK> [--metadata path]`
  - `iroha endorsement set-policy --domain wonderland --committee-id jdga --max-endorsement-age 1000 --required`
  - `iroha endorsement policy --domain wonderland`
  - `iroha endorsement committee --committee-id jdga`
  - `iroha endorsement list --domain wonderland`

Doğrulama uğursuzluqları sabit xəta sətirlərini qaytarır (kvorum uyğunsuzluğu, köhnə/keçmiş təsdiq, əhatə dairəsi uyğunsuzluğu, naməlum məlumat məkanı, itkin komitə).