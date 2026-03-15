---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/chunker-registry.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: registro de chunker
título: Um pedaço de pedaço em SoraFS
sidebar_label: qual chunker
description: Você pode usar o chunker em SoraFS.
---

:::note المصدر المعتمد
Verifique o valor `docs/source/sorafs/chunker_registry.md`. احرص على إبقاء النسختين متزامنتين إلى أن يتم إيقاف مجموعة توثيق Sphinx القديمة.
:::

## Um pedaço de bloco em SoraFS (SF-2a)

Verifique se o SoraFS é capaz de fazer chunking para obter mais informações.
Você pode usar o CDC semver e semver e com digest/multicodec para manifestar e CAR.

على مؤلفي الملفات الرجوع إلى
[`docs/source/sorafs/chunker_profile_authoring.md`](./chunker-profile-authoring.md)
Deixe-os entrar e sair de casa.
وبعد أن تعتمد الحوكمة أي تغيير, اتبع
[./chunker-registry-rollout-checklist.md] e
[O manifesto do staging](./staging-manifest-playbook) está definido
Os jogos do palco e do jogo.

### الملفات

| Espaço para nome | الاسم | SemVer | معرف الملف | الحد الأدنى (بايت) | الهدف (بايت) | الحد الأقصى (بايت) | قناع القطع | Multihash | البدائل | Produtos |
|-----------|-------|----|------------|--------------------|--------------|---------|-----------|-----------|--------|---------|
| `sorafs` | `sf1` | `1.0.0` | `1` | 65536 | 262144 | 524288 | `0x0000ffff` | `0x1f` (BLAKE3-256) | `["sorafs.sf1@1.0.0", "sorafs.sf1@1.0.0"]` | الملف المعتمد المستخدم في jogos SF-1 |

O código de acesso é `sorafs_manifest::chunker_registry` (e [`chunker_registry_charter.md`](./chunker-registry-charter.md)). ويُعبَّر عن كل إدخال كـ `ChunkerProfileDescriptor` يضم:

* `namespace` – O código de segurança do dispositivo (como `sorafs`).
* `name` – تسمية مقروءة للبشر (`sf1`, `sf1-fast`, …).
* `semver` – é um recurso que permite a instalação de arquivos.
* `profile` – `ChunkProfile` Função (mín/alvo/máx/máscara).
* `multihash_code` – O multihash que contém o resumo do pedaço (`0x1f`
  Não há problema em SoraFS).

O manifesto do arquivo é `ChunkingProfileV1`. تسجل البنية بيانات السجل (namespace, nome, semver) إلى جانب
Verifique o CDC e o site do CDC. Você pode usar o produto `profile_id`
والرجوع إلى المعلمات المضمّنة عند ظهور معرفات غير معروفة؛ Verifique se o HTTP está em execução
O código de barras é `Accept-Chunker`. وتفرض قواعد ميثاق السجل أن يكون المقبض المعتمد
(`namespace.name@semver`).

Para configurar o método CLI, use o CLI:

```
$ cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- --list-profiles
[
  {
    "namespace": "sorafs",
    "name": "sf1",
    "semver": "1.0.0",
    "handle": "sorafs.sf1@1.0.0",
    "profile_id": 1,
    "min_size": 65536,
    "target_size": 262144,
    "max_size": 524288,
    "break_mask": "0x0000ffff",
    "multihash_code": 31
  }
]
```

Você pode usar CLI como JSON (`--json-out`, `--por-json-out`, `--por-proof-out`,
`--por-sample-out`). يسهل ذلك تمرير
Você pode usar o aplicativo para obter mais informações.

### مصفوفة التوافق وخطة الإطلاق


Verifique se o `sorafs.sf1@1.0.0` está danificado. يشير "Ponte"
Você pode usar o CARv1 + SHA-256 para obter mais informações (`Accept-Chunker` + `Accept-Digest`).| المكون | الحالة | Produtos |
|--------|--------|-----|
| `sorafs_manifest_chunk_store` | ✅ مدعوم | يتحقق من المقبض المعتمد + البدائل, ويبث التقارير عبر `--json-out=-`, ويفرض ميثاق السجل عبر `ensure_charter_compliance()`. |
| `sorafs_manifest_stub` | ⚠️ قديم | مُنشئ manifesto قديم؛ Use `iroha app sorafs toolkit pack` para CAR/manifest e `--plan=-` para definir o valor do carro. |
| `sorafs_provider_advert_stub` | ⚠️ قديم | مساعد تحقق offline فقط؛ يجب إنتاج provedores de anúncios عبر خط أنابيب النشر والتحقق منها عبر `/v2/sorafs/providers`. |
| `sorafs_fetch` (desenvolvedor orquestrador) | ✅ مدعوم | يقرأ `chunk_fetch_specs` ويفهم حمولة قدرة `range` ويجمع إخراج CARv2. |
| Luminárias para SDK (Rust/Go/TS) | ✅ مدعوم | `export_vectors`; Não se preocupe, não há necessidade de fazer isso. |
| Gateway de gateway Torii | ✅ مدعوم | Use o `Accept-Chunker` e o `Content-Chunker` e o Bridge CARv1 para fazer o downgrade. |

طرح التليمترية:

- **تليمترية جلب الـ chunks** — يصدر CLI الخاص بـ Iroha `sorafs toolkit pack` digests للـ chunk, وبيانات CAR, وجذور PoR Não se preocupe com isso.
- **Anúncios de provedores** — تتضمن حمولة الإعلانات بيانات القدرات والبدائل؛ A solução é `/v2/sorafs/providers` (ou seja, `range`).
- **مراقبة الـ gateway** — على المشغلين الإبلاغ عن أزواج `Content-Chunker`/`Content-Digest` لاكتشاف أي خفض غير متوقع؛ ومن المتوقع أن ينخفض ​​استخدام الـ bridge إلى الصفر قبل الإيقاف.

