---
lang: az
direction: ltr
source: docs/portal/docs/sorafs/developer-releases.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 3a59639046a496f35c3cf80006a3330a25407b8143213f1b02b5ce766a70b4f0
source_last_modified: "2026-01-05T09:28:11.867590+00:00"
translation_last_reviewed: 2026-02-07
title: Release Process
summary: Run the CLI/SDK release gate, apply the shared versioning policy, and publish canonical release notes.
translator: machine-google-reviewed
---

# Buraxılış Prosesi

SoraFS binaries (`sorafs_cli`, `sorafs_fetch`, köməkçilər) və SDK qutuları
(`sorafs_car`, `sorafs_manifest`, `sorafs_chunker`) birlikdə göndərilir. Buraxılış
boru kəməri CLI və kitabxanaları uyğunlaşdırır, lint/test əhatəsini təmin edir və
aşağı axın istehlakçıları üçün artefaktları çəkir. Hər biri üçün aşağıdakı yoxlama siyahısını işlədin
namizəd etiketi.

## 0. Təhlükəsizliyin nəzərdən keçirilməsini təsdiqləyin

Texniki buraxılış qapısını yerinə yetirməzdən əvvəl ən son təhlükəsizlik baxışını çəkin
artefaktlar:

- Ən son SF-6 təhlükəsizlik araşdırması memo-nu endirin ([reports/sf6-security-review](./reports/sf6-security-review.md))
  və buraxılış biletində onun SHA256 hashını qeyd edin.
- Təmir biletinin linkini əlavə edin (məsələn, `governance/tickets/SF6-SR-2026.md`) və qeydiyyatı qeyd edin
  Təhlükəsizlik Mühəndisliyi və Alət İşçi Qrupundan təsdiqləyicilər.
- Memodakı remediasiya yoxlama siyahısının bağlandığını yoxlayın; həll olunmamış maddələr buraxılışa mane olur.
- Paritet qoşqu qeydlərini yükləməyə hazırlaşın (`cargo test -p sorafs_car -- --nocapture sorafs_cli::proof_stream::bounded_channels`)
  manifest paketinin yanında.
- İşlətməyi planlaşdırdığınız imza əmrini təsdiqləyin həm `--identity-token-provider`, həm də açıq-aşkar
  `--identity-token-audience=<aud>` beləliklə, Fulcio əhatə dairəsi buraxılış sübutunda tutulur.

İdarəetməni xəbərdar edərkən və buraxılışı dərc edərkən bu artefaktları daxil edin.

## 1. Buraxılış/sınaq qapısını yerinə yetirin

`ci/check_sorafs_cli_release.sh` köməkçisi formatlaşdırma, Clippy və testləri həyata keçirir
CLI və SDK qutuları arasında iş sahəsi üçün yerli hədəf kataloqu (`.target`)
CI konteynerləri daxilində icra edərkən icazə ziddiyyətlərinin qarşısını almaq üçün.

```bash
CARGO_TARGET_DIR=.target ci/check_sorafs_cli_release.sh
```

Skript aşağıdakı iddiaları yerinə yetirir:

- `cargo fmt --all -- --check` (iş sahəsi)
- `sorafs_car` üçün `cargo clippy --locked --all-targets` (`cli` xüsusiyyəti ilə),
  `sorafs_manifest` və `sorafs_chunker`
- Eyni qutular üçün `cargo test --locked --all-targets`

Hər hansı bir addım uğursuz olarsa, etiketləmədən əvvəl reqressiyanı düzəldin. Buraxılış quruluşları olmalıdır
əsas ilə davamlı; buraxılış budaqlarına düzəlişləri albalı seçməyin. Qapı
həmçinin açarsız imzalama bayraqlarını yoxlayır (`--identity-token-issuer`, `--identity-token-audience`)
müvafiq hallarda verilir; itkin arqumentlər qaçışda uğursuz olur.

## 2. Versiya siyasətini tətbiq edin

Bütün SoraFS CLI/SDK qutuları SemVer istifadə edir:

- `MAJOR`: İlk 1.0 buraxılışı üçün təqdim edilmişdir. 1.0-dan əvvəl `0.y` kiçik zərbə
  **CLI səthində və ya Norito sxemlərində qırılma dəyişikliklərini göstərir**.
  isteğe bağlı siyasətin, telemetriya əlavələrinin arxasında qapalı sahələr).
