---
lang: ja
direction: ltr
source: docs/portal/docs/nexus/overview.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: ネクサスの概要
title: 空の履歴書 Nexus
説明: Iroha 3 の高度な建築履歴書 (Sora Nexus) には、モノレポのドキュメントが含まれています。
---

Nexus (Iroha 3) amplia Iroha 2 はマルチレーンの排出、SDK の互換性をサポートします。新しいページの更新 `docs/source/nexus_overview.md` デル モノレポ パラ ケ ロス レクター デル ポータル エンティエンダン 急速な進歩により、建築物全体が見渡されます。

## ランサミエントのリネアス

- **Iroha 2** - 自動アロハドス パラ コンソルシオス プライベートを解放します。
- **Iroha 3 / Sora Nexus** - ラ レッド パブリック マルチ レーン ドンデ ロス オペラドーレス レジストラン エスパシオ デ ダトス (DS) と、政府機関の監視、清算と監視。
- ミスモ ワークスペース (IVM + ツールチェーン Kotodama) をコンパイルし、SDK の修正、ABI の実際のフィクスチャ Norito シグエン シエンド ポータブルを確認します。ロス オペラドールは Nexus を参照して `iroha3-<version>-<os>.tar.zst` をダウンロードします。 `docs/source/sora_nexus_operator_onboarding.md` の完全な検証リストを参照してください。

## 構築ブロック

|コンポネ |履歴書 |ガンチョス デル ポータル |
|----------|-----------|--------------|
|エスパシオ デ ダトス (DS) |排斥/アルマセナミエントの定義は、政治的行動、有効性の宣言、タリファスのプライバシーと政治の宣言 + DA です。 | [Nexus 仕様](./nexus-spec) の説明を参照してください。 |
|レーン |排出決定フラグメント。世界規模の NPoS を侵害する可能性があります。レーンのクラスには、`default_public`、`public_custom`、`private_permissioned` および `hybrid_confidential` が含まれます。 | El [modelo de LANE](./nexus-lane-model) ジオメトリアのキャプチャ、アルマセナミエントの保持。 |
|移行計画 | Identificadores プレースホルダ、enrutamiento y empaquetado de doble perfil siguen como los despliegues de un Solo LANe evolucionan hacia Nexus。 |ラス [移行ノート](./nexus-transition-notes) 移行に関する文書。 |
|エスパシオ監督 | DS のマニフェストとバージョンを登録するためのコントラト。 Los operadores concilian las entradas del Catalogo contra este Directorio antes de unirse。 | `docs/source/project_tracker/nexus_config_deltas/` の差分をマニフェストするためのラストリーダー。 |
|レーンのカタログ |セクション `[nexus]` は、エイリアス、政治に関する包括的な DA の割り当て ID を設定します。 `irohad --sora --config ... --trace-config` 聴覚に対する主要なカタログの結果。 |米国 `docs/source/sora_nexus_operator_onboarding.md` CLI の記録。 |
|ルーターの清算 | Orquestador de transferencias XOR que conecta LANES CBDC privadas con LANes de Liquidez publicas。 | `docs/source/cbdc_lane_playbook.md` 政治とテレメトリの計算の詳細。 |
|テレメトリア/SLO |パネレス + アラート バホ `dashboards/grafana/nexus_*.json` レーンのアルトゥラ、バックログ DA、清算の遅延、コーラ デ ゴベルナンザの深さのキャプチャ。 | El [テレメトリアの修復計画](./nexus-telemetry-remediation) パネルの詳細、聴覚の証拠に関するアラート。 |

## インスタント・デ・デスリーグ

|ファセ |エンフォク |サリダの基準 |
|------|------|------|
| N0 - ベータセラーダ |レジストラ gestionado por el consejo (`.sora`)、オペラ マニュアル、レーン エスタティコのカタログを組み込みます。 | Manifestos DS フィルマドス + トラスパソス デ ゴベルナンザ エンサヤドス。 |
| N1 - ランサミエント公共 |アナデ・スフィホス `.nexus`、サブバス、自動サービス登録業者、液体 XOR ケーブル。 |リゾルバー/ゲートウェイの同期、事実の和解、紛争のシミュレーションのパネル。 |
| N2 - 拡張 | `.dao`、Reventa の API、analitica、議論のポータル、スチュワードのスコアカードを導入します。 |バージョン管理の成果物、政治的なツールキット、透明性に関する情報。 |
|プエルタ NX-12/13/14 |最高のモーター、テレメトリー、ドキュメントのパネルを備えた、安全な飛行訓練を実現します。 | [Nexus 概要](./nexus-overview) + [Nexus 操作](./nexus-operations) 公開、パネル接続、政治統合。 |

## オペレーターの責任1. **構成の衛生** - レーンとデータスペースの公開カタログを管理 `config/config.toml` します。 `--trace-config` のチケットをリリースするためのアーカイブ。
2. **マニフィストの選択** - 実際のノードを統合するためのスペース ディレクトリのカタログを確認します。
3. **テレメトリアのコベルトゥーラ** - パネル `nexus_lanes.json`、`nexus_settlement.json` および SDK に関するダッシュボードを説明します。 PagerDuty としてアラートを接続し、遠隔測定の修復計画の改訂版を発行します。
4. **事故報告書** - [Nexus 操作](./nexus-operations) の深刻な状況に関する RCA の報告書。
5. **ゴベルナンザの準備** - コンセホ Nexus の影響を受けるレーンとロールバック トリメストラルメントの指示を支援します (`docs/source/project_tracker/nexus_config_deltas/` の手順)。

## ヴァータンビアン

- カノニコの履歴書: `docs/source/nexus_overview.md`
- 詳細情報: [./nexus-spec](./nexus-spec)
- レーンのジオメトリア: [./nexus-lane-model](./nexus-lane-model)
- 移行計画: [./nexus-transition-notes](./nexus-transition-notes)
- テレメトリの修復計画: [./nexus-telemetry-remediation](./nexus-telemetry-remediation)
- オペレーションのランブック: [./nexus-operations](./nexus-operations)
- オペレーターのオンボーディングに関するガイア: `docs/source/sora_nexus_operator_onboarding.md`