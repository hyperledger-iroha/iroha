<!-- Hebrew translation of docs/norito_bridge_release.md -->

---
lang: he
direction: rtl
source: docs/norito_bridge_release.md
status: complete
translator: manual
---

<div dir="rtl">

# אריזת NoritoBridge לשחרור

המדריך מפרט את השלבים לפרסום הכריכות של `NoritoBridge` כספריית XCFramework לשימוש ב-Swift Package Manager וב-CocoaPods, תוך שמירה על תאימות לגרסאות ה-Rust שמספקות את Norito codec. לשילוב מלא בצד האפליקציה (Xcode, ChaChaPoly, ConnectSession) ראו `docs/connect_swift_integration.md`.

> **הערה:** ברגע שבוני macOS עם כלי Apple יהיו זמינים (נעקב על־ידי Release Engineering בבאק־לוג הייעודי) התהליך יאוטומט ב-CI. עד אז יש לבצע את השלבים ידנית בתחנת פיתוח macOS.

## דרישות מקדימות

- מחשב macOS עם כלי שורת הפקודה של Xcode בגרסה היציבה האחרונה.
- כלי Rust בהתאם ל-`rust-toolchain.toml`.
- Swift 5.7 ומעלה.
- CocoaPods (דרך Ruby gems) אם מפרסמים למאגר הרשמי.
- גישה למפתחות החתימה של Hyperledger Iroha עבור תיוג Artefacts של Swift.

## מודל גרסאות

1. בדקו את גרסת הקרייט Rust של Norito (`crates/norito/Cargo.toml`).
2. תייגו את הריפו בגרסת השחרור (למשל `v2.1.0`).
3. השתמשו באותה גרסה עבור Swift Package וה-podspec.
4. בכל שינוי גרסה של הקרייט, חזרו על התהליך ופרסמו Artefact Swift תואם. ניתן להשתמש בסיומות (כגון `-alpha.1`) בעת בדיקות.

## שלבי בנייה

1. הפעלת הסקריפט ליצירת ה-XCFramework:

   ```bash
   ./scripts/build_norito_xcframework.sh --workspace-root "$(pwd)" \
       --output "artifacts/NoritoBridge.xcframework" \
       --profile release
   ```

   הסקריפט מקמפל את ספריית הגשר ליעדי iOS/macOS ומאגד תחת XCFramework.

2. אריזת ה-XCFramework:

   ```bash
   ditto -c -k --sequesterRsrc --keepParent \
     artifacts/NoritoBridge.xcframework \
     artifacts/NoritoBridge.xcframework.zip
   ```

3. עדכון מניפסט Swift (`IrohaSwift/Package.swift`) עם גרסה ו-checksum:

   ```bash
   swift package compute-checksum artifacts/NoritoBridge.xcframework.zip
   ```

   הזינו את ה-checksum ביעד הבינרי.

4. עדכון `IrohaSwift/IrohaSwift.podspec` בגרסה, checksum ו-URL החדש.

5. **רענון כותרות אם נוספו יצואי Bridge.** לדוגמה `connect_norito_set_acceleration_config`. ודאו ש-`Headers/connect_norito_bridge.h` ב-XCFramework זהה ל-`crates/connect_norito_bridge/include/connect_norito_bridge.h`.

6. הריצו את בדיקות Swift לפני תיוג:

   ```bash
   swift test --package-path IrohaSwift
   make swift-ci
   ```

   הפקודה הראשונה בודקת את חבילת Swift (כולל `AccelerationSettings`); השנייה מאמתת פריות פיקסצ'רים, בודקת את פידי הדשבורד ומרנדרת את התקצירים. ב-Buildkite התהליך נשען על מטא-דאטה `ci/xcframework-smoke:<lane>:device_tag`, לכן אחרי שינויי פייפליין או תגי סוכנים ודאו שהמטא-דאטה מופיע.

7. בצעו Commit לארטיפקטים בענף release וצרו Tag.

## פרסום

### Swift Package Manager

- דחפו את ה-Tag לריפו ציבורי.
- ודאו שה-Tag נגיש למדד החבילות.
- לקוחות יכלו לציין `.package(url: "https://github.com/hyperledger/iroha", from: "<version>")`.

### CocoaPods

1. ולידציה מקומית:

   ```bash
   pod lib lint IrohaSwift.podspec --allow-warnings
   ```

2. דחיפה ל-trunk:

   ```bash
   pod trunk push IrohaSwift.podspec
   ```

3. אמתו שהגרסה החדשה מופיעה ב-CocoaPods.

## שיקולי CI

- צרו Job macOS שמריץ את הסקריפט, מעלה ארטיפקטים ושומר checksum.
- התנו שחרור בכך שאפליקציית הדמו תיבנה מול ה-XCFramework.
- שמרו לוגים לצורך דיבוג.

## רעיונות לאוטומציה נוספת

- מעבר ל-`xcodebuild -create-xcframework` לאחר שכל היעדים זמינים.
- שילוב חתימה/נוטריזציה להפצה מעבר לסביבת פיתוח.
- שמירת בדיקות אינטגרציה באותו תזמון ע"י הצמדת תלות SPM לתג השחרור.

</div>
