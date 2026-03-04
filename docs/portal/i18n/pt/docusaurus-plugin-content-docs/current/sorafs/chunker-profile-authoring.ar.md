---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/chunker-profile-authoring.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: autoria de perfil chunker
título: Um pedaço de pedaço de SoraFS
sidebar_label: Qual é o chunker
description: Você pode usar um pedaço de chunker e acessórios em SoraFS.
---

:::note المصدر المعتمد
Verifique o valor `docs/source/sorafs/chunker_profile_authoring.md`. احرص على إبقاء النسختين متزامنتين إلى أن يتم إيقاف مجموعة توثيق Sphinx القديمة.
:::

# دليل تأليف ملفات chunker em SoraFS

Você pode usar o chunker para SoraFS.
Qual é o RFC do formulário (SF-1) e do RFC (SF-2a)
بمتطلبات تأليف واضحة وخطوات تحقق وقوالب مقترح.
للاطلاع على مثال معتمد, راجع
`docs/source/sorafs/proposals/sorafs_sf1_profile_v1.json`
وسجل dry-run المرافق في
`docs/source/sorafs/reports/sf1_determinism.md`.

## نظرة عامة

يجب أن يحقق كل ملف يدخل السجل ما يلي:

- O CDC está usando o CDC e o multihash está disponível para você
- Todos os fixtures são configurados para (JSON Rust/Go/TS + corpora fuzz + شهود PoR) para SDKs downstream
  Ferramentas para ferramentas
- تضمين بيانات جاهزة للحوكمة (namespace, nome, semver) مع إرشادات الهجرة ونوافذ التوافق؛ e
- اجتياز حزمة diff الحتمية قبل مراجعة المجلس.

Certifique-se de que o produto esteja funcionando corretamente.

## ملخص ميثاق السجل

قبل صياغة المقترح, تأكد من مطابقته لميثاق السجل الذي تفرضه
`sorafs_manifest::chunker_registry::ensure_charter_compliance()`:

- معرفات الملفات أعداد صحيحة موجبة تزيد بشكل رتيب دون فجوات.
- يجب أن يظهر المقبض المعتمد (`namespace.name@semver`) no site da empresa
  Não há **الأول**. Verifique o valor do arquivo (como `sorafs.sf1@1.0.0`).
- لا يجوز لأي alias أن يتعارض مع handle معتمد آخر أو أن يتكرر.
- يجب أن تكون alias غير فارغة ومقصوصة من المسافات.

مساعدات CLI:

```bash
# قائمة JSON بكل descripors المسجلة (ids, handles, aliases, multihash)
cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- --list-profiles

# إخراج بيانات ملف افتراضي مرشح (handle معتمد + aliases)
cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- \
  --promote-profile=sorafs.sf1@1.0.0 --json-out=-
```

تحافظ هذه الأوامر على توافق المقترحات مع ميثاق السجل وتوفر البيانات المعتمدة
Não use nada.

## البيانات المطلوبة

| الحقل | الوصف | Mecanismo (`sorafs.sf1@1.0.0`) |
|-------|-------|---------------------------|
| `namespace` | Verifique se há algum problema com isso. | `sorafs` |
| `name` | تسمية مقروءة للبشر. | `sf1` |
| `semver` | Certifique-se de que não há nenhum problema com isso. | `1.0.0` |
| `profile_id` | Você pode fazer isso sem problemas. Verifique o valor do produto e verifique o valor do produto. | `1` |
| `profile_aliases` | Você pode fazer isso com uma chave de fenda (أسماء قديمة, اختصارات). يجب تضمين المقبض المعتمد أولاً. | `["sorafs.sf1@1.0.0"]` |
| `profile.min_size` | Não pegue nenhum pedaço. | `65536` |
| `profile.target_size` | Você não pode usar nenhum pedaço. | `262144` |
| `profile.max_size` | Você não pode usar nenhum pedaço. | `524288` |
| `profile.break_mask` | É um hash rolante (hex). | `0x0000ffff` |
| `profile.polynomial` | ثابت polinômio de engrenagem (hex). | `0x3da3358b4dc173` |
| `gear_seed` | Semente لاشتقاق جدول engrenagem بحجم 64 KiB. | `sorafs-v1-gear` |
| `chunk_multihash.code` | O multihash é um resumo do pedaço. | `0x1f` (BLAKE3-256) |
| `chunk_multihash.digest` | Digest لحزمة luminárias المعتمدة. | `13fa...c482` |
| `fixtures_root` | مسار نسبي يحتوي على fixtures المعاد توليدها. | `fixtures/sorafs_chunker/sorafs.sf1@1.0.0/` |
| `por_seed` | Semente por PoR الحتمية (`splitmix64`). | `0xfeedbeefcafebabe` (مثال) |يجب أن تظهر البيانات الوصفية في وثيقة المقترح وداخل fixtures المولدة حتى يتمكن السجل
e ferramentas para CLI e ferramentas que podem ser usadas para isso. عند الشك, شغّل
CLIs são usados ​​para chunk-store e manifesto com `--json-out=-` para obter mais informações
ملاحظات المراجعة.

### نقاط تماس CLI e والسجل

- `sorafs_manifest_chunk_store --profile=<handle>` — إعادة تشغيل بيانات pedaço e resumo
  O manifesto e o PoR مع المعلمات المقترحة.
