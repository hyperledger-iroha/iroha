---
lang: az
direction: ltr
source: docs/examples/sorafs_release_notes.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 303a947895c10c7673b98e9187c3431c4012093c69d899252c121b53f9c48bb1
source_last_modified: "2026-01-05T09:28:11.823299+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# SoraFS CLI & SDK — Buraxılış Qeydləri (v0.1.0)

## Əsas məqamlar
- `sorafs_cli` indi bütün qablaşdırma boru kəmərini əhatə edir (`car pack`, `manifest build`,
  `proof verify`, `manifest sign`, `manifest verify-signature`) beləliklə, CI qaçışçıları
  sifarişli köməkçilər əvəzinə tək binar. Yeni açarsız imzalama axını standartdır
  `SIGSTORE_ID_TOKEN`, GitHub Fəaliyyətləri OIDC provayderlərini başa düşür və deterministik məlumat yayır
  imza paketinin yanında xülasə JSON.
- `sorafs_car`-in bir hissəsi kimi çoxmənbəli gətirmə *sürət lövhəsi* göndərilir: normallaşır
  provayder telemetriyası, qabiliyyət cəzalarını tətbiq edir, JSON/Norito hesabatlarını davam etdirir və
  paylaşılan qeyd dəftəri vasitəsilə orkestr simulyatorunu (`sorafs_fetch`) qidalandırır.
  `fixtures/sorafs_manifest/ci_sample/` altındakı qurğular deterministi nümayiş etdirir
  CI/CD-nin fərqli olacağı gözlənilən giriş və çıxışlar.
- Buraxılış avtomatlaşdırılması `ci/check_sorafs_cli_release.sh` və kodlaşdırılmışdır
  `scripts/release_sorafs_cli.sh`. Hər buraxılış indi manifest paketini arxivləşdirir,
  imza, `manifest.sign/verify` xülasəsi və tablonun snapshot belə idarəetmə
  rəyçilər boru kəmərini yenidən işə salmadan artefaktları izləyə bilərlər.

## Təkmilləşdirmə Addımları
1. İş yerinizdə düzlənmiş qutuları yeniləyin:
   ```bash
   cargo update -p sorafs_car@0.1.0 --precise 0.1.0
   cargo update -p sorafs_manifest@0.1.0 --precise 0.1.0
   cargo update -p sorafs_chunker@0.1.0 --precise 0.1.0
   ```
2. Fmt/clippy/test əhatəsini təsdiqləmək üçün buraxma qapısını yerli (və ya CI-də) yenidən işə salın:
   ```bash
   CARGO_TARGET_DIR=.target ci/check_sorafs_cli_release.sh \
     | tee artifacts/sorafs_cli_release/v0.1.0/ci-check.log
   ```
3. İmzalanmış artefaktları və xülasələri seçilmiş konfiqurasiya ilə bərpa edin:
   ```bash
   scripts/release_sorafs_cli.sh \
     --config docs/examples/sorafs_cli_release.conf \
     --manifest fixtures/sorafs_manifest/ci_sample/manifest.to \
     --chunk-plan fixtures/sorafs_manifest/ci_sample/chunk_plan.json \
     --chunk-summary fixtures/sorafs_manifest/ci_sample/car_summary.json
   ```
   Yenilənmiş paketləri/sübutları `fixtures/sorafs_manifest/ci_sample/`-ə kopyalayın
   buraxılış yeniləmələri kanonik qurğular.

## Doğrulama
- Buraxılış qapısı öhdəliyi: `c6cc192ac3d83dadb0c80d04ea975ab1fd484113`
  (`git rev-parse HEAD`, darvaza uğur qazandıqdan dərhal sonra).
- `ci/check_sorafs_cli_release.sh` çıxışı: arxivləşdirilmişdir
  `artifacts/sorafs_cli_release/v0.1.0/ci-check.log` (buraxılış paketinə əlavə olunur).
- Manifest paketi həzm: `SHA256 084fa37ebcc4e8c0c4822959d6e93cd63e524bb7abf4a184c87812ce665969be`
  (`fixtures/sorafs_manifest/ci_sample/manifest.bundle.json`).
- Sübut xülasəsi: `SHA256 51f4c8d9b28b370c828998d9b5c87b9450d6c50ac6499b817ac2e8357246a223`
  (`fixtures/sorafs_manifest/ci_sample/proof.json`).
- Manifest həzm (aşağı axın attestasiyasının çarpaz yoxlamaları üçün):
  `BLAKE3 0d4b88b8f95e0cff5a8ea7f9baac91913f32768fc514ce69c6d91636d552559d`
  (`manifest.sign.summary.json`-dən).

## Operatorlar üçün qeydlər
- Torii şlüz indi `X-Sora-Chunk-Range` qabiliyyət başlığını tətbiq edir. Yeniləyin
  icazə siyahıları, beləliklə yeni axın işarəsi əhatə dairələrini təqdim edən müştərilər qəbul edilsin; köhnə nişanlar
  diapazon olmadan iddia dayandırılacaq.
- `scripts/sorafs_gateway_self_cert.sh` manifest yoxlamasını birləşdirir. Qaçarkən
  özünü təsdiq edən qoşqu, yeni yaradılmış manifest paketini təmin edin ki, sarğı edə bilsin
  imza driftində sürətli uğursuzluq.
- Telemetriya tablosları yeni tablo ixracını (`scoreboard.json`) qəbul etməlidir.
  provayderin uyğunluğu, çəki təyinatları və imtina səbəblərini uzlaşdırın.
- Hər buraxılışda dörd kanonik xülasəni arxivləşdirin:
  `manifest.bundle.json`, `manifest.sig`, `manifest.sign.summary.json`,
  `manifest.verify.summary.json`. İdarəetmə biletləri zamanı bu dəqiq fayllara istinad edir
  təsdiq.

## Təşəkkürlər
- Saxlama Komandası - CLI konsolidasiyası, yığın planı rendereri və skorbord
  telemetriya santexnika.
- Tooling WG — buraxma boru kəməri (`ci/check_sorafs_cli_release.sh`,
  `scripts/release_sorafs_cli.sh`) və deterministik qurğu dəsti.
- Gateway Əməliyyatları - qabiliyyət qapaqları, axın-token siyasətinə baxış və yenilənir
  özünü sertifikatlaşdırma oyun kitabları.