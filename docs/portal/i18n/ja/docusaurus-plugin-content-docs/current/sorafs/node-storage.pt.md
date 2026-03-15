---
lang: ja
direction: ltr
source: docs/portal/docs/sorafs/node-storage.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: ノードストレージ
title:デザイン・ド・ストレージ・ドゥ・ノド SoraFS
Sidebar_label: ストレージのデザインとノード
説明: ストレージのアーキテクト、ノード Torii のシステム インターフェイス SoraFS のクォータとフック。
---

:::note フォンテ カノニカ
エスタ・ページナ・エスペルハ`docs/source/sorafs/sorafs_node_storage.md`。マンテンハ・アンバスはコピア・シンクロニザダスとして。
:::

## ストレージの設計 SoraFS (ドラフト)

Esta nota refina como um nodo Iroha (Torii) は、どのような状況でも最適です
SoraFS の利用可能性は、ディスコのローカルパラレルでの教育を目的としています
アルマゼナールとサーバルのチャンク。特定の発見を補完する
`sorafs_node_client_protocol.md` e o trabalho de fixtures SF-1b ao detalhar a
ストレージ、再帰制御、配管の設計
ゲートウェイのノードやカミンホを設定する必要はありません。
OS ドリル操作フィカムなし
[オペラの実行手順書](./node-operations)。

### オブジェクト

- Iroha ディスコの補助プロセスを許可します
  責任が台帳に反映されると、SoraFS sem afetar が証明されました。
- Norito のストレージ決定方法の管理: マニフェスト、
  チャンクの平面、SAO の広告の Proof-of-Retrievability (PoR) を高める
  フォンテ・ド・ベルダデ。
- 割り当てを決定して、ペロ オペラドール パラ ケ ウム ノードをインポートします。
  再帰的に、あなたはピンを取得する必要があります。
