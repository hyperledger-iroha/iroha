---
lang: uz
direction: ltr
source: docs/norito_bridge_release.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: b9dc9862d4806d355fd83c885de92775712a7b32c68c010d29f4fc74229d054b
source_last_modified: "2026-01-06T05:24:53.995808+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# NoritoBridge reliz paketi

Ushbu qo'llanma `NoritoBridge` Swift ulanishlarini nashr qilish uchun zarur bo'lgan qadamlarni belgilaydi.
Swift Package Manager va CocoaPods-dan foydalanish mumkin bo'lgan XCFramework. The
ish oqimi Swift artefaktlarini Rust sandiqning jo'natiladigan relizlari bilan qulflangan bosqichda ushlab turadi
Iroha Norito kodek. Nashr etilganlarni iste'mol qilish bo'yicha oxirigacha ko'rsatmalar uchun
ilova ichidagi artefaktlar (Xcode loyihasi simlari, ChaChaPoly-dan foydalanish va boshqalar), qarang
`docs/connect_swift_integration.md`.

> **Eslatma:** Ushbu oqim uchun CI avtomatizatsiyasi macOS quruvchilari talab qilinganidan so‘ng ishga tushadi
> Apple asboblari onlayn bo'ladi (release Engineering macOS quruvchisi orqasida kuzatilgan).
> Ungacha quyidagi amallar ishlab chiquvchi Mac-da qo'lda bajarilishi kerak.

## Old shartlar

- Eng so'nggi barqaror Xcode buyruq qatori vositalari o'rnatilgan macOS xosti.
- `rust-toolchain.toml` ish maydoniga mos keladigan Rust asboblar zanjiri.
- Swift asboblar zanjiri 5.7 yoki undan yangiroq.
- CocoaPods (Ruby toshlari orqali), agar markaziy xususiyatlar omborida nashr qilinsa.
- Swift artefaktlarini belgilash uchun Hyperledger Iroha reliz imzolash kalitlariga kirish.

## Versiyalash modeli

1. Norito kodek (`crates/norito/Cargo.toml`) uchun Rust sandiq versiyasini aniqlang.
2. Ish joyini reliz identifikatori bilan belgilang (masalan, `v2.1.0`).
3. Swift paketi va CocoaPods podspec uchun bir xil semantik versiyadan foydalaning.
4. Rust qutisi o'z versiyasini oshirganda, jarayonni takrorlang va mos keladiganini nashr qiling
   Tez artefakt. Sinov paytida versiyalar metadata qo'shimchalarini (masalan, `-alpha.1`) o'z ichiga olishi mumkin.

## Qadamlarni qurish

1. XCFrameworkni yig'ish uchun ombor ildizidan yordamchi skriptni chaqiring:

   ```bash
   ./scripts/build_norito_xcframework.sh --workspace-root "$(pwd)" \
       --output "artifacts/NoritoBridge.xcframework" \
       --profile release
   ```

   Skript iOS va macOS maqsadlari uchun Rust ko'prigi kutubxonasini to'playdi va ularni birlashtiradi
   Natijada bitta XCFramework katalogi ostidagi statik kutubxonalar.
   Bundan tashqari, `dist/NoritoBridge.artifacts.json` chiqaradi, ko'prik versiyasini va
   har bir platforma uchun SHA-256 xeshlari (agar `NORITO_BRIDGE_VERSION` bilan versiyani bekor qiling.
   kerak).

2. Tarqatish uchun XCFrameworkni zip qiling:

   ```bash
   ditto -c -k --sequesterRsrc --keepParent \
     artifacts/NoritoBridge.xcframework \
     artifacts/NoritoBridge.xcframework.zip
   ```

3. Yangisiga ishora qilish uchun Swift paketi manifestini (`IrohaSwift/Package.swift`) yangilang.
   versiya va nazorat summasi:

   ```bash
   swift package compute-checksum artifacts/NoritoBridge.xcframework.zip
   ```

   Ikkilik maqsadni aniqlashda nazorat summasini `Package.swift` da yozib oling.

4. `IrohaSwift/IrohaSwift.podspec` ni yangi versiya, nazorat summasi va arxiv bilan yangilang
   URL.

5. **Agar ko'prik yangi eksportga ega bo'lsa, sarlavhalarni qayta tiklang.** Swift ko'prigi endi ochiladi
   `connect_norito_set_acceleration_config` shuning uchun `AccelerationSettings` Metallni almashtira oladi /
   GPU backendlari. `NoritoBridge.xcframework/**/Headers/connect_norito_bridge.h` ga ishonch hosil qiling
   ziplashdan oldin `crates/connect_norito_bridge/include/connect_norito_bridge.h` ga mos keladi.

6. Belgilashdan oldin Swift tekshirish to'plamini ishga tushiring:

   ```bash
   swift test --package-path IrohaSwift
   make swift-ci
   ```

   Birinchi buyruq Swift paketining (jumladan, `AccelerationSettings`) qolishini ta'minlaydi.
   yashil; ikkinchisi armatura paritetini tasdiqlaydi, parite/CI asboblar panelini ko'rsatadi va
   Buildkite-da qo'llaniladigan bir xil telemetriya tekshiruvlarini amalga oshiradi (shu jumladan
   `ci/xcframework-smoke:<lane>:device_tag` metadata talabi).

7. Yaratilgan artefaktlarni reliz bo'limiga topshiring va topshiriqni belgilang.

## Nashr

### Swift paket menejeri

- Tegni umumiy Git omboriga suring.
- Teg paket indeksi (Apple yoki hamjamiyat oynasi) bo'yicha kirish mumkinligiga ishonch hosil qiling.
- Iste'molchilar endi `.package(url: "https://github.com/hyperledger/iroha", from: "<version>")` ga bog'liq bo'lishi mumkin.

### CocoaPods

1. Podni mahalliy sifatida tasdiqlang:

   ```bash
   pod lib lint IrohaSwift.podspec --allow-warnings
   ```

2. Yangilangan podspecni bosing:

   ```bash
   pod trunk push IrohaSwift.podspec
   ```

3. Yangi versiya CocoaPods indeksida paydo bo'lishini tasdiqlang.

## CI fikrlari

- Paket skriptini ishga tushiradigan, artefaktlarni arxivlaydigan va yuklaydigan macOS ishini yarating.
  ish oqimining chiqishi sifatida nazorat summasi yaratildi.
- Gate yangi ishlab chiqarilgan ramkaga qarshi Swift demo ilovasi binosida chiqadi.
- Nosozliklarni tashxislashda yordam berish uchun qurilish jurnallarini saqlang.

## Qo'shimcha avtomatlashtirish g'oyalari

- `xcodebuild -create-xcframework` dan barcha kerakli maqsadlar ochilgandan so'ng to'g'ridan-to'g'ri foydalaning.
- Ishlab chiquvchi mashinalardan tashqarida tarqatish uchun imzolash/notariallashtirishni integratsiyalash.
- SPM-ni mahkamlash orqali paketlangan versiya bilan integratsiya testlarini blokirovkalash bosqichida saqlang
  chiqarish tegiga bog'liqlik.