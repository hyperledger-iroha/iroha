---
lang: ja
direction: ltr
source: docs/portal/docs/sorafs/node-storage.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: ノードストレージ
title: Conception du Stockage du nœud SoraFS
サイドバーラベル: ストックの概念
説明: アーキテクチャ、在庫、クォータ、フックなどのサイクルでのシステム Torii のシステム SoraFS。
---

:::note ソースカノニク
Cette ページは `docs/source/sorafs/sorafs_node_storage.md` を参照します。 Gardez les deux は、スフィンクスの歴史に関する文書の同期をコピーします。
:::

## Conception du Stockage du nœud SoraFS (ブルイヨン)

正確なコメントを確認してください。Iroha (Torii) ソファ上の注意事項を確認してください。
SoraFS の可用性と、ローカルのディスケのパーティーを保存する
ストッカーとサーバーの塊。発見の仕様を完全にする
`sorafs_node_client_protocol.md` SF-1b の詳細な治具の説明
建築物ストック、資源管理およびプロムベリーの建築
到着者とゲートウェイのコードを設定する必要があります。
トルヴァンの訓練を行う
[運用手順書](./node-operations)。

### 目的

- Iroha auxiliaire d'exposer du disque のプロセスを検証する
  プロバイダー SoraFS には、元帳に対する責任がありません。
- Norito による在庫決定とパイロットのモジュール : マニフェスト、
  チャンクの計画、ラシーン Proof-of-Retrievability (PoR) およびプロバイダーの広告
  真実の源。
- 作業を完了するための割り当て制限を設定するアップリケ
  リソースを取得して要求を受け入れます。
- サンタ/テレメトリーの公開者 (PoR の拡張、チャンクのフェッチの遅延、
  プレッションディスク) à la governance et aux client.

### オーニボー建築

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

モジュールの説明:

- **ゲートウェイ** : エンドポイントの公開 HTTP Norito のピンの提案、
  チャンクのフェッチ要求、PoR およびテレメトリーの再実行。イル
  有効なペイロード Norito とリクエストとチャンク ストアのキャッシュ。再利用
  HTTP Torii は新しいデーモンを提供するために存在します。
- **ピン レジストリ** : マニフェストのピンのリスト `iroha_data_model::sorafs`
  et `iroha_core`。 Lorsqu'un マニフェストを受け入れ、ダイジェストを登録します
  マニフェスト、プランのダイジェスト、チャンクのラシーン PoR および容量のフラグ
  プロバイダー。
- **チャンク ストレージ** : マニフェストの実装 `ChunkStore`
  サイン、`ChunkProfile::DEFAULT` 経由の計画のチャンクのマテリアル化、および永続化
  レイアウトを決定するためのチャンクの分割。指紋認証に関するチャンクの認証
  継続的およびメタドンネのPoR afin que l'échantillonnage puisse revalider
  ルフィシエなしで完了。
- **クォータ/スケジューラ** : 操作時の設定制限を課します (最大バイト数、
  最大のピン数、最大数のパラレルフェッチ、TTL のチャンク) などの調整を行うことができます。
  les tâches 元帳 ne soient pas affamées。スケジュール担当者が責任を負います
  CPU が負担するサービス デ プリーヴス PoR およびリクエスト デシャンティヨナージュ。

### 構成

Ajoutez une nouvelle セクション à `iroha_config` :

```toml
[sorafs.storage]
enabled = false
data_dir = "/var/lib/iroha/sorafs"
max_capacity_bytes = "100 GiB"
max_parallel_fetches = 32
max_pins = 10_000
por_sample_interval_secs = 600
alias = "tenant.alpha"            # tag optionnel lisible
adverts:
  stake_pointer = "stake.pool.v1:0x1234"
  availability = "hot"
  max_latency_ms = 500
  topics = ["sorafs.sf1.primary:global"]
```

- `enabled` : 参加を切り替えます。 Quand false、ゲートウェイ renvoie 503 pour les
  エンドポイントのストレージおよび検出後の通知は必要ありません。
- `data_dir` : チャンクのレパートリー、PoR および Télémétrie の分析
  取ってくる。デフォルト `<iroha.data_dir>/sorafs`。
- `max_capacity_bytes` : ピネの塊を注ぐ期間は限られています。フォン・タシュ・ド・フォン
  Rejette les nouvoaux pins は、限界を超えたものを追求します。
- `max_parallel_fetches` : スケジューラーの同意を無視したプラフォンド
  équilibrer Bande passante/IO disque avec la Charge du validateur。
- `max_pins` : マニフェストの最大ピン数を指定せず、前もって適用するアプリケーターを受け入れません
  立ち退き/バックプレッシャー。