- ソース/テレメトリアのエクスポート (Amostragem PoR、チャンクのフェッチの遅延、プレッサオデ
  ディスコ）デ・ボルタ・パラ・ガバナンカ・エ・クライアント。

### アルトニベル建築

```
+--------------------------------------------------------------------+
|                         Iroha/Torii Node                           |
|                                                                    |
|  +----------+      +----------------------+                        |
|  | Torii APIs|<---->|    SoraFS Gateway   |<---------------+       |
|  +----------+      |  (Norito endpoints)  |                |       |
|                    +----------+-----------+                |       |
|                               |                            |       |
|                    +----------v-----------+                |       |
|                    |     Pin Registry     |<---- manifests |       |
|                    |     (State / DB)     |                |       |
|                    +----------+-----------+                |       |
|                               |                            |       |
|                    +----------v-----------+                |       |
|                    |     Chunk Storage    |<---- chunk plans|       |
|                    |      (ChunkStore)    |                |       |
|                    +----------+-----------+                |       |
|                               |                            |       |
|                    +----------v-----------+                |       |
|                    |    Disk Quota/IO     |--pin/serve----->| Fetch |
|                    |      Scheduler       |                | Clients|
|                    +----------------------+                |       |
|                                                                    |
+--------------------------------------------------------------------+
```

モジュロ カーブ:

- **ゲートウェイ**: エンドポイントの公開 HTTP Norito ピンの提案、要求
  チャンクの取得、PoR およびテレメトリの管理。検証ペイロード Norito e
  encaminha requisicoes パラオチャンクストア。スタック HTTP 実行 Torii パラメータ
  evitar um novo デーモン。
- **ピン レジストリ**: マニフェスト rastreado em `iroha_data_model::sorafs` のピン レジストリ
  e `iroha_core`。 Quandoum マニフェストとセキュリティ レジストリ レジストリ ダイジェスト ドゥ
  マニフェスト、ダイジェストはチャンクのプラン、Raiz PoR e flags de capacidade は証明されます。
- **チャンク ストレージ**: ディスコのマニフェストを実装する `ChunkStore`
  アッシナドス、マテリザ プラノス デ チャンク ウサンド `ChunkProfile::DEFAULT`、e
  チャンクのレイアウトを決定的に永続化します。 Cada チャンクとアソシエード ウム
  指紋認証とメタデータの PoR パラケラの指紋認証の有効性
  セイム・リラー・オ・アルキーヴォ・インテイロ。
- **クォータ/スケジューラ**: 設定操作の制限を制限します (最大バイト数)
  ディスコ、ピンペンデンテス最大、フェッチパラレロス最大、TTL デチャンク）e 座標 IO
  タレファスが元帳を実行するようにパラケムを再帰します。おお、スケジューラがやってくれます
  CPU 制限に関するサービス要求に対する応答。

### 設定

Adicione uma nova secao em `iroha_config`:

```toml
[sorafs.storage]
enabled = false
data_dir = "/var/lib/iroha/sorafs"
max_capacity_bytes = "100 GiB"
max_parallel_fetches = 32
max_pins = 10_000
por_sample_interval_secs = 600
alias = "tenant.alpha"            # tag opcional legivel
adverts:
  stake_pointer = "stake.pool.v1:0x1234"
  availability = "hot"
  max_latency_ms = 500
  topics = ["sorafs.sf1.primary:global"]
```

- `enabled`: 参加を切り替えます。 Quando false o ゲートウェイ レトルナ 503 パラ
  ストレージのエンドポイントをノードから検出します。
- `data_dir`: チャンクの管理、テレメトリの監視
  取ってくる。デフォルトは `<iroha.data_dir>/sorafs`。
- `max_capacity_bytes`: チャンク ピナドの制限が制限されています。うまタレファで
  限界を超えて新しいピンをピン留めします。
- `max_parallel_fetches`: スケジューラ パラメータの制限
  バランサー バンダ/IO デ ディスコ コントラ ア カルガ ド バリダドール。
- `max_pins`: マニフェストの最大のピンとアプリケーションの確認
  エヴィッカオ/バックプレッシャー。
- `por_sample_interval_secs`: PoR の自動ジョブのカデンシア。
  Cada ジョブ アモストラ `N` は (マニフェストによる構成) イベントを発行します
  テレメトリア。 `N` 形式の決定性を定義するガバナンス エスカラー `N`
  メタデータ `profile.sample_multiplier` (インテイロ `1-4`) を保存します。おお勇気ある人よ
  ser um numero/string unico ou um objeto com は perfil や exemplo によってオーバーライドされます
  `{"default":2,"sorafs.sf2@1.0.0":3}`。
- `adverts`: 米国の教育機関の広告を掲載する
  `ProviderAdvertV1` (ステーク ポインター、QoS のヒント、トピック)。せおみどをのど
  米国のデフォルトは統治レジストリを実行します。

配管の構成:

- `[sorafs.storage]` e 定義 `iroha_config` como `SorafsStorage` e e
  carregado は arquivo de config を実行し、nodo を実行します。
- `iroha_core` および `iroha_torii` ビルダーのストレージ設定をパスします
  ゲートウェイ e o チャンク ストアを起動しません。
- 開発/テストの存在をオーバーライドします (`SORAFS_STORAGE_*`、`SORAFS_STORAGE_PIN_*`)。
  構成情報を必要とせずに、開発環境を展開します。### CLI のユーティリティ

Enquanto a superficie HTTP do Torii ainda esta sendo ligada, o crate
`sorafs_node` envia uma CLI レベル パラ ケ オペラドール ポッサム オートマティザー ドリル デ
取り込み/エクスポートとバックエンドの永続化。 [crates/sorafs_node/src/bin/sorafs-node.rs:1]

```bash
cargo run -p sorafs_node --bin sorafs-node ingest \
  --data-dir ./storage/sorafs \
  --manifest ./fixtures/manifest.to \
  --payload ./fixtures/payload.bin \
  --plan-json-out ./plan.json
```

- `ingest` マニフェストの説明 `.to` ペイロードのコード化 Norito ペイロードのバイト数
  特派員。部分的な処理を行うための計画を再構築する
  チャンク化はマニフェストを実行し、ダイジェストでパリダーを実行し、チャンクのデータを永続化します。
  オプションで BLOB JSON `chunk_fetch_specs` パラケ ツールをダウンストリームに出力します
  レイアウトが有効です。
- `export` マニフェスト/ペイロードのセキュリティ ID がマニフェストに含まれています
  ディスコ (オプションの JSON プラン) パラケ フィクスチャーは継続的に再現されます。

Ambos os comandos imprimem um resumo Norito JSON em stdout、facilitando em
スクリプト。 CLI の安全性に関するマニフェストの統合テスト
API Torii エントリのペイロードは往復で修正されます。 [crates/sorafs_node/tests/cli.rs:1]

> パリダードHTTP
>
> O ゲートウェイ Torii agora expoe ヘルパー読み取り専用アポイアドス ペロ メスモ
> `NodeHandle`:
>
> - `GET /v2/sorafs/storage/manifest/{manifest_id_hex}` - レトルナまたはマニフェスト
> Norito armazenado (base64) junto com ダイジェスト/メタデータ。 [crates/iroha_torii/src/sorafs/api.rs:1207]
> - `GET /v2/sorafs/storage/plan/{manifest_id_hex}` - チャンクのレトルナ
> deterministico JSON (`chunk_fetch_specs`) パラツールのダウンストリーム。 [crates/iroha_torii/src/sorafs/api.rs:1259]
>
> Esses エンドポイント espelham a Saida do CLI パラケ パイプライン ポッサム トロカール
> スクリプト ロケール プローブ HTTP sem mudar パーサー。 [crates/iroha_torii/src/sorafs/api.rs:1207] [crates/iroha_torii/src/sorafs/api.rs:1259]

### シクロ デ ヴィーダ ド ノド

1. **起動**:
   - ストレージ エスティバー ハビリタド、ノード初期化、チャンク ストア コム
     容量設定の管理。ベースの検証を含めます
     マニフェスト PoR e Fazer リプレイのダドス マニフェスト パラ アクアサー
     キャッシュ。
   - ゲートウェイ SoraFS を行うロータとしてのレジストラ (エンドポイント Norito JSON POST/GET パラ ピン、
     フェッチ、アモストレム PoR、テレメトリア）。
   - 労働者が割り当て量を監視するための管理を開始します。
2. **発見/広告**:
   - Gerar documentos `ProviderAdvertV1` usando capacidade/saude atual, assinar
     com a provada pelo conselho e publicar via Discovery.リスタを利用する
3. **フラクソ デ ピン**:
   - ゲートウェイがマニフェストを受け取ります (チャンク、Raiz PoR を含む)
     アッシナチュラス・ド・コンセーリョ）。エイリアスのリストを検証 (`sorafs.sf1@1.0.0`)
     要求される) チャンクがメタデータのマニフェストに対応することを保証します。
   - Verifica の割り当て。静電容量/ピンの制限を超えた場合は、対応してください
     デ・ポリティカ (Norito estruturado)。
   - `ChunkStore` のチャンクのストリーム、検証期間のダイジェスト
     摂取。 Atualiza 氏は、armazena のメタデータにはレジストリが存在しないと主張しています。
4. **Fluxo デフェッチ**:
   - ディスコのパーティーに必要な料理を提供します。おお、スケジューラーの悪者よ
     `max_parallel_fetches` 電子レトルナ `429` Quando Saturado。
   - テレメトリア エストルトゥラーダ (Norito JSON) com latencia、bytes servidos e を送信します。
     下流のエロパラモニターメントを監視します。
5. **Amostragem PoR**:
   - 労働者よ、選択はペソに比例して現れます (例、バイト数)
     armamazenados) は、チャンク ストアの PoR を決定的に使用します。
   - 広告を含む行政監査の結果を保持します
     テレメトリの証明/エンドポイント。
