---
lang: pt
direction: ltr
source: docs/portal/docs/norito/quickstart.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
título: بدء سريع para Norito
description: Você pode usar o Kotodama para obter mais informações e informações الواحدة.
slug: /norito/início rápido
---

Você pode usar o método Norito e Kotodama para obter mais informações. Exemplo: تشغيل شبكة حتمية بعقدة واحدة, تجميع عقد, اجراء dry-run محلي, ثم ارساله عبر Torii é compatível com CLI.

Faça o download do seu cartão/filme no site da empresa para obter informações mais detalhadas sobre o produto مباشرة عبر `iroha_cli`.

## المتطلبات

- [Docker](https://docs.docker.com/engine/install/) no Compose V2 (é o peer do peer no `defaults/docker-compose.single.yml`).
- سلسلة ادوات Rust (1.76+) لبناء الثنائيات المساعدة اذا لم تقم بتنزيل المنشورة.
- Números `koto_compile` e `ivm_run` e `iroha_cli`. Você pode verificar o espaço de trabalho do checkout e criar artefatos no local:

```sh
cargo install --locked --path crates/ivm --bin koto_compile --bin ivm_run
cargo install --locked --path crates/iroha_cli --bin iroha
```

> الثنائيات اعلاه امنة للتثبيت بجانب بقية espaço de trabalho.
> لا ترتبط مطلقا بـ `serde`/`serde_json`؛ Eu tenho os codecs Norito do site.

## 1. تشغيل شبكة تطوير بعقدة e

O pacote do pacote Docker Compose é o `kagami swarm` (`defaults/docker-compose.single.yml`). يقوم بربط genesis الافتراضي وتكوين العميل e health probes بحيث يكون Torii متاحا على `http://127.0.0.1:8080`.

```sh
docker compose -f defaults/docker-compose.single.yml up --build
```

اترك الحاوية تعمل (في المقدمة او مفصولة). Verifique se o CLI está conectado ao peer através do `defaults/client.toml`.

## 2. كتابة العقد

A configuração do modelo Kotodama é:

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

> Verifique se o Kotodama está danificado. Você pode usar o recurso de configuração [Norito](./examples/) para obter mais informações Não, não.

## 3. Teste e simulação com IVM

Use o bytecode IVM/Norito (`.to`) para obter syscalls Para obter mais informações:

```sh
koto_compile target/quickstart/hello.ko \
  --abi 1 \
  --max-cycles 0 \
  -o target/quickstart/hello.to

ivm_run target/quickstart/hello.to --args '{}'
```

O executor é `info("Hello from Kotodama")` e o syscall `SET_ACCOUNT_DETAIL` é a solução. No caso do `ivm_tool`, o `ivm_tool inspect target/quickstart/hello.to` contém cabeçalho ABI, bits de recurso e pontos de entrada.

## 4. Transfira o bytecode para Torii

Para configurar o bytecode Torii, use CLI. هوية التطوير الافتراضية مشتقة من المفتاح العام في `defaults/client.toml`, لذلك يكون معرّف الحساب:
```
ih58...
```

Verifique o URL do site como Torii e o ID da cadeia e o ID da cadeia:

```sh
iroha --config defaults/client.toml \
  transaction ivm \
  --path target/quickstart/hello.to
```

A CLI usa o Norito e o peer é definido como peer. Execute Docker para syscall `set_account_detail` e execute o CLI para hash do sistema.

## 5. التحقق من تغيير الحالة

استخدم نفس ملف تعريف CLI para detalhes da conta:

```sh
iroha --config defaults/client.toml \
  account meta get \
  --id ih58... \
  --key example | jq .
```

A carga útil JSON é definida como Norito:

```json
{
  "hello": "world"
}
```

اذا كانت القيمة مفقودة, تاكد من ان خدمة Docker compose ما زالت تعمل وان hash المعاملة الذي ابلغ O `iroha` é o `Committed`.

## الخطوات التالية- استكشف [معرض الامثلة](./examples/) المولد تلقائيا لرؤية
  Você pode usar Kotodama para usar syscalls Norito.
- اقرأ [دليل البدء Norito](./getting-started) للحصول على شرح اعمق
  O arquivo /runner e manifestos é IVM.
- Você pode usar o `npm run sync-norito-snippets` para usar o `npm run sync-norito-snippets`
  espaço de trabalho
  A solução de problemas é `crates/ivm/docs/examples/`.