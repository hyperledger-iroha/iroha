<!-- Hebrew translation of docs/source/metal_neon_acceleration_plan.md -->

---
lang: he
direction: rtl
source: docs/source/metal_neon_acceleration_plan.md
status: complete
translator: manual
---

<div dir="rtl">

# תוכנית האצת Metal & NEON (Swift ו-Rust)

המסמך מרכז את התוכנית המשותפת להפיכת האצת החומרה (Metal GPU, ‏NEON / Accelerate SIMD, שילוב StrongBox) לדטרמיניסטית ברחבי מרחב העבודה של Rust וב-Swift SDK. הוא מכסה את פריטי מפת הדרכים תחת **Hardware Acceleration Workstream (macOS/iOS)** ומשמש כמסמך hand-off לצוות ביצועי IVM, לבעלי הגשר של Swift ולבוני הטלמטריה.

> עודכן לאחרונה: ‎2026-01-12  
> בעלי עניין: מוביל ביצועי IVM, מוביל SDK ל-Swift

## יעדים

1. לעשות שימוש חוזר בקרנלי ה-GPU של Rust (Poseidon/BN254/CRC64) על חומרת Apple דרך shader-י Metal, תוך שמירה על פריות דטרמיניסטית מול מסלולי ה-CPU.
2. לחשוף תצורת האצה (`AccelerationConfig`) מקצה לקצה כדי שאפליקציות Swift יוכלו לבחור ב-Metal/NEON/StrongBox בלי לפגוע ב-ABI או בפריות.
3. לאבזר את ה-CI והדשבורדים במדדים על פריות ובנצ'מרקים, ולאתר סטיות בין CPU לבין GPU/SIMD.
4. לשתף לקחים StrongBox / Secure Enclave בין Android (AND2) ל-Swift (IOS4) כדי ששגרות החתימה יישארו מיושרות דטרמיניסטית.

## תוצרים ובעלי תפקיד

| אבן דרך | תוצר | בעלים | יעד |
|---------|------|--------|-----|
| Rust WP2-A/B | ממשקי Metal התואמים את קרנלי CUDA | מוביל ביצועי IVM | פברואר ‎2026 |
| Rust WP2-C | בדיקות פריות BN254 ב-Metal + מסלול CI | מוביל ביצועי IVM | רבעון 2 ‎2026 |
| Swift IOS6 | חיבורי גשר (`connect_norito_set_acceleration_config`), API של SDK, דוגמאות | בעלי גשר Swift | הושלם (ינואר ‎2026) |
| Swift IOS5 | דוגמאות/מסמכים המדגימים שימוש באפשרויות | מוביל DX ל-Swift | רבעון 2 ‎2026 |
| Telemetry | הזנות דשבורד עם פריות ובנצ'מרקים | PM תוכנית Swift / צוות טלמטריה | פיילוט רבעון 2 ‎2026 |
| CI | הרצת smoke ל-XCFramework המשווה CPU מול Metal/NEON על ציוד אמיתי | מוביל QA ל-Swift | רבעון 2 ‎2026 |
| StrongBox | בדיקות פריות חתימה מגובות חומרה (וקטורים משותפים) | TL קריפטו Android / צוות אבטחת Swift | רבעון 3 ‎2026 |

## ממשקים וחוזי API