- `por_sample_interval_secs` : PoR 自動化の仕事のリズム。チャックジョブ
  échantillonne `N` feuilles (構成可能なマニフェスト) と émet des événements de télémétrie。
  La gouvernance peutscaler `N` de manière déterministe via la clé de métadonnées
  `profile.sample_multiplier` (`1-4` と入力)。 La valeur peut être un nombre/une Chaîne unique
  オブジェクト avec は、プロファイル、例 `{"default":2,"sorafs.sf2@1.0.0":3}` をオーバーライドします。
- `adverts` : シャンプの広告を生成する構造利用法
  `ProviderAdvertV1` (ステーク ポインター、ヒント QoS、トピック)。 Si omis, le nœud は les を利用します
  統治登録のデフォルト。プロンベリーの構成:

- `[sorafs.storage]` は `iroha_config` と `SorafsStorage` および料金を決定します
  構成を決定する必要があります。
- `iroha_core` および `iroha_torii` ストレージ構成のトランスメットテント au ビルダー ゲートウェイ
  エ・トー・チャンク・ストア・オー・デマラージュ。
- Des は、既存の開発/テスト (`SORAFS_STORAGE_*`、`SORAFS_STORAGE_PIN_*`) をオーバーライドします。
  構成を管理するための生産管理を行います。

### ユーティリティ CLI

表面 HTTP Torii のテスト アンコール、クール ド ケーブル、クレートの確認
`sorafs_node` スクリプト作成者が操作を行う際に、CLI を使用する必要があります
取り込み/エクスポートとバックエンド永続的な演習。【crates/sorafs_node/src/bin/sorafs-node.rs:1】

```bash
cargo run -p sorafs_node --bin sorafs-node ingest \
  --data-dir ./storage/sorafs \
  --manifest ./fixtures/manifest.to \
  --payload ./fixtures/payload.bin \
  --plan-json-out ./plan.json
```

- `ingest` はマニフェスト `.to` エンコード Norito およびペイロード関連バイトに参加します。
  マニフェストのチャンクの計画を再構築し、プロファイルのチャンクを作成し、適用します
  ダイジェストのパリティ、チャンクのフィチエの永続化、および BLOB のオプションの表示
  JSON `chunk_fetch_specs` は、ダウンストリームの有効なファイル レイアウトを出力します。
- `export` マニフェストおよびペイロード ストックのマニフェストおよびクリティカルな ID を受け入れます
  (avec plan JSON オプション) 再生産可能なフィクスチャを注ぎます。

Norito JSON sur stdout、ce qui facilite le の再開命令の実行
スクリプトによる配管。 LA CLI est couverte par un test d'intégration pour garantir
API の到着前にマニフェストとペイロードを往復修正する Torii.【crates/sorafs_node/tests/cli.rs:1】

> パリテHTTP
>
> ゲートウェイ Torii は、講義の基礎となるヘルパーのデソルメを公開します
> `NodeHandle` :
>
> - `GET /v2/sorafs/storage/manifest/{manifest_id_hex}` — レンボイスファイルマニフェスト
> Norito Stocké (base64) avec Digest/métadonnées.【crates/iroha_torii/src/sorafs/api.rs:1207】
> - `GET /v2/sorafs/storage/plan/{manifest_id_hex}` — チャンクの計画を実行する
> 決定的な JSON (`chunk_fetch_specs`) をダウンストリームに注ぎます。【crates/iroha_torii/src/sorafs/api.rs:1259】
>
> Ces エンドポイントは、パイプラインのパイプラインを通過する CLI を反映しています
> スクリプト locaux aux プローブ HTTP sans Changer de parseurs.【crates/iroha_torii/src/sorafs/api.rs:1207】【crates/iroha_torii/src/sorafs/api.rs:1259】

### サイクル・ド・ヴィー・ドゥ・ヌー1. **デマラージュ** :
   - 在庫をアクティブにし、チャンク ストアのレパートリーを初期化しない
     容量構成など。 Cela には検証とベースの作成が含まれます
     マニフェスト PoR とリプレイのマニフェストは、キャッシュを注ぐものです。
   - ゲートウェイ SoraFS のルートを登録する (エンドポイント Norito JSON POST/GET 注ぎピン、
     フェッチ、シャンティヨナージュ PoR、テレメトリ)。
   - ランサーは、PoR および割り当ての監視を担当する労働者です。
2. **発見/広告** :
   - 文書一般 `ProviderAdvertV1` avec la capacité/santé courante、lessigner
     le conseil および le canal de Discovery を介して発行者による承認を取得します。
     新しいリスト `profile_aliases` を使用して、カノニクのハンドルを取得します
3. **ピンのワークフロー** :
   - マニフェスト署名を要求するゲートウェイ (チャンクの計画、ラシーン PoR、署名を含む)
     デュ・コンセイユ）。 Valider la liste d'alias (`sorafs.sf1@1.0.0` requis) および s'assurer que
     le plan de chunk はマニフェストのメタドンに対応します。
   - 割り当てを検証します。容量の制限/問題の解決策、対応策の制限
     政治的誤り (Norito 構造)。
   - ストリーマーは、`ChunkStore` のチャンクの内容を検証し、取り込みを検証します。
     登録されたマニフェストの情報を収集し、保管します。
