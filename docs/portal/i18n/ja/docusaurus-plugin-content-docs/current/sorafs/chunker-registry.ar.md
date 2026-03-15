---
lang: ja
direction: ltr
source: docs/portal/docs/sorafs/chunker-registry.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: チャンカーレジストリ
タイトル: チャンカー SoraFS
サイドバーラベル: チャンカー
説明: チャンカー SoraFS。
---

:::note ノート
テストは `docs/source/sorafs/chunker_registry.md` です。スフィンクスの正体は、スフィンクスです。
:::

## チャンカー SoraFS (SF-2a)

SoraFS チャンク処理を実行してください。
CDC は、ダイジェスト/マルチコーデックと CAR をマニフェストします。

على مؤلفي الملفات الرجوع إلى
[`docs/source/sorafs/chunker_profile_authoring.md`](./chunker-profile-authoring.md)
ログインしてください。
وبعد أن تعتمد الحوكمة أي تغيير، اتبع
[قائمة تحقق إطلاق السجل](./chunker-registry-rollout-checklist.md) و
[マニフェストステージング](./staging-manifest-playbook)
備品とステージング。

### ああ

|ネームスペース |ああ |セミバージョン | और देखें और देखें और देखें और देखें और देखेंマルチハッシュ |翻訳 |重要 |
|----------|----------|----------|-----------|---------|--------------|---------|-----------|-----------|--------|---------|
| `sorafs` | `sf1` | `1.0.0` | `1` | 65536 | 262144 | 524288 | `0x0000ffff` | `0x1f` (ブレイク3-256) | `["sorafs.sf1@1.0.0", "sorafs.sf1@1.0.0"]` |フィクスチャ SF-1 | フィクスチャ SF-1

يعيش السجل في الشيفرة ضمن `sorafs_manifest::chunker_registry` (ويحكمه [`chunker_registry_charter.md`](./chunker-registry-charter.md))。 عن كل إدخال كـ `ChunkerProfileDescriptor` يضم:

* `namespace` – تجميع منطقي للملفات ذات الصلة (مثل `sorafs`)。
* `name` – テスト (`sf1`、`sf1-fast`、…)。
* `semver` – ログインしてください。
* `profile` – `ChunkProfile` (最小/ターゲット/最大/マスク)。
* `multihash_code` – マルチハッシュ、チャンクのダイジェスト (`0x1f`)
  (SoraFS)。

マニフェストは `ChunkingProfileV1` です。評価 (名前空間、名前、セムバー) 評価
CDC を参照してください。 ينبغي على المستهلكين أولاً محاولة البحث في السجل عبر `profile_id`
والرجوع إلى المعلمات المضمّنة عند ظهور معرفات غير معروفة؛ HTTP メッセージを受信する
`Accept-Chunker` 認証済み。 قواعد ميثاق السجل أن يكون المقبض المعتمد
(`namespace.name@semver`) は、`profile_aliases` を意味します。

CLI での操作:

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

CLI と JSON (`--json-out`、`--por-json-out`、`--por-proof-out`、
`--por-sample-out`) `-` は、標準出力をテストします。 سهل ذلك تمرير
最高のパフォーマンスを見せてください。

### مصفوفة التوافق وخطة الإطلاق


セキュリティは、`sorafs.sf1@1.0.0` をサポートしています。 「橋」
CARv1 + SHA-256 バージョン (`Accept-Chunker` + `Accept-Digest`)。

|ああ、ああ |重要 |
|----------|--------|----------|
| `sorafs_manifest_chunk_store` | ✅ और देखें يتحقق من المقبض المعتمد + البدائل، ويبث التقارير عبر `--json-out=-`، ويفرض ميثاق السجل عبر `ensure_charter_compliance()`。 |
| `sorafs_manifest_stub` | ⚠️ और देखेंマニフェスト マニフェスト`iroha app sorafs toolkit pack` CAR/マニフェスト `--plan=-` を確認してください。 |
| `sorafs_provider_advert_stub` | ⚠️ और देखेंオフラインで使用できます。プロバイダーの広告は、`/v1/sorafs/providers` です。 |
| `sorafs_fetch` (開発者オーケストレーター) | ✅ और देखें CARv2 は `chunk_fetch_specs` 、`range` は CARv2 です。 |
|フィクスチャ SDK (Rust/Go/TS) | ✅ और देखें يُعاد توليدها عبر `export_vectors`؛あなたのことを忘れないでください。 |
|ゲートウェイ Torii | 重要なゲートウェイ✅ और देखेंブリッジ CARv1 は、`Accept-Chunker` をダウングレードします。 |

名前:

- **チャンク** — CLI および Iroha `sorafs toolkit pack` ダイジェスト チャンク CAR および PoR 、、、、、、、、、、。
- **プロバイダーの広告** — 評価とレビューحقّق من التغطية عبر `/v1/sorafs/providers` (مثل وجود قدرة `range`)。
- **مراقبة الـ ゲートウェイ** — على المشغلين الإبلاغ عن أزواج `Content-Chunker`/`Content-Digest` لاكتشاف أي خفض غيرすごい橋を渡って、橋を渡ってください。

