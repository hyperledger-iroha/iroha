---
lang: ar
direction: rtl
source: docs/source/sorafs/manifest_pipeline.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 2572648c9c5aa1d4c346e66440fd14bff98afd55232ba1a7ba1c5fcd505559c6
source_last_modified: "2025-11-02T17:57:27.798590+00:00"
translation_last_reviewed: 2026-01-30
---

# تجزئة SoraFS → مسار المانيفست

تسجّل هذه الملاحظة الحد الأدنى من الخطوات اللازمة لتحويل payload من البايتات إلى مانيفست
مشفّر بـ Norito مناسب للتثبيت (pinning) في سجل SoraFS.

1. **تجزئة الـ payload بشكل حتمي**
   - استخدم `sorafs_car::CarBuildPlan::single_file` (يستخدم داخليًا chunker SF-1)
     لاشتقاق إزاحات chunks وأطوالها وملخصات BLAKE3.
   - تكشف الخطة عن ملخص الـ payload وبيانات chunks الوصفية التي يمكن للأدوات downstream
     إعادة استخدامها لتجميع CAR وجدولة Proof-of-Replication.
   - بدلاً من ذلك، يستقبل النموذج الأولي `sorafs_car::ChunkStore` البايتات ويسجل بيانات
     chunks الوصفية الحتمية لبناء CAR لاحقًا. يشتق الـ store الآن شجرة أخذ عينات PoR
     بحجم 64 KiB / 4 KiB (موسومة بنطاق المجال ومُحاذاة مع chunks) حتى يتمكن المجدولون من
     طلب إثباتات Merkle دون إعادة قراءة الـ payload.
    استخدم `--por-proof=<chunk>:<segment>:<leaf>` لإخراج شاهد JSON لورقة مُعاينة واستخدم
    `--por-json-out` لكتابة لقطة ملخص الجذر للتحقق لاحقًا. اربط `--por-proof` مع
    `--por-proof-out=path` لحفظ الشاهد، واستخدم `--por-proof-verify=path` للتأكد من أن
    إثباتًا موجودًا يطابق `por_root_hex` المحسوب للـ payload الحالي. بالنسبة لعدة أوراق،
    ينتج `--por-sample=<count>` (مع `--por-sample-seed` و`--por-sample-out` اختياريين) عينات
    حتمية مع وضع `por_samples_truncated=true` كلما تجاوز الطلب الأوراق المتاحة.
   - احتفظ بإزاحات/أطوال/ملخصات chunks إذا كنت تنوي بناء إثباتات bundle (مانيفستات CAR،
     جداول PoR).
   - ارجع إلى [`sorafs/chunker_registry.md`](chunker_registry.md) للإدخالات المعتمدة في
     السجل وإرشادات التفاوض.

2. **تغليف مانيفست**
   - مرّر بيانات chunking الوصفية وCID الجذر وتعهدات CAR وسياسة pin وclaims الخاصة بالـ alias
     وتواقيع الحوكمة إلى `sorafs_manifest::ManifestBuilder`.
   - استدعِ `ManifestV1::encode` للحصول على بايتات Norito و`ManifestV1::digest` للحصول على
     الملخص المعتمد المُسجل في Pin Registry.

3. **النشر**
   - أرسل ملخص المانيفست عبر الحوكمة (توقيع المجلس وإثباتات alias) وثبّت بايتات المانيفست
     في SoraFS باستخدام المسار الحتمي.
   - تأكد من أن ملف CAR (وفهرس CAR الاختياري) المشار إليه في المانيفست مخزّن في نفس مجموعة
     pins الخاصة بـ SoraFS.

### البدء السريع للـ CLI

```bash
cargo run -p sorafs_manifest --bin sorafs-manifest-stub   ./docs.tar   --root-cid=0155aa   --car-cid=017112...   --alias-file=docs:sora:alias_proof.bin   --council-signature-file=0123...cafe:council.sig   --metadata=build:ci-123   --manifest-out=docs.manifest   --manifest-signatures-out=docs.manifest.signatures.json   --car-out=docs.car   --json-out=docs.report.json
```

تطبع الأداة ملخصات chunks وتفاصيل المانيفست؛ وعند توفير `--manifest-out` و/أو `--car-out`
فإنها تكتب payload Norito وأرشيف CARv2 مطابقًا للمواصفة (pragma + header + كتل CARv1 +
فهرس Multihash) إلى القرص. إذا مررت مسار دليل، فستقوم الأداة بتمريره بشكل تكراري (ترتيب
معجمي)، وتجزئة كل ملف، وإخراج شجرة dag-cbor ذات جذر دليل يظهر CID الخاص به كجذر لكل من
المانيفست وCAR. يتضمن تقرير JSON ملخص payload لـ CAR المحسوب، والملخص الكامل للأرشيف،
والحجم، وCID الخام، والجذر (مع فصل codec الخاص بـ dag-cbor)، إلى جانب إدخالات alias/metadata
للمانيفست. استخدم `--root-cid`/`--dag-codec` من أجل *التحقق* من الجذر أو الـ codec المحسوب أثناء
تشغيل CI، واستخدم `--car-digest` لفرض hash للـ payload، و`--car-cid` لفرض مُعرّف CAR خام
مُحتسب مسبقًا (CIDv1، codec `raw`، multihash BLAKE3)، و`--json-out` لحفظ JSON المطبوع بجانب
artefacts المانيفست/CAR لأتمتة downstream.

