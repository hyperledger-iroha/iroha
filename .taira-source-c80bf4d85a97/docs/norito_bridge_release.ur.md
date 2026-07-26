---
lang: ur
direction: rtl
source: docs/norito_bridge_release.md
status: complete
translator: manual
source_hash: bc7c766ff5fb0504f4da43a017bf294758800b9c815affc8f97b9bcc94ae8e15
source_last_modified: "2025-11-02T04:40:28.805628+00:00"
translation_last_reviewed: 2025-11-14
---

<div dir="rtl">

<!-- اردو ترجمہ: docs/norito_bridge_release.md (NoritoBridge Release Packaging) -->

# NoritoBridge ریلیز پیکجنگ

یہ گائیڈ بیان کرتا ہے کہ `NoritoBridge` کے Swift bindings کو XCFramework کی شکل میں
کیسے publish کیا جائے، تاکہ انہیں Swift Package Manager اور CocoaPods کے ذریعے
استعمال کیا جا سکے۔ یہ workflow، Swift آرٹی فیکٹس کو Rust crate ریلیز کے ساتھ
ہم آہنگ رکھتا ہے جو Iroha کا Norito codec فراہم کرتا ہے۔ اس بارے میں مکمل
walkthrough کے لیے کہ ان publish شدہ artefacts کو Xcode پروجیکٹس کے اندر کیسے consume
کیا جائے (ChaChaPoly استعمال سمیت)، `docs/connect_swift_integration.md` دیکھیں۔

> **نوٹ:** اس فلو کے لیے CI automation تب شامل کیا جائے گا جب macOS builders پر مطلوبہ
> Apple tooling دستیاب ہو (یہ کام Release Engineering کے macOS builder backlog میں
> track ہو رہا ہے)۔ اس وقت تک، درج ذیل steps کو development Mac پر manual طور پر چلانا
> پڑے گا۔

## پری ریکوئزٹس

- macOS host، جس پر جدید ترین مستحکم Xcode Command Line Tools انسٹال ہوں۔
- Rust toolchain، جو workspace کی `rust-toolchain.toml` فائل سے match کرتا ہو۔
- Swift 5.7 یا اس سے نیا toolchain۔
- CocoaPods (Ruby gems کے ذریعے) اگر آپ مرکزی specs repository پر publish کر رہے ہیں۔
- Hyperledger Iroha کی ریلیز سائننگ keys تک رسائی، تاکہ Swift artefacts کو sign/tag
  کیا جا سکے۔

## ورژننگ ماڈل

1. Norito codec کے Rust crate کی ورژن معلوم کریں (`crates/norito/Cargo.toml`)۔
2. workspace کو ریلیز identifier کے ساتھ tag کریں (مثلاً `v2.1.0`)۔
3. Swift پیکیج اور CocoaPods podspec کے لیے یہی semver ورژن استعمال کریں۔
4. جب Rust crate کی ورژن بڑھے تو اسی پروسیس کو دہرائیں اور اس کے مطابق Swift artifact
   publish کریں۔ ٹیسٹنگ کے دوران ورژنز میں metadata suffixes (مثلاً `-alpha.1`) شامل
   ہو سکتے ہیں۔

## Build کے مراحل

1. ریپوزٹری کے root سے helper سکریپٹ چلا کر XCFramework بنائیں:

   ```bash
   ./scripts/build_norito_xcframework.sh --workspace-root "$(pwd)" \
       --output "artifacts/NoritoBridge.xcframework" \
       --profile release
   ```

   یہ اسکرپٹ Rust bridge لائبریری کو iOS اور macOS targets کے لیے compile کرتا ہے اور
   resulting static libraries کو ایک ہی XCFramework فولڈر میں bundle کرتا ہے۔

2. XCFramework کو distribution کے لیے zip کریں:

   ```bash
   ditto -c -k --sequesterRsrc --keepParent \
     artifacts/NoritoBridge.xcframework \
     artifacts/NoritoBridge.xcframework.zip
   ```

3. Swift پیکیج manifest (`IrohaSwift/Package.swift`) کو نئی version اور checksum
   کی طرف point کروائیں:

   ```bash
   swift package compute-checksum artifacts/NoritoBridge.xcframework.zip
   ```

   binary target define کرتے وقت یہی checksum `Package.swift` میں استعمال کریں۔

4. `IrohaSwift/IrohaSwift.podspec` کو نئی version، checksum اور archive URL کے ساتھ
   اپ ڈیٹ کریں۔

5. **اگر bridge میں نئے exports شامل کیے گئے ہوں تو headers regenerate کریں۔** Swift
   bridge اب `connect_norito_set_acceleration_config` کا export فراہم کرتا ہے تاکہ
   `AccelerationSettings`، Metal/GPU backends کو toggle کر سکے۔ zip بنانے سے پہلے اس
   بات کی تصدیق کریں کہ
   `NoritoBridge.xcframework/**/Headers/connect_norito_bridge.h`،
   `crates/connect_norito_bridge/include/connect_norito_bridge.h` سے match کرتا ہو۔

6. Swift validation suite، tag کرنے سے پہلے چلائیں:

   ```bash
   swift test --package-path IrohaSwift
   make swift-ci
   ```

   پہلی کمانڈ یہ verify کرتی ہے کہ Swift package (بشمول `AccelerationSettings`)
   صحیح compile ہو رہا ہے؛ دوسری کمانڈ fixture parity کو validate کرتی ہے، parity/CI
   dashboards کو render کرتی ہے، اور وہی telemetry checks exercise کرتی ہے جو Buildkite
   میں enforced ہیں (بشمول `ci/xcframework-smoke:<lane>:device_tag` metadata requirement)۔

7. generate شدہ artefacts کو release برانچ میں commit کریں اور commit پر tag لگا دیں۔

## Publishing

### Swift Package Manager

- tag کو public Git repository پر push کریں۔
- یقین کر لیں کہ tag، package index (Apple یا community mirror) کے لیے visible ہے۔
- صارفین اب اس فارم میں dependency شامل کر سکتے ہیں:
  `.package(url: "https://github.com/hyperledger/iroha", from: "<version>")`۔

### CocoaPods

1. pod کو مقامی طور پر validate کریں:

   ```bash
   pod lib lint IrohaSwift.podspec --allow-warnings
   ```

2. updated podspec کو trunk پر push کریں:

   ```bash
   pod trunk push IrohaSwift.podspec
   ```

3. تصدیق کریں کہ نئی version، CocoaPods index میں ظاہر ہو رہی ہے۔

## CI کے حوالے سے چیزیں

- ایسا macOS job بنائیں جو packaging اسکرپٹ چلائے، artefacts کو آرکائیو کرے اور
  generated checksum کو workflow output کے طور پر اپ لوڈ کرے۔
- releases کو اس شرط کے ساتھ gate کریں کہ Swift demo app، نئے XCFramework کے ساتھ
  کامیابی سے build ہو جائے۔
- failures کی ڈائگنوسٹکس میں مدد کے لیے build logs محفوظ رکھیں۔

## اضافی automation آئیڈیاز

- جب تمام مطلوبہ targets exposed ہو جائیں تو `xcodebuild -create-xcframework` کو براہ
  راست استعمال کریں۔
- developer مشینوں کے باہر distribution کے لیے code signing/notarisation کو integrate
  کریں۔
- پیک شدہ version کے ساتھ integration tests کو sync میں رکھنے کے لیے SPM dependency
  کو release tag پر pin کریں۔

</div>

