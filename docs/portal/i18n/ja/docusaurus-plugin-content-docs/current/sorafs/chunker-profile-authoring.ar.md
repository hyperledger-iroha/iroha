---
lang: ja
direction: ltr
source: docs/portal/docs/sorafs/chunker-profile-authoring.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: チャンカープロファイルオーサリング
タイトル: دليل تأليف ملفات chunker في SoraFS
サイドバーラベル: チャンカー
説明: チャンカーとフィクスチャ SoraFS。
---

:::note ノート
テストは `docs/source/sorafs/chunker_profile_authoring.md` です。スフィンクスの正体は、スフィンクスです。
:::

# دليل تأليف ملفات チャンカー في SoraFS

チャンカー SoraFS を確認してください。
RFC 認証 (SF-1) 認証 (SF-2a)
最高のパフォーマンスを見せてください。
للاطلاع على مثال معتمد، راجع
`docs/source/sorafs/proposals/sorafs_sf1_profile_v1.json`
予行演習
`docs/source/sorafs/reports/sf1_determinism.md`。

## いいえ

يجب أن يحقق كل ملف يدخل السجل ما يلي:

- CDC セキュリティ マルチハッシュ セキュリティ セキュリティ
- フィクスチャ (JSON Rust/Go/TS + corpora fuzz + شهود PoR) ダウンストリーム SDK
  ツールを使用する
- تضمين بيانات جاهزة للحوكمة (namespace, name, semver) مع إرشادات الهجرة ونوافذ التوافق؛ و
- 違いを確認してください。

あなたのことを忘れないでください。

## ملخص ميثاق السجل

قبل صياغة المقترح، تأكد من مطابقته لميثاق السجل الذي تفرضه
`sorafs_manifest::chunker_registry::ensure_charter_compliance()`:

- عرفات الملفات أعداد صحيحة موجبة تزيد بشكل رتيب دون فجوات.
- يجب أن يظهر المقبض المعتمد (`namespace.name@semver`) في قائمة البدائل
  أن يكون **الأول**。 بدائل القديمة (مثل `sorafs.sf1@1.0.0`)。
- 別名 أن يتعارض مع ハンドル معتمد آخر أو أن يتكرر。
- يجب أن تكون 別名 غير فارغة ومقصوصة من المسافات。

CLI の場合:

```bash
# قائمة JSON بكل descripors المسجلة (ids, handles, aliases, multihash)
cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- --list-profiles

# إخراج بيانات ملف افتراضي مرشح (handle معتمد + aliases)
cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- \
  --promote-profile=sorafs.sf1@1.0.0 --json-out=-
```

تحافظ هذه الأوامر على توافق المقترحات مع ميثاق السجل وتوفر البيانات المعتمدة
ありがとうございます。

## ああ、

|ああ |ああ | (`sorafs.sf1@1.0.0`) |
|------|------|---------------|
| `namespace` |最高のパフォーマンスを見せてください。 | `sorafs` |
| `name` |重要です。 | `sf1` |
| `semver` |重要な問題は、次のとおりです。 | `1.0.0` |
| `profile_id` | عرف رقمي رتيب يُسند عند إدخال الملف.ありがとうございます。 | `1` |
| `profile_aliases` | مقابض إضافية اختيارية (أسماء قديمة، اختصارات) تُعرض للعملاء أثناء التفاوض。 جب تضمين المقبض المعتمد أولاً 。 | `["sorafs.sf1@1.0.0"]` |
| `profile.min_size` |チャンクを確認してください。 | `65536` |
| `profile.target_size` |チャンクを確認してください。 | `262144` |
| `profile.max_size` |チャンクを確認してください。 | `524288` |
| `profile.break_mask` |ローリング ハッシュ (16 進数)。 | `0x0000ffff` |
| `profile.polynomial` |歯車多項式 (16 進数)。 | `0x3da3358b4dc173` |
| `gear_seed` |シード لاشتقاق جدول gear بحجم 64 KiB。 | `sorafs-v1-gear` |
| `chunk_multihash.code` |マルチハッシュはチャンクをダイジェストします。 | `0x1f` (ブレイク3-256) |
| `chunk_multihash.digest` |試合日程のダイジェスト。 | `13fa...c482` |
| `fixtures_root` |試合の日程を確認してください。 | `fixtures/sorafs_chunker/sorafs.sf1@1.0.0/` |
| `por_seed` |シードは、PoR الحتمية (`splitmix64`) です。 | `0xfeedbeefcafebabe` (特別) |

