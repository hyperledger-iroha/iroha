---
lang: ar
direction: rtl
source: docs/portal/docs/norito/quickstart.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
العنوان: البداية السريعة Norito
الوصف: تحقق من العقد Kotodama واختبره وأكمله باستخدام الأدوات اليدوية ومجموعة فردية تلقائية.
سبيكة: /norito/quickstart
---

توضح هذه الإرشادات العملية التي نتبعها للمبرمجين في أول اتصال باستخدام Norito وKotodama: تحديد مجموعة واحدة، وتجميع العقد، وتنفيذ التشغيل الجاف المحلي، ثم تحقيق الذات عن طريق Torii مع CLI المسطح.

يتم كتابة العقد المبدئي بفاصل زمني/خصم في حساب محدد بحيث يمكنك التحقق بسرعة من التأثير الجانبي باستخدام помощью `iroha_cli`.

## تريبوفانيا

- [Docker](https://docs.docker.com/engine/install/) مع Compose V2 المتضمن (يُستخدم لبدء عينة النظير، المتوافق مع `defaults/docker-compose.single.yml`).
- سلسلة أدوات الصدأ (1.76+) لجميع البينارنيكيين المجهزين، إذا لم تقم بتنزيل المنشورات العامة.
- البينارنيك `koto_compile`، `ivm_run` و`iroha_cli`. يمكنك البحث عن مساحة عمل الخروج، مثل عرض عناصر الإصدار الرائعة أو تنزيلها:

```sh
cargo install --locked --path crates/ivm --bin koto_compile --bin ivm_run
cargo install --locked --path crates/iroha_cli --bin iroha
```

> يتم إنشاء هذه البرامج التعليمية بشكل آمن باستخدام مساحة العمل الأصلية.
> لا يرتبط هذا أبدًا بـ `serde`/`serde_json`; يتم تنفيذ التعليمات البرمجية Norito من النهاية إلى النهاية.

## 1. قم بتثبيت مجموعة التطوير المجانيةفي المستودعات، يوجد Docker Compose Bundle، generirovанный `kagami swarm` (`defaults/docker-compose.single.yml`). يتم تضمين التكوين التلقائي وتكوين العملاء وتحقيقات الصحة، حيث تم توفير Torii إلى `http://127.0.0.1:8080`.

```sh
docker compose -f defaults/docker-compose.single.yml up --build
```

قم بتثبيت الحاوية المخزنة (في الهاتف أو في المقدمة). سيتم إنشاء جميع إصدارات CLI التالية من هذا النظير عبر `defaults/client.toml`.

## 2. سجل العقد

قم بالمشاركة في مدير العمل واحفظ الحد الأدنى من التمهيدي Kotodama:

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

> اقتراح القرص الصلب Kotodama في إصدار التحكم بالنظام. الأمثلة، توسيع البوابة، وكذلك التنزيل في [عرض الأمثلة الأولية Norito](./examples/)، إذا تم شراء المزيد العمل الأولي.

## 3. التجميع والتشغيل الجاف باستخدام IVM

قم بتجميع العقد في رمز البيتكود IVM/Norito (`.to`) وقم بتثبيته محليًا لتتمكن من الاستماع إليه تستمر syscalls في معالجة المجموعات:

```sh
koto_compile target/quickstart/hello.ko \
  --abi 1 \
  --max-cycles 0 \
  -o target/quickstart/hello.to

ivm_run target/quickstart/hello.to --args '{}'
```

يقوم العداء بإخراج السجل `info("Hello from Kotodama")` واستخدام syscall `SET_ACCOUNT_DETAIL` لحماية مضيف التصميم. إذا تم توفير البرنامج التعليمي الاختياري `ivm_tool`، فإن الأمر `ivm_tool inspect target/quickstart/hello.to` يعرض رأس ABI، وبتات الميزات، ونقاط الدخول المصدرة.

## 4. استخدم رمز البيتكود من خلال Toriiلكي تتمكن من العمل، قم بتجميع رمز البيتكود في Torii عبر CLI. يتم الاتصال بهوية التطوير الافتراضية من خلال المفتاح العام في `defaults/client.toml`، حساب المعرف التالي:
```
<katakana-i105-account-id>
```

استخدم ملف التكوين لإضافة عنوان URL Torii ومعرف السلسلة وصفحة المفتاح:

```sh
iroha --config defaults/client.toml \
  transaction ivm \
  --path target/quickstart/hello.to
```

تقوم CLI بترميز المعاملات Norito، وتدعمها وتنفذها من خلال نظير عامل. خطوات تسجيل الدخول Docker لـ syscall `set_account_detail` أو مراقبة واجهة CLI للمعاملات الملتزم بها.

## 5. التحقق من التحسين

استخدم الملف التعريفي لـ CLI لتتمكن من الحصول على تفاصيل الحساب التي تحمل العقد:

```sh
iroha --config defaults/client.toml \
  account meta get \
  --id <katakana-i105-account-id> \
  --key example | jq .
```

يجب عليك رؤية حمولة JSON على القاعدة Norito:

```json
{
  "hello": "world"
}
```

إذا لاحظت أولاً، تأكد من أن الخدمة Docker تقوم بتأليف كل شيء وما هي معاملاتها التي يتم إجراؤها `iroha`, مواد التوصيل `Committed`.

## الخطوات التالية- تعلم هندسة السيارات [معرض الأمثلة الأولية](./examples/)، لتشاهد،
  كيف يتم تعيين المزيد من المقتطفات Kotodama إلى syscalls Norito.
- اقرأ [Norito دليل البدء](./getting-started) لمزيد من المحتوى
  أدوات التجميع/المجمع وبيانات النشر والتحويلات IVM.
- عند العمل على العقود الخاصة بك، استخدم `npm run sync-norito-snippets` в
  مساحة العمل، لتتمكن من إعادة إنشاء المقتطفات وتحميل مدخل المستندات والعناصر
  المزامنة مع المزامنة في `crates/ivm/docs/examples/`.