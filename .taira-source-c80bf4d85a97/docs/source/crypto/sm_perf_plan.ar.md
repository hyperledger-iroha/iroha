---
lang: ar
direction: rtl
source: docs/source/crypto/sm_perf_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 493c3c0f6a991b2a5d04f33f97b7e97bff372271c5c57751ff41f5e86d43cbc7
source_last_modified: "2026-01-03T18:07:57.107521+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

## التقاط أداء SM والخطة الأساسية

الحالة: تمت صياغته — 2025-05-18  
المالكون: مجموعة الأداء (القائد)، Infra Ops (جدولة المختبر)، QA Guild (بوابة CI)  
مهام خريطة الطريق ذات الصلة: SM-4c.1a/b، SM-5a.3b، FASTPQ Stage 7 الالتقاط عبر الأجهزة

### 1. الأهداف
1. سجل متوسطات Neoverse في `sm_perf_baseline_aarch64_unknown_linux_gnu_{scalar,auto,neon_force}.json`. يتم تصدير خطوط الأساس الحالية من الالتقاط `neoverse-proxy-macos` ضمن `artifacts/sm_perf/2026-03-lab/neoverse-proxy-macos/` (تسمية وحدة المعالجة المركزية `neoverse-proxy-macos`) مع توسيع تفاوت مقارنة SM3 إلى 0.70 لـ aarch64 macOS/Linux. عند فتح الوقت المعدني، أعد تشغيل `scripts/sm_perf_capture_helper.sh --matrix --cpu-label neoverse-n2-b01 --output artifacts/sm_perf/<date>/neoverse-n2-b01` على مضيف Neoverse وقم بترقية المتوسطات المجمعة إلى خطوط الأساس.  
2. اجمع متوسطات x86_64 المطابقة حتى يتمكن `ci/check_sm_perf.sh` من حماية كلا الفئتين المضيفتين.  
3. قم بنشر إجراء التقاط قابل للتكرار (الأوامر، تخطيط المصنوعات اليدوية، المراجعين) بحيث لا تعتمد بوابات الأداء المستقبلية على المعرفة القبلية.

### 2. توفر الأجهزة
لا يمكن الوصول إلا إلى مضيفي Apple Silicon (macOS Arm64) في مساحة العمل الحالية. يتم تصدير الالتقاط `neoverse-proxy-macos` باعتباره خط أساس Linux المؤقت، ولكن التقاط متوسطات Neoverse أو x86_64 المعدنية لا يزال يتطلب أجهزة المعمل المشتركة التي يتم تتبعها ضمن `INFRA-2751`، ليتم تشغيلها بواسطة مجموعة عمل الأداء بمجرد فتح نافذة المعمل. يتم الآن حجز نوافذ الالتقاط المتبقية وتتبعها في شجرة المصنوعات اليدوية:

- تم حجز Neoverse N2 المعدني العاري (Tokyo Rack B) بتاريخ 12/03/2026. سيقوم المشغلون بإعادة استخدام الأوامر من القسم 3 وتخزين العناصر تحت `artifacts/sm_perf/2026-03-lab/neoverse-b01/`.
- تم حجز x86_64 Xeon (Zurich Rack D) بتاريخ 2026-03-19 مع تعطيل SMT لتقليل الضوضاء؛ سوف تهبط المصنوعات اليدوية تحت `artifacts/sm_perf/2026-03-lab/xeon-d01/`.
- بعد كلا التشغيلين، قم بترقية الوسيطات إلى JSONs الأساسية وقم بتمكين بوابة CI في `ci/check_sm_perf.sh` (تاريخ التبديل المستهدف: 25-03-2026).

وحتى تلك التواريخ، لا يمكن تحديث سوى الخطوط الأساسية لنظام التشغيل macOS Arm64 محليًا.### 3. إجراء الالتقاط
1. ** سلاسل أدوات المزامنة **  
   ```bash
   rustup override set $(cat rust-toolchain.toml)
   cargo fetch
   ```
