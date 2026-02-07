---
lang: az
direction: ltr
source: docs/source/bridge_finality.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 2e4c6ed5974f623906f51259a634bcad5df703bcec899630ae29f4669b289ab6
source_last_modified: "2026-01-08T21:52:45.509525+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
SPDX-License-Identifier: Apache-2.0
-->

# Körpünün sonluğunun sübutları

Bu sənəd Iroha üçün ilkin körpünün sonluğunu sübut edən səthi təsvir edir.
Məqsəd xarici zəncirlərə və ya yüngül müştərilərə Iroha blokunun olduğunu yoxlamaq imkanı verməkdir.
zəncirdənkənar hesablamalar və ya etibarlı relelər olmadan yekunlaşdırılır.

## Sübut formatı

`BridgeFinalityProof` (Norito/JSON) ehtiva edir:

- `height`: blok hündürlüyü.
- `chain_id`: Çarpaz zəncir təkrarının qarşısını almaq üçün Iroha zəncir identifikatoru.
- `block_header`: kanonik `BlockHeader`.
- `block_hash`: başlığın hashı (müştərilər yoxlamaq üçün yenidən hesablayır).
- `commit_certificate`: təsdiqləyici dəsti + bloku tamamlayan imzalar.
- `validator_set_pops`: Doğrulayıcı dəstinə uyğunlaşdırılmış Sahiblik sübutu baytları
  sifariş (BLS məcmu yoxlaması üçün tələb olunur).

Sübut müstəqildir; heç bir xarici manifestlər və ya qeyri-şəffaf ləkələr tələb olunmur.
Saxlama: Torii son öhdəlik sertifikatı pəncərəsi üçün yekun sübutlara xidmət edir
(konfiqurasiya edilmiş tarix qapağı ilə məhdudlaşır; defolt olaraq vasitəsilə 512 giriş
`sumeragi.commit_cert_history_cap` / `SUMERAGI_COMMIT_CERT_HISTORY_CAP`). Müştərilər
Əgər daha uzun üfüqlərə ehtiyac varsa, sübutları önbelleğe almalı və ya lövbərləməlidir.
Kanonik tuple `(block_header, block_hash, commit_certificate)`-dir: the
Başlığın hashı öhdəçilik sertifikatının daxilindəki hash ilə uyğun olmalıdır və
zəncir identifikatoru sübutu bir kitab dəftərinə bağlayır. Serverlər rədd edir və daxil olur a
Sertifikat fərqli bloka işarə etdikdə `CommitCertificateHashMismatch`
hash.

## Öhdəlik paketi

`BridgeFinalityBundle` (Norito/JSON) əsas sübutu açıq şəkildə genişləndirir
öhdəlik və əsaslandırma:

- `commitment`: `{ chain_id, authority_set { id, validator_set, validator_set_hash, validator_set_hash_version }, block_height, block_hash, mmr_root?, mmr_leaf_index?, mmr_peaks?, next_authority_set? }`
- `justification`: öhdəlik üzərində müəyyən edilmiş səlahiyyətdən imzalar
  faydalı yük (təhsil-sertifikat imzalarını təkrar istifadə edir).
- `block_header`, `commit_certificate`: əsas sübut kimi.

Cari yer tutucu: `mmr_root`/`mmr_peaks` yenidən hesablanmaqla əldə edilir
yaddaşda blok-hash MMR; daxil edilmə sübutları hələ geri qaytarılmır. Müştərilər bilər
bu gün də öhdəlik yükü vasitəsilə eyni hashı yoxlayın.

MMR zirvələri soldan sağa sıralanır. Zirvələri yığmaqla `mmr_root`-i yenidən hesablayın
sağdan sola: `root = H(p_n, H(p_{n-1}, ... H(p_1, p_0)))`.

API: `GET /v1/bridge/finality/bundle/{height}` (Norito/JSON).

Doğrulama əsas sübuta bənzəyir: `block_hash`-i yenidən hesablayın
başlıq, öhdəlik-sertifikat imzalarını yoxlayın və öhdəliyi yoxlayın
sahələr sertifikat və blok hashına uyğun gəlir. Paket bir öhdəlik əlavə edir/
ayırmağa üstünlük verən körpü protokolları üçün əsaslandırma paketi.

## Doğrulama addımları1. `block_header`-dən `block_hash`-i yenidən hesablayın; uyğunsuzluğa görə rədd edin.
2. `commit_certificate.block_hash`-in yenidən hesablanmış `block_hash` ilə uyğunluğunu yoxlayın;
   uyğun olmayan başlıqları rədd edin/sertifikat cütlərini yerinə yetirin.
