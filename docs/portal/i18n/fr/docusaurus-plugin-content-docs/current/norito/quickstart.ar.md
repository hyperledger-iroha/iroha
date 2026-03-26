---
lang: fr
direction: ltr
source: docs/portal/docs/norito/quickstart.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
titre : بدء سريع ل Norito
description: ابن وتحقق وانشر عقد Kotodama باستخدام ادوات الاصدار والشبكة الافتراضية ذات العقدة الواحدة.
limace : /norito/quickstart
---

يعكس هذا الدليل العملي سير العمل الذي نتوقع ان يتبعه المطورون عند تعلم Norito et Kotodama: تشغيل شبكة حتمية بعقدة واحدة، تجميع عقد، اجراء dry-run محلي، ثم ارساله عبر Torii باستخدام CLI المرجعي.

يكتب عقد المثال زوج مفتاح/قيمة في حساب المستدعي حتى تتمكن من التحقق من الاثر الجانبي مباشرة عبر `iroha_cli`.

## المتطلبات

- [Docker](https://docs.docker.com/engine/install/) avec Compose V2 (pour les homologues de `defaults/docker-compose.single.yml`).
- سلسلة ادوات Rust (1.76+) ثنائيات المساعدة اذا لم تقم بتنزيل المنشورة.
- Paramètres `koto_compile` et `ivm_run` et `iroha_cli`. Vous pouvez également utiliser la caisse pour l'espace de travail et créer des artefacts :

```sh
cargo install --locked --path crates/ivm --bin koto_compile --bin ivm_run
cargo install --locked --path crates/iroha_cli --bin iroha
```

> Les tâches à accomplir dans l'espace de travail.
> لا ترتبط مطلقا بـ `serde`/`serde_json`؛ Utilisez les codecs Norito pour votre appareil.

## 1. تشغيل شبكة تطوير بعقدة واحدة

Le bundle comprend Docker Compose et `kagami swarm` (`defaults/docker-compose.single.yml`). يقوم بربط genesis الافتراضي وتكوين العميل و health sondes بحيث يكون Torii متاحا على `http://127.0.0.1:8080`.

```sh
docker compose -f defaults/docker-compose.single.yml up --build
```اترك الحاوية تعمل (في المقدمة او مفصولة). Utilisez la CLI pour établir un lien entre les pairs et `defaults/client.toml`.

## 2. كتابة العقد

انشئ مجلد واحفظ مثال Kotodama البسيط:

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

## 3. Fonctionnement et fonctionnement à sec avec IVM

Vous pouvez utiliser le bytecode IVM/Norito (`.to`) pour utiliser les appels système. للمضيف قبل لمس الشبكة:

```sh
koto_compile target/quickstart/hello.ko \
  --abi 1 \
  --max-cycles 0 \
  -o target/quickstart/hello.to

ivm_run target/quickstart/hello.to --args '{}'
```

يطبع runner سجل `info("Hello from Kotodama")` et syscall `SET_ACCOUNT_DETAIL` على المضيف الوهمي. Il s'agit de l'en-tête ABI, des bits de fonctionnalité et des points d'entrée.

## 4. Utiliser le bytecode Torii

Vous pouvez utiliser le bytecode Torii pour la CLI. Il s'agit d'un lien vers l'application `defaults/client.toml`, qui correspond à l'article suivant :
```
<i105-account-id>
```

Utilisez l'URL de votre URL vers Torii et l'ID de chaîne pour votre recherche :

```sh
iroha --config defaults/client.toml \
  transaction ivm \
  --path target/quickstart/hello.to
```

يقوم CLI بترميز المعاملة وارسالها الى الـ peer العامل. Utilisez Docker pour l'appel système `set_account_detail` et utilisez la CLI pour le hachage.

## 5. التحقق من تغيير الحالة

استخدم نفس ملف تعريف CLI لجلب détails du compte الذي كتبه العقد :

```sh
iroha --config defaults/client.toml \
  account meta get \
  --id <i105-account-id> \
  --key example | jq .
```J'utilise la charge utile JSON pour Norito :

```json
{
  "hello": "world"
}
```

Vous pouvez utiliser la méthode de hachage Docker pour composer un hachage et un hachage. `iroha` et `Committed`.

## الخطوات التالية

- استكشف [معرض الامثلة](./examples/) المولد تلقائيا لرؤية
  Vous pouvez utiliser les appels système Kotodama pour les appels système Norito.
- اقرأ [دليل البدء Norito](./getting-started) للحصول على شرح اعمق
  لادوات المترجم/runner ونشر manifeste وبيانات IVM الوصفية.
- عند التكرار على عقودك، استخدم `npm run sync-norito-snippets` في
  espace de travail لاعادة توليد المقتطفات القابلة للتنزيل حتى تبقى وثائق البوابة والartefacts
  متزامنة مع المصادر تحت `crates/ivm/docs/examples/`.