2. **إنشاء مصفوفة الالتقاط** (لكل مضيف)  
   ```bash
   scripts/sm_perf_capture_helper.sh --matrix \
     --output artifacts/sm_perf/2025-07-lab/${HOSTNAME}
   ```
   يقوم المساعد الآن بكتابة `capture_commands.sh` و`capture_plan.json` ضمن الدليل الهدف. يقوم البرنامج النصي بإعداد مسارات التقاط `raw/*.json` لكل وضع حتى يتمكن فنيو المعمل من تجميع عمليات التشغيل بشكل حتمي.
3. **تشغيل اللقطات**  
   نفذ كل أمر من `capture_commands.sh` (أو قم بتشغيل ما يعادله يدويًا)، مع التأكد من أن كل وضع يصدر كائن JSON blob منظم عبر `--capture-json`. قم دائمًا بتوفير تسمية مضيف عبر `--cpu-label "<model/bin>"` (أو `SM_PERF_CPU_LABEL=<label>`) بحيث تسجل بيانات تعريف الالتقاط وخطوط الأساس اللاحقة الأجهزة الدقيقة التي أنتجت الوسائط. يقوم المساعد بالفعل بتوفير المسار المناسب؛ للتشغيل اليدوي النمط هو:
   ```bash
   SM_PERF_CAPTURE_LABEL=auto \
   scripts/sm_perf.sh --mode auto \
     --cpu-label "neoverse-n2-lab-b01" \
     --capture-json artifacts/sm_perf/2025-07-lab/${HOSTNAME}/raw/auto.json
   ```
4. **التحقق من صحة النتائج**  
   ```bash
   scripts/sm_perf_check \
     artifacts/sm_perf/2025-07-lab/${HOSTNAME}/raw/*.json
   ```
   تأكد من بقاء التباين ضمن ±3% بين عمليات التشغيل. إذا لم يكن الأمر كذلك، فأعد تشغيل الوضع المتأثر ولاحظ إعادة المحاولة في السجل.
5. **الترويج للمتوسطات**  
   استخدم `scripts/sm_perf_aggregate.py` لحساب المتوسطات ونسخها إلى ملفات JSON الأساسية:
   ```bash
   scripts/sm_perf_aggregate.py \
     artifacts/sm_perf/2025-07-lab/${HOSTNAME}/raw/*.json \
     --output artifacts/sm_perf/2025-07-lab/${HOSTNAME}/aggregated.json
   ```
   تلتقط المجموعات المساعدة بواسطة `metadata.mode`، وتتحقق من أن كل مجموعة تشترك في
   نفس `{target_arch, target_os}` الثلاثي، ويصدر ملخص JSON بإدخال واحد
   لكل وضع. الوسيطات التي يجب أن تهبط في الملفات الأساسية تعيش تحتها
   `modes.<mode>.benchmarks`، في حين أن الكتلة `statistics` المصاحبة تسجل
   قائمة العينات الكاملة، الحد الأدنى/الحد الأقصى، المتوسط، والقياسات السكانية للمراجعين وCI.
   بمجرد وجود الملف المجمع، يمكنك الكتابة تلقائيًا لملفات JSON الأساسية (مع
   خريطة التسامح القياسية) عبر:
   ```bash
   scripts/sm_perf_promote_baseline.py \
     artifacts/sm_perf/2025-07-lab/${HOSTNAME}/aggregated.json \
     --out-dir crates/iroha_crypto/benches \
     --target-os unknown_linux_gnu \
     --overwrite
   ```
   تجاوز `--mode` لتقييد مجموعة فرعية أو `--cpu-label` لتثبيت
   اسم وحدة المعالجة المركزية المسجلة عندما يحذفه المصدر المجمع.
   بمجرد انتهاء كلا المضيفين لكل بنية، قم بالتحديث:
   -`sm_perf_baseline_aarch64_unknown_linux_gnu_{scalar,auto,neon_force}.json`
   - `sm_perf_baseline_x86_64_unknown_linux_gnu_{scalar,auto}.json` (جديد)

   تعكس ملفات `aarch64_unknown_linux_gnu_*` الآن `m3-pro-native`
   الالتقاط (تم الاحتفاظ بتسمية وحدة المعالجة المركزية وملاحظات البيانات التعريفية) حتى يتمكن `scripts/sm_perf.sh` من ذلك
   الاكتشاف التلقائي لمضيفي aarch64-unknown-linux-gnu بدون إشارات يدوية. عندما
   اكتمل تشغيل المعمل المعدني، أعد تشغيل `scripts/sm_perf.sh --mode 
   - صناديق الكتابة الأساسية/iroha_crypto/benches/sm_perf_baseline_aarch64_unknown_linux_gnu_.json`
   مع اللقطات الجديدة للكتابة فوق المتوسطات المؤقتة وختم الحقيقي
   تسمية المضيف.

   > المرجع: تم التقاط Apple Silicon لشهر يوليو 2025 (علامة وحدة المعالجة المركزية `m3-pro-local`)
   > تمت أرشفته تحت `artifacts/sm_perf/2025-07-lab/takemiyacStudio.lan/{raw,aggregated.json}`.
   > اعكس هذا التصميم عند نشر عناصر Neoverse/x86 حتى يطلع عليها المراجعون
   > يمكن أن يختلف المخرجات الأولية/المجمعة باستمرار.

