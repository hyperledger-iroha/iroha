---
lang: ja
direction: ltr
source: docs/portal/docs/sorafs/node-storage.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: ノードストレージ
タイトル: ディセーニョ デ アルマセナミエント デル ノード SoraFS
サイドバー_ラベル: アルマセナミエント デル ノドのディセーニョ
説明: アルマセナミエントの建築物、Torii と SoraFS の接続部分のフックを作成します。
---

:::メモ フエンテ カノニカ
`docs/source/sorafs/sorafs_node_storage.md` のページを参照してください。スフィンクスの引退を記念して、記録を作成する必要があります。
:::

## ディセーニョ デ アルマセナミエント デル ノード SoraFS (ボラドール)

Iroha (Torii) は、自分の能力を最大限に発揮するための情報を提供します
SoraFS のデータの入手可能性とディスコ ローカル パラメタの提供
アルマセナルとサーバルのチャンク。発見の特定性を補完する
`sorafs_node_client_protocol.md` SF-1b の詳細なフィクスチャーの管理
ストレージの建築、再帰操作とプロメリアの制御
ゲートウェイのエンドポイントやノードの構成を変更します。
生きたオペラの実践
[ノードの運用手順書](./node-operations)。

### オブジェクト

- Iroha exponga ディスコの手続きを許可します。
  ocioso como proveedor SoraFS sin afectar las responsabilidades del 元帳。
- Norito による決定とガイドの管理方法:
  マニフェスト、プレーンのチャンク、レイス Proof-of-Retrievability (PoR) および広告
  プルードール・ソン・ラ・フェンテ・デ・ベルダド。
- オペラ座の決定を無効にすることは、私たちの計画を無視することです
  ピンを取得するためのさまざまな要求を再帰的に受け入れます。
- エクスポナー サルード/テレメトリア (PoR の検査、チャンクのフェッチの遅延、プレシオン)
  デ・ディスコ）ハシア・ゴベルナンザと顧客。

### アルトニベル建築

```
┌──────────────────────────────────────────────────────────────────────┐
│                         Iroha/Torii Node                             │
│                                                                      │
│  ┌──────────────┐      ┌────────────────────┐                        │
│  │  Torii APIs  │◀────▶│   SoraFS Gateway   │◀───────────────┐       │
│  └──────────────┘      │ (Norito endpoints) │                │       │
│                        └────────┬───────────┘                │       │
│                                 │                            │       │
│                        ┌────────▼────────┐                   │       │
│                        │  Pin Registry   │◀───── manifests   │       │
│                        │ (State / DB)    │                   │       │
│                        └────────┬────────┘                   │       │
│                                 │                            │       │
│                        ┌────────▼────────┐                   │       │
│                        │  Chunk Storage  │◀──── chunk plans  │       │
│                        │  (ChunkStore)   │                   │       │
│                        └────────┬────────┘                   │       │
│                                 │                            │       │
│                        ┌────────▼────────┐                   │       │
│                        │  Disk Quota/IO  │─Pin/serve chunks─▶│ Fetch │
│                        │  Scheduler      │                   │ Clients│
│                        └─────────────────┘                   │       │
│                                                                      │
└──────────────────────────────────────────────────────────────────────┘
```

モドゥロ・クラーベ:

- **ゲートウェイ**: エンドポイントの HTTP Norito の要求、要求を説明します
  チャンクを取得し、PoR とテレメトリを測定します。検証ペイロード Norito y
  enruta las solicitudes ハシアエルチャンクストア。 Torii のスタック HTTP
  ヌエボ デーモンのパラ エビタール。
- **ピン レジストリ**: `iroha_data_model::sorafs` の rastreado マニフェストのピン
  e `iroha_core`。マニフェストを受け入れ、ダイジェストを登録します
  マニフェスト、ダイジェスト、プラン、チャンク、PoR およびフラグの容量デル
  証明者。
- **チャンク ストレージ**: `ChunkStore` ディスコ クエリの実装
  マニフェスト フィルマド、マテリザ プレーン デ チャンク ウサンド `ChunkProfile::DEFAULT`
  レイアウトを決定せずにチャンクを永続化します。 Cada chunk se asocia con un
  PoR に関する情報とメタデータの指紋
  罪は完全にアーカイブされます。
