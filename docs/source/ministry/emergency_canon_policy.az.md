---
lang: az
direction: ltr
source: docs/source/ministry/emergency_canon_policy.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: afd5db8a761f8cc56dcd8f67f047f06b45c3211738fedb6dfff326d84b4e8a68
source_last_modified: "2026-01-22T14:45:02.098548+00:00"
translation_last_reviewed: 2026-02-07
title: Emergency Canon & TTL Policy
summary: Reference implementation notes for roadmap item MINFO-6a covering denylist tiers, TTL enforcement, and governance evidence requirements.
translator: machine-google-reviewed
---

# Fövqəladə Canon və TTL Siyasəti (MINFO-6a)

Yol xəritəsi arayışı: **MINFO-6a — Fövqəladə hallar qaydası və TTL siyasəti**.

Bu sənəd indi Torii və CLI-də göndərilən ləğvetmə siyahısı qaydalarını, TTL tətbiqini və idarəetmə öhdəliklərini müəyyən edir. Operatorlar yeni qeydləri dərc etməzdən və ya fövqəladə hallara müraciət etməzdən əvvəl bu qaydalara əməl etməlidirlər.

## Səviyyə Tərifləri

| Səviyyə | Defolt TTL | Baxış pəncərəsi | Tələblər |
|------|-------------|---------------|--------------|
| Standart | 180 gün (`torii.sorafs_gateway.denylist.standard_ttl`) | yox | `issued_at` təmin edilməlidir. `expires_at` buraxıla bilər; Torii defolt olaraq `issued_at + standard_ttl` olaraq təyin edilir və daha uzun pəncərələri rədd edir. |
| Təcili | 30 gün (`torii.sorafs_gateway.denylist.emergency_ttl`) | 7 gün (`torii.sorafs_gateway.denylist.emergency_review_window`) | Əvvəlcədən təsdiqlənmiş kanona istinad edən boş olmayan `emergency_canon` etiketi tələb edir (məsələn, `csam-hotline`). `issued_at` + `expires_at` 30 günlük pəncərənin içərisinə enməlidir və yoxlama sübutları avtomatik yaradılan son tarixə istinad etməlidir (`issued_at + review_window`). |
| Daimi | İstifadə müddəti yoxdur | yox | Fövqəladə idarəetmə qərarları üçün qorunur. Yazılarda boş olmayan `governance_reference` (səs id, manifest hash və s.) istinad edilməlidir. `expires_at` rədd edilir. |

Defolt parametrlər `torii.sorafs_gateway.denylist.*` vasitəsilə konfiqurasiya edilə bilər və `iroha_cli` Torii faylı yenidən yükləməzdən əvvəl etibarsız girişləri tutmaq üçün məhdudiyyətləri əks etdirir.

## İş axını

1. **Metadata hazırlayın:** `policy_tier`, `issued_at`, `expires_at` (mümkün olduqda) və `emergency_canon`/`governance_reference` (hər JSON8030 daxilində) daxildir.
2. **Yerli olaraq doğrulayın:** `iroha app sorafs gateway lint-denylist --path <denylist.json>`-i işə salın ki, CLI səviyyəyə aid TTL-ləri və tələb olunan sahələri faylın yerinə yetirilməsindən və ya bərkidilməsindən əvvəl tətbiq etsin.
3. **Dəlilləri dərc edin:** auditorların qərarı izləyə bilməsi üçün GAR işi paketinə girişdə göstərilən qanun id və ya idarəetmə arayışını əlavə edin (gündəm paketi, referendum protokolları və s.).
4. **Fövqəladə hallara dair qeydləri nəzərdən keçirin:** fövqəladə hallar qaydaları 30 gün ərzində avtomatik başa çatır. Operatorlar 7 günlük pəncərədə post-fakto baxışını tamamlamalı və nəticəni nazirlik izləyicisində/SoraFS sübut anbarında qeyd etməlidirlər.
5. **Torii-i yenidən yükləyin:** təsdiqləndikdən sonra, `torii.sorafs_gateway.denylist.path` vasitəsilə rədd siyahısı yolunu yerləşdirin və Torii-i yenidən başladın/yenidən yükləyin; iş vaxtı girişləri qəbul etməzdən əvvəl eyni məhdudiyyətləri tətbiq edir.