عند توفير `--manifest-signatures-out` (مع وجود علم `--council-signature*` واحد على الأقل)،
تكتب الأداة أيضًا ظرفًا باسم `manifest_signatures.json` يحتوي على ملخص BLAKE3 للمانيفست،
والملخص SHA3-256 المجمّع لخطة chunks (الإزاحات والأطوال وملخصات BLAKE3 للـ chunks)، وتواقيع
المجلس المُقدمة. يسجل الظرف الآن ملف تعريف chunker بالشكل المعتمد `namespace.name@semver`؛ وتظل
الأظرف الأقدم من نوع `namespace-name` قابلة للتحقق من أجل التوافق. يمكن للأتمتة downstream نشر
الظرف في سجلات الحوكمة أو توزيعه مع artefacts المانيفست وCAR. عند استلام ظرف من موقّع خارجي،
أضف `--manifest-signatures-in=<path>` كي تؤكد CLI الملخصات وتتحقق من كل توقيع Ed25519 مقابل
ملخص المانيفست المحسوب حديثًا.

عندما تكون هناك ملفات تعريف متعددة لـ chunker، يمكنك اختيار أحدها بشكل صريح عبر
`--chunker-profile-id=<id>`. يربط هذا العلم بالمعرّفات الرقمية في [`chunker_registry`](chunker_registry.md)
ويضمن أن تمريرة chunking والمانيفست الناتج يشيران إلى نفس `(namespace, name, semver)`.
فضّل الشكل المعتمد للـ handle في الأتمتة (`--chunker-profile=sorafs.sf1@1.0.0`) لتجنب تثبيت
المعرّفات الرقمية. شغّل `sorafs_manifest_chunk_store --list-profiles` لعرض إدخالات السجل الحالية
(المخرجات تطابق القائمة التي يوفرها `sorafs_manifest_chunk_store`)، أو استخدم
`--promote-profile=<handle>` لتصدير الـ handle المعتمد وبيانات alias الوصفية عند إعداد تحديث للسجل.

يمكن للمدققين طلب شجرة Proof-of-Retrievability كاملة عبر `--por-json-out=path`، التي تُسلسل
ملخصات chunk/segment/leaf لأغراض التحقق بالعينة. يمكن تصدير الشواهد الفردية باستخدام
`--por-proof=<chunk>:<segment>:<leaf>` (والتحقق عبر `--por-proof-verify=path`)، بينما ينتج
`--por-sample=<count>` عينات حتمية وغير مكررة للفحوصات الموضعية.

أي علم يكتب JSON (`--json-out`، `--chunk-fetch-plan-out`، `--por-json-out`، إلخ) يقبل أيضًا `-`
كمسار، مما يسمح لك ببث الـ payload مباشرة إلى stdout دون إنشاء ملفات مؤقتة.

استخدم `--chunk-fetch-plan-out=path` لحفظ مواصفة fetch المرتبة الخاصة بـ chunks (فهرس chunk،
إزاحة الـ payload، الطول، ملخص BLAKE3) التي ترافق خطة المانيفست. يمكن للعملاء متعددي المصادر
تمرير JSON الناتج مباشرة إلى أوركسترايتور fetch الخاص بـ SoraFS دون إعادة قراءة الـ payload الأصلي.
يتضمن تقرير JSON المطبوع بواسطة CLI هذا المصفوفة ضمن `chunk_fetch_specs`. يكشف قسم `chunking` وكائن
`manifest` معًا عن `profile_aliases` إلى جانب الـ handle المعتمد `profile` حتى تتمكن الـ SDKs من
الانتقال من الشكل القديم `namespace-name` دون فقدان التوافق.

عند إعادة تشغيل stub (مثلاً في CI أو في خط إصدار) يمكنك تمرير `--plan=chunk_fetch_specs.json` أو
`--plan=-` لاستيراد المواصفة التي تم توليدها سابقًا. تتحقق CLI من أن فهرس وإزاحة وطول وملخص BLAKE3
لكل chunk لا تزال تتطابق مع خطة CAR المشتقة حديثًا قبل متابعة الإدخال، ما يحمي من الخطط القديمة أو
المتلاعب بها.

### Smoke-test للأوركسترايتور المحلي

يتضمن crate `sorafs_car` الآن `sorafs-fetch`، وهو CLI للمطورين يستهلك مصفوفة `chunk_fetch_specs`
ويُحاكي الاسترجاع متعدد المزوّدين من ملفات محلية. وجّه الأداة إلى JSON الناتج عن `--chunk-fetch-plan-out`،
وقدّم مسارًا واحدًا أو أكثر من payloads الخاصة بالمزوّدين (اختياريًا مع `#N` لزيادة التوازي)، وستتحقق من
chunks، وتعید تجميع الـ payload، وتطبع تقرير JSON يلخص أعداد النجاح/الفشل لكل مزوّد وإيصالات كل chunk:

```
cargo run -p sorafs_car --bin sorafs_fetch --   --plan=chunk_fetch_specs.json   --provider=alpha=./providers/alpha.bin   --provider=beta=./providers/beta.bin#4@3   --output=assembled.bin   --json-out=fetch_report.json   --provider-metrics-out=providers.json   --scoreboard-out=scoreboard.json
```

استخدم هذا التدفق للتحقق من سلوك الأوركسترايتور أو لمقارنة payloads الخاصة بالمزوّدين قبل ربط
نواقل الشبكة الفعلية بعقدة SoraFS.

عندما تحتاج إلى الوصول إلى بوابة Torii حية بدل الملفات المحلية، استبدل أعلام `--provider=/path`
بالخيارات الجديدة الموجهة لـ HTTP:

```
sorafs-fetch   --plan=chunk_fetch_specs.json   --gateway-provider=name=gw-a,provider-id=<hex>,base-url=https://gw-a.example/,stream-token=<base64>   --gateway-manifest-id=<manifest_id_hex>   --gateway-chunker-handle=sorafs.sf1@1.0.0   --gateway-client-id=ci-orchestrator   --json-out=gateway_fetch_report.json
```

تتحقق CLI من stream token، وتفرض محاذاة chunker/profile، وتسجل بيانات gateway الوصفية بجانب إيصالات
المزوّدين المعتادة حتى يتمكن المشغلون من أرشفة التقرير كدليل على rollout (راجع دليل النشر لتدفق blue/green الكامل).

إذا مرّرت `--provider-advert=name=/path/to/advert.to`، فإن CLI تقوم الآن بفك ترميز ظرف Norito والتحقق
من توقيع Ed25519 وفرض أن المزوّد يعلن قدرة `chunk_range_fetch`. يبقي ذلك محاكاة fetch متعددة المصادر
متوافقة مع سياسة القبول في الحوكمة ويمنع الاستخدام العرضي لمزوّدين قدامى لا يستطيعون تلبية طلبات chunk بنطاق.

اللاحقة `#N` تزيد حد التوازي للمزوّد، بينما يحدد `@W` وزن الجدولة (الافتراضي 1 عند عدم تحديده). عندما
يتم توفير adverts أو واصفات gateway، تقوم CLI الآن بتقييم scoreboard الخاص بالأوركسترايتور قبل بدء fetch:
يرث المزوّدون المؤهلون أوزانًا واعية بالتليمترية، ويتم حفظ لقطة JSON عبر `--scoreboard-out=<path>` عند توفيره.
يتم إسقاط المزوّدين الذين يفشلون في التحقق من القدرات أو المواعيد النهائية للحوكمة تلقائيًا مع تحذير لضمان بقاء
التشغيل متوافقًا مع سياسة القبول. راجع
`docs/examples/sorafs_ci_sample/{telemetry.sample.json,scoreboard.json}` لزوج إدخال/إخراج نموذجي.

مرّر `--expect-payload-digest=<hex>` و/أو `--expect-payload-len=<bytes>` لتأكيد أن الـ payload المُجمّع
يطابق توقعات المانيفست قبل كتابة المخرجات — مفيد لاختبارات CI السريعة التي تريد التأكد من أن الأوركسترايتور لم
يُسقط أو يُعيد ترتيب chunks بصمت.

إذا كان لديك بالفعل تقرير JSON تم إنشاؤه بواسطة `sorafs-manifest-stub`، فمرره مباشرة عبر
`--manifest-report=docs.report.json`. ستعيد CLI الخاصة بـ fetch استخدام الحقول المضمنة
`chunk_fetch_specs` و`payload_digest_hex` و`payload_len`، لذلك لا تحتاج لإدارة ملفات خطة أو تحقق منفصلة.

يعرض تقرير fetch أيضًا تليمترية مجمعة للمساعدة في المراقبة:
`chunk_retry_total`، `chunk_retry_rate`، `chunk_attempt_total`،
`chunk_attempt_average`، `provider_success_total`، `provider_failure_total`،
`provider_failure_rate` و`provider_disabled_total` تلتقط الصحة العامة لجلسة fetch وهي مناسبة
للوحات Grafana/Loki أو تأكيدات CI. استخدم `--provider-metrics-out` لكتابة مصفوفة `provider_reports`
فقط إذا كان tooling downstream يحتاج إلى إحصاءات على مستوى المزوّد فقط.

### الخطوات التالية

- التقط بيانات CAR الوصفية بجانب ملخصات المانيفست في سجلات الحوكمة حتى يتمكن المراقبون من التحقق من محتوى CAR
  دون إعادة تنزيل الـ payload.
- ادمج تدفق نشر المانيفست وCAR في CI حتى تُنتج كل عملية بناء docs/artefacts مانيفستًا تلقائيًا وتحصل على
  التواقيع وتثبّت الـ payloads الناتجة.
