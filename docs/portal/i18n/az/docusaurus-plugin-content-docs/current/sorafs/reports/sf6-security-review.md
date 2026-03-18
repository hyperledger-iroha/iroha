---
lang: az
direction: ltr
source: docs/portal/docs/sorafs/reports/sf6-security-review.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: SF-6 Security Review
summary: Findings and follow-up items from the independent assessment of keyless signing, proof streaming, and manifest submission pipelines.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# SF-6 Təhlükəsizlik İcmalı

**Qiymətləndirmə pəncərəsi:** 2026-02-10 → 2026-02-18  
**İnceləmə rəhbərləri:** Təhlükəsizlik Mühəndisliyi Gildiyası (`@sec-eng`), Alətlər üzrə İşçi Qrup (`@tooling-wg`)  
**Əhatə dairəsi:** SoraFS CLI/SDK (`sorafs_cli`, `sorafs_car`, `sorafs_manifest`), sübut axın API-ləri, Torii manifest idarəsi, Sigstore/OIDC inteqrasiyası, CI buraxma qarmaqları.  
**Artifaktlar:**  
- CLI mənbəyi və testləri (`crates/sorafs_car/src/bin/sorafs_cli.rs`)  
- Torii manifest/sübut işləyiciləri (`crates/iroha_torii/src/sorafs/api.rs`)  
- Buraxılış avtomatlaşdırılması (`ci/check_sorafs_cli_release.sh`, `scripts/release_sorafs_cli.sh`)  
- Deterministik paritet qoşqu (`crates/sorafs_car/tests/sorafs_cli.rs`, [SoraFS Orchestrator GA Parity Report](./orchestrator-ga-parity.md))

## Metodologiya

1. **Təhlükə modelləşdirmə seminarları** tərtibatçı iş stansiyaları, CI sistemləri və Torii qovşaqları üçün xəritələşdirilmiş hücumçu imkanları.  
2. **Kod nəzərdən keçirmə** etimadnamə səthlərinə (OIDC token mübadiləsi, açarsız imzalama), Norito manifest təsdiqi və sübut axını geri təzyiqinə yönəldilib.  
3. **Dinamik sınaq** paritet qoşqudan və sifarişli qeyri-səlis disklərdən istifadə edərək təkrar oxunan qurğu manifestləri və simulyasiya edilmiş uğursuzluq rejimləri (token təkrarı, manifest saxtakarlığı, kəsilmiş sübut axınları).  
4. **Konfiqurasiya təftişi** təsdiqlənmiş `iroha_config` defoltları, CLI bayrağı ilə işləmə və deterministik, yoxlanıla bilən qaçışları təmin etmək üçün buraxılış skriptləri.  
5. **Proses müsahibəsi** təsdiqlənmiş remediasiya axını, eskalasiya yolları və Tooling WG buraxılış sahibləri ilə audit sübutlarının əldə edilməsi.

## Tapıntıların xülasəsi

| ID | Ciddilik | Ərazi | Tapmaq | Qətnamə |
|----|----------|------|---------|------------|
| SF6-SR-01 | Yüksək | Açarsız imzalama | OIDC token auditoriyasının defoltları CI şablonlarında gizli idi və icarəçilər arası təkrar oynatma riski daşıyırdı. | Buraxılış qarmaqlarında və CI şablonlarında ([buraxılış prosesi](../developer-releases.md), `docs/examples/sorafs_ci.md`) açıq şəkildə `--identity-token-audience` tətbiqi əlavə edildi. Auditoriya buraxıldıqda CI indi uğursuz olur. |
| SF6-SR-02 | Orta | Sübut axını | Geri təzyiq yolları yaddaşın tükənməsinə imkan verən məhdudiyyətsiz abunəçi buferlərini qəbul etdi. | `sorafs_cli proof stream` deterministik kəsilmə ilə məhdud kanal ölçülərini tətbiq edir, Norito xülasələrini qeyd edir və axını dayandırır; Torii güzgü bağlı cavab hissələrinə yeniləndi (`crates/iroha_torii/src/sorafs/api.rs`). |
| SF6-SR-03 | Orta | Manifest təqdim | `--plan` olmadıqda CLI manifestləri daxil edilmiş yığın planlarını yoxlamadan qəbul etdi. | `sorafs_cli manifest submit` indi `--expect-plan-digest` təmin edilmədikcə CAR həzmlərini yenidən hesablayır və müqayisə edir, uyğunsuzluqları rədd edir və düzəliş göstərişlərini yerinə yetirir. Testlər müvəffəqiyyət/uğursuzluq hallarını əhatə edir (`crates/sorafs_car/tests/sorafs_cli.rs`). |
| SF6-SR-04 | Aşağı | Audit izi | Buraxılış yoxlama siyahısında təhlükəsizlik yoxlaması üçün imzalanmış təsdiq jurnalı yox idi. | Əlavə edilmiş [buraxılış prosesi](../developer-releases.md) GA-dan əvvəl nəzərdən keçirmə memo heşlərinin və giriş biletinin URL-sinin əlavə edilməsini tələb edən bölmə. |

