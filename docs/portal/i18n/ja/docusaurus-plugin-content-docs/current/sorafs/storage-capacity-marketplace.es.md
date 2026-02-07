---
lang: ja
direction: ltr
source: docs/portal/docs/sorafs/storage-capacity-marketplace.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: ストレージ容量マーケットプレイス
title: アルマセナミエント市場 SoraFS
サイドバーラベル: カパシダ市場
説明: 計画 SF-2c パラ・エル・メルカド・デ・キャパシダード、オルデネス・デ・レプリカシオン、テレメトリーおよびフック・デ・ゴベルナンザ。
---

:::メモ フエンテ カノニカ
`docs/source/sorafs/storage_capacity_marketplace.md` のページを参照してください。活動を記録し、記録を保存することができます。
:::

# アルマセナミエント市場 SoraFS (ボラドール SF-2c)

ロードマップ SF-2c のアイテムは、販売プロバイダーを導入します
アルマセナミエント デクララン キャパシダッド コンプロメティダ、レプリカシオン オルデンス
ガナン料金は、ディスポニビリダード・エントレガダに比例します。エステ ドキュメント デリミタ
プリメーラのリリースに必要なリクエストは、トラックの分割に必要です。

## オブジェクト

- Expresar compromisos de capacidad (バイト合計、レーンの制限、有効期限)
  en una 形式の検証可能な消耗品 por gobernanza、transporte SoraNet y Torii。
- プロバイダーの安全宣言、ステークおよび制限を指定します。
  政治的ミエントラ・セ・マンティエン・コンポルタミエント・デターミニスタ。
- ストレージの管理 (レプリケーションの終了、稼働時間、統合の証明)
  料金の分配に関するテレメトリの輸出業者。
- 失効手続きと紛争解決プロバイダーのショーンの証明者
  ペナリサドスまたはレモビドス。

## ドミニオの概念

|コンセプト |説明 |魅力的なイニシャル |
|----------|---------------|----------|
| `CapacityDeclarationV1` |ペイロード Norito は、プロバイダーの ID、チャンカーのパフォーマンス、GiB の互換性、レーンの制限、価格のピスタ、ステーキングと有効期限の妥協点を記述します。 |エスケマ + バリドール en `sorafs_manifest::capacity`。 |
| `ReplicationOrder` |冗長性と SLA の基準を含む、UNO マス プロバイダーに対する CID の指定に関する指示が送信されます。 | Esquema Norito は、Torii + API のスマート コントラクトと比較します。 |
| `CapacityLedger` |オンチェーン/オフチェーンのレジストリは、活動容量の宣言、レプリケーションの順序、手数料の計算基準を確認します。 |スマート コントラクトまたはオフチェーン サービスのスタブのモジュロとスナップショットの決定。 |
| `MarketplacePolicy` |政治的政治は、ステークの最小化、聴衆の要件、刑罰の制限を定義します。 | `sorafs_manifest` の構成構造体 + ゴベルナンザのドキュメント。 |

### Esquemas 実装 (エスタド)

## デグロース デ トラバホ

### 1. 規制対応レジストリ

|タレア |責任者 |メモ |
|------|--|------|
| `CapacityDeclarationV1`、`ReplicationOrderV1`、`CapacityTelemetryV1` を定義します。 | Equipo de Storage / ゴベルナンザ | Usar Norito;バージョンのセマンティコと容量の参照が含まれます。 |
| `sorafs_manifest` のパーサー + バリデータのモジュールを実装します。 |ストレージの装備 |詐欺師 ID は単調、容量の制限、賭け金の要求。 |
| Perfil のチャンカー レジストリからのエクステンダー メタデータ `min_capacity_gib`。 |ツーリングWG |アユダは、クライアントに必要な最小限のハードウェアを不正に使用しています。 |
| `MarketplacePolicy` のガードレールの入場と刑罰のカレンダーに関する文書を編集します。 |ガバナンス評議会 |政治のデフォルトに関する文書も公開されています。 |