## Alətlər və İstinadlar

- İcra zamanı siyasətinin tətbiqi `sorafs::gateway::denylist` (`crates/iroha_torii/src/sorafs/gateway/denylist.rs`)-də yaşayır və yükləyici indi `torii.sorafs_gateway.denylist.*` girişlərini təhlil edərkən səviyyəli metadata tətbiq edir.
- CLI doğrulaması `GatewayDenylistRecord::validate` (`crates/iroha_cli/src/commands/sorafs.rs`) daxilində işləmə semantikasını əks etdirir. TTL-lər konfiqurasiya edilmiş pəncərəni aşdıqda və ya məcburi kanon/idarəetmə istinadları olmadıqda linter uğursuz olur.
- Konfiqurasiya düymələri `torii.sorafs_gateway.denylist` (`crates/iroha_config/src/parameters/{defaults,actual.rs,user.rs}`) altında müəyyən edilmişdir ki, idarəetmə fərqli sərhədləri təsdiq edərsə, operatorlar TTL-ləri düzəldə/nəzərdən keçirə bilsinlər.
- İctimai nümunə rədd siyahısı (`docs/source/sorafs_gateway_denylist_sample.json`) indi hər üç səviyyəni göstərir və yeni girişlər üçün kanonik şablon kimi istifadə edilməlidir.Bu qoruyucu barmaqlıqlar fövqəladə qanunlar siyahısını kodlaşdırmaqla, qeyri-məhdud TTL-lərin qarşısını almaqla və daimi bloklar üçün açıq idarəetmə sübutlarını məcbur etməklə yol xəritəsinin **MINFO-6a** maddəsini təmin edir.

## Qeydiyyatın avtomatlaşdırılması və sübutların ixracı

Fövqəladə hallar üzrə qanun təsdiqləri deterministik reyestr snapshotunu və a
diff paketi Torii inkar siyahısını yükləməzdən əvvəl. Aşağıdakı alətlər
`xtask/src/sorafs.rs` plus CI qoşqu `ci/check_sorafs_gateway_denylist.sh`
bütün iş prosesini əhatə edir.

### Kanonik paket generasiyası

1. İşləyən sənəddə xam qeydləri (adətən idarəetmə tərəfindən nəzərdən keçirilən fayl) mərhələli edin
   kataloq.
2. JSON-u qanuniləşdirin və möhürləyin:
   ```bash
   cargo xtask sorafs-gateway denylist pack \
     --input path/to/denylist.json \
     --out artifacts/ministry/denylist_registry/$(date +%Y%m%dT%H%M%SZ) \
     --label ministry-emergency \
     --force
   ```
   Komanda imzalanmış dostluq `.json` paketini, Norito `.to` yayır
   zərf və idarəetmə rəyçiləri tərəfindən gözlənilən Merkle kök mətn faylı.
   Kataloqu `artifacts/ministry/denylist_registry/` (və ya sizin
   seçilmiş sübut kovası) buna görə də `scripts/ministry/transparency_release.py` edə bilər
   onu daha sonra `--artifact denylist_bundle=<path>` ilə götürün.
3. Yaradılmış `checksums.sha256`-i itələməzdən əvvəl paketin yanında saxlayın
   SoraFS/GAR-a. CI-nin `ci/check_sorafs_gateway_denylist.sh` eyni məşq edir
   `pack` alətlərin işləməsinə zəmanət vermək üçün nümunə rədd siyahısına qarşı köməkçi
   hər buraxılış.

### Fərq + audit paketi

