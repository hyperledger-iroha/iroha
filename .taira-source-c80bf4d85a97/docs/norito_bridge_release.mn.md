---
lang: mn
direction: ltr
source: docs/norito_bridge_release.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: b9dc9862d4806d355fd83c885de92775712a7b32c68c010d29f4fc74229d054b
source_last_modified: "2026-01-06T05:24:53.995808+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# НоритоБриджийг гаргах савлагаа

Энэхүү гарын авлага нь `NoritoBridge` Swift холболтыг нийтлэхэд шаардагдах алхмуудыг тоймлон харуулав.
Swift Package Manager болон CocoaPods-аас хэрэглэж болох XCFramework. The
Ажлын урсгал нь Swift олдворуудыг зөөвөрлөх Rust хайрцгийн хувилбаруудтай хамт цоожтой байлгадаг.
Iroha-ийн Norito кодлогч. Нийтлэгдсэн хэрэглээний талаархи төгсгөлийн зааварчилгааг авахын тулд
програм доторх олдворууд (Xcode төслийн утас, ChaChaPoly хэрэглээ гэх мэт), үзнэ үү
`docs/connect_swift_integration.md`.

> **Тэмдэглэл:** Энэ урсгалд зориулсан CI автоматжуулалт нь macOS-ийн бүтээгчид шаардлагатай үед буух болно
> Apple-ийн хэрэгслүүд онлайн болно (Release Engineering macOS builder backlog-д хянадаг).
> Тэр болтол доорх алхмуудыг хөгжүүлэлтийн Mac дээр гараар гүйцэтгэх ёстой.

## Урьдчилсан нөхцөл

- Хамгийн сүүлийн үеийн тогтвортой Xcode командын шугамын хэрэгслүүдийг суулгасан macOS хост.
- `rust-toolchain.toml` ажлын талбарт таарч тохирох зэв багажны гинж.
- Swift toolchain 5.7 буюу түүнээс дээш.
- CocoaPods (Ruby gems-ээр дамжуулан) техникийн мэдээллийн төв санд нийтэлж байгаа бол.
- Swift олдворуудыг шошголох Hyperledger Iroha хувилбарын гарын үсэг зурах түлхүүрүүдэд хандах.

## Хувилбарын загвар

1. Norito кодлогчийн (`crates/norito/Cargo.toml`) Rust хайрцагны хувилбарыг тодорхойлно уу.
2. Ажлын талбарыг хувилбарын танигчаар тэмдэглэнэ үү (жишээ нь, `v2.1.0`).
3. Swift багц болон CocoaPods podspec-д ижил семантик хувилбарыг ашиглана уу.
4. Rust хайрцаг нь хувилбараа нэмэгдүүлэх үед үйлдлийг давтаж, тохирохыг нийтэл
   Хурдан олдвор. Туршилтын явцад хувилбарууд нь мета өгөгдлийн дагавар (жишээ нь, `-alpha.1`) агуулж болно.

## Барилгын алхамууд

1. Repository root-ээс XCFramework-ийг угсрахын тулд туслах скриптийг дуудна:

   ```bash
   ./scripts/build_norito_xcframework.sh --workspace-root "$(pwd)" \
       --output "artifacts/NoritoBridge.xcframework" \
       --profile release
   ```

   Скрипт нь iOS болон macOS-д зориулсан Rust bridge номын санг эмхэтгэж, багцалсан
   нэг XCFramework лавлах дор статик номын сангууд үүсдэг.
   Энэ нь мөн `dist/NoritoBridge.artifacts.json` ялгаруулж, гүүрний хувилбар болон
   платформ бүрийн SHA-256 хэш (хэрэв `--bridge-version <version>` хувилбарыг дарж бичнэ үү.
   хэрэгтэй).

2. Хуваарилахын тулд XCFramework-г зиплэ:

   ```bash
   ditto -c -k --sequesterRsrc --keepParent \
     artifacts/NoritoBridge.xcframework \
     artifacts/NoritoBridge.xcframework.zip
   ```

