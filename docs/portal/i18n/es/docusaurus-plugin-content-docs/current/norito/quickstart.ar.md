---
lang: es
direction: ltr
source: docs/portal/docs/norito/quickstart.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
título: بدء سريع ل Norito
descripción: ابن وتحقق وانشر عقد Kotodama باستخدام ادوات الاصدار والشبكة الافتراضية ذات العقدة الواحدة.
babosa: /norito/inicio rápido
---

يعكس هذا الدليل العملي سير العمل الذي نتوقع ان يتبعه المطورون عند تعلم Norito and Kotodama لاول مرة: تشغيل Para realizar un funcionamiento en seco, realice un funcionamiento en seco con el cable CLI Torii.

يكتب عقد المثال زوج مفتاح/قيمة في حساب المستدعي حتى تتمكن من التحقق من الاثر الجانبي مباشرة عبر `iroha_cli`.

## المتطلبات

- [Docker](https://docs.docker.com/engine/install/) para Compose V2 (el mismo par العينة المحدد في `defaults/docker-compose.single.yml`).
- Aplicación Rust (1.76+) para que los usuarios puedan acceder a ellos.
- Tarjetas `koto_compile`, `ivm_run` y `iroha_cli`. يمكنك بناؤها من checkout الـ workspace كما هو موضح ادناه او تنزيل artefactos الاصدار المطابقة:

```sh
cargo install --locked --path crates/ivm --bin koto_compile --bin ivm_run
cargo install --locked --path crates/iroha_cli --bin iroha
```

> الثنائيات اعلاه امنة للتثبيت بجانب بقية espacio de trabajo.
> لا ترتبط مطلقا بـ `serde`/`serde_json`؛ Utilice los códecs Norito del fabricante.

## 1. تشغيل شبكة تطوير بعقدة واحدة

Este paquete incluye Docker Compose y el paquete `kagami swarm` (`defaults/docker-compose.single.yml`). يقوم بربط genesis الافتراضي وتكوين العميل و health sondas بحيث يكون Torii متاحا على `http://127.0.0.1:8080`.

```sh
docker compose -f defaults/docker-compose.single.yml up --build
```اترك الحاوية تعمل (في المقدمة او مفصولة). Utilice la CLI para conectar el par con `defaults/client.toml`.

## 2. كتابة العقد

Aquí está el mensaje y el mensaje Kotodama:

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

## 3. Funcionamiento y funcionamiento en seco con IVM

Para obtener el código de bytes IVM/Norito (`.to`) se requiere una llamada al sistema para realizar llamadas al sistema. قبل لمس الشبكة:

```sh
koto_compile target/quickstart/hello.ko \
  --abi 1 \
  --max-cycles 0 \
  -o target/quickstart/hello.to

ivm_run target/quickstart/hello.to --args '{}'
```

El corredor es `info("Hello from Kotodama")` y syscall `SET_ACCOUNT_DETAIL` para su uso. Esta es la configuración `ivm_tool` que incluye `ivm_tool inspect target/quickstart/hello.to`, encabezado ABI, bits de características y puntos de entrada.

## 4. Código de bytes de عبر Torii

Para obtener el código de bytes Torii de la CLI. هوية التطوير الافتراضية مشتقة من المفتاح العام في `defaults/client.toml`, لذلك يكون معرّف الحساب:
```
soraカタカナ...
```

Utilice la URL de Torii y el ID de cadena para obtener el siguiente enlace:

```sh
iroha --config defaults/client.toml \
  transaction ivm \
  --path target/quickstart/hello.to
```

Esta CLI está conectada a Norito, y está conectada a un par. Seleccione Docker y syscall `set_account_detail` y abra la CLI para el hash del archivo.

## 5. التحقق من تغيير الحالة

استخدم نفس ملف تعريف CLI لجلب detalle de cuenta الذي كتبه العقد:

```sh
iroha --config defaults/client.toml \
  account meta get \
  --id soraカタカナ... \
  --key example | jq .
```Esta es la carga útil JSON contenida en Norito:

```json
{
  "hello": "world"
}
```

اذا كانت القيمة مفقودة, تاكد من ان خدمة Docker compose ما زالت تعمل and hash المعاملة الذي ابلغ عنه `iroha` Aquí está el código `Committed`.

## الخطوات التالية

- استكشف [معرض الامثلة](./examples/) المولد تلقائيا لرؤية
  Esta es la configuración Kotodama y la llamada al sistema Norito.
- اقرأ [دليل البدء Norito](./getting-started) للحصول على شرح اعمق
  لادوات المترجم/runner y نشر manifests وبيانات IVM الوصفية.
- عند التكرار على عقودك، استخدم `npm run sync-norito-snippets` في
  espacio de trabajo لاعادة توليد المقتطفات القابلة للتنزيل حتى تبقى وثائق البوابة والartefacts
  Utilice el código `crates/ivm/docs/examples/`.