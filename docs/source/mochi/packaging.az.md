---
lang: az
direction: ltr
source: docs/source/mochi/packaging.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: c7ab0877a6f43402d6ec13a44c4a7c2b68e4a49e6103bb50d7469d9e71aaa953
source_last_modified: "2025-12-29T18:16:35.984945+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# MOCHI Qablaşdırma Bələdçisi

Bu təlimat MOCHI masa üstü nəzarətçi paketini necə qurmağı, yoxlamağı izah edir
yaradılan artefaktları və bu gəmi ilə göndərilən işləmə müddətini tənzimləyin
bağlama. O, təkrarlana bilən qablaşdırmaya diqqət yetirərək sürətli başlanğıcı tamamlayır
və CI istifadəsi.

## İlkin şərtlər

- İş yerindən asılılıqları olan Rust alətlər silsiləsi (versiya 2024 / Rust 1.82+)
  artıq tikilib.
- İstədiyiniz hədəf üçün tərtib edilmiş `irohad`, `iroha_cli` və `kagami`. The
  bundler `target/<profile>/`-dən ikili faylları təkrar istifadə edir.
- `target/` və ya xüsusi paketin çıxışı üçün kifayət qədər disk sahəsi
  təyinat.

Bundleri işə salmazdan əvvəl bir dəfə asılılıqları yaradın:

```bash
cargo build -p irohad -p iroha_cli -p iroha_kagami
```

## Paketin qurulması

Repozitor kökündən xüsusi `xtask` əmrini çağırın:

```bash
cargo xtask mochi-bundle
```

Defolt olaraq bu, a ilə `target/mochi-bundle/` altında buraxılış paketi yaradır
host OS və arxitekturadan əldə edilən fayl adı (məsələn,
`mochi-macos-aarch64-release.tar.gz`). Fərdiləşdirmək üçün aşağıdakı bayraqlardan istifadə edin
tikinti:

- `--profile <name>` – Yük profili seçin (`release`, `debug` və ya
  xüsusi profil).
- `--no-archive` – `.tar.gz` yaratmadan genişləndirilmiş kataloqu saxlayın
  arxiv (yerli sınaq üçün faydalıdır).
- `--out <path>` – əvəzinə xüsusi kataloqa paketlər yazın
  `target/mochi-bundle/`.
- `--kagami <path>` – proqrama daxil etmək üçün əvvəlcədən qurulmuş `kagami` icraedicisini təmin edin
  arxiv. Buraxıldıqda, paketçi ikili faylı yenidən istifadə edir (və ya qurur).
  seçilmiş profil.
- `--matrix <path>` – paket metadatasını JSON matris faylına əlavə edin (əgər yaradılmışdırsa)
  itkin) beləliklə CI boru kəmərləri a-da istehsal olunan hər bir host/profil artefaktını qeyd edə bilər
  qaçmaq. Girişlərə paket kataloqu, manifest yolu və isteğe bağlı SHA-256 daxildir
  arxiv yeri və ən son tüstü testinin nəticəsi.
- `--smoke` – paketlənmiş `mochi --help`-i yüngül tüstü qapısı kimi yerinə yetirin
  yığıldıqdan sonra; Çatışmazlıqlar, dərc edilməzdən əvvəl çatışmayan asılılıqları üzə çıxarır
  artefakt.
- `--stage <path>` – hazır paketi (və istehsal edildikdə arxivi) kopyalayın
  çox platformalı quruluşlar artefaktları birinə yerləşdirə biləcək bir quruluş kataloqu
  əlavə skript olmadan yer.

Komanda `mochi-ui-egui`, `kagami`, `LICENSE`, nümunəni kopyalayır
konfiqurasiya və `mochi/BUNDLE_README.md` paketə daxil edin. Bir deterministik
`manifest.json` ikili fayllarla birlikdə yaradılır, beləliklə CI işləri faylı izləyə bilər
hash və ölçülər.

## Paket tərtibatı və yoxlanılması

