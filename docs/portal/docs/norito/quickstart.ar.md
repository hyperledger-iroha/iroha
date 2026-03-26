---
lang: ar
direction: rtl
source: docs/portal/docs/norito/quickstart.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 7c9a8e27bb8f586eac6bf4925cc69357dfa6d3f94c3dcf9e032b916c27fadf21
source_last_modified: "2025-11-07T12:25:50.867928+00:00"
translation_last_reviewed: 2025-12-30
---

---
title: بدء سريع ل Norito
description: ابن وتحقق وانشر عقد Kotodama باستخدام ادوات الاصدار والشبكة الافتراضية ذات العقدة الواحدة.
slug: /norito/quickstart
---

يعكس هذا الدليل العملي سير العمل الذي نتوقع ان يتبعه المطورون عند تعلم Norito و Kotodama لاول مرة: تشغيل شبكة حتمية بعقدة واحدة، تجميع عقد، اجراء dry-run محلي، ثم ارساله عبر Torii باستخدام CLI المرجعي.

يكتب عقد المثال زوج مفتاح/قيمة في حساب المستدعي حتى تتمكن من التحقق من الاثر الجانبي مباشرة عبر `iroha_cli`.

## المتطلبات

- [Docker](https://docs.docker.com/engine/install/) مع تفعيل Compose V2 (يستخدم لتشغيل peer العينة المحدد في `defaults/docker-compose.single.yml`).
- سلسلة ادوات Rust (1.76+) لبناء الثنائيات المساعدة اذا لم تقم بتنزيل المنشورة.
- الثنائيات `koto_compile` و`ivm_run` و`iroha_cli`. يمكنك بناؤها من checkout الـ workspace كما هو موضح ادناه او تنزيل artifacts الاصدار المطابقة:

```sh
cargo install --locked --path crates/ivm --bin koto_compile --bin ivm_run
cargo install --locked --path crates/iroha_cli --bin iroha
```

> الثنائيات اعلاه امنة للتثبيت بجانب بقية workspace.
> لا ترتبط مطلقا بـ `serde`/`serde_json`؛ يتم فرض Norito codecs من البداية الى النهاية.

## 1. تشغيل شبكة تطوير بعقدة واحدة

يتضمن المستودع bundle من Docker Compose مولد بواسطة `kagami swarm` (`defaults/docker-compose.single.yml`). يقوم بربط genesis الافتراضي وتكوين العميل و health probes بحيث يكون Torii متاحا على `http://127.0.0.1:8080`.

```sh
docker compose -f defaults/docker-compose.single.yml up --build
```

اترك الحاوية تعمل (في المقدمة او مفصولة). جميع اوامر CLI اللاحقة تستهدف هذا الـ peer عبر `defaults/client.toml`.

## 2. كتابة العقد

انشئ مجلد عمل واحفظ مثال Kotodama البسيط:

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

> يفضل حفظ مصادر Kotodama تحت التحكم بالنسخ. توجد ايضا امثلة مستضافة على البوابة ضمن [معرض امثلة Norito](./examples/) اذا كنت تريد نقطة بداية اغنى.

## 3. التجميع و dry-run مع IVM

قم بتجميع العقد الى bytecode IVM/Norito (`.to`) ثم نفذه محليا للتأكد من نجاح syscalls للمضيف قبل لمس الشبكة:

```sh
koto_compile target/quickstart/hello.ko \
  --abi 1 \
  --max-cycles 0 \
  -o target/quickstart/hello.to

ivm_run target/quickstart/hello.to --args '{}'
```

يطبع runner سجل `info("Hello from Kotodama")` ويجري syscall `SET_ACCOUNT_DETAIL` على المضيف الوهمي. اذا كان الثنائي الاختياري `ivm_tool` متاحا، فان `ivm_tool inspect target/quickstart/hello.to` يعرض ABI header و feature bits و entrypoints المصدرة.

## 4. ارسال bytecode عبر Torii

مع استمرار تشغيل العقدة، ارسل bytecode المترجم الى Torii باستخدام CLI. هوية التطوير الافتراضية مشتقة من المفتاح العام في `defaults/client.toml`، لذلك يكون معرّف الحساب:
```
<katakana-i105-account-id>
```

استخدم ملف التكوين لتوفير URL الخاص ب Torii و chain ID ومفتاح التوقيع:

```sh
iroha --config defaults/client.toml \
  transaction ivm \
  --path target/quickstart/hello.to
```

يقوم CLI بترميز المعاملة عبر Norito، وتوقيعها بالمفتاح التطويري، وارسالها الى الـ peer العامل. راقب سجلات Docker ل syscall `set_account_detail` او تابع خرج CLI ل hash المعاملة الملتزمة.

## 5. التحقق من تغيير الحالة

استخدم نفس ملف تعريف CLI لجلب account detail الذي كتبه العقد:

```sh
iroha --config defaults/client.toml \
  account meta get \
  --id <katakana-i105-account-id> \
  --key example | jq .
```

يجب ان ترى payload JSON المعتمد على Norito:

```json
{
  "hello": "world"
}
```

اذا كانت القيمة مفقودة، تاكد من ان خدمة Docker compose ما زالت تعمل وان hash المعاملة الذي ابلغ عنه `iroha` وصل الى حالة `Committed`.

## الخطوات التالية

- استكشف [معرض الامثلة](./examples/) المولد تلقائيا لرؤية
  كيف تتطابق مقتطفات Kotodama الاكثر تقدما مع syscalls Norito.
- اقرأ [دليل البدء Norito](./getting-started) للحصول على شرح اعمق
  لادوات المترجم/runner ونشر manifests وبيانات IVM الوصفية.
- عند التكرار على عقودك، استخدم `npm run sync-norito-snippets` في
  workspace لاعادة توليد المقتطفات القابلة للتنزيل حتى تبقى وثائق البوابة والartefacts
  متزامنة مع المصادر تحت `crates/ivm/docs/examples/`.