#### エスケーマの定義 (実装)- `CapacityDeclarationV1` プロバイダーごとの容量のキャプチャの侵害、正規チャンカーの処理、容量の参照、レーンごとのオプションのキャップ、価格設定のピスタ、メタデータの検証が含まれます。 La validacion asegura stake no cero, handle canonicos, aliases deduplicados, caps por LANE dentro del total declarado y contabilidad de GiB monotónica.【crates/sorafs_manifest/src/capacity.rs:28】
- `ReplicationOrderV1` は、冗長オブジェクト、SLA および割り当ての保証内容を含む、指定された内容を示します。 los validadores imponen は canonicos、プロバイダーの unicos と期限前制限 Torii を処理し、レジストリを管理します。【crates/sorafs_manifest/src/capacity.rs:301】
- `CapacityTelemetryV1` の epoca の高速スナップショット (GiB 宣言対米国、レプリカのコンタドール、稼働時間/PoR) の料金配布。 Las validaciones mantienen el uso dentro de la declaracion y los porcentajes dentro de 0-100%.【crates/sorafs_manifest/src/capacity.rs:476】
- ヘルパー コンパルティド (`CapacityMetadataEntry`、`PricingScheduleV1`、レーン/割り当て/SLA の検証) は、キーの検証、エラーの報告、CI の報告、ツールのダウンストリームの再利用を証明します。【crates/sorafs_manifest/src/capacity.rs:230】
- `PinProviderRegistry` `/v1/sorafs/capacity/state` 経由でオンチェーンでスナップショットを公開、プロバイダーと料金台帳の組み合わせ宣言 Norito JSON determinista.【crates/iroha_torii/src/sorafs/registry.rs:17】【crates/iroha_torii/src/sorafs/api.rs:64】
- 正規の処理、重複の検出、レーンの制限、レプリカの割り当ての保護、テレメトリアのランゴ チェック、ラス レグレシオネス アパレスカンと CI のメディアのチェックを処理するための強制的な検証。【crates/sorafs_manifest/src/capacity.rs:792】
- オペラドールのツール: `sorafs_manifest_stub capacity {declaration, telemetry, replication-order}` ペイロードの読み取り可能な特定のアクションを表示 Norito canonicos、BLOB Base64 y resúmenes JSON para que los operadores preparen fixtures de `/v1/sorafs/capacity/declare`、`/v1/sorafs/capacity/telemetry` y ordenes deローカルの複製と検証。【crates/sorafs_car/src/bin/sorafs_manifest_stub/capacity.rs:1】 `fixtures/sorafs_manifest/replication_order/` (`order_v1.json`、`order_v1.to`) は、`cargo run -p sorafs_car --bin sorafs_manifest_stub -- capacity replication-order` 経由で参照されます。

### 2. 計画的な制御の統合

|タレア |責任者 |メモ |
|------|--|------|
| Agregar ハンドラー Torii `/v1/sorafs/capacity/declare`、`/v1/sorafs/capacity/telemetry`、`/v1/sorafs/capacity/orders` とペイロード Norito JSON。 | Torii チーム | Reflejar la logica del validador; reutilizar ヘルパー Norito JSON。 |
| `CapacityDeclarationV1` のスナップショット、スコアボード、フェッチ、ゲートウェイのプレーンのメタデータをプロパガーします。 |ツーリングWG / オーケストレーター設備 |エクステンダー `provider_metadata` は、マルチソースのスコアリングに関するパラメータの参照により、レーンの制限を制限します。 |
|フェールオーバーのヒントの割り当てやゲートウェイのクライアントに対するレプリケーションの指示が行われます。 |ネットワーキング TL / ゲートウェイ チーム |エルビルダーデルスコアボードは、ゴベルナンザによってオルデネスファームダスを消費します。 |
|ツール CLI: エクステンダー `sorafs_cli` コン `capacity declare`、`capacity telemetry`、`capacity orders import`。 |ツーリングWG | Proveer JSON 決定者 + スコアボードのサリダ。 |

### 3. 市場と統治の政治

|タレア |責任者 |メモ |
|------|--|------|
| Ratificar `MarketplacePolicy` (ステークミニモ、刑罰の数、聴衆の声)。 |ガバナンス評議会 |ドキュメントと改訂履歴をキャプチャーして公開します。 |
|議会は、議会の承認、改修および廃止の宣言を行うために、フックを作成します。 |ガバナンス評議会 / スマートコントラクトチーム |イベント Norito + マニフェストの摂取。 |
| SLA の電報に対する罰則の適用 (手数料の減額、保証金の削減) を実施します。 |ガバナンス評議会 / 財務 |直線コン出力は `DealEngine` で決済されます。 |
|議論とエスカラミエントのプロセスに関するドキュメンタリー。 |ドキュメント / ガバナンス |議論のランブックと CLI のヘルパーの Vincular。 |

