---
lang: ja
direction: ltr
source: docs/portal/docs/sorafs/reports/sf6-security-review.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
タイトル: レヴィサオ・デ・セグランカ SF-6
概要: キーレス署名、証明ストリーミング、マニフェスト環境のパイプラインを独立して利用できます。
---

# レビサオ・デ・セグランカ SF-6

**Janela de avaliacao:** 2026-02-10 -> 2026-02-18  
**修正内容:** Security Engineering Guild (`@sec-eng`)、Tooling Working Group (`@tooling-wg`)  
**Escopo:** SoraFS CLI/SDK (`sorafs_cli`、`sorafs_car`、`sorafs_manifest`)、プルーフ ストリーミング API、Torii マニフェスト処理、integracao Sigstore/OIDC、CI リリース フック。  
**アーティファクト:**  
- CLI テストを行う (`crates/sorafs_car/src/bin/sorafs_cli.rs`)  
- Torii マニフェスト/プルーフ ハンドラー (`crates/iroha_torii/src/sorafs/api.rs`)  
- リリースの自動化 (`ci/check_sorafs_cli_release.sh`、`scripts/release_sorafs_cli.sh`)  
- 確定的パリティ ハーネス (`crates/sorafs_car/tests/sorafs_cli.rs`、[SoraFS Orchestrator GA パリティ レポート](./orchestrator-ga-parity.md))

## メトドロギア

1. **脅威モデリング ワークショップ** 開発者向けのワークステーション、システム CI ノード Torii のマップの容量。  
2. **コード レビュー** 資格情報の表面 (OIDC トークン交換、キーレス署名) に焦点を当て、Norito マニフェストのバックプレッシャーおよび証明ストリーミングを検証します。  
3. **動的テスト** 再実行フィクスチャ マニフェストと擬似障害モード (トークン リプレイ、マニフェスト改ざん、プルーフ ストリーム トランカド) を使用し、パリティ ハーネスとファズを使用してすすり泣くメディアを駆動します。  
4. **構成検査** 有効なデフォルト値は `iroha_config`、CLI フラグ処理およびリリース スクリプトは、確実な監査の実行を保証します。  
5. **プロセスインタビュー** 修復フロー、エスカレーションパス、監査証拠の収集、リリース所有者がツール WG を行うことを確認します。

## 調査結果の概要

| ID |重大度 |エリア |発見 |解像度 |
|----|----------|------|----------|------------|
| SF6-SR-01 |高 |キーレス署名 | OIDC トークン オーディエンスは、クロステナント再生と関連して、CI テンプレートの暗黙的な設定をデフォルトに設定します。 | `--identity-token-audience` リリース フックおよび CI テンプレート ([リリース プロセス](../developer-releases.md)、`docs/examples/sorafs_ci.md`) の適用を明示します。 O CI アゴラ ファルハ Quando a 聴衆と省略。 |
| SF6-SR-02 |中 |プルーフストリーミング |バックプレッシャ パスは、サブスクライバのバッファを最小限に抑え、メモリの枯渇を許可します。 | `sorafs_cli proof stream` アプリケーションのチャネル サイズの制限、通信の決定性、レジストラ Norito 要約、ストリームの中止。 o Torii は、リミッター応答チャンク (`crates/iroha_torii/src/sorafs/api.rs`) のパラメータをミラーリングします。 |
| SF6-SR-03 |中 |マニフェストの提出 | O CLI aceitava マニフェスト sem verificar 埋め込みチャンク プラン quando `--plan` estava ausente。 | `sorafs_cli manifest submit` CAR がメノス クエリを再計算し、比較します。 `--expect-plan-digest` フォルネシド、レジェタンドの不一致、および Exibindo 修復ヒントが表示されます。 cobrem casos de sucesso/falha をテストします (`crates/sorafs_car/tests/sorafs_cli.rs`)。 |
| SF6-SR-04 |低い |監査証跡 | O リリースチェックリスト nao tinha um は、改訂版の承認ログに署名しました。 | [リリース プロセス](../developer-releases.md) に関連するハッシュを確認し、メモと URL を確認して、GA のチケットとサインオフを行ってください。 |

高/中型のコルリジドス デュランテ、ジャネラ デ リヴィサオ、バリダドス ペロ パリティ ハーネスが存在します。ネンハムは潜在的な永続性を批判します。

## コントロールの検証

- **資格情報の範囲:** CI テンプレートを対象者と発行者の明示的に定義します。 o CLI e o リリース ヘルパー falham Rapido a menos que `--identity-token-audience` acompanhe `--identity-token-provider`。  
- **決定論的リプレイ:** マニフェスト提出時の肯定的/否定的なテストを行い、不一致のダイジェストを保証し、継続的に送信されたファルハスと決定性を確認し、事前に説明します。  
- **プルーフ ストリーミング バック プレッシャー:** Torii は、PoR/PoTR の制限を決定する前にストリームを決定し、CLI 保持アペナス レイテンシ サンプル トランカド + シンコ障害の例、プレベニンド クレッシメント セム リミットおよびマンテンド サマリーを決定します。  
- **可観測性:** プルーフ ストリーミング カウンター (`torii_sorafs_proof_stream_*`) CLI は中止理由をキャプチャし、監査ブレッドクラム AOS オペランドを要約します。  
- **ドキュメント:** 開発者ガイド ([開発者インデックス](../developer-index.md)、[CLI リファレンス](../developer-cli.md)) destacam フラグは重要なセキュリティ エスカレーション ワークフローです。

## リリースチェックリストの追加

リリース マネージャー **開発者** は、GA 候補者と継続的な証拠を作成します。1. 最近のセキュリティ レビュー メモをハッシュします (este documento)。  
2. 修復チケットの rastreado パラメータをリンクします (例: `governance/tickets/SF6-SR-2026.md`)。  
3. 対象者/発行者明示的な `scripts/release_sorafs_cli.sh --manifest ... --bundle-out ... --signature-out ...` のほとんどの引数を出力します。  
4. パリティ ハーネスを実行するログ キャプチャ (`cargo test -p sorafs_car -- --nocapture sorafs_cli::proof_stream::bounded_channels`)。  
5. リリース ノートに従って、Torii に有界証明ストリーミング テレメトリ カウンタが含まれていることを確認します。

ナオ コレタール オス アーティファクト アシマ ブロケイア サインオフ デ GA。

**参照アーティファクト ハッシュ (承認 2026-02-20):**

- `sf6_security_review.md` - `66001d0b53d8e7ed5951a07453121c075dea931ca44c11f1fcd1571ed827342a`

## 優れたフォローアップ

- **脅威モデルの更新:** CLI フラグのトリメストラルメントを更新します。  
- **ファジング範囲:** `fuzz/proof_stream_transport`、cobrindo ペイロード ID、gzip、deflate、zstd 経由でファジングされたストリーミング トランスポート エンコーディングの証明。  
- **インシデントのリハーサル:** トークンの侵害とマニフェストのロールバックに関するオペラの実行計画、実際の手順の文書化を保証します。

## 承認

- Security Engineering Guild 代表者: @sec-eng (2026-02-20)  
- ツーリングワーキンググループ代表：@tooling-wg (2026-02-20)

Guarde はアーティファクト バンドルをリリースすることを承認しました。