- `sorafs_manifest_chunk_store --json-out=-` — Limpa chunk-store por stdout
  للمقارنات الآلية.
- `sorafs_manifest_stub --chunker-profile=<handle>` — تأكيد أن manifestos وخطط CAR
  تتضمن المقبض المعتمد والبدائل.
- `sorafs_manifest_stub --plan=-` — إعادة تغذية `chunk_fetch_specs` سابق للتحقق من
  compensações/resumos بعد التغيير.

سجّل مخرجات الأوامر (digests, جذور PoR, hashes para o manifesto) no site da Microsoft
Não há nada que você possa fazer.

## قائمة تحقق الحتمية والتحقق

1. **Equipamentos do إعادة توليد**
   ```bash
   cargo run --locked -p sorafs_chunker --bin export_vectors \
     --signature-out=fixtures/sorafs_chunker/manifest_signatures.json
   ```
2. **تشغيل مجموعة التكافؤ** — يجب أن تكون `cargo test -p sorafs_chunker` e diferencial de chicote
   عبر اللغات (`crates/sorafs_chunker/tests/vectors.rs`) باللون الأخضر مع fixtures الجديدة.
3. **Deixar o corpo fuzz/contrapressão** — `cargo fuzz list` e arnês
   (`fuzz/sorafs_chunker`) não funciona.
4. **التحقق من شهود Prova de Recuperabilidade** — شغّل
   `sorafs_manifest_chunk_store --por-sample=<n>` باستخدام الملف المقترح وأكد تطابق الجذور
   مع manifesto الخاص بالـ fixtures.
5. **Execução de teste para CI** — شغّل `ci/check_sorafs_fixtures.sh` محلياً؛ يجب أن ينجح
   مع fixtures الجديدة e `manifest_signatures.json` الحالي.
6. **Tempo de execução cruzado** — تأكد من أن ربط Go/TS يستهلك JSON المُعاد توليده ويُخرج
   حدود chunk e digests متطابقة.

وثّق الأوامر والـ digests الناتجة في المقترح كي يستطيع Tooling WG إعادة تشغيلها دون تخمين.

### Manifesto / PoR

بعد إعادة توليد fixtures, شغّل مسار manifest بالكامل لضمان بقاء بيانات CAR و PoR متسقة:

```bash
# التحقق من بيانات chunk + PoR مع الملف الجديد
cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- \
  --profile=sorafs.sf2@1.0.0 \
  --json-out=- --por-json-out=- fixtures/sorafs_chunker/input.bin

# توليد manifest + CAR والتقاط chunk fetch specs
cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- \
  fixtures/sorafs_chunker/input.bin \
  --chunker-profile=sorafs.sf2@1.0.0 \
  --chunk-fetch-plan-out=chunk_plan.json \
  --manifest-out=sf2.manifest \
  --car-out=sf2.car \
  --json-out=sf2.report.json

# إعادة التشغيل باستخدام خطة fetch المحفوظة (تمنع offsets القديمة)
cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- \
  fixtures/sorafs_chunker/input.bin \
  --chunker-profile=sorafs.sf2@1.0.0 \
  --plan=chunk_plan.json --json-out=-
```

استبدل ملف الإدخال بأي corpus ممثل مستخدم في fixtures الخاصة بك
(O stream tem cerca de 1 GiB) e digere o conteúdo do site.

## قالب المقترح

Você pode usar o Norito no `ChunkerProfileProposalV1` para obter mais informações
`docs/source/sorafs/proposals/`. O JSON é definido como um arquivo JSON
(استبدل القيم حسب الحاجة):


Faça o Markdown usando o Markdown (`determinism_report`) para armazenar e digerir o pedaço e
Você pode fazer isso sem problemas.

## سير عمل الحوكمة

1. **تقديم PR مع المقترح + fixtures.** ضمّن الأصول المولدة, مقترح Norito, وتحديثات
   `chunker_registry_data.rs`.
2. **مراجعة Tooling WG.**
   يتوافق مع قواعد السجل (لا إعادة لاستخدام المعرفات, الحتمية متحققة).
3. **ظرف المجلس.** بعد الموافقة, يوقع أعضاء المجلس digest المقترح
   (`blake3("sorafs-chunker-profile-v1" || canonical_bytes)`).
   ظرف الملف المخزن مع jogos.
4. **نشر السجل.** يؤدي الدمج إلى تحديث السجل والوثائق والـ fixtures. Como usar o CLI
   Isso significa que você pode fazer isso sem problemas.
   وأبلغ المشغلين عبر migração ledger.

## نصائح التأليف- Você pode usar o chunking em cada lugar.
- تجنب تغيير كود multihash دون تنسيق مع مستهلكي manifesto e gateway; وأضف ملاحظة توافق عند ذلك.
- اجعل sementes لجدول gear قابلة للقراءة لكنها فريدة عالمياً لتسهيل التدقيق.
- خزّن أي artefatos قياس أداء (مثل مقارنات taxa de transferência) ضمن
  `docs/source/sorafs/reports/` é um problema de segurança.

لتوقعات التشغيل أثناء lançamento do registro de migração
(`docs/source/sorafs/migration_ledger.md`). لقواعد المطابقة e قت التشغيل راجع
`docs/source/sorafs/chunker_conformance.md`.