### 4. 医薬品と手数料の配布|タレア |責任者 |メモ |
|------|--|------|
| `CapacityTelemetryV1` を使用して、Torii の測定を展開します。 | Torii チーム |有効な GiB 時間、PoR の終了、稼働時間。 |
| `sorafs_node` のレポート利用状況と SLA の統計に関するパイプラインの計測を実際に行います。 |ストレージチーム |チャンカーのレプリカ処理の線形制御。 |
|決済パイプライン: テレメトリ変換と XOR での複製データの変換、管理者と登録者の台帳のリストの作成。 |財務/保管チーム |取引エンジン/財務省のエクスポートに関するコネクタ。 |
|計測に関するエクスポート ダッシュボード/アラート (取り込みのバックログ、テレメトリが古い)。 |可観測性 | SF-6/SF-7 の Grafana 参照エクステンダー パック。 |

- Torii アホラ expone `/v1/sorafs/capacity/telemetry` y `/v1/sorafs/capacity/state` (JSON + Norito) パラ ケ オペラドールの環境スナップショット デ テレメトリア ポー エポカとロスの検査を回復し、元帳のカノニコ パラ オーディトリアス エンパケタドを確認します。証拠.【crates/iroha_torii/src/sorafs/api.rs:268】【crates/iroha_torii/src/sorafs/api.rs:816】
- 統合 `PinProviderRegistry` は、ミスモ エンドポイントのショーン アクセス可能な複製の順序を確認します。 CLI のヘルパー (`sorafs_cli capacity telemetry --from-file telemetry.json`) は、ハッシュ決定とエイリアス解決の自動化を目的とした公開テレメトリアの検証を行います。
- 測定生成されたスナップショット `CapacityTelemetrySnapshot` フィジャーダス アル スナップショット `metering`、ロス エクスポート Prometheus 食品テーブルロ Grafana インポート リスト `docs/source/grafana_sorafs_metering.json` エクイポス パラメータ実際の監視の GiB 時間の累積、料金 nano-SORA の実際の SLA の合計。【crates/iroha_torii/src/routing.rs:5143】【docs/source/grafana_sorafs_metering.json:1】
- `smoothed_gib_hours` と `smoothed_por_success_bps` を含むスムージングとメーターの測定、スナップショットを含めて、EMA フレンテと米国のクルーズ船の価値を比較します。 pagos.【crates/sorafs_node/src/metering.rs:401】

### 5. 紛争と取り消しの請求

|タレア |責任者 |メモ |
|------|--|------|
|ペイロード `CapacityDisputeV1` を定義します (要求、証拠、プロバイダーのオブジェクト)。 |ガバナンス評議会 |エスケマ Norito + バリダドール。 |
| CLI の最初の紛争対応者 (con adjuntos de evidencia)。 |ツーリングWG | Asegurar のハッシュは証拠のバンドルを決定します。 |
| Agregar は、SLA 反復 (自動エスカレード、議論) を自動的にチェックします。 |可観測性 |安全な傘とゴベルナンザのフック。 |
|取り消しのためのシナリオのドキュメンタリー (グラシアの期間、ピナドスの避難)。 |ドキュメント / ストレージ チーム | Vincular は政治文書とオペラの運用手順書です。 |

## CI のテストの要件

- テスト単位をテストします (`sorafs_manifest`)。
- シミュレーションによる統合テスト: 宣言 -> 複製の順序 -> 計測 -> 支払い。
- CI パラ再ジェネラー宣言/テレメトリアの容量性と管理性のワークフロー (エクステンダー `ci/check_sorafs_fixtures.sh`)。
- レジストリから API をテストします (10,000 のプロバイダー、100,000 のオルデンをシミュレート)。

## テレメトリアとダッシュボード

- ダッシュボードのパネル:
  - プロバイダーによる Capacidad declarada と utilizada の比較。
  - バックログの複製順序と割り当て計画のデモ。
  - SLA の完了 (稼働率 %、PoR 終了)。
  - エポカによる手数料や罰則の発生。