Genişləndirilmiş paket `BUNDLE_README.md`-də sənədləşdirilmiş tərtibata uyğundur:

```
bin/mochi
bin/kagami
config/sample.toml
docs/README.md
manifest.json
LICENSE
```

`manifest.json` faylı SHA-256 hash ilə hər artefaktı sadalayır. Doğrulayın
paketi başqa sistemə kopyaladıqdan sonra:

```bash
jq -r '.files[] | "\(.sha256)  \(.path)"' manifest.json | sha256sum --check
```

CI boru kəmərləri genişləndirilmiş kataloqu keşləyə, arxivi imzalaya və ya dərc edə bilər
buraxılış qeydləri ilə birlikdə manifest. Manifestə generator daxildir
mənşəyi izləməyə kömək etmək üçün profil, hədəf üçlüyü və yaradılma vaxt damğası.

## İcra müddəti ləğv edilir

MOCHI CLI bayraqları və ya vasitəsilə köməkçi binaries və iş vaxtı yerlərini aşkar edir
ətraf mühit dəyişənləri:- `--data-root` / `MOCHI_DATA_ROOT` – həmyaşıd üçün istifadə olunan iş sahəsini ləğv edin
  konfiqurasiya, saxlama və qeydlər.
- `--profile` – topologiyanın ilkin təyinatları arasında keçid (`single-peer`,
  `four-peer-bft`).
- `--torii-start`, `--p2p-start` – ayırarkən istifadə olunan əsas portları dəyişdirin
  xidmətlər.
- `--irohad` / `MOCHI_IROHAD` – xüsusi `irohad` binarda nöqtə.
- `--kagami` / `MOCHI_KAGAMI` – yığılmış `kagami`-i ləğv edin.
- `--iroha-cli` / `MOCHI_IROHA_CLI` – əlavə CLI köməkçisini ləğv edin.
- `--restart-mode <never|on-failure>` – avtomatik yenidən başlamaları söndürün və ya məcbur edin
  eksponensial geriləmə siyasəti.
- `--restart-max <attempts>` – yenidən başlatma cəhdlərinin sayını aradan qaldırın
  `on-failure` rejimində işləyir.
- `--restart-backoff-ms <millis>` – avtomatik yenidən işə salmaq üçün əsas geri çəkilməni təyin edin.
- `MOCHI_CONFIG` – xüsusi `config/local.toml` yolunu təmin edin.

CLI yardımı (`mochi --help`) tam bayraq siyahısını çap edir. Ətraf mühit üstünlük təşkil edir
işə salındıqda qüvvəyə minir və daxilində Parametrlər dialoqu ilə birləşdirilə bilər
UI.

## CI istifadə göstərişləri

- Bir kataloq yaratmaq üçün `cargo xtask mochi-bundle --no-archive`-i işə salın
  platformaya aid alətlərlə sıxışdırılmalıdır (Windows üçün ZiP, tarballs üçün
  Unix).
- `cargo xtask mochi-bundle --matrix dist/matrix.json` ilə paket metadatasını çəkin
  beləliklə, buraxılış işləri hər bir host/profili sadalayan tək JSON indeksini dərc edə bilər
  boru kəmərində istehsal olunan artefakt.
- Hər birində `cargo xtask mochi-bundle --stage /mnt/staging/mochi` (və ya oxşar) istifadə edin
  paketi və arxivi paylaşılan qovluğa yükləmək üçün agent qurun
  nəşr işi istehlak edə bilər.
- Həm arxivi, həm də `manifest.json`-i dərc edin ki, operatorlar paketi yoxlaya bilsinlər
  bütövlük.
- Yaradılmış kataloqu toxum tüstüsü testləri üçün bir quruluş artefaktı kimi saxlayın
  Nəzarətçini deterministik şəkildə paketlənmiş ikili sənədlərlə həyata keçirin.
- Paket hashlərini buraxılış qeydlərində və ya gələcək üçün `status.md` jurnalında qeyd edin
  mənşəyi yoxlayır.