3. Swift багцын манифестийг (`IrohaSwift/Package.swift`) шинэчилж, шинэ рүү чиглүүлнэ үү.
   хувилбар ба шалгах нийлбэр:

   ```bash
   swift package compute-checksum artifacts/NoritoBridge.xcframework.zip
   ```

   Хоёртын зорилтыг тодорхойлохдоо хяналтын нийлбэрийг `Package.swift` дээр тэмдэглэ.

4. `IrohaSwift/IrohaSwift.podspec`-г шинэ хувилбар, шалгах нийлбэр болон архиваар шинэчилнэ үү.
   URL.

5. **Хэрэв гүүр шинэ экспорттой бол толгойг нь сэргээнэ үү.** Свифт гүүр одоо ил гарч байна.
   `connect_norito_set_acceleration_config` тиймээс `AccelerationSettings` нь Металл /
   GPU backends. `NoritoBridge.xcframework/**/Headers/connect_norito_bridge.h` эсэхийг шалгаарай
   зип хийхээс өмнө `crates/connect_norito_bridge/include/connect_norito_bridge.h`-тай таарч байна.

6. Тэмдэглэхээсээ өмнө Swift баталгаажуулалтын багцыг ажиллуулна уу:

   ```bash
   swift test --package-path IrohaSwift
   make swift-ci
   ```

   Эхний тушаал нь Swift багц (`AccelerationSettings` орно) хэвээр үлдэх болно.
   ногоон; хоёр дахь нь бэхэлгээний паритетыг баталгаажуулж, паритет/CI хяналтын самбарыг гаргаж өгдөг
   Buildkite-д хэрэгжүүлсэн ижил телеметрийн шалгалтуудыг хийдэг (үүнд
   `ci/xcframework-smoke:<lane>:device_tag` мета өгөгдлийн шаардлага).

7. Үүсгэсэн олдворуудыг хувилбарын салбар дотор хийж, амлалтаа тэмдэглээрэй.

## Нийтлэл

### Swift багц менежер

- Тагийг нийтийн Git репозитор руу түлхэх.
- Уг шошгыг багцын индексээр (Apple эсвэл олон нийтийн толин тусгал) ашиглах боломжтой эсэхийг шалгаарай.
- Хэрэглэгчид одоо `.package(url: "https://github.com/hyperledger/iroha", from: "<version>")`-ээс хамааралтай болно.

### CocoaPods

1. Под-ыг дотооддоо баталгаажуулна уу:

   ```bash
   pod lib lint IrohaSwift.podspec --allow-warnings
   ```

2. Шинэчилсэн podspec-ийг дарна уу:

   ```bash
   pod trunk push IrohaSwift.podspec
   ```

3. CocoaPods индекс дээр шинэ хувилбар гарч ирэхийг баталгаажуулна уу.

## CI-д анхаарах зүйлс

- Сав баглаа боодлын скриптийг ажиллуулдаг, олдворуудыг архивлаж, байршуулдаг macOS-ийн ажлыг үүсгэ.
  хяналтын нийлбэрийг ажлын урсгалын гаралт болгон үүсгэсэн.
- Gate шинэхэн бүтээгдсэн хүрээний эсрэг Swift demo програмын барилга дээр гарна.
- Алдааг оношлоход туслахын тулд бүтээх бүртгэлийг хадгал.

## Автоматжуулалтын нэмэлт санаанууд

- Шаардлагатай бүх зорилтууд ил гарсны дараа шууд `xcodebuild -create-xcframework` ашиглана уу.
- Хөгжүүлэгчийн машинаас гадуур түгээх гарын үсэг/нотариатыг нэгтгэх.
- SPM-ийг хавчуулж багцалсан хувилбартай интеграцийн тестийг шат шатанд нь байлгаарай
  хувилбарын шошгоноос хамаарал.