- `PATCH`: Baq həlləri, yalnız sənədlər üçün buraxılışlar və asılılıq yeniləmələri
  müşahidə olunan davranışı dəyişməyin.

Həmişə `sorafs_car`, `sorafs_manifest` və `sorafs_chunker`-i eyni yerdə saxlayın
versiyaya uyğun olaraq aşağı axın SDK istehlakçıları tək uyğunlaşdırılmış versiyadan asılı ola bilər
simli. Versiyaları vurarkən:

1. Hər qutunun `Cargo.toml`-də `version =` sahələrini yeniləyin.
2. `Cargo.lock`-i `cargo update -p <crate>@<new-version>` vasitəsilə bərpa edin (
   iş sahəsi açıq versiyaları tətbiq edir).
3. Köhnə artefaktların qalmamasını təmin etmək üçün buraxma qapısını yenidən işə salın.

## 3. Buraxılış qeydlərini hazırlayın

Hər buraxılışda CLI, SDK və
idarəetməyə təsir edən dəyişikliklər. İçindəki şablondan istifadə edin
`docs/examples/sorafs_release_notes.md` (onu buraxılış artefaktlarınıza köçürün
kataloqunu daxil edin və bölmələri konkret detallarla doldurun).

Minimum məzmun:

- **Vurğulananlar**: CLI və SDK istehlakçıları üçün xüsusiyyət başlıqları.
  tələblər.
- **Təkmilləşdirmə addımları**: TL; Yük asılılıqlarını aradan qaldırmaq və yenidən işə salmaq üçün DR əmrləri
  deterministik qurğular.
- **Yoxlama**: əmr çıxışı hashləri və ya zərfləri və dəqiq
  `ci/check_sorafs_cli_release.sh` revizyonu icra edildi.

Doldurulmuş buraxılış qeydlərini etiketə əlavə edin (məsələn, GitHub buraxılış gövdəsi) və saxlayın
onları deterministik şəkildə yaradılan artefaktlarla yanaşı.

## 4. Boşaltma qarmaqlarını yerinə yetirin

İmza paketini yaratmaq üçün `scripts/release_sorafs_cli.sh`-i işə salın və
hər buraxılışla göndərilən yoxlama xülasəsi. Sarmalayıcı CLI qurur
lazım olduqda, `sorafs_cli manifest sign`-ə zəng edir və dərhal təkrar oxuyur
`manifest verify-signature` beləliklə etiketləmədən əvvəl uğursuzluqlar üzə çıxır. Misal:

```bash
scripts/release_sorafs_cli.sh \
  --manifest artifacts/site.manifest.to \
  --chunk-plan artifacts/site.chunk_plan.json \
  --chunk-summary artifacts/site.car.json \
  --bundle-out artifacts/release/manifest.bundle.json \
  --signature-out artifacts/release/manifest.sig \
  --identity-token-provider=github-actions \
  --identity-token-audience=sorafs-release \
  --expect-token-hash "$(cat .release/token.hash)"
```

Məsləhətlər:

- Sizdə buraxılış daxiletmələrini (faydalı yük, planlar, xülasələr, gözlənilən token hash) izləyin
  repo və ya yerləşdirmə konfiqurasiyası, beləliklə skriptin təkrarlana bilən qalması. CI qurğusu
  `fixtures/sorafs_manifest/ci_sample/` altındakı paket kanonik düzeni göstərir.
- `.github/workflows/sorafs-cli-release.yml`-də baza CI avtomatlaşdırılması; idarə edir
  buraxılış qapısı, yuxarıdakı skripti çağırır və paketləri/imzaları arxivləşdirir
  iş axını artefaktları. Eyni əmr əmrini əks etdirin (buraxılış qapısı → işarəsi →
  yoxlayın) digər CI sistemlərində yoxlanış qeydləri yaradılan hashlərlə üst-üstə düşsün.
- Yaradılmış `manifest.bundle.json`, `manifest.sig`,
  `manifest.sign.summary.json` və `manifest.verify.summary.json` birlikdə—onlar
  idarəetmə bildirişində istinad edilən paketi formalaşdırın.
- Buraxılış kanonik qurğuları yenilədikdə, yenilənmiş manifesti kopyalayın,
  yığın planı və xülasələr `fixtures/sorafs_manifest/ci_sample/` (və yeniləmə
  `docs/examples/sorafs_ci_sample/manifest.template.json`) etiketləmədən əvvəl.
  Aşağı axın operatorları buraxılışı təkrar istehsal etmək üçün öhdəlik götürülmüş qurğulardan asılıdır
  bağlama.
