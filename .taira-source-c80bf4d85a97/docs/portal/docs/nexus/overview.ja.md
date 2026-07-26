---
lang: ja
direction: ltr
source: docs/portal/docs/nexus/overview.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: dd986adf52d15dfb82f4396cfa6891efd54c78f528d7621c355dd6d8624f0a02
source_last_modified: "2025-11-10T17:36:53.333906+00:00"
translation_last_reviewed: 2025-12-30
---

---
id: nexus-overview
title: Sora Nexus 概要
description: Iroha 3 (Sora Nexus) アーキテクチャの高レベル概要と、モノレポの正本ドキュメントへの参照。
---

Nexus (Iroha 3) は Iroha 2 を multi-lane 実行、ガバナンスでスコープされたデータスペース、全 SDK で共有されるツール群で拡張します。このページはモノレポの新しい概要 `docs/source/nexus_overview.md` を反映し、ポータル読者がアーキテクチャの要素がどう噛み合うかを素早く理解できるようにしています。

## リリースライン

- **Iroha 2** - コンソーシアムやプライベートネットワーク向けのセルフホスト展開。
- **Iroha 3 / Sora Nexus** - オペレーターがデータスペース (DS) を登録し、ガバナンス、決済、オブザーバビリティの共通ツールを継承する multi-lane 公開ネットワーク。
- 両ラインは同じワークスペース (IVM + Kotodama toolchain) からビルドされるため、SDK 修正、ABI 更新、Norito のフィクスチャは移植性を保ちます。オペレーターは `iroha3-<version>-<os>.tar.zst` をダウンロードして Nexus に参加し、フルスクリーンのチェックリストは `docs/source/sora_nexus_operator_onboarding.md` を参照してください。

## 構成要素

| コンポーネント | 概要 | ポータルの参照先 |
|-----------|---------|--------------|
| データスペース (DS) | ガバナンス定義の実行/ストレージ領域。1 つ以上の lane を持ち、バリデータ集合、プライバシークラス、料金 + DA ポリシーを宣言します。 | マニフェストスキーマは [Nexus spec](./nexus-spec) を参照してください。 |
| Lane | 実行の決定論的シャードで、グローバル NPoS リングが並べ替えるコミットメントを発行します。`default_public`、`public_custom`、`private_permissioned`、`hybrid_confidential` が lane クラスです。 | [Lane モデル](./nexus-lane-model) はジオメトリ、ストレージプレフィックス、保持を扱います。 |
| 移行計画 | プレースホルダ識別子、ルーティング段階、デュアルプロファイルのパッケージングが、単一 lane 展開が Nexus へ進化する過程を追跡します。 | 各移行フェーズは [Transition notes](./nexus-transition-notes) に記載されています。 |
| Space Directory | DS マニフェストとバージョンを保存するレジストリコントラクト。オペレーターは参加前にカタログのエントリをこのディレクトリと照合します。 | マニフェスト差分トラッカーは `docs/source/project_tracker/nexus_config_deltas/` にあります。 |
| Lane カタログ | `[nexus]` 設定セクションが lane ID をエイリアス、ルーティングポリシー、DA 閾値へマップします。`irohad --sora --config … --trace-config` は監査向けに解決済みカタログを出力します。 | CLI の手順は `docs/source/sora_nexus_operator_onboarding.md` を参照してください。 |
| 決済ルーター | XOR 転送をオーケストレーションし、プライベート CBDC lane をパブリック流動性 lane に接続します。 | `docs/source/cbdc_lane_playbook.md` がポリシーノブとテレメトリゲートを説明します。 |
| テレメトリ/SLOs | `dashboards/grafana/nexus_*.json` 配下のダッシュボードとアラートが lane 高さ、DA バックログ、決済レイテンシ、ガバナンスキューの深さを捕捉します。 | [Telemetry remediation plan](./nexus-telemetry-remediation) がダッシュボード、アラート、監査証跡を説明します。 |

## ロールアウトのスナップショット

| フェーズ | フォーカス | 退出条件 |
|-------|-------|---------------|
| N0 - クローズドベータ | カウンシル管理の registrar (`.sora`)、手動オペレーターオンボーディング、静的 lane カタログ。 | 署名済み DS マニフェスト + ガバナンス引き継ぎのリハーサル。 |
| N1 - 公開ローンチ | `.nexus` サフィックス、オークション、セルフサービス registrar、XOR 決済配線を追加。 | resolver/gateway 同期テスト、請求照合ダッシュボード、紛争机上演習。 |
| N2 - 拡張 | `.dao`、再販 API、アナリティクス、紛争ポータル、スチュワードのスコアカードを導入。 | コンプライアンス成果物の版管理、ポリシー審査ツールキットのオンライン化、財務透明性レポート。 |
| NX-12/13/14 ゲート | コンプライアンスエンジン、テレメトリダッシュボード、ドキュメントはパートナーパイロット前に同時出荷が必要。 | [Nexus overview](./nexus-overview) + [Nexus operations](./nexus-operations) 公開、ダッシュボード接続、ポリシーエンジン統合。 |

## オペレーターの責務

1. **設定衛生** - `config/config.toml` を公開された lane / dataspace カタログと同期し、`--trace-config` の出力を各リリースチケットに保管します。
2. **マニフェスト追跡** - 参加またはノード更新前に、カタログ項目を最新の Space Directory バンドルと照合します。
3. **テレメトリカバレッジ** - `nexus_lanes.json`、`nexus_settlement.json`、関連 SDK ダッシュボードを公開し、PagerDuty にアラートを接続してテレメトリ修復計画に沿った四半期レビューを実施します。
4. **インシデント報告** - [Nexus operations](./nexus-operations) の重大度マトリクスに従い、5 営業日以内に RCA を提出します。
5. **ガバナンス準備** - lane に影響する Nexus 評議会の投票に参加し、四半期ごとにロールバック手順をリハーサルします（`docs/source/project_tracker/nexus_config_deltas/` で追跡）。

## 参照

- 正本の概要: `docs/source/nexus_overview.md`
- 詳細仕様: [./nexus-spec](./nexus-spec)
- Lane ジオメトリ: [./nexus-lane-model](./nexus-lane-model)
- 移行計画: [./nexus-transition-notes](./nexus-transition-notes)
- テレメトリ修復計画: [./nexus-telemetry-remediation](./nexus-telemetry-remediation)
- 運用ランブック: [./nexus-operations](./nexus-operations)
- オペレーターオンボーディングガイド: `docs/source/sora_nexus_operator_onboarding.md`
