---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/quickstart.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# O código é SoraFS

Você pode usar o chunker do SF-1,
A solução de problemas de segurança do computador é SoraFS.
وازنه مع [التعمّق في خط أنابيب المانيفست](manifest-pipeline.md)
Deixe-os entrar e sair de casa.

## المتطلبات الأساسية

- A ferrugem (`rustup update`) não pode ser removida.
- Acesso: [Ed25519 usando OpenSSL](https://github.com/hyperledger-iroha/iroha/tree/master/defaults/dev-keys#readme)
  لتوقيع المانيفستات.
- Configuração: Node.js ≥ 18 é necessário que o arquivo Docusaurus seja instalado.

Use `export RUST_LOG=info` para configurar a interface CLI.

## 1. تحديث الـ fixtures الحتمية

Você pode usar o método de fragmentação (chunking) do SF-1. كما يُصدر الأمر مظاريف
مانيفست موقعة عند تزويد `--signing-key`; Instalação `--allow-unsigned`
Não.

```bash
cargo run -p sorafs_chunker --bin export_vectors -- --allow-unsigned
```

Informações:

-`fixtures/sorafs_chunker/sf1_profile_v1.{json,rs,ts,go}`
-`fixtures/sorafs_chunker/manifest_blake3.json`
- `fixtures/sorafs_chunker/manifest_signatures.json` (não disponível)
-`fuzz/sorafs_chunker/sf1_profile_v1_{input,backpressure}.json`

## 2. Carga útil e carga útil

A configuração `sorafs_chunker` é feita de forma simples e rápida:

```bash
echo "SoraFS deterministic chunking" > /tmp/docs.txt
cargo run -p sorafs_chunker --bin sorafs-chunk-dump -- /tmp/docs.txt \
  > /tmp/docs.chunk-plan.json
```

Descrição:

- `profile` / `break_mask` – `sorafs.sf1@1.0.0`.
- `chunks[]` – Coloque um pedaço de papel e BLAKE3 em pedaços.

Os fixtures são usados para fornecer suporte ao proptest.
Veja mais:

```bash
cargo test -p sorafs_chunker streaming_backpressure_fuzz_matches_batch
```

## 3. ابنِ ووقّع مانيفست

Faça pedaços de pedaços e وتواقيع الحوكمة em qualquer lugar
`sorafs-manifest-stub`. يوضح الأمر أدناه payload لملف واحد؛ مرّر مسار دليل لحزم
شجرة (تسير CLI ترتيبًا معجميًا).

```bash
cargo run -p sorafs_manifest --bin sorafs-manifest-stub -- \
  /tmp/docs.txt \
  --chunker-profile=sorafs.sf1@1.0.0 \
  --manifest-out=/tmp/docs.manifest \
  --manifest-signatures-out=/tmp/docs.manifest_signatures.json \
  --json-out=/tmp/docs.report.json \
  --allow-unsigned
```

Modelo `/tmp/docs.report.json` da seguinte forma:

- `chunking.chunk_digest_sha3_256` – بصمة SHA3 للإزاحات/الأطوال, تطابق fixtures الخاصة
  É um pedaço.
- `manifest.manifest_blake3` – BLAKE3 بصمة الموقعة ضمن ظرف المانيفست.
- `chunk_fetch_specs[]` – تعليمات جلب مرتبة للأوركستراتورات.

Você pode usar o dispositivo `--signing-key` e `--signer`.
يتحقق الأمر من كل توقيع Ed25519 قبل كتابة الظرف.

## 4. حاكِ الاسترجاع متعدد المزوّدين

Use o CLI para criar pedaços de pedaços de dados, arquivos e arquivos.
Isso pode ser feito por meio do CI e do computador.

```bash
cargo run -p sorafs_car --bin sorafs_fetch -- \
  --plan=/tmp/docs.report.json \
  --provider=primary=/tmp/docs.txt \
  --output=/tmp/docs.reassembled \
  --json-out=/tmp/docs.fetch-report.json
```

Descrição:

- `payload_digest_hex` não está disponível.
- `provider_reports[]` تعرض أعداد النجاح/الفشل لكل مزود.
- O modelo `chunk_retry_total` está equipado com contrapressão.
- مرّر `--max-peers=<n>` لتقييد عدد المزوّدين المجدولين للتشغيل وإبقاء محاكاة CI مركّزة
  على المرشحين الأساسيين.
- `--retry-budget=<n>` يتجاوز العدد الافتراضي لمحاولات إعادة المحاولة لكل chunk (3)
  Certifique-se de que o produto esteja funcionando corretamente.

Use `--expect-payload-digest=<hex>` e `--expect-payload-len=<bytes>` para obter mais informações
A carga útil não está disponível.

## 5. Palavras-chave- **تكامل الحوكمة** – مرّر بصمة المانيفست و`manifest_signatures.json` إلى سير عمل المجلس
  Não há Pin Registry no site.
- **التفاوض مع السجل** – راجع [`sorafs/chunker_registry.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/chunker_registry.md)
  Você pode fazer isso com cuidado. ينبغي للأتمتة تفضيل المعالجات القياسية
  (`namespace.name@semver`) não funciona.
- **أتمتة CI** – أضف الأوامر أعلاه إلى خطوط إصدار النشر حتى تنشر المستندات والـ fixtures
  والآرتيفاكت مانيفستات حتمية جنبًا إلى جنب مع بيانات وصفية موقعة.