- `sorafs_cli proof stream` məhdud kanal yoxlaması üçün işləmə jurnalını çəkin və onu
  axın təhlükəsizliyinin aktiv qaldığını sübut etmək üçün paket buraxın.
- Buraxılış qeydlərində imzalama zamanı istifadə olunan dəqiq `--identity-token-audience`-i qeyd edin; idarəçilik
  nəşri təsdiq etməzdən əvvəl auditoriyanı Fulcio siyasətinə qarşı yoxlayır.

Buraxılışda a olan zaman `scripts/sorafs_gateway_self_cert.sh` istifadə edin
gateway rollout. Attestasiyanı sübut etmək üçün onu eyni manifest paketinə yönəldin
namizəd artefaktına uyğundur:

```bash
scripts/sorafs_gateway_self_cert.sh --config docs/examples/sorafs_gateway_self_cert.conf \
  --manifest artifacts/site.manifest.to \
  --manifest-bundle artifacts/release/manifest.bundle.json
```

## 5. Tag edin və dərc edin

Yoxlamalar keçdikdən və qarmaqlar tamamlandıqdan sonra:

1. İkili faylları təsdiqləmək üçün `sorafs_cli --version` və `sorafs_fetch --version`-i işə salın
   yeni versiyanı bildirin.
2. Buraxılış konfiqurasiyasını qeydiyyatdan keçmiş `sorafs_release.toml`-də hazırlayın
   (üstünlük verilir) və ya yerləşdirmə reponuz tərəfindən izlənilən digər konfiqurasiya faylı. çəkinin
   ad-hoc mühit dəyişənlərinə etibar etmək; ilə CLI-yə yollar keçir
   `--config` (və ya ekvivalenti) buna görə buraxılış daxiletmələri açıqdır və
   təkrarlana bilən.
3. İmzalanmış teq (üstünlük verilir) və ya annotasiya edilmiş teq yaradın:
   ```bash
   git tag -s sorafs-vX.Y.Z -m "SoraFS CLI & SDK vX.Y.Z"
   git push origin sorafs-vX.Y.Z
   ```
4. Artefaktları yükləyin (CAR paketləri, manifestlər, sübut xülasələri, buraxılış qeydləri,
   sertifikatlaşdırma nəticələri) idarəetmədən sonra layihə reyestrinə
   [yerləşdirmə təlimatı](./developer-deployment.md) daxilində yoxlama siyahısı. Buraxılış varsa
   zərb edilmiş yeni qurğular, onları paylaşılan qurğu repo və ya obyekt mağazasına itələyin
   audit avtomatlaşdırılması nəşr edilmiş paketi mənbə nəzarətindən fərqləndirə bilər.
5. İmzalanmış etiketə, buraxılış qeydlərinə keçidlərlə idarəetmə kanalını xəbərdar edin,
   manifest paketi/imza heshləri, arxivləşdirilmiş `manifest.sign/verify` xülasələri,
   və hər hansı attestasiya zərfləri. CI iş URL-ni (və ya log arxivini) daxil edin
   `ci/check_sorafs_cli_release.sh` və `scripts/release_sorafs_cli.sh` işlədi. Yeniləyin
   auditorların artefaktlara dair təsdiqləri izləyə bilməsi üçün idarəetmə bileti; zaman
   `.github/workflows/sorafs-cli-release.yml` iş elanları bildirişləri, əlaqə saxlayın
   ad-hoc xülasələri yapışdırmaq əvəzinə qeyd edilmiş hash nəticələri.

## 6. Buraxılışdan sonrakı təqib

- Sənədlərin yeni versiyaya yönəldilməsini təmin edin (sürətli başlanğıclar, CI şablonları)
  yenilənir və ya heç bir dəyişiklik tələb olunmur.
- İş davam edərsə, yol xəritəsi qeydləri faylı (məsələn, miqrasiya bayraqları, köhnəlmə
- Auditorlar üçün buraxılış qapısının çıxış jurnallarını arxivləşdirin—onları imzalanmışların yanında saxlayın
  artefaktlar.

Bu boru kəmərinin ardınca CLI, SDK qutuları və idarəetmə girovları saxlanılır
hər buraxılış dövrü üçün kilid addımı.