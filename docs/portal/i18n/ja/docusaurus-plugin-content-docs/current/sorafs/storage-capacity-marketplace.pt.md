---
lang: ja
direction: ltr
source: docs/portal/docs/sorafs/storage-capacity-marketplace.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: ストレージ容量マーケットプレイス
title: アルマゼナメントの容量市場 SoraFS
サイドバー_ラベル: 容量のマーケットプレイス
説明: Plano SF-2c は、市場の容量、レプリカの管理、テレメトリのフックなどの管理に使用されます。
---

:::note フォンテ カノニカ
エスタ・ページナ・エスペルハ`docs/source/sorafs/storage_capacity_marketplace.md`。 Mantenha ambos os locais alinhados enquanto a documentacao herdada permanecer ativa.
:::

# アルマゼナメントの市場 SoraFS (ラスクーニョ SF-2c)

項目 SF-2c のロードマップの紹介、市場管理、プロバイダーの紹介
軍事大作戦を宣言し、大災害を補償し、レプリカの命令を受け取る
ガンハム料金は、ディスポニビリダーデ・アントリーグに比例します。エステ ドキュメント デリミタ
OS は、最初のリリースで Exigidos をリリースし、Trilhas acionaveis を分割します。

## オブジェクト

- Expressar compromisos de capacidade (バイト数、レーンの制限、期限切れ)
  管理者向けの検証用の形式、SoraNet の転送 Torii。
- アロカーは、ステーク宣言、宣言、およびアコード コムのプロバイダーにピンを付けます
  政治的規制は、決定性を維持するために必要です。
- Medir entrega de armazenamento (複製の成功、稼働時間、証明)
  Integridade) 販売代理店のテレメトリ料金の輸出業者。
- 証明者は、revogacao および議論に関するプロバイダーのプロセスをデソネストス セジャムに提供します。
  ペナリザドスかレモビドス。

## コンセイトス・デ・ドミニオ

|コンセイト |説明 |エントレガヴェルのイニシャル |
|----------|-----------|----------|
| `CapacityDeclarationV1` |ペイロード Norito プロバイダーの ID の説明、チャンカーのサポート、GiB の保証、レーンの制限、価格設定のヒント、ステーキングと有効期限の補償。 |エスケマ + バリダドール em `sorafs_manifest::capacity`。 |
| `ReplicationOrder` |プロバイダーに対する管理上の CID のマニフェストには、冗長性と SLA の基準も含まれます。 | Esquema Norito compartilhado com Torii + API のスマート コントラクト。 |
| `CapacityLedger` |オンチェーン/オフチェーンのレジストリは、活動の容量、レプリカの管理、パフォーマンスの指標、および手数料の発生を宣言します。 |スマート コントラクトとオフチェーン コム サービスのスタブのモジュロを決定します。 |
| `MarketplacePolicy` |政治は、ステークミニモ、聴衆と刑罰の義務を定義します。 | `sorafs_manifest` の構成構造体 + 統治文書。 |

### エスケマスの実装 (ステータス)

## デスドブラメント・デ・トラバリョ

### 1. スキーマのレジストリ

|タレファ |レスポンス |メモ |
|------|-------|------|
| `CapacityDeclarationV1`、`ReplicationOrderV1`、`CapacityTelemetryV1` を定義します。 |ストレージ チーム / ガバナンス | Usar Norito;バージョンのセマンティコと容量の参照が含まれます。 |
| `sorafs_manifest` のパーサー + バリデータのモジュールを実装します。 |ストレージチーム | ID の単調性、容量の制限、賭け金の要求をインポートします。 |
| Estender メタデータは、perfil のチャンカー レジストリ com `min_capacity_gib` を実行します。 |ツーリングWG | Ajuda は、パフォーマンスに関するハードウェアの最低限の要件を満たしています。 |
| Redigir documento `MarketplacePolicy` は、ペナルティのカレンダーとカレンダーのガードレールをキャプチャします。 |ガバナンス評議会 |政治に関するデフォルトの文書が公開されています。 |