- アラート:
  - 最小限の容量を提供するプロバイダー。
  - レプリケーションの順序 > SLA。
  - パイプラインの計量の秋。

## ドキュメントの作成可能

- オペレータの容量宣言、リノベーションの妥協と監視の利用に関するガイア デ オペレータ。
- 政治的宣言、エミミール・オルデネスおよびマネハル論争に関するガイア・デ・ゴベルナンサ。
- API パラメタのエンドポイントの容量とレプリケーションの形式を参照します。
- 開発者向けのマーケットプレイスに関する FAQ。

## GA の準備チェックリスト

ロードマップ **SF-2c** の項目は、具体的な証拠と製造におけるロールアウトのブロック図です
感染状況の把握、紛争の解決、オンボーディングの管理。米国ロス・アーティファクト・シギエンテス
実装の同期と受け入れ基準を管理します。### 夜行性と和解 XOR
- スナップショットの保存と台帳のエクスポート XOR パラミスマ
  ベンタナ、ルエゴ・エヘクタ：
  ```bash
  python3 scripts/telemetry/capacity_reconcile.py \
    --snapshot artifacts/sorafs/capacity/state_$(date +%F).json \
    --ledger artifacts/sorafs/capacity/ledger_$(date +%F).ndjson \
    --label nightly-capacity \
    --json-out artifacts/sorafs/capacity/reconcile_$(date +%F).json \
    --prom-out "${SORAFS_CAPACITY_RECONCILE_TEXTFILE:-artifacts/sorafs/capacity/reconcile.prom}"
  ```
  エル・ヘルパー・セール・コン・コディゴ・ノー・セロ・シ・ヘイ・和解、ペナルティザシオネス・ファルタンテス/エクシーバス・イ
  Prometheus をテキストファイル形式で再開します。
- 警告 `SoraFSCapacityReconciliationMismatch` (en `dashboards/alerts/sorafs_capacity_rules.yml`)
  調整レポートのギャップを解消する。ダッシュボードを生き生きとさせる
  `dashboards/grafana/sorafs_capacity_penalties.json`。
- JSON とハッシュのアーカイブ `docs/examples/sorafs_capacity_marketplace_validation/`
  ジュント・コン・ロス・パケテス・デ・ゴベルナンサ。

### 論争と斬り込みの証拠
- `sorafs_manifest_stub capacity dispute` 経由での議論の提示 (テスト:
  `cargo test -p sorafs_car --test capacity_cli`) パラマンテナーペイロード canonicos。
- Ejecuta `cargo test -p iroha_core -- capacity_dispute_replay_is_deterministic` とラススイート
  法廷での罰則 (`record_capacity_telemetry_penalises_persistent_under_delivery`)
  論争や論争は、決定的な再現を意味します。
- Sigue `docs/source/sorafs/dispute_revocation_runbook.md` 証拠とエスカラミエントのキャプチャ。
  ストライキの攻撃と検証の報告。

### プロバイダーのオンボーディングとスモーク テストの終了
- 宣言/テレメトリア コン `sorafs_manifest_stub capacity ...` y の再作成
  送信前に CLI をテストする必要があります (`cargo test -p sorafs_car --test capacity_cli -- capacity_declaration`)。
- Envialos via Torii (`/v1/sorafs/capacity/declare`) y luego captura `/v1/sorafs/capacity/state` mas
  Grafana のスクリーンショット。シグ エル フルホ デ サリダ en `docs/source/sorafs/capacity_onboarding_runbook.md`。
- 調停に関する企業成果物のアーカイブ
  `docs/examples/sorafs_capacity_marketplace_validation/`。

## 依存関係と優先順位

1. ターミナル SF-2b (政治的承認) — 市場はプロバイダーの検証に依存します。
2. Torii の統合に向けて、エスケーマ + レジストリ (este doc) を実装します。
3. 利用者への支払いを計量する完全なパイプライン。
4. パソ決勝: 不正行為による手数料の分配をコントロールする
   ステージング時に測定を検証します。

エステドキュメントを参照するためのロードマップの進捗状況を確認します。アクチュアリーザ エル
市長選挙のロードマップ (エスケマ、計画管理、統合、計測、
manejo de disputas) alcance estado 機能が完了しました。