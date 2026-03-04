---
lang: az
direction: ltr
source: docs/source/jdg_sdn.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 1ee87ee60e2e8c9d9636b282231b33de3cf1fd7240c8d31d0a0a1673651dcef1
source_last_modified: "2025-12-29T18:16:35.972838+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

% JDG-SDN sertifikatları və rotasiya

Bu qeyd Secret Data Node (SDN) sertifikatları üçün icra modelini əks etdirir
Yurisdiksiya Data Guardian (JDG) axını tərəfindən istifadə olunur.

## Öhdəlik formatı
- `JdgSdnCommitment` əhatə dairəsini birləşdirir (`JdgAttestationScope`), şifrələnmiş
  faydalı yük hash və SDN açıq açarı. Möhürlər çap edilmiş imzalardır
  (`SignatureOf<JdgSdnCommitmentSignable>`) domen etiketli faydalı yük üzərində
  `iroha:jurisdiction:sdn:commitment:v1\x00 || norito(signable)`.
- Struktur təsdiqləmə (`validate_basic`) tətbiq edir:
  - `version == JDG_SDN_COMMITMENT_VERSION_V1`
  - etibarlı blok diapazonları
  - boş olmayan möhürlər
  - vasitəsilə həyata keçirildikdə attestasiyaya qarşı əhatə dairəsi bərabərliyi
    `JdgAttestation::validate_with_sdn`/`validate_with_sdn_registry`
- Təkmilləşdirmə attestasiya təsdiqləyicisi tərəfindən idarə olunur (imzalayan+faydalı yük hash
  unikallıq) saxlanılan/təkrarlanan öhdəliklərin qarşısını almaq üçün.

## Qeydiyyat və rotasiya siyasəti
- SDN açarları `(Algorithm, public_key_bytes)` ilə idarə olunan `JdgSdnRegistry`-də yaşayır.
- `JdgSdnKeyRecord` aktivləşdirmə hündürlüyünü, isteğe bağlı pensiya hündürlüyünü qeyd edir,
  və isteğe bağlı ana açar.
- Rotasiya `JdgSdnRotationPolicy` tərəfindən idarə olunur (hazırda: `dual_publish_blocks`
  üst-üstə düşən pəncərə). Uşaq açarının qeydiyyatı valideyn pensiyasını yeniləyir
  `child.activation + dual_publish_blocks`, qoruyucularla:
  - itkin düşən valideynlər rədd edilir
  - aktivləşdirmələr ciddi şəkildə artırılmalıdır
  - lütf pəncərəsini aşan üst-üstə düşmələr rədd edilir
- Reyestr köməkçiləri status üçün quraşdırılmış qeydləri (`record`, `keys`) üzə çıxarır
  və API məruz qalması.

## Doğrulama axını
- `JdgAttestation::validate_with_sdn_registry` strukturu əhatə edir
  attestasiya yoxlamaları və SDN tətbiqi. `JdgSdnPolicy` mövzuları:
  - `require_commitments`: PII/gizli yüklər üçün mövcudluğu təmin edin
  - `rotation`: valideyn pensiyasını yeniləyərkən istifadə olunan lütf pəncərəsi
- Hər bir öhdəlik aşağıdakılar üçün yoxlanılır:
  - struktur etibarlılıq + attestasiya-miqyas uyğunluğu
  - qeydiyyatdan keçmiş açar mövcudluğu
  - təsdiqlənmiş blok diapazonunu əhatə edən aktiv pəncərə (təqaüd hədləri artıq
    ikili nəşr lütfünü daxil edin)
  - domen etiketli öhdəlik orqanının üzərində etibarlı möhür
- Sabit xətalar operator sübutları üçün indeksi üzə çıxarır:
  `MissingSdnCommitments`, `UnknownSdnKey`, `InactiveSdnKey`, `InvalidSeal`,
  və ya struktur `Commitment`/`ScopeMismatch` nasazlıqları.

## Operator runbook
- **Təminat:** ilk SDN açarını `activated_at` ilə və ya ondan əvvəl qeydiyyatdan keçirin
  birinci gizli blokun hündürlüyü. Əsas barmaq izini JDG operatorlarına dərc edin.
- **Rotate:** varisi açarı yaradın, onu `rotation_parent` ilə qeydiyyatdan keçirin
  cari açarı göstərin və valideyn pensiyasının bərabər olduğunu təsdiqləyin
  `child_activation + dual_publish_blocks`. Faydalı yük öhdəliklərini ilə yenidən möhürləyin
  üst-üstə düşən pəncərə zamanı aktiv açar.
- **Audit:** Torii/status vasitəsilə reyestr şəkillərini (`record`, `keys`) ifşa edin
  auditorların aktiv açarı və ehtiyat pəncərələrini təsdiq edə bilməsi üçün səthlər. Xəbərdarlıq
  təsdiq edilmiş diapazon aktiv pəncərədən kənara düşərsə.
- **Bərpa:** `UnknownSdnKey` → reyestrdə möhürləmə açarının daxil olmasını təmin edin;
  `InactiveSdnKey` → aktivləşdirmə hündürlüklərini fırladın və ya tənzimləyin; `InvalidSeal` →
  faydalı yükləri yenidən möhürləyin və sertifikatları təzələyin.## İş vaxtı köməkçisi
- `JdgSdnEnforcer` (`crates/iroha_core/src/jurisdiction.rs`) siyasəti paketləşdirir +
  reyestr və `validate_with_sdn_registry` vasitəsilə attestasiyaları təsdiqləyir.
- Qeydiyyatlar Norito kodlu `JdgSdnKeyRecord` paketlərindən yüklənə bilər (bax.
  `JdgSdnEnforcer::from_reader`/`from_path`) və ya yığılmışdır
  Qeydiyyat zamanı fırlanma qoruyucularını tətbiq edən `from_records`.
- Operatorlar Norito paketini Torii/status üçün sübut kimi saxlaya bilərlər
  surfacing isə eyni faydalı yük qəbul tərəfindən istifadə icraçı qidalanır və
  konsensus mühafizəçiləri. Vahid qlobal enforcer vasitəsilə başlanğıcda işə salına bilər
  `init_enforcer_from_path` və `enforcer()`/`registry_snapshot()`/`sdn_registry_status()`
  canlı siyasəti + status/Torii səthləri üçün əsas qeydləri ifşa edin.

## Testlər
- `crates/iroha_data_model/src/jurisdiction.rs`-də reqressiya əhatəsi:
  `sdn_registry_accepts_active_commitment`, `sdn_registry_rejects_unknown_key`,
  `sdn_registry_rejects_inactive_key`, `sdn_registry_rejects_bad_signature`,
  `sdn_registry_sets_parent_retirement_window`,
  `sdn_registry_rejects_overlap_beyond_policy`, mövcud ilə yanaşı
  struktur attestasiya/SDN doğrulama testləri.