6. **エヴィッカオ / ノルマの達成**:
   - パドラオの新たな目標を達成するために、容量を減らしてください。オプシオナルメンテ、
     オペラドール ポデム構成政治政治 (例: TTL、LRU) およびモデル
     デ・ガバナンカ・エスティベル・デフィニド。クォータ推定値を前提とした設計
     operacoes de unpin iniciadas pelo operador。

### 容量の宣言とスケジュールの統合の宣言- `CapacityDeclarationRecord` と `/v2/sorafs/capacity/declare` の Torii 更新情報
  パラ o `CapacityManager` embutido、de modo que cada nodo constroi uma visao em meria
  デ・スアス・アロカコスはチャンカー・レーンとコンプロメティダスです。 O マネージャーはスナップショットを読み取り専用で公開します
  テレメトリアに関する (`GET /v2/sorafs/capacity/state`) レーンの権限を無効にする
  アンテス・デ・ノバス・オルデンス・セレム・アシータス。 [crates/sorafs_node/src/capacity.rs:1] [crates/sorafs_node/src/lib.rs:60]
- O エンドポイント `/v2/sorafs/capacity/schedule` aceita ペイロード `ReplicationOrderV1`
  エミミドス・ペラ・ガバナンカ。奇跡を命令し、ローカルを証明し、マネージャーを検証する
  重複のスケジュール、チャンカー/レーンの容量の検証、スライスとレトルナの予約
  うーん、`ReplicationPlan` 容量制限が発生しています。
  orquestracao possam seguir com a ingestao. Ordens para outros provedores sao
  reconhecidas com resposta `ignored` パラ ファシリター ワークフロー マルチ オペレーター。 [crates/iroha_torii/src/routing.rs:4845]