- **クォータ/スケジューラ**: オペランドの設定を制限します (最大バイト数)
  デ ディスコ、パインズ ペンディエンテス マキシモス、フェッチ パラレロス マキシモス、TTL デ チャンク)
  y 座標 IO パラケラスタレアスデル台帳は、再帰的な罪を犯しません。エル
  スケジューラは、CPU に関する問題についての優先的な要求を実行します。

### 構成

`iroha_config` の新しいセクションの合計:

```toml
[sorafs.storage]
enabled = false
data_dir = "/var/lib/iroha/sorafs"
max_capacity_bytes = "100 GiB"
max_parallel_fetches = 32
max_pins = 10_000
por_sample_interval_secs = 600
alias = "tenant.alpha"            # etiqueta opcional legible
adverts:
  stake_pointer = "stake.pool.v1:0x1234"
  availability = "hot"
  max_latency_ms = 500
  topics = ["sorafs.sf1.primary:global"]
```

- `enabled`: 参加を切り替えます。ゲートウェイが誤って応答した場合
  503 パラエンドポイントのストレージとノードの検出を通知します。
- `data_dir`: チャンク、テレメトリのデータ管理ディレクトリ
  取ってくる。デフォルトは `<iroha.data_dir>/sorafs` です。
- `max_capacity_bytes`: チャンクの期間は限られています。うなたれで
  ヌエボスのピンは、アルカンサ エル リミテに保存されます。
- `max_parallel_fetches`: スケジューラ パラメータに関する同意事項の説明
  平衡アンチョ・デ・バンダ/IOデ・ディスコ・コン・ラ・カルガ・デル・バリダドール。
- `max_pins`: マニフェストの最大数のピンが受け入れられるかどうかを確認します
  アプリカーエビクション/バックプレッシャー。
- `por_sample_interval_secs`: 自動音楽のカデンシア
  PoR。 Cada trabajo muestrea `N` hojas (マニフェストごとに構成可能) を発行します
  テレメトリのイベント。 La gobernanza puede escalar `N` deforma determinista
  メタデータ `profile.sample_multiplier` (エンテロ `1-4`) を確立します。
  El valor puede ser un número/string único o un objeto con は perfil をオーバーライドします。
  例 `{"default":2,"sorafs.sf2@1.0.0":3}`。
- `adverts`: 完全なキャンペーンに基づく広告の構造
  `ProviderAdvertV1` (ステーク ポインター、QoS のヒント、トピック)。シセオミテエル
  nodo usa はデフォルトのレジストリ デ ゴベルナンザを設定します。

構成プロメリア:- `[sorafs.storage]` は `iroha_config` と `SorafsStorage` を定義します。
  構成ノードのアーカイブです。
- `iroha_core` y `iroha_torii` ストレージの構成を変更しました
  ビルダーとゲートウェイとチャンク ストアとアランク。
- 開発/テスト (`SORAFS_STORAGE_*`、`SORAFS_STORAGE_PIN_*`) に関するオーバーライドが存在します。
  設定のアーカイブに関する詳細な製品の設計。

### CLI の活用法

Torii ケーブル、電子箱の最上位 HTTP の管理
`sorafs_node` には、CLI リビアナ パラ ケ ロス オペラドール プエダンが含まれます
自動バックエンド永続的な取り込み/エクスポートのドリル。【crates/sorafs_node/src/bin/sorafs-node.rs:1】

```bash
cargo run -p sorafs_node --bin sorafs-node ingest \
  --data-dir ./storage/sorafs \
  --manifest ./fixtures/manifest.to \
  --payload ./fixtures/payload.bin \
  --plan-json-out ./plan.json
```

- `ingest` マニフェストの説明 `.to` Norito ペイロードのバイト数が失われています
  特派員。部分的な部分の計画を再構築する
  マニフェストのチャンク化、ダイジェストのパリダーの無効化、チャンクのアーカイブの永続化、
  BLOB の JSON `chunk_fetch_specs` を発行するオプションがあります。
  ダウンストリームの有効な EL レイアウト。