### 4. تخطيط القطع الأثرية وتسجيل الخروج
```
artifacts/sm_perf/
  2025-07-lab/
    neoverse-b01/
      raw/
      aggregated.json
      run-log.md
    neoverse-b02/
      …
    xeon-d01/
    xeon-d02/
```
- يسجل `run-log.md` تجزئة الأمر ومراجعة git والمشغل وأي حالات شاذة.
- يتم تغذية ملفات JSON المجمعة مباشرة في التحديثات الأساسية ويتم إرفاقها بمراجعة الأداء في `docs/source/crypto/sm_perf_baseline_comparison.md`.
- تقوم QA Guild بمراجعة المصنوعات اليدوية قبل تغيير الخطوط الأساسية والتوقيع في `status.md` ضمن قسم الأداء.### 5. الجدول الزمني لبوابة CI
| التاريخ | معلم | العمل |
|------|-----------|--------|
| 2025-07-12 | تكتمل لقطات Neoverse | قم بتحديث ملفات `sm_perf_baseline_aarch64_*` JSON، وقم بتشغيل `ci/check_sm_perf.sh` محليًا، وافتح PR مع إرفاق العناصر. |
| 2025-07-24 | اكتملت لقطات x86_64 | إضافة ملفات أساسية جديدة + بوابة في `ci/check_sm_perf.sh`؛ تأكد من أن ممرات CI المتقاطعة تستهلكها. |
| 2025-07-27 | إنفاذ CI | تمكين سير العمل `sm-perf-gate` للتشغيل على كلا الفئتين المضيفتين؛ تفشل عمليات الدمج إذا تجاوزت الانحدارات التفاوتات التي تم تكوينها. |

### 6. التبعيات والاتصالات
- تنسيق تغييرات الوصول إلى المعمل عبر `infra-ops@iroha.tech`.  
- تقوم مجموعة عمل الأداء بنشر تحديثات يومية في قناة `#perf-lab` أثناء تشغيل عمليات الالتقاط.  
- تقوم QA Guild بإعداد فرق المقارنة (`scripts/sm_perf_compare.py`) حتى يتمكن المراجعون من تصور الدلتا.  
- بمجرد دمج الخطوط الأساسية، قم بتحديث `roadmap.md` (SM-4c.1a/b، SM-5a.3b) و`status.md` مع ملاحظات إكمال الالتقاط.

من خلال هذه الخطة، يكتسب عمل تسريع SM وسائط قابلة للتكرار، وبوابات CI، ومسار أدلة يمكن تتبعه، مما يرضي عنصر الإجراء "نوافذ المختبر الاحتياطية والتقاط الوسائط".

### 7. بوابة CI والدخان المحلي

