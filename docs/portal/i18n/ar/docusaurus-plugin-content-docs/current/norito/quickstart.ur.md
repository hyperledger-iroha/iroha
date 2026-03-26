---
lang: ar
direction: rtl
source: docs/portal/docs/norito/quickstart.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
العنوان: Norito کوئک ستارٹ
الوصف: ريليز تولنج و سيلفر سنجل باير تعمل على Kotodama لشبكات الإنترنت وويليام كريت والنشر.
سبيكة: /norito/quickstart
---

هذه الإرشادات التفصيلية عبارة عن عمل رائع وهي بمثابة بطاقة متوقعة تنبأ بها ويلبرز على Norito وKotodama في الوقت المناسب: ها تم إنشاء برنامج سنجل-بير الجديد للكريكيت، وهو عبارة عن لعبة تشغيل جاف، وهو ريفرنس CLI الذي تم إصداره Torii .

مثال على الاتصال بالمفتاح/القيمة يمكّنك من استخدام `iroha_cli` وهو تأثير جانبي ثابت على الكمبيوتر.

## پیشگی تتطلبے

- [Docker](https://docs.docker.com/engine/install/) جس ميں Compose V2 فعال (اسے `defaults/docker-compose.single.yml` هو عبارة عن نموذج نظير لبدء الاستخدام).
- سلسلة أدوات الصدأ (1.76+) هي ثنائيات مساعدة متاحة للجميع إذا كانت الثنائيات شائعة الاستخدام.
- `koto_compile` و`ivm_run` و`iroha_cli` الثنائيات. يمكنك أيضًا إجراء عملية الخروج من مساحة العمل من خلال مطابقة عناصر الإصدار أو تنزيلها:

```sh
cargo install --locked --path crates/ivm --bin koto_compile --bin ivm_run
cargo install --locked --path crates/iroha_cli --bin iroha
```

> تم حفظ الثنائيات السابقة في مساحة العمل بالكامل.
> لا يوجد رقم `serde`/`serde_json`؛ برامج الترميز Norito شاملة ومفيدة.

## 1. بداية عمل سنجل-بير ديف الجديدةيتضمن الإصدار `kagami swarm` النسخة Docker Compose Bundle (`defaults/docker-compose.single.yml`). تم إرجاع كل هذه الجينات وتكوين العميل وتحقيقات الصحة إلى Torii `http://127.0.0.1:8080`.

```sh
docker compose -f defaults/docker-compose.single.yml up --build
```

شبكة شيلتا دي (المقدمة أو منفصلة). بعد كل مكالمات CLI `defaults/client.toml`، تم تحديث نظيراتها من النساء.

##2

مثال على عمل تجاري خارجي والحد الأدنى من Kotodama مثال محفوظ:

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

> Kotodama تم تحسين التحكم في الإصدار بشكل أكبر. مثال على الملف المستضاف [معرض أمثلة Norito](./examples/) يمكن تنزيله إذا كان هناك المزيد من النقاط المهمة.

## 3.IVM متحرك وجاف

الكود الثانوي IVM/Norito (`.to`) هو محرك أقراص محمول وهو يقوم بعمل ما هو جديد أول مكالمات نظام المضيف التي يتم تصديقها:

```sh
koto_compile target/quickstart/hello.ko \
  --abi 1 \
  --max-cycles 0 \
  -o target/quickstart/hello.to

ivm_run target/quickstart/hello.to --args '{}'
```

عداء `info("Hello from Kotodama")` لاغ پرنٹ كرتا ومضيف ساخر کے خلاف `SET_ACCOUNT_DETAIL` syscall انجام ديتا . إذا قمت باختراع `ivm_tool` الجهاز الثنائي أو إلى `ivm_tool inspect target/quickstart/hello.to` رأس ABI، وبتات الميزات ونقاط الدخول المُصدَّرة متاحة أيضًا.

## 4. Torii يتم تفعيل الرمز الثانوي

في ما يلي، تم إضافة رمز بايت محمول إلى CLI وهو Torii. تم استخدام هوية التطوير `defaults/client.toml` للمفتاح العام الموجود، وهو عبارة عن معرف الحساب:
```
soraカタカナ...
```Torii عنوان URL ومعرف السلسلة ومفتاح التوقيع اسم المستخدم للتكوين:

```sh
iroha --config defaults/client.toml \
  transaction ivm \
  --path target/quickstart/hello.to
```

CLI Norito هو عبارة عن تشفير كرتا ومفتاح تطوير يوقع كرتا ومفتاح تطوير نظير يرسل كرتا. `set_account_detail` syscall الذي يقوم Docker بتسجيل تجزئة المعاملات الملتزم بها أو لإخراج CLI بطريقة ما.

## 5. تغيير الحالة إلى حالة طوارئ

لم يتم تقديم هذا الدليل الأساسي لـ CLI وتفاصيل الحساب:

```sh
iroha --config defaults/client.toml \
  account meta get \
  --id soraカタカナ... \
  --key example | jq .
```

يبدو أن حمولة JSON المدعومة Norito مدعومة:

```json
{
  "hello": "world"
}
```

إذا لم يكن تطبيق Word متاحًا، فلن تتمكن من تتبع ملف Docker الذي يقوم بإنشاء ملف كامل و`iroha` الذي تم نشر تقرير به عبر الإنترنت. `Committed` تم الوصول إلى الحالة.

##اگلے المراحل

- معرض الصور الخاص بك [مثال للمعرض](./examples/)
  تم إنشاء العديد من مقتطفات Kotodama المتقدمة ومكالمات نظام Norito.
- مزيد من المعلومات [Norito دليل البدء](./getting-started)
  أدوات المترجم/العداء ونشر البيان وبيانات تعريف IVM واضحة المعالم.
- التكرار التكراري لمساحة العمل `npm run sync-norito-snippets`
  المقتطفات القابلة للتنزيل هي نسخ مكررة من المستندات والمستندات والعناصر، `crates/ivm/docs/examples/` تلك المصادر التي تمت مزامنتها تلقائيًا.