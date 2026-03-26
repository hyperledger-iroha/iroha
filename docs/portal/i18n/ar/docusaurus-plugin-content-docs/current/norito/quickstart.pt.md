---
lang: ar
direction: rtl
source: docs/portal/docs/norito/quickstart.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
العنوان: Inicio Rapido do Norito
الوصف: تم إنشاء عقد Kotodama وصلاحيته ونشره باستخدام أدوات الإصدار ومنصة إعادة التدوير من نظير فريد.
سبيكة: /norito/quickstart
---

هذه الخطوة هي عبارة عن تدفق يتطلع المطورون إلى تعلمه Norito و Kotodama من البداية: بدء حتمية جديدة من نظير واحد، وتجميع عقد، وتنفيذ التشغيل الجاف محليًا ثم إرساله عبر Torii مع سطر الأوامر المرجعي.

عقد نموذجي محسوب على أساس الشجاعة/القيمة في حساب الدعوة حتى تتمكن من التحقق من تأثير الضمان على الفور مع `iroha_cli`.

## المتطلبات المسبقة

- [Docker](https://docs.docker.com/engine/install/) com Compose V2 habilitado (يُستخدم لبدء أو نظير نموذج محدد في `defaults/docker-compose.single.yml`).
- Toolchain Rust (1.76+) لتجميع الثنائيات المساعدة دون حذف المنشورات.
- ثنائيات `koto_compile`، `ivm_run` و`iroha_cli`. يمكن تجميع العناصر من الخروج من مساحة العمل كما هو موضح أو يخفض عناصر الإصدار المقابلة:

```sh
cargo install --locked --path crates/ivm --bin koto_compile --bin ivm_run
cargo install --locked --path crates/iroha_cli --bin iroha
```

> الثنائيات هنا آمنة لتثبيتها مع بقية مساحة العمل.
> Eles nunca fazem link com `serde`/`serde_json`; برنامج الترميز Norito هو تطبيق من نقطة إلى نقطة.

## 1. ابدأ تطويرًا من نظير فريديشتمل المستودع على حزمة Docker قم بتكوينها بواسطة `kagami swarm` (`defaults/docker-compose.single.yml`). يتم توصيله بسجل التكوين وتكوين العميل وتحقيقات الصحة حتى يتمكن Torii من الوصول إلى `http://127.0.0.1:8080`.

```sh
docker compose -f defaults/docker-compose.single.yml up --build
```

Deixe o حاوية روداندو (سواء كانت أولية أو منفصلة). جميع المكالمات من CLI اللاحقة يمكن مقارنتها عبر `defaults/client.toml`.

## 2.اكتب العقد

قم بإنشاء دليل العمل وحفظه أو الحد الأدنى من نموذج Kotodama:

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

> قم باختيار الخطوط Kotodama من خلال التحكم بالعكس. تم توفير نماذج المستضافات على البوابة أيضًا في [معرض النماذج Norito](./examples/) إذا كنت تبحث عن نقطة من الجزء الأكبر من ريكو.

## 3. قم بتجميع التشغيل الجاف مع IVM

قم بتجميع عقد للرمز الثانوي IVM/Norito (`.to`) وتنفيذه محليًا للتأكد من أن مكالمات النظام تعمل قبل إعادة التشغيل:

```sh
koto_compile target/quickstart/hello.ko \
  --abi 1 \
  --max-cycles 0 \
  -o target/quickstart/hello.to

ivm_run target/quickstart/hello.to --args '{}'
```

يقوم المشغل بطباعة السجل `info("Hello from Kotodama")` وتنفيذ أو استدعاء النظام `SET_ACCOUNT_DETAIL` ضد المضيف المقلدة. إذا تم توفير خيار ثنائي اختياري `ivm_tool`، أو `ivm_tool inspect target/quickstart/hello.to` أو رأس ABI، فإن وحدات البت الخاصة بنظام التشغيل ونقاط الدخول المصدرة.

## 4. قم بإرسال الرمز الثانوي عبر Toriiبعقد اجتماع جديد، يمكنك إرسال الرمز الثانوي المترجم إلى Torii باستخدام CLI. معرف تطوير الموقع ومشتق من نشره على `defaults/client.toml`، يحمل أو معرف الحساب
```
<katakana-i105-account-id>
```

استخدم ملف التكوين لإنشاء عنوان URL لـ Torii أو معرف السلسلة ومفتاح الاختراق:

```sh
iroha --config defaults/client.toml \
  transaction ivm \
  --path target/quickstart/hello.to
```

يتم ترميز CLI للمعاملات باستخدام Norito، ويتم تنفيذها بمهمة التطوير وإرسالها إلى نظير التنفيذ. راقب سجلات Docker لـ syscall `set_account_detail` أو راقب رسالة CLI لتجزئة المعاملات الملتزم بها.

## 5. التحقق من حالة الوضع

استخدم نفس ملف CLI للبحث عن تفاصيل الحساب التي تعقدها:

```sh
iroha --config defaults/client.toml \
  account meta get \
  --id <katakana-i105-account-id> \
  --key example | jq .
```

تم تطوير إصدار الحمولة النافعة JSON المدعومة لـ Norito:

```json
{
  "hello": "world"
}
```

إذا كانت القيمة موجودة، فتأكد من أن الخدمة Docker تقوم بتكوين هذه الخدمة وأن تجزئة المعاملة التي تم الإبلاغ عنها بواسطة `iroha` قد انتقلت إلى الحالة `Committed`.

## بروكسيموس باسوس- استكشاف [معرض الأمثلة](./examples/) الذي يتم عرضه تلقائيًا للعرض
  مثل المقتطفات Kotodama الأكثر تقدمًا يتم تعيينها لـ syscalls Norito.
- Leia o [دليل البداية Norito](./getting-started) للشرح
  أكثر عمقًا في أدوات المترجم/المشغل، قم بنشر البيانات والبيانات التعريفية IVM.
- للرجوع إلى عقودنا، استخدم `npm run sync-norito-snippets` بدون مساحة عمل
  قم بإعادة إنشاء المقتطفات بشكل مخفي وإدارة المستندات إلى البوابة الإلكترونية والعناصر المتزامنة كخطوط في `crates/ivm/docs/examples/`.