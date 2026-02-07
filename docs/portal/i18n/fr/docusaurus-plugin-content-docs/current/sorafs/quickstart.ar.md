---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/quickstart.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# البدء السريع في SoraFS

Il s'agit d'un morceau de chunker SF-1,
وتوقيع المانيفست، ومسار الجلب متعدد المزوّدين الذي يدعم خط أنابيب تخزين SoraFS.
وازنه مع [التعمّق في خط أنابيب المانيفست](manifest-pipeline.md)
للحصول على ملاحظات التصميم ومرجع أعلام سطر الأوامر.

## المتطلبات الأساسية

- Utilisez Rust (`rustup update`) pour votre ordinateur.
- Version : [زوج مفاتيح Ed25519 by OpenSSL](https://github.com/hyperledger-iroha/iroha/tree/master/defaults/dev-keys#readme)
  لتوقيع المانيفستات.
- Version : Node.js ≥ 18 est la version Docusaurus.

Utilisez `export RUST_LOG=info` pour utiliser la CLI.

## 1. تحديث الـ luminaires الحتمية

Il s'agit d'une méthode de fragmentation (chunking) pour SF-1. كما يُصدر الأمر مظاريف
مانيفست موقعة عند تزويد `--signing-key`؛ استخدم `--allow-unsigned` أثناء التطوير
المحلي فقط.

```bash
cargo run -p sorafs_chunker --bin export_vectors -- --allow-unsigned
```

المخرجات:

-`fixtures/sorafs_chunker/sf1_profile_v1.{json,rs,ts,go}`
-`fixtures/sorafs_chunker/manifest_blake3.json`
- `fixtures/sorafs_chunker/manifest_signatures.json` (إذا تم التوقيع)
-`fuzz/sorafs_chunker/sf1_profile_v1_{input,backpressure}.json`

## 2. La charge utile et la charge utile

استخدم `sorafs_chunker` لتقسيم ملف أو أرشيف عشوائي:

```bash
echo "SoraFS deterministic chunking" > /tmp/docs.txt
cargo run -p sorafs_chunker --bin sorafs-chunk-dump -- /tmp/docs.txt \
  > /tmp/docs.chunk-plan.json
```

الحقول الأساسية:

- `profile` / `break_mask` – يؤكد معاملات `sorafs.sf1@1.0.0`.
- `chunks[]` – Fichiers et fichiers BLAKE3 pour les morceaux.

لـ calendriers الأكبر، شغّل اختبار الانحدار المبني على proptest لضمان تزامن التقسيم
بالتدفق وبالدفعات:

```bash
cargo test -p sorafs_chunker streaming_backpressure_fuzz_matches_batch
```

## 3. ابنِ وووقّع مانيفست

لفّ خطة الـ chunks والكنى وتواقيع الحوكمة في مانيفست باستخدام
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

راجع `/tmp/docs.report.json` pour:

- `chunking.chunk_digest_sha3_256` – Pour SHA3 pour les appareils/الأطوال، تطابق luminaires
  Je suis un chunker.
- `manifest.manifest_blake3` – Mettez BLAKE3 en ligne avec votre appareil.
- `chunk_fetch_specs[]` – تعليمات جلب مرتبة للأوركستراتورات.

Vous devez utiliser le lien `--signing-key` et `--signer`.
يتحقق الأمر من كل توقيع Ed25519 قبل كتابة الظرف.

## 4. حاكِ الاسترجاع متعدد المزوّدين

Utilisez la CLI pour ajouter des morceaux à vos morceaux.
Il s'agit d'une solution pour CI et CI.

```bash
cargo run -p sorafs_car --bin sorafs_fetch -- \
  --plan=/tmp/docs.report.json \
  --provider=primary=/tmp/docs.txt \
  --output=/tmp/docs.reassembled \
  --json-out=/tmp/docs.fetch-report.json
```

التحققات:

- `payload_digest_hex` يجب أن يطابق تقرير المانيفست.
- `provider_reports[]` تعرض أعداد النجاح/الفشل لكل مزود.
- Le `chunk_retry_total` est un système de contre-pression.
- مرّر `--max-peers=<n>` لتقييد عدد المزوّدين المجدولين للتشغيل وإبقاء محاكاة CI مركّزة
  على المرشحين الأساسيين.
- `--retry-budget=<n>` يتجاوز العدد الافتراضي لمحاولات إعادة المحاولة لكل chunk (3)
  لتسريع كشف تراجعات الأوركستراتور عند حقن الأعطال.

Pour `--expect-payload-digest=<hex>` et `--expect-payload-len=<bytes>` pour
La charge utile est également disponible.

## 5. خطوات التالية- **تكامل الحوكمة** – مرّر بصمة المانيفست و`manifest_signatures.json` إلى سير عمل المجلس
  لكي يتمكن Pin Registry من إعلان التوافر.
- **التفاوض مع السجل** – راجع [`sorafs/chunker_registry.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/chunker_registry.md)
  قبل تسجيل ملفات تعريف جديدة. ينبغي للأتمتة تفضيل المعالجات القياسية
  (`namespace.name@semver`) على المعرفات الرقمية.
- **أتمتة CI** – أضف الأوامر أعلاه إلى خطوط إصدار النشر حتى تنشر المستندات والـ luminaires
  والآرتيفاكت مانيفستات حتمية جنبًا إلى جنب مع بيانات وصفية موقعة.