- `ci/check_sm_perf.sh` هي نقطة دخول CI الأساسية. يتم إصداره إلى `scripts/sm_perf.sh` لكل وضع في `SM_PERF_MODES` (الإعداد الافتراضي هو `scalar auto neon-force`) ويقوم بتعيين `CARGO_NET_OFFLINE=true` بحيث تعمل المقاعد بشكل حتمي على صور CI.  
- يقوم `.github/workflows/sm-neon-check.yml` الآن باستدعاء البوابة الموجودة على مشغل macOS Arm64 بحيث يقوم كل طلب سحب بتمرين الثلاثي العددي/التلقائي/قوة النيون عبر نفس المساعد المستخدم محليًا؛ سيتم ربط مسار Linux/Neoverse التكميلي بمجرد التقاط x86_64 للأرض ويتم تحديث الخطوط الأساسية لوكيل Neoverse مع التشغيل المعدني.  
- يمكن للمشغلين تجاوز قائمة الأوضاع محليًا: يقوم `SM_PERF_MODES="scalar" bash ci/check_sm_perf.sh` بقص التشغيل إلى مسار واحد لإجراء اختبار دخان سريع، بينما تتم إعادة توجيه الوسيطات الإضافية (على سبيل المثال `--tolerance 0.20`) مباشرة إلى `scripts/sm_perf.sh`.  
- `make check-sm-perf` يغطي الآن البوابة لراحة المطور؛ يمكن لوظائف CI استدعاء البرنامج النصي مباشرة بينما يستغل مطورو macOS هدف الإنشاء.  
- بمجرد وصول الخطوط الأساسية Neoverse/x86_64، سيلتقط نفس البرنامج النصي JSON المناسب عبر منطق الاكتشاف التلقائي للمضيف الموجود بالفعل في `scripts/sm_perf.sh`، لذلك ليست هناك حاجة إلى أسلاك إضافية في سير العمل بما يتجاوز تعيين قائمة الوضع المطلوب لكل تجمع مضيف.

### 8. مساعد التحديث ربع السنوي- قم بتشغيل `scripts/sm_perf_quarterly.sh --owner "<name>" --cpu-label "<label>" [--quarter YYYY-QN] [--output-root artifacts/sm_perf]` لسك دليل ربع مختوم مثل `artifacts/sm_perf/2026-Q1/<label>/`. يغلف المساعد `scripts/sm_perf_capture_helper.sh --matrix` ويصدر `capture_commands.sh`، و`capture_plan.json`، و`quarterly_plan.json` (المالك + بيانات تعريف الربع) حتى يتمكن مشغلو المعمل من جدولة عمليات التشغيل بدون خطط مكتوبة بخط اليد.
- تنفيذ `capture_commands.sh` الذي تم إنشاؤه على المضيف المستهدف، وتجميع المخرجات الأولية باستخدام `scripts/sm_perf_aggregate.py --output <dir>/aggregated.json`، وتعزيز الوسيطات في JSONs الأساسية عبر `scripts/sm_perf_promote_baseline.py --out-dir crates/iroha_crypto/benches --overwrite`. أعد تشغيل `ci/check_sm_perf.sh` للتأكد من بقاء التفاوتات باللون الأخضر.
- عندما تتغير الأجهزة أو سلاسل الأدوات، قم بتحديث تفاوتات/ملاحظات المقارنة في `docs/source/crypto/sm_perf_baseline_comparison.md`، وقم بتشديد تفاوتات `ci/check_sm_perf.sh` إذا استقرت المتوسطات الجديدة، وقم بمحاذاة أي حدود لوحة معلومات/تنبيه مع الخطوط الأساسية الجديدة حتى تظل إنذارات العمليات ذات معنى.
- تنفيذ `quarterly_plan.json`، و`capture_plan.json`، و`capture_commands.sh`، وJSON المجمعة جنبًا إلى جنب مع التحديثات الأساسية؛ إرفاق نفس العناصر بتحديثات الحالة/خارطة الطريق من أجل إمكانية التتبع.