### Rust (`ivm::AccelerationConfig`)
- לשמר את השדות הקיימים (`enable_metal`, `enable_cuda`, `max_gpus`, ספי עומס).
- להוסיף warm-up ל-Metal כדי למנוע שיהוי שימוש ראשון (Rust ‎#15875).
- לחשוף API-ים למדווחי דשבורד, לדוגמה `ivm::vector::metal_status()` המחזיר {enabled, parity, last_error}.
- להזרים מדדי בנצ'מרק (זמני Merkle, תפוקת CRC) דרך Hooks טלמטריים עבור `ci/xcode-swift-parity`.

### C FFI (`connect_norito_bridge`)
- מבנה חדש `connect_norito_acceleration_config` (קיים).
- גטרים זמינים: `connect_norito_get_acceleration_config` (תצורה בלבד) ו-`connect_norito_get_acceleration_state` (תצורה + סטטוס פריות), כך שהממשק שיקף את ה-setter.
- לתעד את פריסת המבנה בהערות כותרת עבור צרכני SPM / CocoaPods.

### Swift (`AccelerationSettings`)
- ברירת מחדל: Metal פעיל, CUDA כבוי, ספים יורשים (nil).
- ערכים שליליים נזרקים; `apply()` נקרא אוטומטית בידי `IrohaSDK`.
- כאשר Rust יחשוף דיאגנוסטיקה לפריות, ישקף זאת בצד Swift.
- לעדכן דוגמאות ו-README כדי להדגים הפעלה וטיוב טלמטריה.

### טלמטריה (דשבורדים ומייצאים)
- הזנת פריות (`mobile_parity.json`):
  - `acceleration.metal/neon/strongbox` → {enabled, parity, perf_delta_pct}
  - ‎`perf_delta_pct`‎ מציין השוואה מול מסלול CPU.
- הזנת CI (`mobile_ci.json`):
  - `acceleration_bench.metal_vs_cpu_merkle_ms` → {cpu, metal}
  - `acceleration_bench.neon_crc64_throughput_mb_s` → ערך Double
- המייצאים יאספו נתונים מהרצות Rust benchmarks או CI (למשל הרצת מיקרו-בנצ' בתוך ‎`ci/xcode-swift-parity`‎).

## אסטרטגיית בדיקות

1. **בדיקות פריות יחידה (Rust):** לוודא שקורנלי Metal מפיקים אותה תוצאה כמו CPU עבור וקטורים דטרמיניסטיים; להריץ תחת ‎`cargo test -p ivm --features metal`‎.
2. **Smoke harness ב-Swift:** להרחיב את רץ הבדיקות של IOS6 כך שיבצע קידוד Merkle/CRC64 במסלולי CPU מול Metal גם על אמולטורים וגם על מכשירים עם StrongBox; לרשום תוצאות ומצב פריות.
3. **CI:** לעדכן את `norito_bridge_ios.yml` (הקורא כבר את ‎`make swift-ci`‎) כך שיפיק מדדי האצה, יוודא שהמטא-דאטה של Buildkite `ci/xcframework-smoke:<lane>:device_tag` קיים, ויכשל במקרה של סטיות פריות או ביצועים.
4. **דשבורדים:** להבטיח ששדות חדשים מוצגים בכלי CLI, ושמייצאים מפיקים נתונים לפני שהדשבורדים הופכים ל-Live.

## שאלות פתוחות

1. **שחרור משאבי Metal:** לוודא שהקריאה `warm_up_metal()` היא אידמפוטנטית ובטוחה ללifecycle של אפליקציית Swift (ללא דליפות בעת מעבר רקע/קדמה).
2. **בסיסי בנצ'מרק:** לקבוע ספי ביצועים (איזה delta הוא רגרסיה) בין Metal ל-CPU ולהטמיע התראות בדשבורד.
3. **נפילות StrongBox:** לתאם טיפול בכשלי attestation בין Android ל-Swift, ולשתף מסמכי fallback להפקת מפתחות.
4. **אחסון טלמטריה:** לבחור יעד לאחסון הזנות JSON (S3, ארטיפקטים של build וכו') כאשר המייצאים יפעלו.

## צעדים הבאים (רבעון 1 ‎2026)

- [ ] Rust: להשלים מסמך ממשק Kernel של Metal ולשתף Header עם צוות Swift.
- [x] Swift: לחשוף אפשרויות האצה ברמת ה-SDK (הושלם ינואר ‎2026).
- [ ] טלמטריה: להגדיר את יישום המייצאים וחיבורם ל-CI עבור השדות החדשים.
- [ ] QA ל-Swift: להרחיב את תוכנית smoke עם מטריצת כיסוי לאצה.
- [ ] לעדכן את `status.md` כאשר המייצאים יתחילו לפלוט נתונים אמיתיים (יעד רבעון 2 ‎2026).

</div>
