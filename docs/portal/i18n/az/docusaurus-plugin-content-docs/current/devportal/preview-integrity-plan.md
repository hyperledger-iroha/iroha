---
id: preview-integrity-plan
lang: az
direction: ltr
source: docs/portal/docs/devportal/preview-integrity-plan.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: Checksum-Gated Preview Plan
sidebar_label: Preview Integrity Plan
description: Implementation roadmap for securing the docs portal preview pipeline with checksum validation and notarised artefacts.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

Bu plan hər bir portal önizləmə artefaktını dərc edilməzdən əvvəl yoxlanıla bilən etmək üçün tələb olunan qalan işləri təsvir edir. Məqsəd rəyçilərin CI-də qurulmuş dəqiq snapşotu endirməsinə, yoxlama cəmi manifestinin dəyişməz olmasına və Norito metadata ilə SoraFS vasitəsilə ilkin baxışın aşkar edilməsinə zəmanət verməkdir.

## Məqsədlər

- **Deterministik quruluşlar:** `npm run build`-in təkrarlana bilən məhsul istehsal etdiyinə və həmişə `build/checksums.sha256` yaymasına əmin olun.
- **Təsdiqlənmiş önizləmələr:** Hər bir ilkin baxış artefaktının yoxlama məbləği manifesti ilə göndərilməsini tələb edin və yoxlama uğursuz olduqda nəşrdən imtina edin.
- **Norito tərəfindən dərc edilmiş metadata:** Norito JSON kimi qabaqcadan baxış deskriptorlarını (metadata, yoxlama məbləği digest, SoraFS CID) davam etdirin, beləliklə idarəetmə alətləri buraxılışları yoxlaya bilsin.
- **Operator alətləri:** İstehlakçıların yerli olaraq işlədə biləcəyi bir addımlı doğrulama skriptini təmin edin (`./docs/portal/scripts/preview_verify.sh --build-dir build --descriptor <path> --archive <path>`); skript indi yoxlama cəmini + deskriptorun doğrulama axınını uçdan-uca əhatə edir. Standart önizləmə əmri (`npm run serve`) indi bu köməkçini avtomatik olaraq `docusaurus serve`-dən əvvəl işə salır, beləliklə, yerli snapshotlar yoxlama cəminə qapalı qalır (`npm run serve:verified` açıq ləqəb kimi saxlanılır).

## Mərhələ 1 — CI İcrası

1. `.github/workflows/docs-portal-preview.yml`-i yeniləyin:
   - Docusaurus quruluşundan sonra `node docs/portal/scripts/write-checksums.mjs`-i işə salın (artıq yerli olaraq çağırılıb).
   - `cd build && sha256sum -c checksums.sha256`-i yerinə yetirin və uyğunsuzluqda işi uğursuz edin.
   - Quraşdırma qovluğunu `artifacts/preview-site.tar.gz` olaraq paketləyin, yoxlama məbləği manifestini kopyalayın, `scripts/generate-preview-descriptor.mjs`-ə zəng edin və `scripts/sorafs-package-preview.sh`-i JSON konfiqurasiyası ilə icra edin (bax `docs/examples/sorafs_preview_publish.json`) beləliklə iş axını həm müəyyən metadata, həm də I0NT0108 verir. bağlama.
   - Statik saytı, metadata artefaktlarını (`docs-portal-preview`, `docs-portal-preview-metadata`) və SoraFS paketini (`docs-portal-preview-sorafs`) yükləyin ki, manifest, CAR xülasəsi və plan yenidən yoxlanılmadan qurulsun.
2. Çəkmə sorğularında yoxlama məbləğinin yoxlanılması nəticəsini ümumiləşdirən CI nişanı şərhi əlavə edin (✅ `docs-portal-preview.yml` GitHub Skripti şərh addımı vasitəsilə həyata keçirilir).
3. `docs/portal/README.md`-də (CI bölməsi) iş prosesini sənədləşdirin və dərc yoxlama siyahısındakı yoxlama addımlarına keçid edin.

## Doğrulama Skripti

`docs/portal/scripts/preview_verify.sh` manuel `sha256sum` çağırışları tələb etmədən yüklənmiş önizləmə artefaktlarını təsdiqləyir. Skripti işə salmaq üçün `npm run serve` (və ya açıq-aşkar `npm run serve:verified` ləqəbi) istifadə edin və yerli görüntüləri paylaşarkən bir addımda `docusaurus serve`-i işə salın. Doğrulama məntiqi:

1. `build/checksums.sha256`-ə qarşı müvafiq SHA alətini (`sha256sum` və ya `shasum -a 256`) işə salır.
2. İstəyə görə, ilkin baxış deskriptorunun `checksums_manifest` həzmini/fayl adını və təmin edildikdə, önizləmə arxivi həzmini/fayl adını müqayisə edir.
3. Hər hansı uyğunsuzluq aşkar edildikdə sıfırdan çıxır ki, rəyçilər saxtalaşdırılmış önizləmələri bloklaya bilsinlər.

İstifadə nümunəsi (CI artefaktlarını çıxardıqdan sonra):

```bash
./docs/portal/scripts/preview_verify.sh \
  --build-dir build \
  --descriptor artifacts/preview-descriptor.json \
  --archive artifacts/preview-site.tar.gz
```

CI və buraxılış mühəndisləri hər dəfə önizləmə paketini endirdikdə və ya buraxılış biletinə artefakt əlavə etdikdə skriptə zəng etməlidirlər.

## Mərhələ 2 — SoraFS Nəşriyyat

1. Önizləmə iş prosesini aşağıdakı işlərlə genişləndirin:
   - Quraşdırılmış saytı `sorafs_cli car pack` və `manifest submit` istifadə edərək SoraFS quruluş şlüzinə yükləyir.
   - Qaytarılmış manifest həzmini və SoraFS CID-ni çəkir.
   - `{ commit, branch, checksum_manifest, cid }`-i Norito JSON-a (`docs/portal/preview/preview_descriptor.json`) seriyalaşdırır.
2. Deskriptoru qurma artefaktının yanında saxlayın və CID-i çəkmə sorğusu şərhində göstərin.
3. Gələcək dəyişikliklərin metadata sxemini sabit saxlamasını təmin etmək üçün quru işləmə rejimində `sorafs_cli` tətbiq edən inteqrasiya testləri əlavə edin.

## Mərhələ 3 — İdarəetmə və Audit

1. `docs/portal/schemas/` altında deskriptor strukturunu təsvir edən Norito sxemini (`PreviewDescriptorV1`) dərc edin.
2. Aşağıdakıları tələb etmək üçün DOCS-SORA nəşriyyat yoxlama siyahısını yeniləyin:
   - Yüklənmiş CID-ə qarşı `sorafs_cli manifest verify` işləyir.
   - Buraxılış PR təsvirində yoxlama cəmi manifest həzminin və CID-nin qeyd edilməsi.
3. Buraxılış səsləri zamanı deskriptoru yoxlama məbləği manifestinə qarşı yoxlamaq üçün idarəetmə avtomatlaşdırılmasını bağlayın.

## Çatdırılma və Mülkiyyət

| Mərhələ | Sahib(lər) | Hədəf | Qeydlər |
|-----------|----------|--------|-------|
| CI yoxlama məbləğinin icrası başladı | Sənəd İnfrastruktur | 1-ci həftə | Uğursuzluq qapısı + artefakt yükləmələri əlavə edir. |
| SoraFS önizləmə nəşri | Sənəd İnfrastruktur / Saxlama Komandası | Həftə 2 | Hazırlanma etimadnaməsinə və Norito sxem yeniləmələrinə giriş tələb edir. |
| İdarəetmə inteqrasiyası | Sənədlər/DevRel Rəhbəri / İdarəetmə İş Qrupu | 3-cü həftə | Sxemi dərc edir + yoxlama siyahılarını və yol xəritəsi qeydlərini yeniləyir. |

## Açıq Suallar

- Hansı SoraFS mühiti ilkin baxış artefaktlarını saxlamalıdır (səhnələşdirmə və xüsusi önizləmə zolağı)?
- Dərcdən əvvəl önizləmə deskriptorunda ikili imzalara (Ed25519 + ML-DSA) ehtiyacımız varmı?
- `sorafs_cli` işlədərkən CI iş axını manifestləri təkrarlana bilən saxlamaq üçün orkestr konfiqurasiyasını (`orchestrator_tuning.json`) bağlamalıdırmı?

`docs/portal/docs/reference/publishing-checklist.md`-də qərarlar alın və bilinməyənlər həll edildikdən sonra bu planı yeniləyin.