- `export` マニフェスト/ペイロードの ID を受け入れます
  ディスコ (オプションの JSON プラン) パラケロスの試合、シガン、シエンドの再現可能
  アントレ・エントルノス。

アンボスコマンドは再開を妨げます Norito JSON の標準出力、簡単な使用
スクリプト。安全な環境での統合のための CLI の開発
マニフェストとペイロードを再構成して、問題が発生する前に修正する
Torii の API。【crates/sorafs_node/tests/cli.rs:1】

> パリダッドHTTP
>
> ゲートウェイ Torii アホラ expone ヘルパー デ ソロ講義レスパルダードス ポル ミスモ
> `NodeHandle`:
>
> - `GET /v2/sorafs/storage/manifest/{manifest_id_hex}` — 開発マニフェスト
> Norito アルマセナド (base64) ジュント コン ダイジェスト/メタデータ。【crates/iroha_torii/src/sorafs/api.rs:1207】
> - `GET /v2/sorafs/storage/plan/{manifest_id_hex}` — チャンクの開発計画
> determinista JSON (`chunk_fetch_specs`) パラツールのダウンストリーム。【crates/iroha_torii/src/sorafs/api.rs:1259】
>
> Estos エンドポイント reflejan la salida del CLI パラ ケ ロス パイプライン プエダン
> スクリプトのロケールとプローブの HTTP 罪のカンビアパーサーのパス。【crates/iroha_torii/src/sorafs/api.rs:1207】【crates/iroha_torii/src/sorafs/api.rs:1259】

### シクロ デ ヴィーダ デル ノド1. **アランク**:
   - 初期のチャンク ストア コンストラクションを利用できるようになりました。
     ディレクトリと容量の設定。 Esto には認証情報が含まれます
     ベース・デ・データ・マニフェスト PoR y reproducir マニフェスト・フィジャドス・パラ・カレンダー
     キャッシュ。
   - レジストラー デル ゲートウェイ SoraFS (エンドポイント Norito JSON POST/GET パラ ピン、
     フェッチ、ムエストレオ PoR、テレメトリア）。
   - 管理者は警察の監視を担当します。
2. **発見/広告**:
   - 一般文書 `ProviderAdvertV1` usando la capacidad/salud 実際、
     発見により、企業はクラーベの方針を決定し、公開されます。
     米国の新しいリスト `profile_aliases` は、CANONICOS Y を処理します
3. **フルホ デ ピン**:
   - マニフェスト ファームのゲートウェイ レシピ (チャンクの計画、PoR の作成を含む)
     フィルマス・デル・コンセホ）。別名リストの有効性 (`sorafs.sf1@1.0.0` 要求)
     マニフェストのメタデータとチャンクの一致を確認できます。
   - ベリフィカ・クオタス。安全性の制限/制限は、安全性を超えて応答する必要があります
     政治エラー (Norito 構造体)。
   - 大量のデータのストリーム `ChunkStore`、検証はデュランテ ラをダイジェストします
     摂取。マニフェストとアルマセナのメタデータを実際に作成する
     レジストロ。
4. **フェッチのフルホ**:
   - ディスコやランゴ、チャンクなどのソリチュードを提供します。エルスケジューラインポーネ
     `max_parallel_fetches` y devuelve `429` cuando está saturado.
   - テレメトリ構造体 (Norito JSON) をレイテンシア、バイト サービスに出力します。
     ダウンストリーム監視時のエラー状態を監視します。
5. **ムエストレオ PoR**:
   - 労働者の選択は比例ペソを示します (バイト数、バイト数)
     アルマセナドス) y ejecuta muestreo determinista usando el árbol PoR del chunk ストア。
   - 継続的な結果を含む聴取結果を保持します。
     証明者の広告/テレメトリのエンドポイント。
6. **追放/強制退去**:
   - クアンド・セ・アルカンサ・ラ・キャパシダード・エル・ノード・レチャザ・ヌエボス・ピンズ・ポー・ディフェクト。
     追放の政治を構成するロス・オペラドールのオプション
     (サンプル、TTL、LRU) 安全なモデルを確認できます。ポル
     アホラ エル ディセーニョは、規制を解除して操作を行う必要があります。
     エル・オペラドール。