試合の試合結果、試合結果、試合結果、試合結果など
ツールと CLI のツールが必要です。 और देखें
CLI は、チャンクストアとマニフェスト `--json-out=-` と互換性があります。
重要です。

### CLI を使用してください。

- `sorafs_manifest_chunk_store --profile=<handle>` — チャンクとダイジェスト
  マニフェストを表示します。
- `sorafs_manifest_chunk_store --json-out=-` — チャンクストアの標準出力
  ああ、それは。
- `sorafs_manifest_stub --chunker-profile=<handle>` — 自動車マニフェスト
  最高です。
- `sorafs_manifest_stub --plan=-` — 認証済み `chunk_fetch_specs` 認証済み
  オフセット/ダイジェスト。

セキュリティ セキュリティ (ダイジェスト、PoR ハッシュ、マニフェスト) セキュリティ セキュリティ
और देखें

## قائمة تحقق الحتمية والتحقق

1. **試合の試合**
   ```bash
   cargo run --locked -p sorafs_chunker --bin export_vectors \
     --signature-out=fixtures/sorafs_chunker/manifest_signatures.json
   ```
2. **テスト結果** — テスト結果 `cargo test -p sorafs_chunker` とハーネスの差分
   (`crates/sorafs_chunker/tests/vectors.rs`) フィクスチャーを確認してください。
3. **コーパス ファズ/バック プレッシャー** — `cargo fuzz list` およびハーネス
   (`fuzz/sorafs_chunker`) ログインしてください。
4. **取得可能性の証明** — 評価
   `sorafs_manifest_chunk_store --por-sample=<n>` 認証済み 認証済み 認証済み 認証済み
   マニフェストの備品。
5. **予行演習 للـ CI** — شغّل `ci/check_sorafs_fixtures.sh` محلياً؛ヤステル
   備品は `manifest_signatures.json` です。
6. **クロスランタイム** — Go/TS يستهلك JSON المُعاد توليده ويُخرج
   チャンクとダイジェスト。ツール WG のダイジェストを確認してください。

### マニフェスト/PoR

フィクスチャーの確認 マニフェストの確認 CAR の PoR の確認:

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

ستبدل ملف الإدخال بأي corpus مستخدم في fixtures الخاصة بك
(ストリーム ストリーム 1 GiB) は、ストリームをダイジェストします。

## いいえ

Norito من نوع `ChunkerProfileProposalV1` محفوظة ضمن
`docs/source/sorafs/proposals/`。 يوضح قالب JSON أدناه الشكل المتوقع
(アオアシ):


マークダウン マークダウン (`determinism_report`) セキュリティ チャンク ダイジェスト
और देखें

## سير عمل الحوكمة

1. **تقديم PR مع المقترح + fixtures.** ضمّن الأصول المولدة، مقترح Norito، وتحديثات
   `chunker_registry_data.rs`。
2. **ツール WG.** セキュリティ WG の開発
   يتوافق مع قواعد السجل (لا إعادة لاستخدام المعرفات، الحتمية متحققة)。
3. ************************************************************************************************************************
   (`blake3("sorafs-chunker-profile-v1" || canonical_bytes)`) ويضيفون توقيعاتهم إلى
   試合の備品。
4. ** フィクスチャ。** フィクスチャ。 CLI アプリケーション
   お金を節約してください。
   移行台帳。

## ナオミ

- チャンク処理を実行する必要があります。
- マルチハッシュとゲートウェイのマルチハッシュأضف ملاحظة توافق عند ذلك.
- シードとギアの組み合わせ。
- アーティファクトの数 (スループット)
  `docs/source/sorafs/reports/` です。

移行元帳のロールアウト
(`docs/source/sorafs/migration_ledger.md`)。 قواعد المطابقة وقت التشغيل راجع
`docs/source/sorafs/chunker_conformance.md`。