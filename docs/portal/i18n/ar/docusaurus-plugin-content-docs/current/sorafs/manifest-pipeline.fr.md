---
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/manifest-pipeline.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# تقطيع SoraFS → خط أنابيب البيان

هذا يكمل عملية التشغيل السريع لتتبع خط أنابيب النوبة الذي يحول الثمانيات إلى كتلة كبيرة
تم تعديل البيانات Norito إلى Pin Registry لـ SoraFS. تم تعديل المحتوى
[`docs/source/sorafs/manifest_pipeline.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/manifest_pipeline.md)؛
راجع هذا المستند لمعرفة المواصفات القياسية وسجل التغيير.

## 1. طريقة تحديد القطعة

SoraFS يستخدم ملف التعريف SF-1 (`sorafs.sf1@1.0.0`): تجزئة مستوحاة من FastCDC مع
حد أدنى من القطعة يبلغ 64 كيلو بايت، وكابل بحجم 256 كيلو بايت، وحد أقصى يبلغ 512 كيلو بايت، وقناع
دي تمزق `0x0000ffff`. تم تسجيل ملف التعريف في `sorafs_manifest::chunker_registry`.

### مساعدين الصدأ

- `sorafs_car::CarBuildPlan::single_file` – Émet les offsets, longueurs and empreintes BLAKE3
  القطع المعلقة على تحضير الميتادونيه CAR.
- `sorafs_car::ChunkStore` - دفق الحمولات، والاستمرار في تحويل القطع واشتقاقها
  إثبات إمكانية الاسترجاع (PoR) يبلغ 64 كيلو بايت / 4 كيلو بايت.
- `sorafs_chunker::chunk_bytes_with_digests` – مساعد المكتبة في متابعة اثنين من CLIs.

### Outils CLI

```bash
cargo run -p sorafs_chunker --bin sorafs-chunk-dump -- ./payload.bin \
  > chunk-plan.json
```

يحتوي JSON على الإزاحات المنتظمة والأطوال وأنظمة القطع. احفظه
خطة عند بناء البيانات أو مواصفات الجلب للمنسق.

### تيموانس بوريعرض `ChunkStore` `--por-proof=<chunk>:<segment>:<leaf>` و`--por-sample=<count>` لمعرفة ذلك
المدققون الأقوياء يطالبون بمجموعات من Témoins Déterministes. Associez ces flags à
`--por-proof-out` أو `--por-sample-out` لتسجيل JSON.

## 2. المغلف هو بيان

`ManifestBuilder` يجمع بين الأجزاء المتداخلة مع أجزاء الإدارة :

- CID racine (dag-cbor) et engagements CAR.
- Preuves d'alias et déclarations de capacité des fournisseurs.
- توقيعات المشورة والخيارات الإضافية (على سبيل المثال، معرفات البناء).

```bash
cargo run -p sorafs_manifest --bin sorafs-manifest-stub -- \
  ./payload.bin \
  --chunker-profile=sorafs.sf1@1.0.0 \
  --manifest-out=payload.manifest \
  --manifest-signatures-out=payload.manifest_signatures.json \
  --json-out=payload.report.json
```

طلعات مهمة:

- `payload.manifest` – تم ترميز ثماني البيانات في Norito.
- `payload.report.json` – السيرة الذاتية مقبولة للبشر/الأتمتة، بما في ذلك
  `chunk_fetch_specs`، `payload_digest_hex`، يقوم بتشغيل CAR واستبدال الأسماء المستعارة.
- `payload.manifest_signatures.json` – مغلف يحتوي على l'empreinte BLAKE3 dumanife،
  l'empreinte SHA3 du Plan de Chunks et des Signatures Ed25519 triées.

استخدم `--manifest-signatures-in` للتحقق من المغلفات الأربع من خلال التوقيعات
الخارجية قبل التسجيل، و`--chunker-profile-id` أو `--chunker-profile=<handle>` من أجل
قم بقفل اختيار التسجيل.

## 3. الناشر والمثبت1. **التفويض بالحوكمة** – توفير إدارة البيان وتغليف المعلومات
   التوقيعات في المجلس حتى يكون الدبوس مقبولاً. المراجعون الخارجيون يفعلون ذلك
   حافظ على التحكم SHA3 في خطة القطع مع التحكم في البيان.
2. **Pinner les payloads** – Chargez l'archive CAR (وخيار فهرس CAR) المرجعي في
   البيان مقابل Pin Registry. تأكد من أن البيان ومشاركة السيارة
   نفس عنصر CID.
3. **تسجيل القياس عن بعد** – الحفاظ على علاقة JSON، وكثافة الأداء، والمقياس بالكامل
   جلب التحف من خلال تحريرها. يتم تسجيل لوحات المعلومات هذه بشكل مباشر
   المشغلون ويساعدون على إعادة إنتاج الأحداث دون تنزيل حمولات كبيرة.

## 4. محاكاة جلب أدوات متعددة

`تشغيل البضائع -p sorafs_car --bin sorafs_fetch -- --plan=payload.report.json \
  --provider=alpha=providers/alpha.bin --provider=beta=providers/beta.bin#4@3 \
  --output=payload.bin --json-out=fetch_report.json`- `#<concurrency>` زيادة التوازي من قبل المزود (`#4` ci-dessus).
- `@<weight>` ضبط تحيز الطلب ; القيمة الافتراضية هي 1.
- `--max-peers=<n>` يحد من عدد الموردين المخططين للتنفيذ عند تنفيذه
  découverte renvoie plus de المرشحين الذين يرغبون في ذلك.
- `--expect-payload-digest` و`--expect-payload-len` يحميان من الفساد الصامت.
- `--provider-advert=name=advert.to` التحقق من قدرات المورد قبل الاستخدام
  في المحاكاة.
- `--retry-budget=<n>` يستبدل رقم القطعة المؤقتة (بشكل افتراضي: 3) حتى لا
  CI يعرض بشكل أسرع التراجعات خلال سيناريوهات اللوحة.

`fetch_report.json` يعرض المقاييس المتوافقة (`chunk_retry_total`، `provider_failure_rate`،
وما إلى ذلك) التكيف مع التأكيدات CI وقابلية الملاحظة.

## 5. متابعة التسجيل والحوكمة يوميًا

عند اقتراح ملفات تعريف جديدة للقطعة:

1. قم بتعديل الواصف في `sorafs_manifest::chunker_registry_data`.
2. قم بإعداد `docs/source/sorafs/chunker_registry.md` والمخططات المرتبطة به.
3. قم بإعادة ضبط التركيبات (`export_vectors`) والتقاط البيانات المسجلة.
4. الحصول على تقرير التوافق مع الميثاق مع توقيعات الحوكمة.

تعمل الأتمتة على منح المقابض التقليدية (`namespace.name@semver`) خصوصية
قم بالرجوع إلى المعرفات الرقمية عندما يكون ذلك ضروريًا للتسجيل.