سياسة الإيقاف: بعد اعتماد ملف خلف, حدّد نافذة نشر مزدوجة (موثقة في المقترح) قبل وسم
`sorafs.sf1@1.0.0` é uma ponte para o Bridge CARv1 e para o Bridge CARv1.

لفحص شاهد PoR محدد, قدم مؤشرات chunk/segment/leaf واحفظ البرهان على القرص إذا رغبت:

```
$ cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- ./docs.tar \
    --por-proof=0:0:0 --por-proof-out=leaf.proof.json
```

Você pode usar um dispositivo de armazenamento de dados (`--profile-id=1`) ou uma solução de problemas
(`--profile=sorafs.sf1@1.0.0`)؛ صيغة المقبض مناسبة للسكريبتات التي تمرر namespace/name/semver
مباشرة من بيانات الحوكمة.

Use `--promote-profile=<handle>` para JSON no banco de dados (que está no site da empresa).
O valor do arquivo `chunker_registry_data.rs` é o seguinte:

```
$ cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- \
    --promote-profile=sorafs.sf1@1.0.0
```

يتضمن التقرير الرئيسي (وملف البرهان الاختياري) Digest الجذري, وbytes الأوراق المُعاينة
(مشفرة بالهيكس), وdigests الأشقاء للـ segmento/pedaço بحيث يمكن للمدققين إعادة هاش طبقات
64 KiB/4 KiB de tamanho `por_root_hex`.

للتحقق من برهان موجود مقابل حمولة, مرر المسار عبر
`--por-proof-verify` (é necessário usar CLI para `"por_proof_verified": true`):

```
$ cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- ./docs.tar \
    --por-proof-verify=leaf.proof.json
```

Para obter mais informações sobre o produto `--por-sample=<count>`, você pode obter sementes / sementes de sementes.
تضمن CLI ترتيبًا حتميًا (`splitmix64` semeado) e وتقوم بالاقتطاع تلقائيًا عندما تتجاوز
Nome do produto:```
$ cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- ./docs.tar \
    --por-sample=8 --por-sample-seed=0xfeedface --por-sample-out=por.samples.json
```
```

يعكس manifest stub البيانات نفسها، وهو مناسب عند برمجة اختيار `--chunker-profile-id` في
الـ pipelines. كما تقبل CLIs الخاصة بـ chunk store صيغة المقبض المعتمد
(`--profile=sorafs.sf1@1.0.0`) لتجنب ترميز معرفات رقمية صلبة في سكريبتات البناء:

```
$ carga run -p sorafs_manifest --bin sorafs_manifest_stub -- --list-chunker-profiles
[
  {
    "perfil_id": 1,
    "namespace": "sorafs",
    "nome": "sf1",
    "semver": "1.0.0",
    "handle": "sorafs.sf1@1.0.0",
    "tamanho_mín": 65536,
    "tamanho_alvo": 262144,
    "tamanho máximo": 524288,
    "break_mask": "0x0000ffff",
    "código_multihash": 31
  }
]
```

يتطابق الحقل `handle` (`namespace.name@semver`) مع ما تقبله CLIs عبر
`--profile=…`، مما يجعل نسخه مباشرة إلى الأتمتة آمنًا.

### التفاوض على chunkers

تعلن البوابات والعملاء الملفات المدعومة عبر provider adverts:

```
ProviderAdvertBodyV1 {
    ...
    chunk_profile: profile_id (em inglês)
    capacidades: [...]
}
```

يُعلن جدولة chunk متعددة المصادر عبر القدرة `range`. يقبلها CLI عبر
`--capability=range[:streams]` حيث يشفّر اللاحق الرقمي الاختياري التوازي المفضل لدى المزود
لجلب النطاق (على سبيل المثال، `--capability=range:64` تعلن ميزانية 64 stream).
وعند حذفه، يعود المستهلكون إلى تلميح `max_streams` العام المنشور في مكان آخر من الإعلان.

عند طلب بيانات CAR، يجب أن يرسل العملاء ترويسة `Accept-Chunker` بقائمة tuples
`(namespace, name, semver)` بحسب ترتيب التفضيل:

```

Faça o download do cartão de crédito do site (`sorafs.sf1@1.0.0`) e faça o download do software
Código `Content-Chunker`. تضمن manifests الملف المختار كي تتمكن العقد اللاحقة من التحقق
Você pode usar chunks no HTTP.

### CARRO توافق

Você pode usar o manifesto do CIDv1 com `dag-cbor` (`0x71`). وللتوافق القديم نحتفظ
Configuração do CARv1+SHA-2:

* **المسار الأساسي** – CARv2, resumo do BLAKE3 (`0x1f` multihash),
  `MultihashIndexSorted`, e isso pode ser feito de outra forma.
  Você pode usar o `Accept-Chunker` ou o `Accept-Digest: sha2-256`.

Não há nenhum resumo do resumo do arquivo.

### المطابقة

* يطابق ملف `sorafs.sf1@1.0.0` الـ fixtures العامة في
  `fixtures/sorafs_chunker` e instruções de uso
  `fuzz/sorafs_chunker`. Você pode usar o Rust, o Go e o Node para usá-lo.
* Use `chunker_registry::lookup_by_profile` para obter informações sobre `ChunkProfile::DEFAULT`.
* Você pode manifestar os manifestos em `iroha app sorafs toolkit pack` e `sorafs_manifest_stub`.