#### スキーマの定義 (実装)- `CapacityDeclarationV1` プロバイダーごとの容量の妥協点のキャプチャ、チャンカーのキャノニコス処理、容量の参照、レーンごとのオプションのキャップ、価格設定のヒント、検証のジャネラ、およびメタデータが含まれます。ゼロを保証し、canonicos、エイリアス deduplicados、Caps por LANE dentro を完全に宣言し、GiB モノトニカメンテ クレッセントを管理します。 [crates/sorafs_manifest/src/capacity.rs:28]
- `ReplicationOrderV1` Vincula は、冗長性、SLA の制限、および属性の管理に関する属性を示しています。 validadores impoem は canonicos を処理し、Torii の期限までに unicos とプロバイダーを制限し、レジストリを命令します。 [crates/sorafs_manifest/src/capacity.rs:301]
- `CapacityTelemetryV1` は、エポカのスナップショット (GiB 宣言対ユーティリティ、レプリカの管理、稼働時間のパーセント/PoR) を分配し、料金を支払います。 0 ～ 100% の制限を管理します。 [crates/sorafs_manifest/src/capacity.rs:476]
- ヘルパー比較 (`CapacityMetadataEntry`、`PricingScheduleV1`、レーン/アトリビューカオ/SLA の検証) ダウンストリームの CI ツールのエラー再利用レポートを実行します。 [crates/sorafs_manifest/src/capacity.rs:230]
- `PinProviderRegistry` は、`/v1/sorafs/capacity/state` 経由でオンチェーンでスナップショットを公開し、プロバイダーとエントラダを組み合わせて、Norito JSON で料金台帳を決定します。 [crates/iroha_torii/src/sorafs/registry.rs:17] [crates/iroha_torii/src/sorafs/api.rs:64]
- 正規の認証、重複の検出、ポーレーンの制限、レプリカの保護、CI なしでのテレメトリの範囲のチェックの有効性を確認するための強制執行。 [crates/sorafs_manifest/src/capacity.rs:792]
- オペラドールのツール: `sorafs_manifest_stub capacity {declaration, telemetry, replication-order}` ペイロードの合法的な仕様を変換 Norito canonicos、BLOB Base64 電子履歴の JSON パラメタ オペラドールが `/v1/sorafs/capacity/declare`、`/v1/sorafs/capacity/telemetry` ローカルで有効なレプリカのフィクスチャを準備します。 [crates/sorafs_car/src/bin/sorafs_manifest_stub/capacity.rs:1] `fixtures/sorafs_manifest/replication_order/` (`order_v1.json`、`order_v1.to`) および `cargo run -p sorafs_car --bin sorafs_manifest_stub -- capacity replication-order` 経由で参照できるフィクスチャ。

### 2. 計画管理の統合

|タレファ |レスポンス |メモ |
|------|-------|------|
|追加のハンドラー Torii `/v1/sorafs/capacity/declare`、`/v1/sorafs/capacity/telemetry`、`/v1/sorafs/capacity/orders` com ペイロード Norito JSON。 | Torii チーム | Espelhar 論理学は検証します。 reutilizar ヘルパー Norito JSON。 |
|プロパガーのスナップショット `CapacityDeclarationV1` は、メタデータ、スコアボード、オーケストレーター、プラノス、フェッチ、ゲートウェイを実行します。 |ツーリング WG / オーケストレーター チーム | Estender `provider_metadata` com References de Capacidade para que o スコアリングのマルチソース休憩時間は、ポーレーンを制限します。 |
|オーケストレーター/ゲートウェイのクライアントのレプリカの順序や、フェールオーバーのヒントなどのオリエンタル アトリビュートが提供されます。 |ネットワーキング TL / ゲートウェイ チーム |おお、ビルダーはスコアボードをコンサム・オーデンス・アッシナダス・ペラ・ガバナンカにする。 |
|ツール CLI: estender `sorafs_cli` com `capacity declare`、`capacity telemetry`、`capacity orders import`。 |ツーリングWG | Fornecer JSON 決定論 + スコアボードの説明。 |

### 3. 市場と統治の政治

|タレファ |レスポンス |メモ |
|------|-------|------|
| Ratificar `MarketplacePolicy` (ステークミニモ、刑罰の乗数、聴衆の声)。 |ガバナンス評議会 |ドキュメントを公開し、改訂履歴をキャプチャします。 |
|議会の承認、改修、および宣言の宣言に関する政府の追加フック。 |ガバナンス評議会 / スマートコントラクトチーム |イベント Norito + マニフェストの摂取。 |
| SLA の電報に対する罰則のカレンダー (手数料の軽減、保証金の削減) を実装します。 |ガバナンス評議会 / 財務 | Alinhar com は `DealEngine` を実行して決済を出力します。 |
|議論の過程とエスカロナメントのドキュメンタリー。 |ドキュメント / ガバナンス |議論のランブックと CLI のヘルパーのリンカ。 |