1. Yeni paketi istifadə edərək əvvəlki istehsal snapshotu ilə müqayisə edin
   xtask fərq köməkçisi:
   ```bash
   cargo xtask sorafs-gateway denylist diff \
     --old artifacts/ministry/denylist_registry/2026-05-01/denylist_old.json \
     --new artifacts/ministry/denylist_registry/2026-05-14/denylist_new.json \
     --report-json artifacts/ministry/denylist_registry/2026-05-14/denylist_diff.json
   ```
   JSON hesabatı bütün əlavələri/çıxarmaları sadalayır və sübutları əks etdirir
   `MinistryDenylistChangeV1` tərəfindən istehlak edilən struktur (istinad edir
   `docs/source/sorafs_gateway_self_cert.md` və uyğunluq planı).
2. Hər kanon sorğusuna `denylist_diff.json` əlavə edin (bu, nə qədər olduğunu sübut edir
   girişlərə toxunuldu, hansı səviyyə dəyişdi və hansı dəlil hash xəritələri
   kanonik dəstə).
3. Fərqlər avtomatik olaraq yaradıldıqda (CI və ya buraxılış boru kəmərləri) ixrac edin
   `denylist_diff.json` yolu `--artifact denylist_diff=<path>` vasitəsilə
   şəffaflıq manifestində onu təmizlənmiş ölçülərlə yanaşı qeyd edir. Eyni CI
   köməkçi CLI xülasə addımını işlədən `--evidence-out <path>`-i qəbul edir və
   nəticədə əldə edilən JSON-u sonradan dərc etmək üçün tələb olunan yerə köçürür.

### Nəşr və şəffaflıq1. Paketi + fərq artefaktlarını rüblük şəffaflıq kataloquna atın
   (`artifacts/ministry/transparency/<YYYY-Q>/denylist/`). Şəffaflıq
   azad köməkçisi sonra onları daxil edə bilər:
   ```bash
   scripts/ministry/transparency_release.py \
     --quarter 2026-Q3 \
     --output-dir artifacts/ministry/transparency/2026-Q3 \
     --sanitized artifacts/ministry/transparency/2026-Q3/sanitized_metrics.json \
     --dp-report artifacts/ministry/transparency/2026-Q3/dp_report.json \
     --artifact denylist_bundle=artifacts/ministry/denylist_registry/2026-05-14/denylist_new.json \
     --artifact denylist_diff=artifacts/ministry/denylist_registry/2026-05-14/denylist_diff.json
   ```
2. Rüblük hesabatda yaradılan paketə/fərqə istinad edin
   (`docs/source/ministry/reports/<YYYY-Q>.md`) və eyni yolları əlavə edin
   GAR səs paketi, beləliklə auditorlar dəlil izini giriş olmadan təkrarlaya bilsinlər
   daxili CI. `ci/check_sorafs_gateway_denylist.sh --evidence-out \
   artefacts/ministry/denylist_registry//denylist_evidence.json` indi
   paket/fərq/dəlil quru-çalışmasını yerinə yetirir (`iroha_cli tətbiqi sorafs şlüzünü çağırır)
   sübut` başlıq altında) belə ki, avtomatlaşdırma xülasə ilə yanaşı davam edə bilər
   kanonik dəstələr.
3. Nəşr edildikdən sonra idarəetmə yükünü vasitəsilə birləşdirin
   `cargo xtask ministry-transparency anchor` (avtomatik olaraq çağırılır
   `transparency_release.py`, `--governance-dir` təmin edildikdə)
   denylist reyestrinin həzmi şəffaflıqla eyni DAG ağacında görünür
   azad edin.

Bu prosesdən sonra “reyestr avtomatlaşdırılması və sübut ixracı” bağlanır.
boşluq `roadmap.md:450`-də səslənir və hər bir fövqəladə halın olmasını təmin edir
qərar təkrarlana bilən artefaktlar, JSON fərqləri və şəffaflıq jurnalı ilə gəlir
girişlər.

