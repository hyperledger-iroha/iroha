---
lang: az
direction: ltr
source: docs/norito_bridge_release.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: b9dc9862d4806d355fd83c885de92775712a7b32c68c010d29f4fc74229d054b
source_last_modified: "2026-01-06T05:24:53.995808+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# NoritoBridge Buraxılış Qablaşdırması

Bu təlimat `NoritoBridge` Swift bağlamalarını dərc etmək üçün tələb olunan addımları təsvir edir.
Swift Package Manager və CocoaPods-dan istehlak edilə bilən XCFramework. The
iş axını Swift artefaktlarını göndərilən Rust sandığı buraxılışları ilə kilidli addımda saxlayır
Iroha-in Norito kodek. Nəşr olunanların istehlakına dair başdan-başa təlimatlar üçün
proqram daxilində artefaktlar (Xcode layihə kabelləri, ChaChaPoly istifadəsi və s.), baxın
`docs/connect_swift_integration.md`.

> **Qeyd:** Bu axın üçün CI avtomatlaşdırılması macOS qurucuları tələb olunan tələblərə cavab verdikdən sonra enəcək
> Apple alətləri onlayn olur (Release Engineering macOS builder backlog-da izlənilir).
> O vaxta qədər aşağıdakı addımlar inkişaf etdirici Mac-də əl ilə yerinə yetirilməlidir.

## İlkin şərtlər

- Ən son sabit Xcode komanda xətti alətləri quraşdırılmış macOS hostu.
- `rust-toolchain.toml` iş sahəsinə uyğun olan Rust alətlər silsiləsi.
- Swift toolchain 5.7 və ya daha yeni.
- CocoaPods (Ruby daşları vasitəsilə) mərkəzi xüsusiyyətlər deposunda dərc olunarsa.
- Swift artefaktlarının etiketlənməsi üçün Hyperledger Iroha buraxılış imzalama düymələrinə giriş.

## Versiyalaşdırma modeli

1. Norito kodek (`crates/norito/Cargo.toml`) üçün Rust sandıq versiyasını müəyyənləşdirin.
2. İş sahəsini buraxılış identifikatoru ilə etiketləyin (məsələn, `v2.1.0`).
3. Swift paketi və CocoaPods podspec üçün eyni semantik versiyadan istifadə edin.
4. Rust qutusu öz versiyasını artırdıqda, prosesi təkrarlayın və uyğunluğu dərc edin
   Sürətli artefakt. Test zamanı versiyalara metadata şəkilçiləri (məsələn, `-alpha.1`) daxil ola bilər.

## Addımlar qurun

1. XCFramework-i yığmaq üçün depo kökündən köməkçi skripti çağırın:

   ```bash
   ./scripts/build_norito_xcframework.sh --workspace-root "$(pwd)" \
       --output "artifacts/NoritoBridge.xcframework" \
       --profile release
   ```

   Skript iOS və macOS hədəfləri üçün Rust körpüsü kitabxanasını tərtib edir və onları birləşdirir
   nəticədə vahid XCFramework kataloqu altında statik kitabxanalar.
   O, həmçinin körpü versiyasını tutan və `dist/NoritoBridge.artifacts.json` yayır
   platforma başına SHA-256 hashları (əgər `--bridge-version <version>` ilə versiyanı ləğv edin
   lazımdır).

2. Paylanma üçün XCFramework-i sıxın:

   ```bash
   ditto -c -k --sequesterRsrc --keepParent \
     artifacts/NoritoBridge.xcframework \
     artifacts/NoritoBridge.xcframework.zip
   ```

3. Yeni paketə işarə etmək üçün Swift paket manifestini (`IrohaSwift/Package.swift`) yeniləyin
   versiya və yoxlama məbləği:

   ```bash
   swift package compute-checksum artifacts/NoritoBridge.xcframework.zip
   ```

   İkili hədəfi təyin edərkən yoxlama məbləğini `Package.swift`-də qeyd edin.

4. `IrohaSwift/IrohaSwift.podspec`-i yeni versiya, yoxlama məbləği və arxivlə yeniləyin
   URL.

5. **Körpü yeni ixraclar əldə edərsə, başlıqları bərpa edin.** Swift körpüsü indi ifşa edir
   `connect_norito_set_acceleration_config` buna görə də `AccelerationSettings` Metal /
   GPU arxa uçları. `NoritoBridge.xcframework/**/Headers/connect_norito_bridge.h`-dən əmin olun
   zipdən əvvəl `crates/connect_norito_bridge/include/connect_norito_bridge.h` ilə uyğun gəlir.

6. Etiketləmədən əvvəl Swift doğrulama paketini işə salın:

   ```bash
   swift test --package-path IrohaSwift
   make swift-ci
   ```

   Birinci əmr Swift paketinin (`AccelerationSettings` daxil olmaqla) qalmasını təmin edir
   yaşıl; ikincisi armatur paritetini təsdiq edir, paritet/CI tablosunu göstərir və
   Buildkite-də tətbiq edilən eyni telemetriya yoxlamalarını həyata keçirir (o cümlədən
   `ci/xcframework-smoke:<lane>:device_tag` metadata tələbi).

7. Yaradılmış artefaktları buraxılış bölməsində təhvil verin və öhdəliyi işarələyin.

## Nəşriyyat

### Swift Paket Meneceri

- Teqi ictimai Git deposuna itələyin.
- Etiketin paket indeksi (Apple və ya icma güzgüsü) ilə əlçatan olduğundan əmin olun.
- İstehlakçılar indi `.package(url: "https://github.com/hyperledger/iroha", from: "<version>")`-dən asılı ola bilərlər.

### CocoaPods

1. Qrupu yerli olaraq yoxlayın:

   ```bash
   pod lib lint IrohaSwift.podspec --allow-warnings
   ```

2. Yenilənmiş podspec-i itələyin:

   ```bash
   pod trunk push IrohaSwift.podspec
   ```

3. Yeni versiyanın CocoaPods indeksində göründüyünü təsdiqləyin.

## CI mülahizələri

- Qablaşdırma skriptini işlədən, artefaktları arxivləşdirən və yükləyən macOS işi yaradın
  iş axını çıxışı kimi yoxlama məbləği yaradıldı.
- Gate, yeni hazırlanmış çərçivəyə qarşı Swift demo tətbiqi binasında buraxılır.
- Uğursuzluqların diaqnostikasında kömək etmək üçün quraşdırma qeydlərini saxlayın.

## Əlavə avtomatlaşdırma ideyaları

- Bütün tələb olunan hədəflər üzə çıxdıqdan sonra birbaşa `xcodebuild -create-xcframework` istifadə edin.
- Tərtibatçı maşınlarından kənar paylama üçün imza/notariallaşdırmanı inteqrasiya edin.
- SPM-i bağlayaraq paketlənmiş versiya ilə inteqrasiya testlərini kilidli addımda saxlayın
  buraxılış etiketindən asılılıq.