### 4. メータリングと料金の分配|タレファ |レスポンス |メモ |
|------|-------|------|
| `CapacityTelemetryV1` から Torii を測定します。 | Torii チーム |有効な GiB 時間、継続的な PoR、稼働時間。 |
|パイプラインの計測は、`sorafs_node` パラレポートを使用して、SLA の統計情報を取得します。 |ストレージチーム | Alinhar はチャンカーのレプリカを処理します。 |
|決済のパイプライン: コンバーターテレメトリア + データベースとデノミナドと XOR のレプリカ、政府機関のレジストラと台帳を作成します。 |財務/保管チーム | Conectar はディール エンジン / 財務省輸出を担当します。 |
|メーターを実行するためのダッシュボード/アラートのエクスポート (取り込みのバックログ、テレメトリの古さ)。 |可観測性 | SF-6/SF-7 の Grafana 参照のエステル パック。 |

- Torii アゴラ エキスポ `/v1/sorafs/capacity/telemetry` e `/v1/sorafs/capacity/state` (JSON + Norito) パラ ケ オペラドールのスナップショット デ テレメトリア ポート エポカ電子検査装置の回復と元帳のカノニコ パラ オーディトリア エンパコタメント証拠。 [crates/iroha_torii/src/sorafs/api.rs:268] [crates/iroha_torii/src/sorafs/api.rs:816]
- Integracao `PinProviderRegistry` は、エンドポイントのアクセス許可を保証します。 CLI のヘルパー (`sorafs_cli capacity telemetry --from-file telemetry.json`) は、自動化されたコンポーネントのハッシュ決定性と解決策を実行する一部の公開テレメトリを検証します。
- 計測製品のスナップショット `CapacityTelemetrySnapshot` 修正スナップショット `metering`、エクスポート Prometheus ボード Grafana インポート前のスナップショット `docs/source/grafana_sorafs_metering.json` 装備実際の GiB 時間の累積監視、料金 nano-SORA プロジェクトの SLA コンプライアンス。 [crates/iroha_torii/src/routing.rs:5143] [docs/source/grafana_sorafs_metering.json:1]
- `smoothed_gib_hours` と `smoothed_por_success_bps` を含むスナップショットと EMA コントラ コンタドール ブルートス アメリカドス ペラ ガバナンカ ペイアウトを含むスムージングとメーターの比較。 [crates/sorafs_node/src/metering.rs:401]

### 5. 紛争の報告とレボガカオ

|タレファ |レスポンス |メモ |
|------|-------|------|
| Definir ペイロード `CapacityDisputeV1` (回収、証拠、プロバイダー アルボ)。 |ガバナンス評議会 |スキーマ Norito + バリデータ。 |
|紛争解決者に対する CLI のサポート (com anexos de evidencia)。 |ツーリングWG |保証バンドルのハッシュ決定性は証拠を保証します。 |
| Adicionar は、SLA 繰り返し (auto-escalada para disputa) を自動的にチェックします。 |可観測性 |統治上の制限とフック。 |
|レボガカオのドキュメンタリー プレイブック (ピリオド デ グラサ、エバキュアオ デ ダドス ピナドス)。 |ドキュメント / ストレージ チーム |政治文書と運用ブックのリンク。 |

## テストと CI の要件

- スキーマの新しい検証をテストします (`sorafs_manifest`)。
- シミュレーションの統合テスト: 宣言 -> 複製命令 -> 計測 -> 支払い。
- CI パラ再生宣言/テレメトリアの容量制限のワークフローは、永久監視機能として保証されます (エステル `ci/check_sorafs_fixtures.sh`)。
- API レジストリのテスト デ カルガ (類似の 10,000 プロバイダー、100,000 オーデン)。

## Telemetria とダッシュボード