Bütün yüksək/orta tapıntılar baxış pəncərəsi zamanı düzəldildi və mövcud paritet qoşqu vasitəsilə təsdiq edildi. Heç bir gizli kritik problem qalmayıb.

## Nəzarət Validasiyası

- **Etibarnamənin əhatə dairəsi:** Defolt CI şablonları indi açıq auditoriya və emitent təsdiqlərini mandat edir; `--identity-token-audience` `--identity-token-provider` ilə müşayiət olunmasa, CLI və buraxma köməkçisi hər ikisi tez uğursuz olur.  
- **Deterministik təkrar:** Yenilənmiş testlər müsbət/mənfi açıq-aşkar təqdimetmə axınlarını əhatə edir, uyğun olmayan həzmlərin qeyri-deterministik uğursuzluqlar olaraq qalmasını və şəbəkəyə toxunmadan əvvəl ortaya çıxmasını təmin edir.  
- **İsbatlı axın geri təzyiqi:** Torii indi PoR/PoTR elementlərini məhdud kanallar üzərindən ötürür və CLI yalnız kəsilmiş gecikmə nümunələrini + beş uğursuzluq nümunəsini saxlayır, deterministik xülasələri saxlayaraq abunəçilərin hədsiz artımının qarşısını alır.  
- **Müşahidə oluna bilənlik:** Sübut axını sayğacları (`torii_sorafs_proof_stream_*`) və CLI xülasələri operatorları audit qırıntıları ilə təmin edərək, dayandırılma səbəblərini qeyd edir.  
- **Sənədləşdirmə:** Tərtibatçı təlimatları ([developer indeksi](../developer-index.md), [CLI arayışı](../developer-cli.md)) təhlükəsizliyə həssas bayraqları və eskalasiya iş axınlarını çağırır.

## Buraxılış Yoxlama Siyahısına Əlavələr

Buraxılış menecerləri GA namizədini irəli sürərkən ** aşağıdakı sübutları əlavə etməlidirlər:

1. Ən son təhlükəsizlik araşdırması memo-nun hash (bu sənəd).  
2. İzlənən remediasiya biletinə keçid (məsələn, `governance/tickets/SF6-SR-2026.md`).  
3. Açıq auditoriya/emitent arqumentlərini göstərən `scripts/release_sorafs_cli.sh --manifest ... --bundle-out ... --signature-out ...` çıxışı.  
4. Paritet qoşqundan tutulan qeydlər (`cargo test -p sorafs_car -- --nocapture sorafs_cli::proof_stream::bounded_channels`).  
5. Torii buraxılış qeydlərinə məhdud sübut axını telemetriya sayğaclarının daxil olmasının təsdiqi.

Yuxarıdakı artefaktların toplanmaması GA imzalanmasını bloklayır.

**İstinad artefakt həşləri (2026-02-20-dən buraxılış):**

- `sf6_security_review.md` — `66001d0b53d8e7ed5951a07453121c075dea931ca44c11f1fcd1571ed827342a`

## Görkəmli İzləmələr

- **Təhlükə modelinin yenilənməsi:** Bu baxışı rüblük və ya əsas CLI bayrağı əlavələrindən əvvəl təkrarlayın.  
- **Fuzzing əhatə dairəsi:** Sübut axını nəqliyyat kodlaşdırmaları `fuzz/proof_stream_transport` vasitəsilə identifikasiya, gzip, deflate və zstd faydalı yükləri əhatə edir.  
- **Hadisə məşqi:** Sənədləşmənin praktiki prosedurları əks etdirməsini təmin edərək, token kompromis və açıq-aşkar geri çəkilməni simulyasiya edən operator məşqi planlaşdırın.

## Təsdiq

- Təhlükəsizlik Mühəndisliyi Gildiyasının nümayəndəsi: @sec-eng (2026-02-20)  
- Alətlər üzrə İşçi Qrupun nümayəndəsi: @tooling-wg (2026-02-20)

İmzalanmış təsdiqləri buraxılış artefakt paketi ilə birlikdə saxlayın.