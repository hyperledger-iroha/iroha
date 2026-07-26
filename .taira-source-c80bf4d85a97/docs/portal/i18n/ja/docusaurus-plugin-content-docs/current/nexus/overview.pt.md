---
lang: ja
direction: ltr
source: docs/portal/docs/nexus/overview.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: ネクサスの概要
タイトル: ヴィサオ ジェラル ド ソラ Nexus
説明: Iroha 3 (Sora Nexus) の高度なアーキテクチャに関するドキュメントは、モノリポジトリを行う canonicos のパラ os ドキュメントに含まれています。
---

Nexus (Iroha 3) estende Iroha 2 com は、マルチレーンを実行し、SDK のコンパートメントを管理するための管理を実行します。新しいページへのアクセス `docs/source/nexus_overview.md` は、アーキテクチャーの構築と同様に、ポータルの迅速な管理を行うためのモノリポジトリではありません。

## リリースを解除する

- **Iroha 2** - インプラントは、コンソルシオスとプライベートの自動病院を提供します。
- **Iroha 3 / Sora Nexus** - マルチレーン オンデ オペラドールの登録機関であるダドス (DS) と政府機関の監視、液体監視の再発行。
- 管理ワークスペース (IVM + ツールチェーン Kotodama) をコンパイルするためのアンバス、SDK の修正、ABI フィクスチャ Norito の永久保存。オペレータはバンドル `iroha3-<version>-<os>.tar.zst` パラメータ番号 Nexus; `docs/source/sora_nexus_operator_onboarding.md` のチェックリストを参照してください。

## 建設ブロック

|コンポネ |レスモ |ポントス・ド・ポータル |
|----------|-----------|--------------|
|エスパコ デ ダドス (DS) |行政執行/政府の明確な統治を行うため、レーン、有効性の確認、プライバシー保護および政治的分類 + DA を宣言します。 | Veja [Nexus 仕様](./nexus-spec) はマニフェストに準拠しています。 |
|レーン |シャードの実行決定性。世界的なNPoSの秩序を無視して妥協を表明します。レーンのクラスとしては、`default_public`、`public_custom`、`private_permissioned`、`hybrid_confidential` が含まれます。 | O [modelo de LANE](./nexus-lane-model) キャプチャジオメトリア、プレフィックスデアルマゼナメント、保持。 |
|プラノ デ トランシソン |プレースホルダー、Nexus のパラメータを識別します。 | [notas de transicao](./nexus-transition-notes) documentam cada fase de migracao として。 |
|宇宙ディレクトリ |アルマゼナマニフェストとDSのバージョンを登録します。オペレーターは、カタログを作成する前に、カタログを作成します。 |マニフェストの差分を保存するためのスレッドは、`docs/source/project_tracker/nexus_config_deltas/` です。 |
|レーンのカタログ | `[nexus]` は、エイリアス、政治、DA の制限などのマップ ID を設定します。 `irohad --sora --config ... --trace-config` 聴覚的なカタログの解決策。 | CLI ではそのまま `docs/source/sora_nexus_operator_onboarding.md` を使用します。 |
|ロテドール デ リキッドダソン |転送サービス XOR は、CBDC のプライベート コム レーンと液体パブリック レーンを接続します。 | `docs/source/cbdc_lane_playbook.md` 政治のノブとテレメトリーのゲートの詳細。 |
|テレメトリア/SLO |ダッシュボード + アラートは、`dashboards/grafana/nexus_*.json` レーンの詳細、DA のバックログ、液体の遅延、および政府の綿密な情報をキャプチャします。 | O [テレメトリア対策計画](./nexus-telemetry-remediation) ダッシュボード、聴覚証拠のアラートの詳細。 |

## ロールアウトのスナップショット

|ファセ |フォコ |基準 |
|------|------|------|
| N0 - ベータフェチャダ |レジストラ gerenciado pelo conselho (`.sora`)、オンボーディング マニュアル ド オペラドール、カタログ ド レーン エスタティコ。 | DS のシナドス宣言 + 政府への引き継ぎ。 |
| N1 - ランカメント パブリック | Adiciona sufixos `.nexus`、leiloes、レジストラのセルフサービス、cabeamento de liquidacao XOR。 |リゾルバー/ゲートウェイのテスト、コブランカの調停ダッシュボード、紛争の実行。 |
| N2 - エクスパンサオ | `.dao`、復讐の API、分析、紛争ポータル、スチュワードのスコアカードの紹介。 |コンプライアンスバージョンの技術、オンラインでの陪審の政治ツールキット、透明性の高い関係性。 |
|ゲート NX-12/13/14 |モーターのコンプライアンス、ダッシュボードのテレメトリーとドキュメントの開発、安全性、安全性、および安全性を確保します。 | [Nexus 概要](./nexus-overview) + [Nexus 操作](./nexus-operations) パブリックド、ダッシュボード リガド、政治的統合。 |

## オペレーターには責任があります1. **設定の衛生** - mantenha `config/config.toml` データスペースのカタログと公開情報。 `--trace-config` のチケットをリリースしてください。
2. **マニフェストのストリーム** - カタログの内容を調整し、最近のスペース ディレクトリの登録番号を確認します。
3. **Cobertura de telemetria** - SDK 関連の OS ダッシュボード `nexus_lanes.json`、`nexus_settlement.json` を説明します。 PagerDuty は、テレメトリの計画に準拠した修正を行うため、アラートに接続します。
4. **事件に関する** - [Nexus 作戦](./nexus-operations) は、RCA が緊急事態に対処していることを報告します。
5. **統治期間** - コンセルホ Nexus の影響を受けるレーンとロールバック トリメストラメンテの指示に参加します (`docs/source/project_tracker/nexus_config_deltas/` 経由のラストリード)。

## ヴェジャ・タンベム

- ビザオ カノニカ: `docs/source/nexus_overview.md`
- 具体的な詳細: [./nexus-spec](./nexus-spec)
- レーンのジオメトリア: [./nexus-lane-model](./nexus-lane-model)
- トランジション計画: [./nexus-transition-notes](./nexus-transition-notes)
- テレメトリの修復計画: [./nexus-telemetry-remediation](./nexus-telemetry-remediation)
- オペラのランブック: [./nexus-operations](./nexus-operations)
- オペレーターのオンボーディングに関するガイア: `docs/source/sora_nexus_operator_onboarding.md`