- ダッシュボードの機能:
  - プロバイダーによる Capacidade declarada と utilizada の比較。
  - バックログのレプリカとメディアのバックログ。
  - SLA への準拠 (稼働率 %、PoR の分類)。
  - エポカによる手数料や罰則の発生。
- アラート:
  - プロバイダーは最小限の容量を提供します。
  - 複製命令 > SLA。
  - Falhas にはパイプラインの計測はありません。

## ドキュメントの作成

- 緊急事態宣言におけるオペレータの保護、妥協と監視の監視。
- 政府に関する承認宣言、発行命令およびライダー コムの議論。
- API パラメタのエンドポイントの容量およびレプリカの順序の形式の参照。
- マーケットプレイス開発者向けの FAQ。

## 一般提供準備チェックリスト

ロードマップ **SF-2c** の項目、展開、生産、証拠、具体化
冷静な対応、紛争の対処、オンボーディング。 os artefatos abaixo para を使用する
同期を実行するための基準を管理します。### XOR を調整するための調整
- 静電容量のスナップショットをエクスポートし、台帳 XOR をメスマ ジャネラにエクスポートします。
  デポジットを実行します:
  ```bash
  python3 scripts/telemetry/capacity_reconcile.py \
    --snapshot artifacts/sorafs/capacity/state_$(date +%F).json \
    --ledger artifacts/sorafs/capacity/ledger_$(date +%F).ndjson \
    --label nightly-capacity \
    --json-out artifacts/sorafs/capacity/reconcile_$(date +%F).json \
    --prom-out "${SORAFS_CAPACITY_RECONCILE_TEXTFILE:-artifacts/sorafs/capacity/reconcile.prom}"
  ```
  おお、ヘルパー サイ コム コディゴ ナオ ゼロ エム 和解、あなたは罰則をオーセンテス/過剰に放出します
  um resumo Prometheus em 形式のテキストファイル。
- 警告 `SoraFSCapacityReconciliationMismatch` (`dashboards/alerts/sorafs_capacity_rules.yml`)
  報告書のギャップを調整するための計量基準の相違。ダッシュボード ficam em
  `dashboards/grafana/sorafs_capacity_penalties.json`。
- `docs/examples/sorafs_capacity_marketplace_validation/` の JSON ハッシュを取得します
  ジュント・コン・パコート・デ・ガバナンカ。

### 論争とスラッシュの証拠
- `sorafs_manifest_stub capacity dispute` 経由で議論をアーカイブする (テスト:
  `cargo test -p sorafs_car --test capacity_cli`) パラメータ ペイロード canonicos。
- `cargo test -p iroha_core -- capacity_dispute_replay_is_deterministic` e をスイートとして実行
  ペナリダード (`record_capacity_telemetry_penalises_persistent_under_delivery`) パラプロバール
  論争はファゼムリプレイを決定的に切り裂きます。
- シガ `docs/source/sorafs/dispute_revocation_runbook.md` 証拠とエスカロナメントのキャプチャ。
  無効な関係を検証するためのリンク。

### プロバイダーのオンボーディングと終了スモーク テスト
- `sorafs_manifest_stub capacity ...` のデクララソン/テレメトリア コムのアートファトを再生成します。
  送信前に CLI をテストします (`cargo test -p sorafs_car --test capacity_cli -- capacity_declaration`)。
- Torii (`/v1/sorafs/capacity/declare`) 経由のサブメタと `/v1/sorafs/capacity/state` のキャプチャ
  スクリーンショットは Grafana です。 Siga o fluxo de Saida em `docs/source/sorafs/capacity_onboarding_runbook.md`。
- 調停に関する成果物や成果物を保管する
  `docs/examples/sorafs_capacity_marketplace_validation/`。

## 依存性とシーケンス

1. SF-2b の最終決定 (政治的承認) - 市場はプロバイダーの承認に依存します。
2. Torii を統合するために、スキーマとレジストリ (エステート ドキュメント) を実装します。
3. 支払い前の支払いを計量する完全なパイプライン。
4. Etapa の最終決定: 政府による手数料管理の分配
   ステージング前の検証。

進歩的な開発者は、エステドキュメントを参照するロードマップはありません。を実現する
ロードマップ アシム ケ カダ セカオ プリンシパル (スキーマ、プランノ デ コントロール、統合、メータリング、
tratamento de disputa) ステータス機能が完了しました。