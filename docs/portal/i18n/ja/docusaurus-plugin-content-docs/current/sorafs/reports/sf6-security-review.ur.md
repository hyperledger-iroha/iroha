---
lang: ja
direction: ltr
source: docs/portal/docs/sorafs/reports/sf6-security-review.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
タイトル: SF-6 SF-6
概要: キーレス署名、証明ストリーミング、マニフェスト、パイプライン、追跡、追跡
---

#SF-6 और देखें

**評価期間:** 2026-02-10 → 2026-02-18  
**レビューリーダー:** Security Engineering Guild (`@sec-eng`)、Tooling Working Group (`@tooling-wg`)  
**範囲:** SoraFS CLI/SDK (`sorafs_cli`、`sorafs_car`、`sorafs_manifest`)、証明ストリーミング API、Torii マニフェスト処理、 Sigstore/OIDC 統合、CI リリース フック  
**アーティファクト:**  
- CLI ソースのテスト (`crates/sorafs_car/src/bin/sorafs_cli.rs`)  
- Torii マニフェスト/プルーフ ハンドラー (`crates/iroha_torii/src/sorafs/api.rs`)  
- リリースの自動化 (`ci/check_sorafs_cli_release.sh`、`scripts/release_sorafs_cli.sh`)  
- 確定的パリティ ハーネス (`crates/sorafs_car/tests/sorafs_cli.rs`、[SoraFS Orchestrator GA パリティ レポート](./orchestrator-ga-parity.md))

## 方法論

1. **脅威モデリング ワークショップ** 開発者ワークステーション、CI システム、Torii ノード、攻撃者の能力マップ  
2. **コード レビュー** 資格情報面 (OIDC トークン交換、キーレス署名)、Norito マニフェスト検証、証明ストリーミング バック プレッシャー  
3. **動的テスト** フィクスチャ マニフェスト リプレイ 故障モード シミュレート (トークン リプレイ、マニフェスト改ざん、切り捨てられたプルーフ ストリーム) パリティ ハーネス 特注ファズ ドライブ  
4. **構成の検査** `iroha_config` のデフォルト、CLI フラグの処理、リリース スクリプトの検証、確定的、監査可能な実行  
5. **インタビューのプロセス** 修復フロー、エスカレーション パス、監査証拠の収集、ツール WG、所有者のリリース、確認、

## 調査結果の概要

| ID |重大度 |エリア |発見 |解像度 |
|----|----------|------|----------|------------|
| SF6-SR-01 |高 |キーレス署名 | OIDC トークン オーディエンス デフォルト CI テンプレート 暗黙的 クロステナント リプレイ|リリース フック、CI テンプレート、`--identity-token-audience`、明示的な強制、([リリース プロセス](../developer-releases.md)、`docs/examples/sorafs_ci.md`)。視聴者省略 ہونے پر CI اب 失敗 ہوتا ہے۔ |
| SF6-SR-02 |中 |プルーフストリーミング |バックプレッシャー パスと無制限のサブスクライバ バッファによるメモリ枯渇の発生| `sorafs_cli proof stream` 制限されたチャネル サイズにより、決定論的な切り捨てが強制されます。 Norito 要約ログが適用されます。 ストリームの中止が行われます。 Torii 応答チャンクをミラーリングします (`crates/iroha_torii/src/sorafs/api.rs`)。 |
| SF6-SR-03 |中 |マニフェストの提出 | CLI マニフェスト 埋め込みチャンク プランの検証 `--plan` موجود نہ تھا۔ | `sorafs_cli manifest submit` CAR ダイジェストの計算 比較の計算 `--expect-plan-digest` 不一致の拒否 修復のヒントدکھاتا ہے۔テストの成功/失敗のケースは、(`crates/sorafs_car/tests/sorafs_cli.rs`) をカバーしています。 |
| SF6-SR-04 |低い |監査証跡 |リリースチェックリスト سیکیورٹی ریویو کے لیے 署名済み承認ログ شامل نہیں تھا۔ | [リリースプロセス](../developer-releases.md) میں سیکشن شامل کیا گیا جو レビューメモハッシュ اور サインオフチケット URL کو GA سے پہلے Attach کرنے کا تقاضا کرتا ❁❁❁❁ |

高/中程度の所見のレビュー ウィンドウ 修正 パリティ ハーネス 検証 検証潜在的な重大な問題

## コントロールの検証

- **資格情報の範囲:** デフォルトの CI テンプレート、明示的な対象ユーザー、発行者のアサーションCLI リリース ヘルパー `--identity-token-audience` ٩ے بغیر `--identity-token-provider` کے 早く失敗します ہوتے ہیں۔  
- **決定論的リプレイ:** 更新されたテストの肯定的/否定的なマニフェスト提出フローは、テストをカバーし、不一致のダイジェストをカバーし、非決定的な失敗をカバーします。 پہلے 表面 ہوں۔  
- **ストリーミング バック プレッシャーの証明:** Torii PoR/PoTR アイテムと制限付きチャネル、ストリーム ストリーム、切り捨てられたレイテンシ サンプル + 障害サンプル、無制限の加入者の増加決定論的要約 برقرار رکھتا ہے۔  
- **可観測性:** プルーフ ストリーミング カウンター (`torii_sorafs_proof_stream_*`) CLI 概要、中止理由キャプチャ、オペレーター、監査ブレッドクラム、監査ブレッドクラム  
- **ドキュメント:** 開発者ガイド ([開発者インデックス](../developer-index.md)、[CLI リファレンス](../developer-cli.md)) セキュリティに依存するフラグ エスカレーション ワークフロー

## リリースチェックリストの追加

リリースマネージャーが GA 候補者を昇進させるための証拠 ** を添付してください:1. ترین سیکیورٹی ریویو メモ کا ハッシュ (یہ دستاویز)۔  
2. 追跡された修復チケットのリンク (メッセージ: `governance/tickets/SF6-SR-2026.md`)  
3. `scripts/release_sorafs_cli.sh --manifest ... --bundle-out ... --signature-out ...` 出力 明示的な対象者/発行者の引数 説明  
4. パリティ ハーネスのキャプチャされたログ (`cargo test -p sorafs_car -- --nocapture sorafs_cli::proof_stream::bounded_channels`)  
5. Torii リリース ノート 有界証明ストリーミング テレメトリ カウンタの説明

アーティファクトを確認する GA サインオフを確認する

**参照アーティファクト ハッシュ (2026-02-20 サインオフ):**

- `sf6_security_review.md` — `66001d0b53d8e7ed5951a07453121c075dea931ca44c11f1fcd1571ed827342a`

## 優れたフォローアップ

- **脅威モデルの更新:** یہ ریویو ہر سہ ماہی یا بڑے CLI フラグの追加 سے پہلے دوبارہ کریں۔  
- **ファジング対象範囲:** ストリーミング トランスポート エンコーディングの証明、`fuzz/proof_stream_transport`、ファズ、アイデンティティ、gzip、デフレート、zstd ペイロード、カバー、サポート。  
- **インシデントのリハーサル:** トークン侵害、マニフェストのロールバック、シミュレート、オペレーターの演習、ドキュメントの作成、実践された手順の反映

## 承認

- Security Engineering Guild 代表者: @sec-eng (2026-02-20)  
- ツーリングワーキンググループ代表：@tooling-wg (2026-02-20)

署名済みの承認がアーティファクト バンドルをリリースする必要があります。