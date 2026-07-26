---
lang: az
direction: ltr
source: docs/source/bridge_finality.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 1cbd248fe14e63d00f002f09e1663181f3ab9bd99124ffeb89c56763b784046b
source_last_modified: "2026-07-12"
translation_last_reviewed: 2026-07-12
translator: machine-google-reviewed
---

<!--
SPDX-License-Identifier: Apache-2.0
-->

# Körpü yekunluğu sübutları

Bu sənəd ilk buraxılış üçün körpü yekunluğu formatını müəyyən edir. Sübut Sumeragi v2
tərəfindən yaradılan və davamlı saxlanılan dəqiq yekunluq dəlilini daşıyır. Sübut
zərfinin sxem versiyası `1`, daxilindəki konsensus protokolunun versiyası isə `2`-dir.
Sumeragi v1 sertifikatına proyeksiya, dekoder və ya ehtiyat yol yoxdur.

## Dəqiq sübut formatı

Norito və ya Norito JSON ilə kodlanan `BridgeFinalityProof` yalnız üç sahədən ibarətdir:

```text
{ version, block_header, finality_artifact }
```

- `version` mütləq `1` olmalıdır;
- `block_header` tələb olunan hündürlüyün kanonik `BlockHeader`-idir;
- `finality_artifact` həmin blok üçün saxlanmış dəqiq `V2FinalityArtifact`-dır. O,
  height-context roster sırası ilə hər validatorun BLS-normal PoP-unu
  (`validator_set_pops`) davamlı şəkildə özündə saxlayır.

Artefakt tam və dəyişməz `HeightContext`, dəqiq `BlockSubject`, blok hash-i, CommitQC və
rosterə uyğun PoP-ları ehtiva edir. Height context zənciri, epoch-u, rosteri,
`DualQuorum`-u, DA düzülüşünü, leader seed-i və digər konsensus məlumatlarını dondurur.
Epoch-u bitirən ana blokun context-i optional `next_epoch_snapshot` daşıyır; bu sahə
context id-yə daxil olduğuna görə, övlad rosterə icazə verməzdən əvvəl ana CommitQC onu
autentifikasiya edir. Finalized snapshot növbəti epoch parametrləri ilə yanaşı
`epoch_end_height` və növbəti rosterə uyğun `validator_set_pops`-u da autentifikasiya edir.

## Davamlı saxlama və yoxlama

Kura finality dərc edilməzdən və ya block body çıxarılmazdan əvvəl dəqiq kanonik header-i
və `commitment_index` sırasındakı SCCP arxivini dəyişməz retained record-da saxlayır.
Finality artefaktı sonra eyni header-lə ayrıca dəyişməz record-da saxlanır. Sübut qurucusu
yalnız retained header və finality record-u oxuyur; tarixi block body-yə və ya dəyişən WSV
payload-ına ehtiyac duymur. Çatışmayan, korlanmış, ziddiyyətli və ya yoxlanmayan record
qapalı şəkildə rədd edilir.

Stateless yoxlayıcı version, chain, height, header hash, header-in canonical predecessor-i və
view-u, context, subject və CommitQC-ni
dəqiq tutuşdurur və artefaktdakı bütün PoP-ları yoxlayır. İmzalayan indekslər ciddi artan
və sərhəd daxilində olmalıdır. CommitQC həm validator sayı, həm də səs gücü quorumunu
ödəməli, dəqiq Sumeragi v2 vote preimage üzərində BLS aggregate signature düzgün olmalıdır.

## Etibar lövbəri və ardıcıl yoxlama

Ayrı sübut yalnız daşıdığı roster altında daxili uyğunluğu göstərir.
`BridgeFinalityVerifier` ilk sübutdan əvvəl açıq şəkildə etibarlı `HeightContextId` tələb
edir. Sonra yalnız dərhal növbəti hündürlüyü qəbul edir və övlad context-in parent
CommitQC-sini əvvəlki dondurulmuş roster və PoP ilə yoxlayır. Epoch daxilində övlad
artifact əvvəlki artifact-ın PoP-larını kopyalayır; sərhəddə epoch, roster, quorum, seed və
PoP əvvəlki ana context-də CommitQC ilə autentifikasiya edilmiş `next_epoch_snapshot`-a,
o cümlədən `epoch_end_height`-a uyğun olmalıdır. Köhnə, atlanmış və əlaqəsiz hündürlüklər rədd edilir.

SCCP eyni `BridgeFinalityProof`-dan istifadə edir. Mesajın verdiyi roster altında imza
təkbaşına etibar deyil; governance ilə bərkidilmiş checkpoint context/artefaktından mesaj
artefaktına qədər hər dərhal ardıcıl keçid yoxlanmalıdır.

## Bundle və API

`BridgeFinalityBundle` dəqiq `{ commitment, finality_proof }` formasındadır. Commitment:
`{ chain_id, height_context_id, block_height, block_hash }`.

- `GET /v1/bridge/finality/{height}` `BridgeFinalityProof` qaytarır;
- `GET /v1/bridge/finality/bundle/{height}` `BridgeFinalityBundle` qaytarır.

Retained kanonik header və ya dəqiq davamlı v2 artefaktı yoxdursa və ya yanlışdırsa, hər iki
endpoint qapalı şəkildə uğursuz olur. Block-body eviction düzgün sübutu əlçatmaz etmir.
Naməlum sahələr, dəstəklənməyən versiyalar və köhnə sübut formaları rədd edilməlidir.