- 結論のフック (例を示し、次のことを実行します) チャママム
  `POST /v2/sorafs/capacity/complete` パラ リベラル リザーバス経由
  `CapacityManager::complete_order`。スナップショットを含むレスポスタ
  `ReplicationRelease` (残りの残り、チャンカー/レーン) パラケ ツール
  近くのオーデム sem ポーリングを実行できます。トラバーリョ フトゥロ
  パイプラインは、チャンク ストアの論理的な取り込みを実行します。
  [crates/iroha_torii/src/routing.rs:4885] [crates/sorafs_node/src/capacity.rs:90]
- O `TelemetryAccumulator` embutido pode ser mutado via
  `NodeHandle::update_telemetry`、バックグラウンド登録の労働者を許可します
  PoR/稼働時間と最終的な派生ペイロードのカノニコス
  `CapacityTelemetryV1` sem tocar nos internos do スケジューラ。 [crates/sorafs_node/src/lib.rs:142] [crates/sorafs_node/src/telemetry.rs:1]

### インテグラコエスとトラバルホの未来

- **ガバナンカ**: estender `sorafs_pin_registry_tracker.md` com telemetria de
  ストレージ (成功した PoR、ディスコの利用法)。政治政策の承認
  安全な広告を最小限に抑えるために、最小限の容量を確保してください。
- **クライアントの SDK**: ストレージの新しい構成をエクスポートします (ディスコの制限、エイリアス)
  ポッサム ブートストラップ ノードのプログラムを実行します。
- **テレメトリア**: メトリクスの統合スタックが存在します (Prometheus /
  OpenTelemetry) ストレージカメラとダッシュボードの監視のための指標を提供します。
- **Seguranca**: 非同期コム プールのストレージ モジュールのロッド
  バックプレッシャーとプールのio_uringによるチャンクのサンドボックス化を考慮する
  東京のリミッタード パラ エヴィター ケ クライアントは、マリシオーソス エスゴテム リカーソスです。

ストレージのオプションと決定性を考慮した最適な設計管理
OS ノブの必要なパラ オペラドール参加者による可用性の確認
SoraFS。 `iroha_config`、`iroha_core`、`iroha_torii` を実行します。
ゲートウェイ Norito はありません。ツールを使用して広告と証明を行うことができます。