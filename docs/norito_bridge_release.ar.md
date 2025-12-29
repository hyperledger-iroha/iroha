---
lang: ar
direction: rtl
source: docs/norito_bridge_release.md
status: complete
translator: manual
source_hash: bc7c766ff5fb0504f4da43a017bf294758800b9c815affc8f97b9bcc94ae8e15
source_last_modified: "2025-11-02T04:40:28.805628+00:00"
translation_last_reviewed: 2025-11-14
---

<div dir="rtl">

<!-- الترجمة العربية لـ docs/norito_bridge_release.md (NoritoBridge Release Packaging) -->

# حزمة إصدار NoritoBridge

توضّح هذه الوثيقة الخطوات المطلوبة لنشر الـ bindings الخاصة بـ Swift تحت اسم
`NoritoBridge` على شكل XCFramework يمكن استهلاكه من Swift Package Manager و
CocoaPods. يحافظ هذا الـ workflow على تزامن artefacts الخاصة بـ Swift مع إصدارات crate
المكتوب بـ Rust والذي يقدّم Norito codec الخاص بـ Iroha. للحصول على شرح تفصيلي حول
استهلاك الـ artifacts المنشورة داخل تطبيق (إعداد مشروع Xcode، استخدام ChaChaPoly،
إلخ)، راجع `docs/connect_swift_integration.md`.

> **ملاحظة:** ستتم إضافة أتمتة CI لهذا المسار بمجرد توفّر builders تعمل بنظام macOS
> مع أدوات Apple المطلوبة (يتم تتبع هذه المهمة في backlog الخاص ببناة macOS في فريق
> Release Engineering). حتى ذلك الحين يجب تنفيذ الخطوات التالية يدويًا على جهاز Mac
> خاص بالمطوّر.

## المتطلبات المسبقة

- جهاز macOS يحتوي على أحدث إصدار مستقر من Xcode Command Line Tools.
- Toolchain لـ Rust متوافق مع `rust-toolchain.toml` الموجود في جذر الـ workspace.
- Toolchain لـ Swift بإصدار 5.7 أو أحدث.
- أداة CocoaPods (عبر Ruby gems) إذا كنت ستنشر إلى مستودع الـ specs المركزي.
- إمكانية الوصول إلى مفاتيح توقيع الإصدارات الخاصة بـ Hyperledger Iroha من أجل
  تمييز artefacts الخاصة بـ Swift.

## نموذج إدارة الإصدارات

1. حدّد إصدار crate Rust الخاص بـ Norito codec (`crates/norito/Cargo.toml`).
2. ضع tag على الـ workspace باستخدام معرف الإصدار (مثلًا `v2.1.0`).
3. استخدم نفس الإصدار semver لحزمة Swift وملف podspec الخاص بـ CocoaPods.
4. عند زيادة إصدار crate في Rust، كرّر نفس العملية وانشر artefact Swift مطابقًا.
   يمكن أن تتضمن الإصدارات لاحقة metadata (مثلًا `-alpha.1`) أثناء الاختبارات.

## خطوات البناء (Build)

1. من جذر الـ repository، شغّل سكربت المساعدة (helper) لتجميع الـ XCFramework:

   ```bash
   ./scripts/build_norito_xcframework.sh --workspace-root "$(pwd)" \
       --output "artifacts/NoritoBridge.xcframework" \
       --profile release
   ```

   يقوم السكربت ببناء مكتبة الـ bridge المكتوبة بـ Rust لأهداف iOS و macOS ثم يجمع
   الـ static libraries الناتجة في مجلد XCFramework واحد.

2. اضغط (zip) الـ XCFramework للتوزيع:

   ```bash
   ditto -c -k --sequesterRsrc --keepParent \
     artifacts/NoritoBridge.xcframework \
     artifacts/NoritoBridge.xcframework.zip
   ```

3. حدّث manifest الخاص بحزمة Swift (`IrohaSwift/Package.swift`) للإشارة إلى الإصدار
   والـ checksum الجديدين:

   ```bash
   swift package compute-checksum artifacts/NoritoBridge.xcframework.zip
   ```

   سجّل قيمة الـ checksum في `Package.swift` عند تعريف الـ binary target.

4. حدّث ملف `IrohaSwift/IrohaSwift.podspec` بإصدار جديد و checksum جديد و URL الأرشيف.

5. **أعد توليد الـ headers إذا تمّت إضافة exports جديدة إلى الـ bridge.** يُظهر bridge
   في Swift الآن `connect_norito_set_acceleration_config` بحيث يستطيع
   `AccelerationSettings` تفعيل backends الخاصة بـ Metal/GPU. تأكّد من أن
   `NoritoBridge.xcframework/**/Headers/connect_norito_bridge.h` يطابق
   `crates/connect_norito_bridge/include/connect_norito_bridge.h` قبل عمل zip.

6. شغّل مجموعة اختبارات التحقق في Swift قبل إنشاء tag للإصدار:

   ```bash
   swift test --package-path IrohaSwift
   make swift-ci
   ```

   يتأكد الأمر الأول من أن حزمة Swift (بما في ذلك `AccelerationSettings`) ما زالت
   تُبنى بنجاح، بينما يتحقق الأمر الثاني من par
   ity الخاصة بالـ fixtures، ويولّد لوحات par
   ity/CI، ويمرّ بنفس اختبارات الـ telemetry المفروضة على Buildkite (بما في ذلك
   شرط وجود metadata من نوع `ci/xcframework-smoke:<lane>:device_tag`).

7. نفّذ commit للـ artifacts المُولّدة على فرع release ثم ضع tag على هذا الـ commit.

## النشر (Publishing)

### Swift Package Manager

- ادفع (push) الـ tag إلى مستودع Git عام.
- تأكّد من أن الـ tag مرئية لمؤشر الحزم (Apple أو mirror الخاص بالمجتمع).
- يمكن للمستهلكين الآن الاعتماد على:
  `.package(url: "https://github.com/hyperledger/iroha", from: "<version>")`.

### CocoaPods

1. تحقّق من الـ pod محليًا:

   ```bash
   pod lib lint IrohaSwift.podspec --allow-warnings
   ```

2. ادفع podspec المحدّث إلى trunk:

   ```bash
   pod trunk push IrohaSwift.podspec
   ```

3. تأكّد من ظهور الإصدار الجديد في فهرس CocoaPods.

## اعتبارات CI

- أنشئ job خاص بـ macOS يقوم بتشغيل سكربت الـ packaging، أرشفة artefacts، ورفع
  الـ checksum المُولَّد كمخرج (output) للـ workflow.
- اجعل إصدارات الـ release تعتمد على بناء (build) تطبيق Swift demo ضد الـ framework
  الجديد.
- احتفظ بسجلات الـ build للمساعدة في تشخيص الأعطال.

## أفكار إضافية للأتمتة

- استخدام `xcodebuild -create-xcframework` مباشرة بمجرد تعريض جميع الـ targets
  المطلوبة.
- دمج عملية التوقيع/notarisation لدعم التوزيع خارج أجهزة المطورين.
- الحفاظ على اختبارات التكامل متزامنة مع الإصدار المُعبأ عن طريق تثبيت (pin)
  اعتماد SPM على tag الإصدار.

</div>