سياسة الإيقاف: بعد اعتماد ملف خلف، حدّد نافذة نشر مزدوجة (موثقة في المقترح) قبلうーん
`sorafs.sf1@1.0.0` ブリッジ CARv1 がサポートされています。PoR のチャンク/セグメント/リーフの定義:

```
$ cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- ./docs.tar \
    --por-proof=0:0:0 --por-proof-out=leaf.proof.json
```

يمكنك اختيار ملف عبر معرف رقمي (`--profile-id=1`) أو عبر المقبض المسجل
(`--profile=sorafs.sf1@1.0.0`)名前空間/名前/semver 名前空間/名前/semver
重要な意味を持っています。

ستخدم `--promote-profile=<handle>` لإخراج كتلة JSON من البيانات الوصفية (بما في ذلك كل البدائل
المسجلة) يمكن لصقها في `chunker_registry_data.rs` عند ترقية ملف افتراضي جديد:

```
$ cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- \
    --promote-profile=sorafs.sf1@1.0.0
```

セキュリティ (وملف البرهان الاختياري) ダイジェスト الجذري، وbytes الأوراق المُعاينة
(مشفرة بالهيكس) ダイジェスト الأشقاء للـ セグメント/チャンク بحيث يمكن للمدققين إعادة هاش طبقات
64 KiB/4 KiB مقابل قيمة `por_root_hex`。

للتحقق من برهان موجود مقابل حمولة، مرر المسار عبر
`--por-proof-verify` (تضيف CLI الحقل `"por_proof_verified": true` عندما يتطابق الشاهد مع الجذر المحسوب):

```
$ cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- ./docs.tar \
    --por-proof-verify=leaf.proof.json
```

أخذ عينات على دفعات، استخدم `--por-sample=<count>` ويمكنك تمرير シード/مسار إخراج اختياري。
CLI の評価 (`splitmix64` シード) の評価 (`splitmix64` シード) の評価。
回答:

```
$ cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- ./docs.tar \
    --por-sample=8 --por-sample-seed=0xfeedface --por-sample-out=por.samples.json
```
```

يعكس manifest stub البيانات نفسها، وهو مناسب عند برمجة اختيار `--chunker-profile-id` في
الـ pipelines. كما تقبل CLIs الخاصة بـ chunk store صيغة المقبض المعتمد
(`--profile=sorafs.sf1@1.0.0`) لتجنب ترميز معرفات رقمية صلبة في سكريبتات البناء:

```
$ Cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- --list-chunker-profiles
[
  {
    「プロファイルID」: 1、
    "名前空間": "sorafs",
    "名前": "sf1",
    "semver": "1.0.0",
    "ハンドル": "sorafs.sf1@1.0.0",
    "min_size": 65536、
    「ターゲットサイズ」: 262144、
    "最大サイズ": 524288、
    "ブレークマスク": "0x0000ffff",
    「マルチハッシュコード」: 31
  }
】
```

يتطابق الحقل `handle` (`namespace.name@semver`) مع ما تقبله CLIs عبر
`--profile=…`، مما يجعل نسخه مباشرة إلى الأتمتة آمنًا.

### التفاوض على chunkers

تعلن البوابات والعملاء الملفات المدعومة عبر provider adverts:

```
ProviderAdvertBodyV1 {
    ...
    chunk_profile: profile_id (ضمنيًا عبر السجل)
    能力: [...]
}
```

يُعلن جدولة chunk متعددة المصادر عبر القدرة `range`. يقبلها CLI عبر
`--capability=range[:streams]` حيث يشفّر اللاحق الرقمي الاختياري التوازي المفضل لدى المزود
لجلب النطاق (على سبيل المثال، `--capability=range:64` تعلن ميزانية 64 stream).
وعند حذفه، يعود المستهلكون إلى تلميح `max_streams` العام المنشور في مكان آخر من الإعلان.

عند طلب بيانات CAR، يجب أن يرسل العملاء ترويسة `Accept-Chunker` بقائمة tuples
`(namespace, name, semver)` بحسب ترتيب التفضيل:

```

تختار البوابات ملفًا مدعومًا من الطرفين (الافتراضي `sorafs.sf1@1.0.0`) وتعكس القرار عبر ترويسة
`Content-Chunker`。は、マニフェスト الملف المختار كي تتمكن العقد اللاحقة من التحقق
HTTP のチャンクを取得します。

### 車

CIDv1 マニフェスト `dag-cbor` (`0x71`)。 حتفق القديم نحتفظ
CARv1+SHA-2:

* ** セキュリティ** – CARv2 ダイジェスト BLAKE3 (`0x1f` マルチハッシュ)
  `MultihashIndexSorted`، والملف مسجل كما سبق。
  هذا الخيار عندما يهمل العميل `Accept-Chunker` أو يطلب `Accept-Digest: sha2-256`。

ダイジェスト版をご覧ください。

### ああ

* يطابق ملف `sorafs.sf1@1.0.0` الـ 備品 العامة في
  `fixtures/sorafs_chunker` 認証済み
  `fuzz/sorafs_chunker`。 Rust وGo وNode を開発してください。
* تؤكد `chunker_registry::lookup_by_profile` أن معلمات الوصف تطابق `ChunkProfile::DEFAULT` للحماية من الانحرافات العرضية.
* は、`iroha app sorafs toolkit pack` と `sorafs_manifest_stub` を表します。