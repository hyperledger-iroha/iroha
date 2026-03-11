---
lang: ar
direction: rtl
source: docs/portal/docs/norito/quickstart.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
العنوان: Inicio Rapido de Norito
الوصف: أنشئ عقدًا Kotodama وصادق عليه وأرسله باستخدام أدوات الإصدار واللون الأحمر المحدد مسبقًا من نظير منفرد.
سبيكة: /norito/quickstart
---

يشير هذا إلى التدفق الذي نتوقعه من المطورين للتعرف على Norito وKotodama للمرة الأولى: ترتيب محدد أحمر لنظير واحد، وتجميع عقد، وتنفيذ عملية تشغيل محلية وإرسالها من أجل Torii مع سطر الأوامر المرجعي.

يصف عقد المثال نفس المستوى/القيمة في حساب المتصل حتى تتمكن من التحقق من التأثير الجانبي للوسيط باستخدام `iroha_cli`.

## المتطلبات السابقة

- [Docker](https://docs.docker.com/engine/install/) مع Compose V2 المؤهلة (يُستخدم لبدء النظير المحدد في `defaults/docker-compose.single.yml`).
- Toolchain de Rust (1.76+) لإنشاء الثنائيات المساعدة في حالة عدم تنزيل المنشورات.
- ثنائيات `koto_compile`، `ivm_run` و`iroha_cli`. يمكنك البناء من خلال الخروج من مساحة العمل كما هو موضح أدناه أو تنزيل عناصر الإصدار المقابلة:

```sh
cargo install --locked --path crates/ivm --bin koto_compile --bin ivm_run
cargo install --locked --path crates/iroha_cli --bin iroha
```

> يتم تثبيت الثنائيات السابقة بأمان جنبًا إلى جنب مع بقية مساحة العمل.
> غير متضمن `serde`/`serde_json`; يتم تطبيق برامج الترميز Norito من طرف إلى طرف.

## 1. بدء تطوير أحمر من نظير واحد فقطيتضمن المستودع حزمة من Docker قم بالإنشاء من أجل `kagami swarm` (`defaults/docker-compose.single.yml`). قم بتوصيل التكوين بسبب الخلل وتكوين العميل والمسبارات الصحية حتى يمكن الوصول إلى Torii في `http://127.0.0.1:8080`.

```sh
docker compose -f defaults/docker-compose.single.yml up --build
```

قم بتحريك الحاوية (في البداية أو في حالة تفريغها). جميع المكالمات اللاحقة لـ CLI تتجه نحو هذا النظير في الوسط `defaults/client.toml`.

## 2. قم بتحرير العقد

أنشئ دليل عمل وحافظ على الحد الأدنى من نموذج Kotodama:

```sh
mkdir -p target/quickstart
cat > target/quickstart/hello.ko <<'KO'
// Writes a deterministic account detail for the transaction authority.

seiyaku Hello {
  // Optional initializer invoked during deployment.
  hajimari() {
    info("Hello from Kotodama");
  }

  // Public entrypoint that records a JSON marker on the caller.
  kotoage fn write_detail() {
    set_account_detail(
      authority(),
      name!("example"),
      json!{ hello: "world" }
    );
  }
}
KO
```

> اختر الحفاظ على مصادر Kotodama والتحكم في الإصدارات. تتوفر الأمثلة المقدمة في البوابة أيضًا في [معرض الأمثلة Norito](./examples/) إذا كنت ترغب في الحصول على نقطة مشاركة كاملة.

## 3. قم بتجميع التشغيل الجاف مع IVM

قم بتجميع العقد مع الرمز الثانوي IVM/Norito (`.to`) وتنفيذه محليًا للتأكد من أن مكالمات المضيف تعمل قبل النقر على اللون الأحمر:

```sh
koto_compile target/quickstart/hello.ko \
  --abi 1 \
  --max-cycles 0 \
  -o target/quickstart/hello.to

ivm_run target/quickstart/hello.to --args '{}'
```

يقوم المشغل بطباعة السجل `info("Hello from Kotodama")` وتشغيل استدعاء النظام `SET_ACCOUNT_DETAIL` مقابل محاكاة المضيف. إذا كان الخيار الثنائي `ivm_tool` متاحًا، فسيقوم `ivm_tool inspect target/quickstart/hello.to` بتمكين ABI المغطى وأجزاء الميزات ونقاط الدخول المصدرة.

## 4. قم بإرسال الرمز الثانوي عبر Toriiمع العقدة للتصحيح، أرسل الرمز الثانوي المجمع إلى Torii باستخدام CLI. يتم استخلاص معرف التطوير المعيب من المفتاح الرئيسي المنشور في `defaults/client.toml`، لأنه يتم حساب معرف الحساب
```
i105...
```

استخدام ملف التكوين لإخفاء عنوان URL الخاص بـ Torii ومعرف السلسلة ومفتاح الشركة:

```sh
iroha --config defaults/client.toml \
  transaction ivm \
  --path target/quickstart/hello.to
```

تقوم CLI بتدوين المعاملة باستخدام Norito، والشركة مع مفتاح التطوير وإرسال النظير للتنفيذ. راقب سجلات Docker للنظام `set_account_detail` أو راقب خروج CLI لتجزئة المعاملة المخترقة.

## 5. التحقق من تغيير الحالة

استخدم نفس ملف CLI للحصول على تفاصيل الحساب لكتابة العقد:

```sh
iroha --config defaults/client.toml \
  account meta get \
  --id i105... \
  --key example | jq .
```

قم بإعادة تشغيل الحمولة النافعة JSON التي تم الرد عليها بواسطة Norito:

```json
{
  "hello": "world"
}
```

إذا أخطأت القيمة، تأكد من أن الخدمة Docker يتم إنشاؤها بعد التنفيذ وأن تجزئة المعاملة التي تم الإبلاغ عنها بواسطة `iroha` هي في الحالة `Committed`.

## الخطوة التالية- استكشاف [معرض الأمثلة](./examples/) الذي تم إنشاؤه تلقائيًا للعرض
  مثل الأجزاء Kotodama يتم تعيينها بشكل متقدم عند استدعاء النظام Norito.
- اشرح [دليل البداية Norito](./getting-started) للشرح
  أدوات أكثر عمقًا للمترجم/المشغل، ونشر البيانات والبيانات التعريفية لـ IVM.
- عندما نعود إلى عقودنا الخاصة، الولايات المتحدة الأمريكية `npm run sync-norito-snippets` en el
  مساحة العمل لإعادة إنشاء المقتطفات القابلة للتنزيل بطريقة تشبه مستندات البوابة والعناصر المصطنعة
  يتم الاحتفاظ بالمزامنة مع التيارات في `crates/ivm/docs/examples/`.