<!--
  SPDX-License-Identifier: Apache-2.0
-->

---
lang: ja
direction: ltr
source: docs/source/nexus_overview.md
status: complete
translator: LLM (Codex)
translation_last_reviewed: 2026-01-01
---

# Nexus 概要とオペレーター向けコンテキスト

**ロードマップ:** NX-14 — Nexus ドキュメント & オペレーターランブック  
**ステータス:** 2026-03-24 作成（`docs/source/nexus_operations.md` と対で運用）  
**想定読者:** プログラムマネージャー、運用エンジニア、パートナーチーム。詳細仕様（`docs/source/nexus.md`、`docs/source/nexus_lanes.md`、`docs/source/nexus_transition_notes.md`）へ進む前に Sora Nexus (Iroha 3) の構造を 1 ページで把握したい読者向け。

## 1. リリースラインと共通ツールチェーン

- **Iroha 2** は、従来どおりコンソーシアム/セルフホスト環境向けのリリースラインとして維持されます。
- **Iroha 3 / Sora Nexus** はマルチレーン実行、データスペース、共通ガバナンスを導入します。同じリポジトリ・ツールチェーン・CI が Iroha 2/3 の両方を構築するため、IVM・Kotodama・SDK への修正は Nexus にも即座に反映されます。
- **アーティファクト:** `iroha3-<version>-<os>.tar.zst` バンドルおよび OCI イメージには、バイナリ・サンプル設定・Nexus プロファイルメタデータが含まれます。オペレーターは `docs/source/sora_nexus_operator_onboarding.md` を参照してアーティファクト検証フローを実行します。
- **共通 SDK サーフェス:** Rust / Python / JS/TS / Swift / Android の各 SDK は同じ Norito スキーマとアドレスフィクスチャ（`fixtures/account/address_vectors.json`）を消費します。ウォレットや自動化はフォーマットを切り替えることなく Iroha 2 と Nexus のネットワークを行き来できます。

## 2. アーキテクチャの構成要素

| コンポーネント | 説明 | 主要リファレンス |
|----------------|------|------------------|
| **データスペース (DS)** | ガバナンスでスコープされた実行ドメイン。バリデータ構成、秘匿クラス、料金ポリシー、DA プロファイルを定義し、1 つ以上のレーンを所有します。 | `docs/source/nexus.md`、`docs/source/nexus_transition_notes.md` |
| **レーン** | 決定論的な実行/状態のシャード。レーンマニフェストはバリデータ集合、決済フック、テレメトリメタデータ、ルーティング権限を宣言します。グローバルコンセンサスリングがレーンコミットメントの順序を整列させます。 | `docs/source/nexus_lanes.md` |
| **Space Directory** | DS マニフェスト、バリデータローテーション、ケイパビリティ付与を保持するレジストリ契約と CLI。署名済み履歴を保持し、監査時に状態を再構築できます。 | `docs/source/nexus.md#space-directory` |
| **レーンカタログ** | `config.toml` の `[nexus]` セクション。レーン ID とエイリアス、ルーティングポリシー、保持設定を対応付けます。`irohad --sora --config … --trace-config` で実効カタログを確認できます。 | `docs/source/sora_nexus_operator_onboarding.md` |
| **Settlement Router** | レーン間の XOR 移動（例: プライベート CBDC ↔ 公開流動性）をルーティングします。ポリシー初期値は `docs/source/cbdc_lane_playbook.md` に記載。 | `docs/source/cbdc_lane_playbook.md` |
| **テレメトリ & SLO** | `dashboards/grafana/nexus_*.json` のダッシュボード/アラートはレーン高、DA バックログ、決済遅延、ガバナンスキュー深さを可視化します。是正計画は `docs/source/nexus_telemetry_remediation_plan.md` で追跡します。 | `dashboards/grafana/nexus_lanes.json`、`dashboards/alerts/nexus_audit_rules.yml` |

### レーン/データスペースのクラス

