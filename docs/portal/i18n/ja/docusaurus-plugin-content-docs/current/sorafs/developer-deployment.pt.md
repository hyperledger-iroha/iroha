---
lang: ja
direction: ltr
source: docs/portal/docs/sorafs/developer-deployment.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: 開発者デプロイメント
タイトル: SoraFS の展開ノート
Sidebar_label: デプロイメントに関する注意事項
説明: CI パラプロダクションのパイプライン SoraFS のチェックリスト。
---

:::note フォンテ カノニカ
エスタ・ページナ・エスペルハ`docs/source/sorafs/developer/deployment.md`。マンテンハ・アンバスはコピア・シンクロニザダスとして。
:::

# デプロイメントに関する注意事項

SoraFS の決定性を維持するためのワークフローでは、CI パラメーションの作成に必要な主要なガードレール オペレーションを実行します。ゲートウェイのフェラメントや軍事安全の証明として、エスタ チェックリストを使用してください。

## 飛行前

- **レジストリの登録** - チャンカーのパフォーマンスのマニフェスト参照を確認し、メスマ トゥプラ `namespace.name@semver` (`docs/source/sorafs/chunker_registry.md`) を確認します。
- **入学許可の政治** - `manifest submit` (`docs/source/sorafs/provider_admission_policy.md`) に必要な OS プロバイダーの広告やエイリアス証明を改訂します。
- **Runbook do pin registry** - mantenha `docs/source/sorafs/runbooks/pin_registry_ops.md` por perto para cenarios de recuperacao (ロタカオ デ エイリアス、ファルハス デ レプリカカオ)。

## アンビエンテを設定する

- ゲートウェイは、ストリーミングのエンドポイントの使用を可能にし (`POST /v2/sorafs/proof/stream`)、CLI からテレメトリの履歴を送信します。
- politica `sorafs_alias_cache` usando ospadroes em `iroha_config` ou o helper do CLI (`sorafs_cli manifest submit --alias-*`) を設定します。
- Forneca ストリーム トークン (ou credenciais Torii) シークレット マネージャー segro 経由。
- テレメトリアの Habilite エクスポータ (`torii_sorafs_proof_stream_*`、`torii_sorafs_chunk_range_*`) スタック Prometheus/OTel を羨望します。

## 展開戦略

1. **青/緑で表示されます**
   - `manifest submit --summary-out` パラメータ検索ロールアウトを使用します。
   - `torii_sorafs_gateway_refusals_total` パラ キャプターの不一致とキャパシダーデ セドを観察します。
2. **証明の検証**
   - 展開時に `sorafs_cli proof stream` を実行します。遅延コストのインディカル スロットルが層の構成で有効であることが証明されています。
   - `proof verify` デブ・ファザー・パートは、煙テストのポスト・ピン・パラ・ギャランティール・ク・オ・カー・ホスペダド・ペロス・プロヴェドーレス・アインダ・コレスポンデ・アオ・ダイジェストをマニフェストします。
3. **テレメトリのダッシュボード**
   - `docs/examples/sorafs_proof_streaming_dashboard.json`、Grafana をインポートしません。
   - ピン レジストリ (`docs/source/sorafs/runbooks/pin_registry_ops.md`) とチャンク範囲の統計。
4. **ハビリタカオのマルチソース**
   - `docs/source/sorafs/runbooks/multi_source_rollout.md` のパフォーマンスやオルケストラドールのロールアウト、オーディオのスコアボード/テレメトリアのアーキテクトを保存します。

## 事件の対処法

- Siga os caminhos de escalonament em `docs/source/sorafs/runbooks/`:
  - `sorafs_gateway_operator_playbook.md` ゲートウェイのストリーム トークンのパラメータ。
  - `dispute_revocation_runbook.md` は複製に関する紛争を解決します。
  - `sorafs_node_ops.md` パラ マヌテンカオ ノ ニベル デ ノド。
  - `multi_source_rollout.md` パラメタは orquestrador をオーバーライドし、ピアおよび etapa のロールアウトをブラックリストに登録します。
- API として PoR トラッカーの存在を確認して、ガバナンス ログを API 経由で検証し、遅延の異常を登録します。

## プロキシモス パソス

- マルチソースフェッチ (SF-6b) チェガーの自動実行命令 (`sorafs_car::multi_fetch`) を統合します。
- PDP/PoTR SF-13/SF-14 のアップグレードに対応。 o CLI は、証拠を確立するために必要な段階を選択して、ドキュメントを作成します。

パイプライン SoraFS の製造プロセスの反復観察を行うための、CI 受信用のクイックスタートと展開用の組み合わせはありません。