### TTL və Canon Evidence Helper

Paket/fərq cütü istehsal edildikdən sonra tutmaq üçün CLI sübut köməkçisini işə salın
İdarəetmənin tələb etdiyi TTL xülasələri və fövqəladə halların nəzərdən keçirilməsi üçün son tarixlər:

```bash
iroha app sorafs gateway evidence \
  --denylist artifacts/ministry/denylist_registry/2026-05-14/denylist.json \
  --out artifacts/ministry/denylist_registry/2026-05-14/denylist_evidence.json \
  --label csam-canon-2026-05
```

Komanda mənbə JSON-u hash edir, hər girişi təsdiqləyir və kompakt buraxır
ehtiva edən xülasə:

- `kind` və ən erkən/ən son olan siyasət səviyyəsi üzrə ümumi girişlər
  vaxt nişanları müşahidə edilir.
- `emergency_reviews[]` siyahısı, hər bir fövqəladə halı öz ilə sadalayır
  deskriptor, effektiv bitmə, icazə verilən maksimum TTL və hesablanmış
  `review_due_by` son tarix.

`denylist_evidence.json`-i paketlənmiş paketin/diffin yanında əlavə edin ki, auditorlar
CLI-ni yenidən işə salmadan TTL uyğunluğunu təsdiqləyin. Artıq yaranan CI işləri
paketlər köməkçiyə müraciət edə və sübut artefaktını dərc edə bilər (məsələn
`ci/check_sorafs_gateway_denylist.sh --evidence-out <path>` zəng edərək), təmin edilməsi
hər kanon sorğusu ardıcıl xülasə ilə düşür.

### Merkle reyestrinin sübutu

MINFO-6-da təqdim edilən Merkle reyestri operatorlardan məlumatları dərc etməyi tələb edir
TTL xülasəsi ilə yanaşı kök və hər giriş sübutları. Qaçışdan dərhal sonra
sübut köməkçisi, Merkle artefaktlarını ələ keçirin:

```bash
iroha app sorafs gateway merkle snapshot \
  --denylist artifacts/ministry/denylist_registry/2026-05-14/denylist.json \
  --json-out artifacts/ministry/denylist_registry/2026-05-14/denylist_merkle_snapshot.json \
  --norito-out artifacts/ministry/denylist_registry/2026-05-14/denylist_merkle_snapshot.to

iroha app sorafs gateway merkle proof \
  --denylist artifacts/ministry/denylist_registry/2026-05-14/denylist.json \
  --index 12 \
  --json-out artifacts/ministry/denylist_registry/2026-05-14/denylist_merkle_proof_entry_12.json \
  --norito-out artifacts/ministry/denylist_registry/2026-05-14/denylist_merkle_proof_entry_12.to
```Snapshot JSON BLAKE3 Merkle kökünü, yarpaq sayını və hər birini qeyd edir
deskriptor/hash cütü beləliklə GAR səsləri hash edilmiş dəqiq ağaca istinad edə bilsin.
`--norito-out`-in təchizatı JSON ilə yanaşı `.to` artefaktını saxlayır,
şlüzlər reyestr qeydlərini qırmadan birbaşa Norito vasitəsilə qəbul edir
stdout. `merkle proof` istənilən üçün istiqamət bitlərini və bacı-qardaş heşləri yayır
sıfır əsaslı giriş indeksi, hər biri üçün əlavə sübut əlavə etməyi asanlaşdırır
GAR memo-da qeyd olunan fövqəladə vəziyyət qanunu - isteğe bağlı Norito nüsxəsi sübutu saxlayır
kitabçada paylanmağa hazırdır. Sonra JSON və Norito artefaktlarını saxlayın
TTL xülasəsi və fərq paketinə, beləliklə, şəffaflıq buraxılışları və idarəetmə
lövbərlər eyni kökə istinad edir.