- `default_public`: ソラ議会が管理する完全公開ワークロード。
- `public_custom`: 経済設計を独自化しつつ公開性を維持するレーン。
- `private_permissioned`: CBDC やコンソーシアム向け。コミットメントと証明のみを公開します。
- `hybrid_confidential`: ゼロ知識証明と選択的開示を組み合わせたレーン。

各レーンは次を宣言します。

1. **レーンマニフェスト:** Space Directory で追跡されるガバナンス承認済みメタデータ。
2. **データ可用性ポリシー:** イレーザ符号パラメータ、復旧手順、監査要件。
3. **テレメトリプロファイル:** ダッシュボードとオンコールランブック。ガバナンスでレーンの状態が変わるたびに更新が必要です。

## 3. ロールアウトタイムライン

| フェーズ | フォーカス | 終了条件 |
|----------|------------|----------|
| **N0 – クローズドベータ** | 議会管理のレジストラ、`.sora` のみ、手動オンボーディング。 | DS マニフェストが署名済み、レーンカタログ固定、ガバナンスリハーサル記録済み。 |
| **N1 – パブリックローンチ** | `.nexus` サフィックス、オークション、自動レジストラ、XOR 財務省との決済連携。 | リゾルバ/ゲートウェイ同期テストが成功、課金ダッシュボード稼働、ディスプート卓上訓練完了。 |
| **N2 – 拡張** | `.dao`、リセラ API、分析、紛争ポータル、スチュワードスコアカード。 | コンプライアンス証跡が版管理され、政策陪審ツールキット稼働、財務透明化レポート公開。 |
| **NX-12/13/14 ゲート** | コンプライアンスエンジン、テレメトリダッシュボード、ドキュメントが同時にそろってはじめてパートナーパイロットを開放。 | `docs/source/nexus_overview.md` と `docs/source/nexus_operations.md` が公開され、ダッシュボードにアラートが配線され、ポリシーエンジンがガバナンスに接続済み。 |

## 4. オペレーターの責務

| 責務 | 説明 | エビデンス |
|------|------|------------|
| コンフィグ衛生 | 公開されたレーン/データスペースカタログと `config/config.toml` を同期し、差分をチケットに記録。 | `irohad --sora --config … --trace-config` の出力をリリースアーカイブへ保存。 |
| マニフェスト監視 | Space Directory の更新を監視し、ローカルキャッシュ/許可リストを更新。 | 署名済みマニフェストバンドルをオンボーディングチケットに添付。 |
| テレメトリカバレッジ | セクション 2 記載のダッシュボードへアクセス可能にし、PagerDuty アラートと四半期レビューを記録。 | オンコールレビュー議事録 + Alertmanager エクスポート。 |
| インシデント報告 | `docs/source/nexus_operations.md` の重大度マトリクスに従い、5 営業日以内に事後レポートを提出。 | インシデント ID ごとのテンプレート。 |
| ガバナンス準備 | レーンポリシー変更時は Nexus 議会投票へ参加し、四半期ごとにロールバック手順をリハーサル。 | `docs/source/project_tracker/nexus_config_deltas/` 配下の出席記録とチェックリスト。 |

## 5. 関連ドキュメント

- **詳細仕様:** `docs/source/nexus.md`
- **レーン幾何/ストレージ:** `docs/source/nexus_lanes.md`
- **移行計画と暫定ルーティング:** `docs/source/nexus_transition_notes.md`
- **オペレーターオンボーディング:** `docs/source/sora_nexus_operator_onboarding.md`
- **CBDC レーンポリシー & 決済計画:** `docs/source/cbdc_lane_playbook.md`
- **テレメトリ是正 & ダッシュボード:** `docs/source/nexus_telemetry_remediation_plan.md`
- **ランブック/インシデント手順:** `docs/source/nexus_operations.md`

リンク先のドキュメントに大きな変更が入った場合や新しいレーン種別・ガバナンスフローが追加された場合は、ロードマップ NX-14 に合わせて本概要も更新してください。