### 容量の宣言とスケジュールの統合の宣言

- Torii アホラは、`CapacityDeclarationRecord` の実際の再送信を行います
  `/v2/sorafs/capacity/declare` ハシア エル `CapacityManager` 埋め込み、モード変更
  メモリア デ サス アサインナシオンのコンプロメティダを構築するためのノード
  チャンカーとレーン。マネージャーがテレメトリーでの単独講義のスナップショットを公開
  (`GET /v2/sorafs/capacity/state`) 事前にレーンを予約する必要があります
  aceptar nuevos pedidos.【crates/sorafs_node/src/capacity.rs:1】【crates/sorafs_node/src/lib.rs:60】
- El エンドポイント `/v2/sorafs/capacity/schedule` アセプタ ペイロード `ReplicationOrderV1`
  エミディドス・ポル・ゴベルナンザ。クアンド・ラ・オルデン・アプンタ・アル・プロヴェードル・ローカル・エル・マネージャー
  再スケジュールの重複、チャンカー/レーンの容量確認、予約ラ
  フランジャとデブエルブ UN `ReplicationPlan` の容量制限に関する説明
  継続的に摂取を続けてください。ラス オーデネス パラ
  otros proveedores se reconocen con una respuesta `ignored` para facilitar flujos
  マルチオペレーター.【crates/iroha_torii/src/routing.rs:4845】
- 完全なフック (完全なフック、摂取時のディスパラドス)
  ラマン `POST /v2/sorafs/capacity/complete` パラ リベラル リザーバス ヴィア
  `CapacityManager::complete_order`。スナップショットを含むレスペスタ
  `ReplicationRelease` (チャンカー/レーンの残りの合計、残差) パラメータ
  ラス・ヘルラミエンタス・デ・オルケスタシオン・プエダン・エンコラー・ラ・シギエンテ・オルデン罪世論調査。
  Trabajo futuro ケーブルのパイプライン デル チャンク ストア ウナ ベス ケ ラ
  テラスでの摂取ロジック。【crates/iroha_torii/src/routing.rs:4885】【crates/sorafs_node/src/capacity.rs:90】
- El `TelemetryAccumulator` embebido puede mutarse mediante
  `NodeHandle::update_telemetry`、労働者と安全計画を許可する
  PoR/稼働時間と最終的な派生ペイロードの正規記録を登録します
  `CapacityTelemetryV1` 内部スケジューラに対するものです。【crates/sorafs_node/src/lib.rs:142】【crates/sorafs_node/src/telemetry.rs:1】

### インテグラシオネスとトラバホの未来- **ガバナンザ**: テレメトリ用エクステンダー `sorafs_pin_registry_tracker.md`
  ストレージ (PoR の保存、ディスコの利用)。ラス・ポリティクス・デ・アドミシオン
  安全性を確保するために必要な容量を最小限に抑えることができます。
  広告。
- **クライアントの SDK**: ストレージの新しい構成の説明者 (制限事項)
  ディスコ、別名) プエダン ブートストラップ ノードス パラ ケ ヘラミエンタス デ ジェスティオン
  プログラム。
- **テレメトリア**: メトリカスが存在する積分制御スタック (Prometheus /
  OpenTelemetry) ダッシュボードのストレージ管理に関するパラメタ
  観察可能です。
- **Seguridad**: プールのストレージモジュールの取り出し
  非同期バックプレッシャーとチャンクの講義のサンドボックス化を考慮
  io_uring o pools acotados de tokio para evitar que clientes maliciosos agoten
  再帰的。

必要に応じて決定的な方法を選択して、マンティエンを選択します。
ロス・オペラドーレス・ロス・ノブズ・ニーセサリオス・パラ・キャパを実行する必要があります
SoraFS データの可用性。 `iroha_config` でのカンビオの実装、
`iroha_core`、`iroha_torii` およびゲートウェイ Norito、広告ツールの利用
証明者。