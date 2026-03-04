---
lang: ja
direction: ltr
source: docs/portal/docs/sorafs/reports/sf6-security-review.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
タイトル: 安全保障レビュー SF-6
概要: 署名なしで独立した評価と管理、マニフェストの監視ストリーミングとパイプラインの検証。
---

# 安全保障レビュー SF-6

**評価の評価:** 2026-02-10 → 2026-02-18  
**レビュー責任者:** セキュリティ エンジニアリング ギルド (`@sec-eng`)、ツール ワーキング グループ (`@tooling-wg`)  
**ペリメトル:** SoraFS CLI/SDK (`sorafs_cli`、`sorafs_car`、`sorafs_manifest`)、ストリーミングの API、Torii のマニフェストの取得、統合Sigstore/OIDC、CI の解放フック。  
**アーティファクト:**  
- ソース CLI とテスト (`crates/sorafs_car/src/bin/sorafs_cli.rs`)  
- ハンドラーのマニフェスト/証明 Torii (`crates/iroha_torii/src/sorafs/api.rs`)  
- 自動化リリース (`ci/check_sorafs_cli_release.sh`、`scripts/release_sorafs_cli.sh`)  
- 決定的なパリティのハーネス (`crates/sorafs_car/tests/sorafs_cli.rs`、[パリティの関係 GA SoraFS オーケストレーター](./orchestrator-ga-parity.md))

## 方法論

1. **脅威モデリングのアトリエ** は、開発者の投稿、システム CI および新しい Torii の地図作成機能を備えています。  
2. **コード レビュー** 識別情報の表面 (トークン OIDC の変更、透明な署名)、マニフェスト Norito の検証、およびプルーフ ストリーミングのバック プレッシャー。  
3. **テスト ダイナミクス** は、ハーネス ド パリテおよびファズ ドライブ デディを介して、フィクスチャのマニフェストとパンニング モードのシミュレート (トークン リプレイ、マニフェスト改ざん、プルーフ ストリーム トロンケ) をテストします。  
4. **構成の検査** デフォルト `iroha_config` の有効性、CLI のフラグおよびリリース保証の実行決定および監査可能性のあるスクリプトの監視。  
5. **プロセスの開始** 修復の流動性の確認、エスカレードの管理、およびツーリング WG の所有者によるリリースの証拠および監査のキャプチャ。

## 統計の履歴書

| ID |セヴェリテ |ゾーン |コンスタット |解像度 |
|----|----------|------|----------|------------|
| SF6-SR-01 |エレヴェ |シグネチャー サンクレ |トークン OIDC のデフォルトは、テンプレート CI を暗黙的に示し、テナント間での危険なリプレイを意味します。 |リリースとテンプレートのフックの明示的な `--identity-token-audience` に注意してください ([リリース プロセス](../developer-releases.md)、`docs/examples/sorafs_ci.md`)。 CI échoue désormais si l'audience est omise。 |
| SF6-SR-02 |モエンヌ |プルーフストリーミング |制限のない加入者のバッファーに対するバックプレッシャーの受け入れ、永続的な記録の記録。 | `sorafs_cli proof stream` チャンネルの結果を強制し、切り捨てを決定し、履歴を記録します。 Norito またはストリームを中止します。 le miroir Torii 応答チャンク (`crates/iroha_torii/src/sorafs/api.rs`) を確認してください。 |
| SF6-SR-03 |モエンヌ |マニフェスト使節 | CLI は、検証者なしでマニフェストを受け入れ、チャンク プランは、`--plan` が存在しません。 | `sorafs_cli manifest submit` CAR のダイジェストを再計算して比較します。 `--expect-plan-digest` 4 つを調べ、不一致と修復のヒントを説明します。成功/診断テストのテスト (`crates/sorafs_car/tests/sorafs_cli.rs`)。 |
| SF6-SR-04 |フェイブル |監査証跡 |リリースのチェックリストは、安全性を確認するための承認署名を回避する必要はありません。 |セクション [リリース プロセス](../developer-releases.md) の添付ファイルのハッシュとレビューのメモとサインオフのチケットの URL を削除します。 |

Tous les constats high/medium オンテ コリジェ ペンダント ラ フェネートル ドゥ レビューと有効性パー ル ハーネス ドゥ パリテが存在します。 Aucun 問題の潜在的なネ レストを批判します。

## 制御の検証- **識別情報のポートレート:** 対象者と発行者に明示的に適用される CI のテンプレート。 CLI およびリリースのヘルパーは、`--identity-token-provider` に準拠した `--identity-token-audience` を高速にリリースします。  
- **決定的な再生:** マニフェストの結果を示すテストや否定的なテスト、不一致のテスト、非決定的なテスト、および事前に確認された結果のダイジェストを保証します。  
- **バックプレッシャー プルーフ ストリーミング :** Torii は、チャネル ボーン、および CLI 経由で PoR/PoTR アイテムを拡散し、遅延トロンケス + サンプル デシェックを保存し、履歴書を制限することなくクロワッサンスを回避します。決定論者。  
- **監視可能性:** 証明ストリーミング (`torii_sorafs_proof_stream_*`) および履歴 CLI のキャプチャ、レゾン ダン、監査操作のパンくずリスト。  
- **ドキュメント :** 開発ガイド ([開発者インデックス](../developer-index.md)、[CLI リファレンス](../developer-cli.md)) シグナル、フラグ、賢明なワークフロー、エスカレード。

## リリースのチェックリストにかかる手順

GA のプロモーションを担当するリリース マネージャー **doivent** が参加します:

1. 安全性と最新のレビューのメモ (CE ドキュメント)。  
2. 救済チケットの先取特権と権利 (例: `governance/tickets/SF6-SR-2026.md`)。  
3. `scripts/release_sorafs_cli.sh --manifest ... --bundle-out ... --signature-out ...` モントラントの引数の対象者/発行者が明示的に示す出力。  
4. ハーネスのキャプチャをログに記録します (`cargo test -p sorafs_car -- --nocapture sorafs_cli::proof_stream::bounded_channels`)。  
5. リリース ノート Torii には、ストリーミングの証明に関するテストの内容が含まれています。

コレクターがアーティファクトを作成し、承認ブロックを作成する必要があります。

**アーティファクトの参照ハッシュ (2026 年 2 月 20 日の承認) :**

- `sf6_security_review.md` — `66001d0b53d8e7ed5951a07453121c075dea931ca44c11f1fcd1571ed827342a`

## スイビス アン アテント

- **脅威モデルの確認:** フラグ CLI のレビューを確認します。  
- **クーベルチュールファジング:** `fuzz/proof_stream_transport`、クーベルチュールアイデンティティ、gzip、deflate などの zstd を介したトランスポートプルーフストリーミングソンントファジングのコード。  
- **インシデントの繰り返し:** 計画立案者は、トークンの侵害やマニフェストのロールバックをシミュレートする操作を実行し、ドキュメントを参照して手順の実践を保証します。

## 承認

- セキュリティ エンジニアリング ギルド代表 : @sec-eng (2026-02-20)  
- ツーリングワーキンググループ代表：@tooling-wg (2026-02-20)

Conserver les approbationssignées avec le Bundle d'artefacts de release.