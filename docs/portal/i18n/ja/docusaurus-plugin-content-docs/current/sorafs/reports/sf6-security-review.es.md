---
lang: ja
direction: ltr
source: docs/portal/docs/sorafs/reports/sf6-security-review.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
タイトル: 改訂版 SF-6
概要: キーレス署名、プルーフ ストリーミング、マニフェスト環境のパイプラインの独立した評価に関するセキュリティ。
---

# 改訂版 SF-6

**ベンタナの評価:** 2026-02-10 -> 2026-02-18  
**改訂リーダー:** Security Engineering Guild (`@sec-eng`)、Tooling Working Group (`@tooling-wg`)  
**Alcance:** SoraFS CLI/SDK (`sorafs_cli`、`sorafs_car`、`sorafs_manifest`)、API 実証ストリーミング、Torii のマニフェストの管理、統合Sigstore/OIDC、CI でのフックのリリース。  
**アーティファクト:**  
- CLI テスト (`crates/sorafs_car/src/bin/sorafs_cli.rs`)  
- Torii (`crates/iroha_torii/src/sorafs/api.rs`) のマニフェスト/プルーフ ハンドラー  
- リリースの自動化 (`ci/check_sorafs_cli_release.sh`、`scripts/release_sorafs_cli.sh`)  
- ハーネス・デ・パリダード・デターミニスタ (`crates/sorafs_car/tests/sorafs_cli.rs`、[Reporte de Paridad GA del Orchestrator SoraFS](./orchestrator-ga-parity.md))

## メトドロギア

1. **脅威モデリングのワークショップ** 開発者向けの管理システム、システム CI およびノード Torii のアタカンテス マップ。  
2. **コード レビュー** 資格情報の管理 (トークン OIDC、キーレス署名)、マニフェスト Norito の検証、およびバックプレッシャーとプルーフ ストリーミング。  
3. **ディナミコのテスト** マニフェスト デ フィクスチャとシミュレーション モード (トークン リプレイ、マニフェスト改ざん、プルーフ ストリーム トランカド) を再現し、パリダーとファズを使用してメディダを駆動します。  
4. **構成の検査** `iroha_config` の有効なデフォルト、CLI およびスクリプトのフラグ管理、リリースパラメタの監査可能性の決定。  
5. **Entrevista de proceso** は、ツール WG の所有者とリリースの聴覚障害者に対する修復、エスカラミエントの記録および証拠の収集を確認します。

## ハラスゴスの履歴書

| ID |セベリダッド |エリア |ハラズゴ |解決策 |
|----|----------|------|----------|------------|
| SF6-SR-01 |アルタ |キーレス署名 |トークン OIDC のデフォルトは、CI テンプレートの暗黙的なものであり、再生エンターテナントと協議します。 | CI のリリース テンプレートのフックで `--identity-token-audience` のアプリケーションを明示的に確認します ([リリース プロセス](../developer-releases.md)、`docs/examples/sorafs_ci.md`)。 CI アホラ フォールラ シ セ オミテ ラ オーディエンシア。 |
| SF6-SR-02 |メディア |プルーフストリーミング |背圧を受け入れ、罪の限界を緩和し、記憶を取り戻すことができます。 | `sorafs_cli proof stream` 決定的な決定を下すため、運河のタマノスを破棄し、登録を再開します。 Norito ストリームを中止します。 Torii は実際のアコタル チャンク デ レスペスタ (`crates/iroha_torii/src/sorafs/api.rs`) です。 |
| SF6-SR-03 |メディア |マニフェスト環境 | El CLI の aceptaba は、チャンクの検証面、`--plan` estaba ausente を明示します。 | `sorafs_cli manifest submit` CAR サルボの再計算と比較ダイジェストの証明 `--expect-plan-digest`、修復の不一致と修正のほとんどのピスタ。 Los テスト cubren casos de exito/falla (`crates/sorafs_car/tests/sorafs_cli.rs`)。 |
| SF6-SR-04 |バハ |監査証跡 |セキュリティ チェックリストのリリース チェックリストは、セキュリティ ポリシーの改訂に関する承認申請の記録を作成します。 | [リリース プロセス](../developer-releases.md) では、GA のサインオフ前にリビジョンのメモと URL の付属ハッシュが必要です。 |

高い/中程度のセキュリティ デュランテ ラ ベンタナ デ リビジョンとセキュリティ セキュリティ コン エル ハーネスの存在を確認してください。潜在的な批判を提起するクエダンはいない。

## コントロールの検証- **資格情報:** CI および発行者の明示的な監査テンプレートの損失。 CLI およびヘルパーは、`--identity-token-provider` に付随する落下速度一斉射撃 `--identity-token-audience` をリリースします。  
- **決定的な再生:** 現実の状況をテストし、実際の状況をテストし、実際の状況を確認し、結果をダイジェストし、最終的な結果を検出します。  
- **バック プレッシャーとプルーフ ストリーミング:** Torii アホラ送信アイテム PoR/PoTR の sobre canales acotados、Y el CLI retene Solo muestras truncadas de latencia + cinco ejemplos de falla、evitando crecimiento sin limite y manteniendoresumenes deterministas。  
- **観察結果:** プルーフ ストリーミング (`torii_sorafs_proof_stream_*`) のコンタドールは、CLI の再開で、アボート、オーディトリアのパンくずリストをキャプチャします。  
- **ドキュメント:** 開発者向けガイド ([開発者インデックス](../developer-index.md)、[CLI リファレンス](../developer-cli.md)) は、エスカラミエントのセキュリティ ワークフローを考慮したフラグを示します。

## リリースのチェックリストに追加

ロサンゼルスのリリースマネージャー**デベン**は、GA候補者の証拠となる証拠を提出します:

1. 安全な文書の改訂版をハッシュします。  
2. 救済策のチケットをリンクします (例、`governance/tickets/SF6-SR-2026.md`)。  
3. オーディオ/発行者の明示的な `scripts/release_sorafs_cli.sh --manifest ... --bundle-out ... --signature-out ...` 引数の出力。  
4. ハーネスのキャプチャをログに記録します (`cargo test -p sorafs_car -- --nocapture sorafs_cli::proof_stream::bounded_channels`)。  
5. Torii のリリース ノートには、テレメトリのコンタドールと証明ストリーミング アコタドが含まれています。

GA のサインオフで前方のブロックを収集する必要はありません。

**参照成果物のハッシュ (承認 2026-02-20):**

- `sf6_security_review.md` — `66001d0b53d8e7ed5951a07453121c075dea931ca44c11f1fcd1571ed827342a`

## セギミエントス・ペンディエンテス

- **脅威モデルの実際:** CLI でフラグを立てる前に、リビジョン 3 回を繰り返します。  
- **ファジングのコベルトゥーラ:** `fuzz/proof_stream_transport` 経由での転送および証明ストリーミングのエンコーディングの損失、ペイロード ID、gzip、deflate、zstd の暗号化。  
- **事件の対処:** シミュレート トークン侵害とマニフェストのロールバックによる緊急プログラムのプログラマ、文書の参照手順の確認。

## アプロバシオン

- セキュリティ エンジニアリング ギルド代表: @sec-eng (2026-02-20)  
- ツーリングワーキンググループの代表者: @tooling-wg (2026-02-20)

すべての成果物をリリースするために必要なすべての企業が公開されます。