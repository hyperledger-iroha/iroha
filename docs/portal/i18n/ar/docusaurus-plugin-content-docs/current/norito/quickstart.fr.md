---
lang: ar
direction: rtl
source: docs/portal/docs/norito/quickstart.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
العنوان: ترسيم الزواج Norito
الوصف: قم بإنشاء عقد Kotodama وصلاحيته ونشره مع إطلاق الإصدار والشبكة أحادية الزوج بشكل افتراضي.
سبيكة: /norito/quickstart
---

توضح هذه الإرشادات سير العمل الذي نتابعه من المطورين عندما نكتشف Norito وKotodama للمرة الأولى: إنشاء شبكة محددة أحادية الزوج، وتجميع عقد، والتشغيل المحلي المحلي، ثم المبعوث عبر Torii لو CLI دي المرجع.

يوضح عقد المثال زوجًا من المفتاح/القيمة في حساب المستأنف حتى تتمكن من التحقق من تأثير اللوحة على الفور باستخدام `iroha_cli`.

## المتطلبات الأساسية

- [Docker](https://docs.docker.com/engine/install/) مع Compose V2 نشط (استخدم من أجل تحديد الزوج المحدد في `defaults/docker-compose.single.yml`).
- Toolchain Rust (1.76+) لإنشاء الثنائيات المساعدة إذا لم تقم بتنزيلها دون أي منشورات.
- الثنائيات `koto_compile`، `ivm_run` و`iroha_cli`. يمكنك إنشاء مستندات بعد الخروج من مساحة العمل مثل هذه أو تنزيل عناصر الإصدار المقابلة:

```sh
cargo install --locked --path crates/ivm --bin koto_compile --bin ivm_run
cargo install --locked --path crates/iroha_cli --bin iroha
```

> الملفات الثنائية التي لا تحتوي على خطر التثبيت مع بقية مساحة العمل.
> Ils ne lient jamais `serde`/`serde_json` ; يتم تطبيق برامج الترميز Norito بشكل متكرر.

## 1. إنشاء شبكة تطوير أحاديةيشتمل المستودع على حزمة Docker قم بتكوين النوع على قدم المساواة `kagami swarm` (`defaults/docker-compose.single.yml`). يتم توصيل التكوين افتراضيًا، ويتيح عميل التكوين والمسبارات الصحية إمكانية الانضمام إلى Torii و`http://127.0.0.1:8080`.

```sh
docker compose -f defaults/docker-compose.single.yml up --build
```

اترك الحاوية تدور (في الخطة الرئيسية أو المنفصلة). Toutes les Commandes CLI suivantes ciblent ce Pair عبر `defaults/client.toml`.

## 2. اكتب العقد

أنشئ مرجعًا للعمل وسجل المثال Kotodama الأدنى :

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

> تفضيل الحفاظ على المصادر Kotodama والتحكم في الإصدار. الأمثلة الرائعة على الباب متاحة أيضًا في [جاليري الأمثلة Norito](./examples/) إذا كنت تريد نقطة انطلاق أكثر ثراءً.

## 3. المترجم والتشغيل الجاف مع IVM

قم بتجميع العقد بالرمز الثانوي IVM/Norito (`.to`) وقم بتنفيذ الموقع لتأكيد إعادة تشغيل مكالمات المضيف قبل لمس إعادة التشغيل:

```sh
koto_compile target/quickstart/hello.ko \
  --abi 1 \
  --max-cycles 0 \
  -o target/quickstart/hello.to

ivm_run target/quickstart/hello.to --args '{}'
```

يقوم المشغل بطباعة السجل `info("Hello from Kotodama")` وتشغيل استدعاء النظام `SET_ACCOUNT_DETAIL` ضد محاكاة المضيف. إذا كان الخيار الثنائي `ivm_tool` متاحًا، فإن `ivm_tool inspect target/quickstart/hello.to` يعرض واجهة ABI وأجزاء الميزات ونقاط الإدخال المصدرة.

## 4. احصل على الرمز الثانوي عبر Toriiأثناء المضي قدمًا في عملية التنفيذ، قم بإرسال الكود الثانوي إلى Torii مع CLI. معرف التطوير الافتراضي مشتق من المفتاح العام في `defaults/client.toml`، دونك معرف الحساب هو
```
<i105-account-id>
```

استخدم ملف التكوين لتزويد عنوان URL Torii ومعرف السلسلة ومفتاح التوقيع:

```sh
iroha --config defaults/client.toml \
  transaction ivm \
  --path target/quickstart/hello.to
```

تقوم واجهة CLI بتشفير المعاملة باستخدام Norito، والعلامة باستخدام مفتاح التطوير وإرسالها أثناء عملية التنفيذ. قم بمراقبة السجلات Docker من أجل syscall `set_account_detail` أو قم بفرز CLI لتجزئة المعاملة الملتزم بها.

## 5. التحقق من تغيير الحالة

استخدم الملف التعريفي CLI لاستعادة تفاصيل الحساب التي تكتب بها العقد:

```sh
iroha --config defaults/client.toml \
  account meta get \
  --id <i105-account-id> \
  --key example | jq .
```

يجب عليك رؤية الحمولة JSON adosse إلى Norito :

```json
{
  "hello": "world"
}
```

إذا كانت القيمة غائبة، فتحقق من أن الخدمة Docker تؤلف جولة طوال الوقت وأن إشارة تجزئة المعاملة وفقًا لـ `iroha` تصل إلى الحالة `Committed`.

## Etapes suivantes- استكشف [معرض الأمثلة](./examples/) الذي يتم إنشاؤه تلقائيًا لعرضه
  التعليق على المقتطفات Kotodama بالإضافة إلى التقدم يتم تعيينه إلى مكالمات النظام Norito.
- اقرأ [الدليل Norito البدء](./getting-started) للشرح
  بالإضافة إلى التعرف على أدوات التجميع/المشغل، ونشر البيانات والبيانات التعريفية IVM.
- عند الرجوع إلى العقود الخاصة بك، استخدم `npm run sync-norito-snippets` في
  مساحة العمل لإعادة إنشاء المقتطفات القابلة للتحميل حتى تبقى مستندات الباب والعناصر المصطنعة
  يتزامن مع المصادر مع `crates/ivm/docs/examples/`.