3. `chain_id`-in gözlənilən Iroha zəncirinə uyğunluğunu yoxlayın.
4. `commit_certificate.validator_set`-dən `validator_set_hash`-i yenidən hesablayın və
   onun qeydə alınmış hash/versiyaya uyğun olduğunu yoxlayın.
5. `validator_set_pops` uzunluğunun validator dəstinə uyğun olduğundan əmin olun və doğrulayın
   BLS açıq açarına qarşı hər bir PoP.
6. Başlıq hash-dən istifadə edərək öhdəçilik sertifikatında imzaları yoxlayın
   istinad edilən validator açıq açarları və indeksləri; kvorumu həyata keçirmək
   (`2f+1`, `n>3`, başqa `n`) və dublikat/diapazondan kənar indeksləri rədd edin.
7. Doğrulayıcı dəstinin hashini müqayisə edərək, istəyə görə etibarlı yoxlama məntəqəsinə qoşulun
   bağlanmış dəyərə (zəif subyektivlik lövbəri).
8. İsteğe bağlı olaraq gözlənilən dövr lövbərinə bağlayın, belə ki, köhnə/yenidən sübutlar
   lövbər qəsdən fırlanana qədər dövrlər rədd edilir.

`BridgeFinalityVerifier` (`iroha_data_model::bridge` ilə) bu çekləri tətbiq edir,
zəncir identifikatoru/hündürlük driftindən imtina, validator-set hash/versiya uyğunsuzluqları, çatışmayan
və ya etibarsız PoP-lər, dublikat/diapazondan kənar imzalayanlar, etibarsız imzalar və
kvorumu hesablamadan əvvəl gözlənilməz dövrlər
yoxlayıcı.

## İstinad yoxlayıcı

`BridgeFinalityVerifier` gözlənilən `chain_id` və isteğe bağlı etibarlı qəbul edir
validator dəsti və dövr ankerləri. Başlıq/blok-hash/ tətbiq edir.
commit-certificate tuple, validator-set hash/versiyasını yoxlayır, yoxlayır
reklam edilən təsdiqləyici siyahısına qarşı imzalar/kvorum və ən son izləyir
köhnəlmiş/atlanmış sübutları rədd etmək üçün hündürlük. Lövbərlər verildikdə rədd edir
açıq `UnexpectedEpoch`/ ilə dövrlər/kadrlar üzrə təkrarlar
`UnexpectedValidatorSet` səhvləri; lövbərsiz ilk sübutları qəbul edir
dublikatı tətbiq etməyə davam etməzdən əvvəl validator-set hash və epoxa
deterministik səhvlərlə diapazon/qeyri-kafi imzalar.

## API səthi

- `GET /v1/bridge/finality/{height}` - üçün `BridgeFinalityProof` qaytarır
  tələb olunan blok hündürlüyü. `Accept` vasitəsilə məzmun danışıqları Norito və ya dəstəkləyir
  JSON.
- `GET /v1/bridge/finality/bundle/{height}` - `BridgeFinalityBundle` qaytarır
  tələb olunan hündürlük üçün (öhdəlik + əsaslandırma + başlıq/sertifikat).

## Qeydlər və təqiblər

- Sübutlar hazırda saxlanılan öhdəlik sertifikatlarından əldə edilir. Sərhədli
  tarix öhdəliyin sertifikatının saxlanması pəncərəsini izləyir; müştərilər keş saxlamalıdırlar
  daha uzun üfüqlərə ehtiyac varsa, lövbər sübutları. Pəncərədən kənar sorğular geri qaytarılır
  `CommitCertificateNotFound(height)`; səhvi üzə çıxarın və bir vəziyyətə qayıdın
  lövbərlənmiş nəzarət məntəqəsi.
- Uyğun olmayan `block_hash` ilə təkrar və ya saxta sübut (başlıq vs.
  sertifikat) `CommitCertificateHashMismatch` ilə rədd edilir; müştərilər etməlidir
  imzanın yoxlanılmasından əvvəl eyni dəst yoxlamasını yerinə yetirin və atın
  uyğun olmayan yüklər.
- Gələcək iş sübut ölçüsünü azaltmaq üçün MMR/səlahiyyət tərəfindən müəyyən edilmiş öhdəlik zəncirləri əlavə edə bilər
  daha zəngin öhdəlik zərflərində öhdəlik sertifikatı.