4. **フェッチのワークフロー**:
   - ディスク要求範囲のチャンク要求を処理します。スケジューラが課す
     `max_parallel_fetches` および renvoie `429` en cas desaturation。
   - 構造の詳細 (Norito JSON) 平均レイテンス、バイト数など
     下流側の監視を監視します。
5. **エシャンティヨナージュ PoR** :
   - 労働者選択のマニフェスト比例配分 (例: バイト在庫)
     そして、l'arbre PoR du chunkストア経由で決定を実行します。
   - 行政監査の結果を保存し、履歴書を含める
     広告プロバイダー/テレメトリのエンドポイント。
6. **エビクション/割り当てのアプリケーション** :
   - 安全性を重視し、デフォルトで新しいピンを再発行する必要はありません。えん
     オプション、政治政策の管理者による運用管理 (例: TTL、LRU)
     統治の定義を決定するモデル。すぐに注いで、デザインを想定してください
     割り当ては厳密であり、操作の開始を決定する操作を制限します。

### 能力とスケジュールの統合に関する宣言

- Torii 日々のレミゼ・デソルメ `CapacityDeclarationRecord` ピュイ `/v2/sorafs/capacity/declare`
  vers le `CapacityManager` embarqué, de sorte que Chaque no construit une vue en memoire de ses
  チャンカー/レーンの割り当て。ル マネージャーがスナップショットを読み取り専用で公開し、テレメトリーを提供します
  (`GET /v2/sorafs/capacity/state`) プロフィールと前衛的なレーンのアップリケ予約
  nouvelles commandes ne soient acceptées.【crates/sorafs_node/src/capacity.rs:1】【crates/sorafs_node/src/lib.rs:60】
- エンドポイント `/v2/sorafs/capacity/schedule` ペイロード `ReplicationOrderV1` を政府として受け入れます。
  ローカルのプロバイダー、マネージャーが二重に計画を検証し、有効性を確認します。
  容量チャンカー/レーン、保留ラ・トランシュ、およびレンヴォワ・アン `ReplicationPlan` 容量保持機能
  オーケストラの演奏を最大限に楽しんでください。 Les ordres pour d'autres プロバイダー
  `ignored` はワークフローのマルチオペレーションを促進します。【crates/iroha_torii/src/routing.rs:4845】
- Desフック・デ・コンプリート（摂取後の成功の解除）控訴人
  `POST /v2/sorafs/capacity/complete` は、`CapacityManager::complete_order` 経由で予約を自由に送信してください。
  スナップショット `ReplicationRelease` (残りのレストラン、残りのチャンカー/レーン) を含む応答
  投票なしで指揮を執るオーケストラの指揮を執ります。将来は苦労しないでください
  パイプラインのチャンクストアの摂取量のセラプレテ。【crates/iroha_torii/src/routing.rs:4885】【crates/sorafs_node/src/capacity.rs:90】
- Le `TelemetryAccumulator` embarqué peut être muté via `NodeHandle::update_telemetry`、permettant aux
  PoR/稼働時間とペイロードの管理を担当する労働者が正規の登録を行う
  `CapacityTelemetryV1` sans toucher aux internes du scheduler.【crates/sorafs_node/src/lib.rs:142】【crates/sorafs_node/src/telemetry.rs:1】

### 統合と今後の取り組み- **管理** : étendre `sorafs_pin_registry_tracker.md` avec la télémétrie de Stockage
  (成功事例 PoR、利用状況に関するディスク)。 Les politiques d'admission peuvent exiger une capacité
  広告を最小限に抑えるためには、PoR を最小限に抑えてください。
- **SDK クライアント** : ストレージの新しい構成を公開する (ディスク、エイリアスを制限する) キューを作成する
  プログラムごとにブートストラップを実行する必要があります。
- **Télémétrie** : 存在するメトリックのスタックの整数 (Prometheus /
  OpenTelemetry) は、ストレージ デバイスのダッシュボードの監視を可能にします。
- **セキュリティ** : プールとストレージのモジュールを非同期で実行し、バックプレッシャーを回避します
  io_uring とプールを介してチャンクのサンドボックス化とレクチャーを想定し、東京生まれのプールを実現します。
  des client malveillants d'épuiser les ressources。

ストレージ オプションのモジュールと操作性を決定するための設計保守
必要なボタンは、SoraFS のディスポニビリテ デ ドンネのソファに参加者を注ぎます。息子の実装
`iroha_config`、`iroha_core`、`iroha_torii` およびゲートウェイ Norito、ainsi による変更の変更
プロバイダーの広告を表示します。