---
lang: ar
direction: rtl
source: docs/portal/docs/norito/quickstart.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
العنوان: البدء السريع لـ Norito
description: ابن وتحقق وانشر عقد Kotodama باستخدام ادوات الاصدار والشبكة الافتراضية لاتحادة الاستماع.
سبيكة: /norito/quickstart
---

يعكس هذا الدليل تشغيلي سير العمل الذي لانتبعه المطورون عند تعلم Norito و Kotodama لاول مرة: شبكة حتمية بعقدة واحدة، تجميع عقد، إجراءات التشغيل الجاف المحلي، ثم إرساله عبر Torii باستخدام CLI المرجعي.

كاتب المثال زوج مفتاح/قيمة في حساب المستدعي حتى المباشرة من التحقق من الاثر باي مباشرة عبر `iroha_cli`.

##متطلبات

- [Docker](https://docs.docker.com/engine/install/) مع تفعيل Compose V2 (يستخدم لتشغيل نظير العين أداة في `defaults/docker-compose.single.yml`).
- سلسلة ادوات Rust (1.76+) لإنتاج المزيد من المساعدة اذا لم تتابع النشرة.
- ثنائيات `koto_compile` و`ivm_run` و`iroha_cli`. يمكنك الباخرة من الخروج الـ Workspace كما هو موضح ادناه او تنزيل artifacts الاصدار المطابقة:

```sh
cargo install --locked --path crates/ivm --bin koto_compile --bin ivm_run
cargo install --locked --path crates/iroha_cli --bin iroha
```

> ثنائيات أعلاه أمنية للتثبيت بجانب مساحة العمل المتبقية.
> لا ترتبط مطلقا بـ `serde`/`serde_json`؛ يتم فرض برامج الترميز Norito من البداية حتى النهاية.

## 1. تشغيل شبكة تطوير بعقدة واحدة

تتضمن حزمة المستودع من Docker Compose Generator بواسطة `kagami swarm` (`defaults/docker-compose.single.yml`). يقوم بربط الجينات الافتراضية وتكوين العملاء والمسبارات الصحية بحيث يكون Torii متاحا على `http://127.0.0.1:8080`.

```sh
docker compose -f defaults/docker-compose.single.yml up --build
```لتترك اللمس (في المشروع او مفصولة). جميع اوامر CLI التاليين هذا الـ Peer عبر `defaults/client.toml`.

## 2. كتابة المؤتمر

منشئ عمل نسخة واحفظ مثال Kotodama نموذج بسيط:

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

> يفضل حفظ المصادر Kotodama تحت التحكم بالنسخ. هناك أيضًا مثلة مستضافة على البوابة ضمن [معرض امثلة Norito](./examples/) اذا كنت تريد نقطة بداية اغنى.

## 3. التأسيس والتشغيل الجاف بـ IVM

قم بتجميع العقد الى bytecode IVM/Norito (`.to`) ثم ينفذه محليا للتأكد من نجاح syscalls للمضيف قبل لمس الشبكة:

```sh
koto_compile target/quickstart/hello.ko \
  --abi 1 \
  --max-cycles 0 \
  -o target/quickstart/hello.to

ivm_run target/quickstart/hello.to --args '{}'
```

يطبع سجل runner `info("Hello from Kotodama")` ويجري syscall `SET_ACCOUNT_DETAIL` على الإعدادات الوهمي. إذا كان ثنائي الاختياري `ivm_tool` متاحا، فان `ivm_tool inspect target/quickstart/hello.to` يعرض ABI header و feature bits و inputpoints المصدرة.

## 4. إرسال bytecode عبر Torii

مع العقدة، ارسل bytecode المترجم إلى Torii باستخدام CLI. هوية التطوير التكتيكية التكتيكية من المفتاح العام في `defaults/client.toml`، لذلك تم تعريف الحساب:
```
soraカタカナ...
```

استخدم هذا الحل لتحسين عنوان URL الخاص بـ Torii ومعرف السلسلة ومفتاح التوقيع:

```sh
iroha --config defaults/client.toml \
  transaction ivm \
  --path target/quickstart/hello.to
```

يقوم CLI بترميز مخصص عبر Norito، وتوقيعها بالمفتاح التطويري، وارسالها الى الـ Peer Worker. راقب سجلات Docker لـ syscall `set_account_detail` أو تابع تحرير CLI لتجزئة التجزئة الملتزمة.

## 5. التحقق من تغيير الحالة

استخدم نفس ملف تعريف CLI لجلب تفاصيل الحساب الذي كتبه العقد:

```sh
iroha --config defaults/client.toml \
  account meta get \
  --id soraカタカナ... \
  --key example | jq .
```يجب أن ترى الحمولة JSON المؤهلة على Norito:

```json
{
  "hello": "world"
}
```

اذا كانت القيمة مفقودة، متأكد من ان خدمة Docker قم بتكوين ما تعمل hash مكيف الذي بلغه `iroha` وصل الى حالة `Committed`.

##الخطوات التالية

- مهاجمة [معرض الامثلة](./examples/) المولد المستقل
  كيف تتطابق مقتطفات Kotodama الاكثر تقدما مع syscalls Norito.
- اقرأ [دليل المبتدئين Norito](./getting-started) للحصول على شرح العمق
  لادوات المترجم/عداء ونشر البيانات IVM الوصفية.
- عند التكرار على مدى عقودك، استخدم `npm run sync-norito-snippets` في
  مساحة العمل لاعادة توليد مقتطفات تعليمية للتنزيل حتى تستمر وثائق البوابة والمصنوعات اليدوية
  متزامنة مع المصدر تحت `crates/ivm/docs/examples/`.