---
lang: ja
direction: ltr
source: docs/portal/docs/sorafs/chunker-registry-charter.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: チャンカーレジストリチャーター
タイトル: SoraFS チャンカー レジストリ憲章
Sidebar_label: チャンカー レジストリ憲章
説明: チャンカー プロファイルの提出と承認、ガバナンス憲章
---

:::note メモ
:::

# SoraFS チャンカー レジストリ ガバナンス憲章

> **批准:** 2025-10-29 ソラ議会インフラパネルによる (参照)
> `docs/source/sorafs/council_minutes_2025-10-29.md`)。ガバナンス وٹ درکار ہے؛
> 実装 ٹیموں کو اس دستاویز کو اس وقت تک 規範 سمجھنا ہوگا جب تک کوئی نئی 憲章 منظور نہ ہو۔

チャーター SoraFS チャンカー レジストリ 進化する プロセス ロールを定義する
[チャンカー プロファイル オーサリング ガイド](./chunker-profile-authoring.md) ٩ی تکمیل کرتی ہے اور یہ بیان کرتی ہے کہ نئے
プロフィール 提案する レビューする 承認する 廃止する

## 範囲

یہ チャーター `sorafs_manifest::chunker_registry` کی ہر エントリー پر لاگو ہے اور
ツール、レジストリ、マニフェスト CLI、プロバイダー広告 CLI、
SDK)。 یہ エイリアス ハンドル不変条件を強制する ہے جنہیں
`chunker_registry::ensure_charter_compliance()` 日:

- プロファイル ID 整数 ہوتے ہیں جو 単調 طور پر بڑھتے ہیں۔
- 正規ハンドル `namespace.name@semver` **لازم** ہے کہ `profile_aliases` میں پہلی
- エイリアス文字列のトリム 固有の文字列 エントリ 正規ハンドル 衝突 衝突

## 役割

- **著者** – 提案書 フィクスチャーの再生成 決定論的証拠 جمع کرتے ہیں۔
- **ツール ワーキング グループ (TWG)** – 公開チェックリスト 提案の検証 レジストリの不変条件の検証
- **ガバナンス評議会 (GC)** – TWG レポートのレビュー 提案書の封筒 署名 公開/非推奨スケジュールの承認 承認
- **ストレージ チーム** – レジストリ実装の保守、ドキュメントの更新、公開

## ライフサイクル ワークフロー

1. **提案書の提出**
   - 著者オーサリング ガイド 検証チェックリスト
     `docs/source/sorafs/proposals/` 価 `ChunkerProfileProposalV1` JSON 価数
   - CLI の出力結果:
     ```bash
     cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- --list-profiles
     cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- \
       --promote-profile=<handle> --json-out=-
     cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- \
       --chunker-profile=<handle> --json-out=-
     ```
   - 日程、提案、決定性レポート、レジストリの更新、PR の送信

2. **ツーリングのレビュー (TWG)**
   - 検証チェックリスト (フィクスチャ、ファズ、マニフェスト/PoR パイプライン)。
   - `cargo test -p sorafs_car --chunker-registry` چلائیں اور یقینی بنائیں کہ
     `ensure_charter_compliance()` エントリ کے ساتھ پاس ہو۔
   - CLI 動作 (`--list-profiles`、`--promote-profile`、ストリーミング)
     `--json-out=-`) کی توثیق کریں کہ یہ 更新されたエイリアス اور ハンドル دکھاتا ہے۔
   - 所見の合否ステータスの確認

3. **評議会の承認 (GC)**
   - TWG レポート、提案メタデータ、レビュー、レビュー
   - 提案ダイジェスト (`blake3("sorafs-chunker-profile-v1" || bytes)`) 記号
     署名 議会の封筒 میں شامل کریں جو 備品 کے ساتھ رکھا جاتا ہے۔
   - 投票結果とガバナンス議事録

4. **出版物**
   - PR マージ:
     - `sorafs_manifest::chunker_registry_data`。
     - ドキュメント (`chunker_registry.md`、オーサリング/準拠ガイド)。
     - 試合結果の決定性レポート。
   - オペレーター SDK チームのプロフィール 計画されたロールアウトの概要

5. **非推奨/廃止**
   - 提案、プロフィール、置き換え、デュアル公開ウィンドウ
     (猶予期間) アップグレード計画 شامل ہونا چاہیے۔
     移行台帳を更新する

6. **緊急の変更**
   - ホットフィックスの削除 議会の投票 承認 承認
   - TWG リスク軽減手順文書、インシデント ログの更新、およびリスク軽減手順の文書化

## 期待されるツール

- `sorafs_manifest_chunk_store` は `sorafs_manifest_stub` を公開します:
  - レジストリ検査 `--list-profiles`。
  - プロファイルは、正規メタデータ ブロック、`--promote-profile=<handle>` をプロモートします。
  - レポート標準出力ストリーム `--json-out=-` 再現可能なレビュー ログ
- `ensure_charter_compliance()` 関連バイナリの起動
  (`manifest_chunk_store`、`provider_advert_stub`)。 CI テストが失敗しました
  エントリ憲章 خلاف ورزی کریں۔

## 記録の保管

- 決定論レポート کو `docs/source/sorafs/reports/` میں محفوظ کریں۔
- チャンカー فیصلوں کا حوالہ دینے والی 議会議事録
  `docs/source/sorafs/migration_ledger.md` میں موجود ہیں۔
- レジストリの変更 `roadmap.md` اور `status.md` پڈیٹ کریں۔

## 参考文献

- オーサリングガイド: [チャンカープロファイルオーサリングガイド](./chunker-profile-authoring.md)
- 適合性チェックリスト: `docs/source/sorafs/chunker_conformance.md`
- レジストリ参照: [チャンカー プロファイル レジストリ](./chunker-registry.md)