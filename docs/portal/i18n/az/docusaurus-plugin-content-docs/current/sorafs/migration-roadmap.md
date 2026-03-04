---
lang: az
direction: ltr
source: docs/portal/docs/sorafs/migration-roadmap.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: "SoraFS Migration Roadmap"
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

> [`docs/source/sorafs/migration_roadmap.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/migration_roadmap.md)-dən uyğunlaşdırılıb.

# SoraFS Miqrasiya Yol Xəritəsi (SF-1)

Bu sənəddə əldə edilən miqrasiya təlimatı fəaliyyət göstərir
`docs/source/sorafs_architecture_rfc.md`. SF-1 çatdırılmalarını genişləndirir
icraya hazır mərhələlər, keçid meyarları və sahiblərin yoxlama siyahıları, beləliklə, saxlama,
SoraFS tərəfindən dəstəklənən nəşrə ev sahibliyi edən artefakt.

Yol xəritəsi qəsdən deterministikdir: hər bir mərhələ tələb olunanı adlandırır
artefaktlar, əmr çağırışları və attestasiya addımları belə aşağı axın boru kəmərləri
eyni nəticələr verir və idarəetmə audit edilə bilən izi saxlayır.

## Mərhələ İcmal

| Mərhələ | Pəncərə | Əsas Məqsədlər | Göndərilməli | Sahiblər |
|----------|--------|---------------|-----------|--------|
| **M1 – Deterministik İcra** | 7-12 həftə | Boru kəmərləri gözlənilən bayraqları qəbul edərkən, imzalanmış qurğuları və səhnə ləqəbi sübutlarını tətbiq edin. | Gecə fikstür yoxlanışı, şura tərəfindən imzalanmış manifestlər, ləqəb reyestrinin səhnələşdirmə girişləri. | Saxlama, İdarəetmə, SDK |

Mərhələ statusu `docs/source/sorafs/migration_ledger.md`-də izlənilir. Hamısı
bu yol xəritəsinə edilən dəyişikliklər idarəetməni saxlamaq və buraxmaq üçün kitabı yeniləməli olmalıdır
sinxron mühəndislik.

## İş axını

### 2. Deterministik Pinninqə Qəbul Edilməsi

| Addım | Mərhələ | Təsvir | Sahib(lər) | Çıxış |
|------|-----------|-------------|----------|--------|
| Quraşdırma məşqləri | M0 | Yerli parça həzmlərini `fixtures/sorafs_chunker` ilə müqayisə edən həftəlik quru qaçışlar. `docs/source/sorafs/reports/` altında hesabat dərc edin. | Saxlama Provayderləri | `determinism-<date>.md` keçid/uğursuz matrisi ilə. |
| İmzaların tətbiqi | M1 | İmzalar və ya sürüşmə aşkar edilərsə, `ci/check_sorafs_fixtures.sh` + `.github/workflows/sorafs-fixtures-nightly.yml` uğursuz olur. İnkişafı ləğv etmək üçün PR-ə əlavə edilmiş idarəetmədən imtina tələb olunur. | Tooling WG | CI jurnalı, imtina biletinin linki (əgər varsa). |
| Gözləmə bayraqları | M1 | Boru kəmərləri çıxışları bağlamaq üçün açıq gözləntilərlə `sorafs_manifest_stub` çağırır: | Sənədlər CI | Gözləmə bayraqlarına istinad edən yenilənmiş skriptlər (aşağıdakı əmr blokuna baxın). |
| Reyestrdə ilk sancma | M2 | `sorafs pin propose` və `sorafs pin approve` manifest təqdimatlarını əhatə edir; CLI defolt olaraq `--require-registry`-dir. | İdarəetmə Əməliyyatları | Reyestr CLI audit jurnalı, uğursuz təkliflər üçün telemetriya. |
| Müşahidə oluna bilən paritet | M3 | Prometheus/Grafana idarə panelləri yığın inventarları reyestr manifestlərindən ayrıldıqda xəbərdar edir; çağırış üzrə əməliyyatlara ötürülən xəbərdarlıqlar. | Müşahidə qabiliyyəti | İdarə paneli linki, xəbərdarlıq qayda identifikatorları, GameDay nəticələri. |

#### Kanonik nəşriyyat əmri

```bash
cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- docs/book \
  --manifest-out artifacts/docs/book/2025-11-01/docs.manifest \
  --manifest-signatures-out artifacts/docs/book/2025-11-01/docs.manifest_signatures.json \
  --car-out artifacts/docs/book/2025-11-01/docs.car \
  --chunk-fetch-plan-out artifacts/docs/book/2025-11-01/docs.fetch_plan.json \
  --car-digest=13fa919c67e55a2e95a13ff8b0c6b40b2e51d6ef505568990f3bc7754e6cc482 \
  --car-size=429391872 \
  --root-cid=f40101... \
  --dag-codec=0x71
```

Həzm, ölçü və CID dəyərlərini qeyd edilmiş gözlənilən istinadlarla əvəz edin
artefakt üçün miqrasiya kitabçası girişi.

### 3. Ləqəb Keçid və Əlaqələr

| Addım | Mərhələ | Təsvir | Sahib(lər) | Çıxış |
|------|-----------|-------------|----------|--------|
| Səhnələşdirmədə ləqəb sübutları | M1 | Pin Registry quruluş mühitində ləqəb iddialarını qeyd edin və manifestlərə Merkle sübutlarını əlavə edin (`--alias`). | İdarəetmə, Sənədlər | Sübut paketi manifest + ləqəb adı ilə kitab şərhinin yanında saxlanılır. |
| Sübutun icrası | M2 | Şlüzlər təzə `Sora-Proof` başlıqları olmayan manifestləri rədd edir; CI sübutları əldə etmək üçün `sorafs alias verify` addımını qazanır. | Şəbəkə | Şlüz konfiqurasiya yaması + doğrulama uğurunu əldə edən CI çıxışı. |

### 4. Rabitə və Audit

- **Mühasibat uçotu nizam-intizamı:** hər bir vəziyyət dəyişikliyi (fiksator sürüşməsi, reyestrin təqdim edilməsi,
  ləqəb aktivləşdirilməsi) tarixli qeyd əlavə etməlidir
  `docs/source/sorafs/migration_ledger.md`.
- **İdarəetmə protokolu:** pin reyestrində dəyişiklikləri təsdiq edən şura iclasları və ya
  ləqəb siyasətləri həm bu yol xəritəsinə, həm də kitab kitabçasına istinad etməlidir.
- **Xarici rabitə:** DevRel hər bir mərhələdə status yeniləmələrini dərc edir (blog +
  dəyişiklik qeydindən çıxarış) deterministik zəmanətləri və ləqəb vaxt qrafiklərini vurğulayır.

## Asılılıqlar və Risklər

| Asılılıq | Təsir | Azaldılması |
|------------|--------|------------|
| Pin Registry müqaviləsinin mövcudluğu | M2 pin-ilk buraxılışını bloklayır. | Təkrar testləri ilə M2-dən əvvəl mərhələli müqavilə; reqressiya olmayana qədər zərf geri qaytarılmasını təmin edin. |
| Şura imza açarları | Manifest zərfləri və reyestr təsdiqləri üçün tələb olunur. | `docs/source/sorafs/signing_ceremony.md` sənədində imzalanma mərasimi; üst-üstə düşmə və kitab qeydi ilə düymələri döndürün. |
| SDK buraxılış kadansı | Müştərilər M3-dən əvvəl ləqəb sübutlarına riayət etməlidirlər. | SDK buraxılış pəncərələrini mərhələ qapıları ilə uyğunlaşdırın; şablonları buraxmaq üçün miqrasiya yoxlama siyahıları əlavə edin. |

Qalıq risklər və azalma tədbirləri `docs/source/sorafs_architecture_rfc.md`-də əks olunur
və düzəlişlər edildikdə çarpaz istinad edilməlidir.

## Meyarlar Yoxlama Siyahısından çıxın

| Mərhələ | Meyarlar |
|----------|----------|
| M1 | - Ardıcıl yeddi gün ərzində gecə fikstür işi yaşıl. <br /> - CI-də təsdiqlənmiş ləqəb sübutları. <br /> - İdarəetmə gözləmə bayrağı siyasətini təsdiq edir. |

## Dəyişiklik İdarəetmə

1. Bu faylı yeniləyərək PR vasitəsilə düzəlişlər təklif edin **və**
   `docs/source/sorafs/migration_ledger.md`.
2. PR təsvirində idarəetmə protokollarını və CI sübutlarını dəstəkləyən əlaqə.
3. Birləşmə zamanı xülasə və gözlənilən yaddaşa + DevRel poçt siyahısına bildirin
   operator hərəkətləri.

Bu prosedurdan sonra SoraFS buraxılışının deterministik qalmasını təmin edir,
Nexus buraxılışında iştirak edən komandalar arasında audit edilə bilən və şəffafdır.