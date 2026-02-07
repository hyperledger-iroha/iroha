---
lang: ja
direction: ltr
source: docs/portal/docs/sorafs/orchestrator-config.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: オーケストレーター構成
タイトル: オーケストラ構成 SoraFS
Sidebar_label: オーケストラの構成
説明: マルチソースをフェッチするオーケストレーションの構成者、テレメトリの検査とデボーグを行うインタプリタ。
---

:::note ソースカノニク
Cette ページは `docs/source/sorafs/developer/orchestrator.md` を参照します。 Gardez les deux は、歴史的な文書の保存と同期をコピーします。
:::

# マルチソースをフェッチするための組織ガイド

SoraFS パイロットのマルチソースを取得するための組織
4 つのパブリッシャーのアンサンブルを決定するためのパラレルとデターミニスト
soutenus par la gouvernance を広告します。 Ce ガイドのエクスプリーク コメント コンフィギュラー
l’orchestrateur、quels signalaux d’échecAttendre ペンダント、ロールアウトとケルス
情報の流れを明らかにし、安全性を示します。

## 1. アンサンブルの構成を確認する

トロワソースの構成を統合する指揮者:

|出典 |目的 |メモ |
|----------|----------|----------|
| `OrchestratorConfig.scoreboard` |監査の結果を正規化し、監査の結果を有効にし、スコアボードの JSON を使用して永続化します。 |アドセ、`crates/sorafs_car::scoreboard::ScoreboardConfig`。 |
| `OrchestratorConfig.fetch` |実行制限のアップリケ (再試行の予算、合意のボーン、検証のバスキュール)。 | `FetchOptions` と `crates/sorafs_car::multi_fetch` をマップします。 |
|パラメータ CLI / SDK |ペアの名前を制限し、地域の外交官を制限し、拒否/ブーストの政治を公開します。 | `sorafs_cli fetch` ces フラグの指示を公開します。 `OrchestratorConfig` 経由の SDK ファイル プロパジェント。 |

`crates/sorafs_orchestrator::bindings` シリアル番号のヘルパー JSON
Norito JSON、レンダントポータブルエンターバインディング SDK などで構成が完了しました
自動化。

### 1.1 構成 JSON の例

```json
{
  "scoreboard": {
    "latency_cap_ms": 6000,
    "weight_scale": 12000,
    "telemetry_grace_secs": 900,
    "persist_path": "/var/lib/sorafs/scoreboards/latest.json"
  },
  "fetch": {
    "verify_lengths": true,
    "verify_digests": true,
    "retry_budget": 4,
    "provider_failure_threshold": 3,
    "global_parallel_limit": 8
  },
  "telemetry_region": "iad-prod",
  "max_providers": 6,
  "transport_policy": "soranet_first"
}
```

Persistez le fichier via l’empilement Hobbyuel `iroha_config` (`defaults/`、ユーザー、
実際の) 配備の決定に関するヘリテント デ ミームの限界の決定
レヌー。ロールアウト SNNet-5a のプロファイルと返信を直接調整するだけで、
`docs/examples/sorafs_direct_mode_policy.json` および適応症を参照してください
アソシエは `docs/source/sorafs/direct_mode_pack.md` です。

### 1.2 準拠の上書き

SNNet-9 は、オーケストラの統治者としての適合性パイロットを統合します。
新しいオブジェクト `compliance` および構成 Norito JSON キャプチャ ファイル
直接のみのモードでパイプラインを取り出すカーブアウト:

```json
"compliance": {
  "operator_jurisdictions": ["US", "JP"],
  "jurisdiction_opt_outs": ["US"],
  "blinded_cid_opt_outs": [
    "C6B434E5F23ABD318F01FEDB834B34BD16B46E0CC44CD70536233A632DFA3828"
  ],
  "audit_contacts": ["mailto:compliance@example.org"]
}
```

- `operator_jurisdictions` 運用コード ISO-3166 alpha-2 宣言
  オーケストラのインスタンス。法則性を正常に保つためのコード
  解析中。
- `jurisdiction_opt_outs` 統治登録を参照。ロスクウネ
  管轄オペレートアパライト ダン ラ リスト、オーケストラストラトゥール 課す
  `transport_policy=direct-only` とフォールバックの存在
  `compliance_jurisdiction_opt_out`。
- `blinded_cid_opt_outs` マニフェストのダイジェストのリスト (CID マスク、エンコード
  16進法）。オーストラリアのペイロード特派員
  直接のみの計画とフォールバックの公開
  `compliance_blinded_cid_opt_out` はテレビ番組です。
- `audit_contacts` enregistre les URI que la gouvernance at des opérateurs
  ルールのプレイブック GAR です。
- `attestations` は、正当な証拠を示す書類をキャプチャします
  政治。 `jurisdiction` オプションの Chaque entrée 定義 (コード ISO-3166)
  alpha-2)、un `document_uri`、le `digest_hex` canonique à 64 文字、le
  タイムスタンプ発行 `issued_at_ms`、および `expires_at_ms` オプション。セス
  オーケストラ監査のチェックリストに含まれるアーティファクト
  outils de gouvernance puissent relier les は補助文書の署名を無効にします。

Fournissez le bloc de conformité via l’empilement Havetuel de Configuration pour
決定を無効にする操作を再実行します。オーケストラストラトゥール
書き込みモードに適合するアップリケのヒント: SDK の要求に従う
`upload-pq-only`、司法権のオプトアウトとマニフェストのバスキュレントトゥージュール
直接輸送のみと高速輸送を実現
適合は存在しません。

Les canonique d’opt-out 住民のカタログ
`governance/compliance/soranet_opt_outs.json` ;ル・コンセイユ・ド・ガバナンス出版
les misses à jour via des releases taggées。設定完了の例を示さない
(証明を含む) 責任ある責任者
`docs/examples/sorafs_compliance_policy.json`、その他のプロセス操作
ルをキャプチャー
[GAR 準拠のプレイブック](../../../source/soranet/gar_compliance_playbook.md)。

### 1.3 パラメータ CLI および SDK|フラッグ / チャンピオン |エフェット |
|--------------|------|
| `--max-peers` / `OrchestratorConfig::with_max_providers` |スコアボードのフィルタリングを制限します。 Mettez `None` は資格のあるユーザーを使用します。 |
| `--retry-budget` / `FetchOptions::per_chunk_retry_limit` |プラフォンヌはチャンクごとに再試行します。デパサー・ラ・リミット・デクランシュ `MultiSourceError::ExhaustedRetries`。 |
| `--telemetry-json` |スコアボードのレイテンス/検査のスナップショットを注入します。 `telemetry_grace_secs` は対象外です。 |
| `--scoreboard-out` |スコアボードの計算 (適格者 + 不適格者) は、実行後の検査を継続します。 |
| `--scoreboard-now` |スコアボードのタイムスタンプ (Unix 秒) を追加して、試合結果を決定します。 |
| `--deny-provider` / フック・デ・ポリティック・デ・スコア |広告のない最高のマニエール決定を除外します。ブラックリストに登録して迅速な対応を可能にします。 |
| `--boost-provider=name:delta` |統治に触れることなく、ラウンドロビンの信用を評価します。 |
| `--telemetry-region` / `OrchestratorConfig::with_telemetry_region` |ダッシュボードの構造とログの構造を地理情報や漠然としたロールアウトの観点から分析します。 |
| `--transport-policy` / `OrchestratorConfig::with_transport_policy` |デフォルト `soranet-first` は、オーケストラのマルチソース ベースのメンテナンスをサポートします。 `direct-only` ダウングレードまたは適合指令を使用し、`soranet-strict` 補助パイロット PQ のみを使用します。規定に準拠した変更を無効にします。 |

SoraNet の初期設定の変更、および参照ファイルのロールバックの最初の変更
ブロカントSNNet特派員。 SNNet-4/5/5a/5b/6a/7/8/12/13 の卒業後、
la gouvernance durcira la posture requise (vers `soranet-strict`) ;ディシラ、レ
インシデントの動機を無効にする doivent privilégier `direct-only` など
荷送人はロールアウトのログを作成します。

Tous les flags ci-dessus acceptent la syntaxe `--` dans `sorafs_cli fetch` et le
ビネールオリエンテ開発者 `sorafs_fetch`。 SDK のミーム オプションを公開する
ビルダータイプ経由。

### 1.4 キャッシュガードのジェスチャ

La CLI ケーブル ディソルメ le sélecteur de Guards SoraNet pour permettre aux
前衛的なマニエールを決定するリレーの操作
SNNet-5 のロールアウトが完了しました。トロワ ヌーボー フラグ制御 CE フラックス:

|旗 |目的 |
|------|----------|
| `--guard-directory <PATH>` |ポイントとアンフィシエの JSON 記述子、リレーのコンセンサス、および最近 (スーアンサンブル ci-dessous)。ディレクトリを渡すと、フェッチの前にキャッシュ デ ガードが実行されます。 |
| `--guard-cache <PATH>` | Norito の `GuardSet` エンコードを永続化します。 nouveau ディレクトリのキャッシュを実行する必要があります。 |
| `--guard-target <COUNT>` / `--guard-retention-days <DAYS>` |保護者の名前を付けたエントリ (デフォルト 3 時間) および保持期間 (デフォルト 30 時間) のオプションをオーバーライドします。 |
| `--guard-cache-key <HEX>` | 32 オクテットのオプションを使用して、MAC Blake3 の安全なキャッシュを安全に再利用できます。 |

スキーマコンパクトで使用されるディレクトリのペイロードの保護:

Le flag `--guard-directory` ペイロードのデソルメに参加 `GuardDirectorySnapshotV2`
Norito のエンコード。スナップショット バイネアの内容:

- `version` — スキーマのバージョン (実際 `2`)。
- `directory_hash`、`published_at_unix`、`valid_after_unix`、`valid_until_unix` —
  完全な認証を取得するためのコンセンサスとデヴァント通信のメタドン。
- `validation_phase` — 証明書の政治ゲート (`1` = 自動ライザー
  seule 署名 Ed25519、`2` = プレフェレール レ ダブル署名、`3` = exiger
  ダブルスの署名)。
- `issuers` — 行政管理部門 `fingerprint`、`ed25519_public` など
  `mldsa65_public`。指紋が計算できなくなります
  `BLAKE3("soranet.src.v2.issuer" || ed25519 || u32(len(ml-dsa)) || ml-dsa)`。
- `relays` — SRCv2 バンドルのリスト (出撃)
  `RelayCertificateBundleV2::to_cbor()`)。 Chaque バンドルには説明が含まれています
  リレー、レ・フラッグ・ド・キャパシテ、ラ・ポリティークML-KEM、レ・シグニチャー・ダブルス
  Ed25519/ML-DSA-65。

La CLI は、前衛的なデクラレを確認するためのチャック バンドルを検証します。
fusionner ファイル ディレクトリのキャッシュ保護機能。 JSON の遺産について説明します
ソントプラス受入者; SRCv2 ソンションに必要なスナップショット。CLI のアベック `--guard-directory` を融合させてコンセンサスを追加してください
最近のアベック ファイル キャッシュが存在します。 Le sélecteur conserve les Guards épinglés encore
ディレクトリの保持および資格のあるディレクトリを有効にします。レ
nouveau は期限切れの les entrées を置き換えます。キャッシュを取り出してから
ミス・ア・ジュール・エスト・レクリット・ダン・ル・シュミン・フォーニ、`--guard-cache`経由、ガーダン・レ
決定的なセッション。 SDK の再現性の向上
控訴人`GuardSelector::select(&RelayDirectory, existing_guard_set, now_unix_secs)`
注入剤ファイル `GuardSet` の結果は `SorafsGatewayFetchOptions` です。

`ml_kem_public_hex` PQ の優先順位を決定します
ペンダントルロールアウトSNNet-5。レ・トグル・デテープ (`anon-guard-pq`,
`anon-majority-pq`, `anon-strict-pq`) 自動化ファイルの劣化
リレーの古典: lorsqu’un ガード PQ est disponible、le sélecteur suprime les
セッションのお気に入りのクラシックをピン留めします
握手ハイブリッド。 CLI/SDK エクスポーズル ミックス結果の履歴書
`anonymity_status`/`anonymity_reason`、`anonymity_effective_policy`、
`anonymity_pq_selected`、`anonymity_classical_selected`、`anonymity_pq_ratio`、
`anonymity_classical_ratio` et les Champs associés de candidats/déficit/delta de
供給、レンダントは、ブラウンアウトとフォールバックの古典を明示します。

バンドルされた SRCv2 を完了するためのセキュリティ ディレクトリのディレクトリ
`certificate_base64`。 L’orchestrateur デコード チャク バンドル、再検証
署名 Ed25519/ML-DSA およびキャッシュの証明書分析を保存する
デ・ガード。 Lorsqu'un certificat est present、il devient la source canonique pour
PQ のクラス、ハンドシェイクとポンドの事前説明 ;証明書
説明の保存期間は有効期限となります。
回路などのサイクルの遅延に関するプロパジェントの認証を取得します
`telemetry::sorafs.guard` および `telemetry::sorafs.circuit` 経由で公開します。
検証のための委託品、握手および観察のスイート
非デダブルスの署名はチャックガードを注ぎます。

CLI のヘルパーを使用して、スナップショットを調整し、編集することができます。

```bash
sorafs_cli guard-directory fetch \
  --url https://directory.soranet.dev/mainnet_snapshot.norito \
  --output ./state/guard_directory.norito \
  --expected-directory-hash <directory-hash-hex>

sorafs_cli guard-directory verify \
  --path ./state/guard_directory.norito \
  --expected-directory-hash <directory-hash-hex>
```

`fetch` スナップショット SRCv2 の監視と監視、
タンディス QUE `verify` rejoue le パイプラインの検証と成果物問題の検証
d’autres équipes, en émettant un résumé JSON qui reflète la sortie du sélecteur
CLI/SDK を保護します。

### 1.5 回路開発サイクルのジェスチャー

リレーのディレクトリと警備員のキャッシュのディレクトリ、オーケストラの指揮者
建設前および建設中のサーキットでの活動のサイクル
サーキットの再構築 SoraNet のアバンチャクフェッチ。構成設定
スー `OrchestratorConfig` (`crates/sorafs_orchestrator/src/lib.rs:305`) ドゥ経由
ヌーヴォーシャン：

- `relay_directory`: ディレクトリ SNNet-3 の転送ファイルのスナップショット
  中央/出口のマニエール選択を決定します。
- `circuit_manager`: 設定オプション (デフォルトでアクティブ) 制御ファイル
  TTL 回路。

Norito JSON はブロック `circuit_manager` の分割を受け入れます:

```json
"circuit_manager": {
  "enabled": true,
  "circuit_ttl_secs": 900
}
```

ディレクトリ経由の SDK 転送
`SorafsGatewayFetchOptions::relay_directory`
(`crates/iroha/src/client.rs:320`) などの CLI ファイルの自動化機能
`--guard-directory` テスト フォーニ (`crates/iroha_cli/src/commands/sorafs.rs:365`)。

Le gestionnaire renouvelle les circles lorsque les métadonnées de Guard Changent
(エンドポイント、PQ またはタイムスタンプの詳細) TTL の有効期限が切れています。ル・ヘルパー
`refresh_circuits` アバント チャック フェッチの呼び出し (`crates/sorafs_orchestrator/src/lib.rs:1346`)
ログ `CircuitEvent` は、操作上の決定を追跡するために必要です
liées aucycle de vie。ルソークテスト
`circuit_manager_latency_soak_remains_stable_across_rotations`
(`crates/sorafs_orchestrator/src/soranet.rs:1479`) 遅延が安定しています
シュール・トロワ・ローテーション・デ・ガード。ヴォワール・ル・ラポール・アソシエ・ダンス
`docs/source/soranet/reports/circuit_stability.md:1`。

### 1.6 プロキシ QUIC ローカル

L'orchestrateur peut optionnellement lancer un proxy QUIC local afin que les
拡張機能のナビゲーターとアダプター SDK は証明書を取得しません
警備員のクレ・デュ・キャッシュ・ド・ガード。プロキシは、ループバック、終了アドレスに嘘をつきます
接続 QUIC とマニフェスト Norito 証明書などの証明書
キャッシュ ド ガード オプションのクライアント。輸送機関のレベヌメント
プロキシは `sorafs_orchestrator_transport_events_total` 経由で実行されます。

le nouveau ブロック `local_proxy` による JSON のオーケストラによるプロキシのアクティブ化:

```json
"local_proxy": {
  "bind_addr": "127.0.0.1:9443",
  "telemetry_label": "dev-proxy",
  "guard_cache_key_hex": "0123456789ABCDEF0123456789ABCDEF0123456789ABCDEF0123456789ABCDEF",
  "emit_browser_manifest": true,
  "proxy_mode": "bridge",
  "prewarm_circuits": true,
  "max_streams_per_circuit": 64,
  "circuit_ttl_hint_secs": 300,
  "norito_bridge": {
    "spool_dir": "./storage/streaming/soranet_routes",
    "extension": "norito"
  },
  "kaigi_bridge": {
    "spool_dir": "./storage/streaming/soranet_routes",
    "extension": "norito",
    "room_policy": "public"
  }
}
```- `bind_addr` プロキシのアドレスを制御 (ポート `0` を使用)
  デマンダー・アン・ポール・エフェメール)。
- `telemetry_label` ダッシュボードの区別を維持するための指標を設定する
  プロキシとセッションのフェッチ。
- `guard_cache_key_hex` (オプション) プロキシのエクスポーザのキャッシュを許可
  CLI/SDK を保護し、拡張機能をナビゲートします。
- `emit_browser_manifest` マニフェスト ファイル拡張機能によるバスキュール ファイルの復元
  プヴァンストッカーとバリダー。
- `proxy_mode` プロキシ リレー トラフィック ロケメントを選択してください (`bridge`) ou
  回路の拡張を目的とした SDK を開発する
  ソラネット (`metadata-only`)。 `bridge` のデフォルトのプロキシ ;チョイセズ
  `metadata-only` は、リレーラー ファイルをフラックスせずにエクスポージャー ファイルのマニフェストをポストアン ポストします。
- `prewarm_circuits`、`max_streams_per_circuit`、`circuit_ttl_hint_secs`
  予算内で役立つナビゲートツールのヒントを公開します
  回路を再利用するためのパラレルとコンプレンドル。
- `car_bridge` (オプション) ローカル アーカイブ CAR のポイント対キャッシュ解除。ル・シャン
  `extension` 接尾辞を制御して、lorsque la cible omet `*.car` ;定義
  `allow_zst = true` ペイロードのサーバー指示 `*.car.zst` 圧縮前。
- `kaigi_bridge` (オプション) Kaigi スプールのルートをプロキシとして公開します。ル・シャン
  `room_policy` ブリッジ機能モードを通知します `public` ou
  `authenticated` クライアントのラベルの事前選択ナビゲートを完了します
  GAR の流用。
- `sorafs_cli fetch` 公開ファイルは `--local-proxy-mode=bridge|metadata-only` をオーバーライドします
  et `--local-proxy-norito-spool=PATH`、バスキュラー ル モードの永続化
  ポインタとスプールの代替手段として、JSON の修飾子は使用されません。
- `downgrade_remediation` (オプション) ダウングレードの自動フックを構成します。
  Lorsqu’il est activé、l’orchestrateur surveille la télémétrie desリレーを注ぐ
  `threshold` によるダウングレードおよび失効後のラファール検出
  フェネートル `window_secs`、プロキシ ローカルと `target_mode` の強制 (デフォルトのまま)
  `metadata-only`)。停止をダウングレードし、すべての代理人を務めます
  `cooldown_secs` 以降の `resume_mode`。 `modes` ポア リミッターを活用
  le déclencheur à des rôles de Relay spécifiques (par défaut les Relays d’entrée)。

プロキシ トーナメント アン モード ブリッジを使用して、複数のサービス アプリケーションを検索します。

- **`norito`** – 顧客との関係に関する解決策の解決策
  `norito_bridge.spool_dir`。 Les cibles Sont sanitizees (パス・ド・トラバーサル、パス・ド)
  chemins absolus)、et lorsqu’un fichier n’a pas d’extension、le suffixe configuré
  ストリーマーのペイロードとナビゲーターの最新のアップリケ。
- **`car`** – `car_bridge.cache_dir` に関する解決策の文献、遺産
  初期設定の拡張機能とペイロード圧縮の拒否機能
  si `allow_zst` はアクティブです。レ・ブリッジ・レウシス・レポンデント・アベック `STREAM_ACK_OK`
  クライアントのアーカイブに関する最新の転送情報
  パイプラインの検証。

ドゥーカス、プロキシフォーニット l’HMAC デュキャッシュタグ (Quand une clé de cache)
ハンドシェイクの事前準備) およびテレメトリのレゾン コードの登録
`norito_*` / `car_*` ダッシュボードを分割してください
成功、技術管理および衛生管理。

`Orchestrator::local_proxy().await` ファイルを流すときにファイル ハンドルを公開します
控訴人は PEM du certificat を発行し、マニフェスト ナビゲーターを受け取ります
あなたは、申請の猶予を求めています。

**マニフェスト v2** の登録を確認するためのプロキシの活動を開始します。
存在する証明書とキャッシュ ガード ガード、v2 に関する証明書の追加:

- `alpn` (`"sorafs-proxy/1"`) および表 `capabilities` はクライアントに注力します
  利用者のフラックスプロトコルを確認します。
- Un `session_id` ハンドシェイクおよびブロック デ サルエージ `cache_tagging` 送達
  HMAC のセッションとタグの保護。
- 回路およびガード選択のヒント (`circuit`、`guard_selection`、
  `route_hints`) 統合ナビゲーターは、UI に加えて豊富な機能を備えています
  アヴァン・ルーベルチュール・デ・フラックス。
- `telemetry_v2` 安全性と機密性を高めるための手段
  l’インストルメンテーションロケール。
- チャック `STREAM_ACK_OK` (`cache_tag_hex` を含む)。 Les clients refletent la valeur
  `x-sorafs-cache-tag` リクエストは HTTP または TCP から送信されます
  リポジトリのキャッシュを保存するための選択。Ces チャンピオン s’ajoutent au マニフェスト v2 ;クライアントは消費者を支援します
明示的な表現は支持者であり、無視者は残ります。

## 2. 意味の意味

容量と予算の厳格なアップリケの検証を行う管理者
アヴァン・ド・トランスファー・ル・モワンドル・オクテット。トロワのカテゴリーのコレクション:

1. **Échecs d’éligibilité (pre-vol).** Les fournisseurs sans capacité de plage、
   広告は有効期限があり、商品の発送も完了します
   スコアボードと計画のオミス。 CLI remplissent le の履歴書
   tableau `ineligible_providers` 運用上のレゾンの平均
   ログをスクレーパーなしで管理する検査官。
2. **Épuisement à l’execution.** チャック・フルニシュール・スーツ・レ・エシェック・コンセキュティフ。
   Une fois `provider_failure_threshold` atteint, le fournisseur est marqué
   `disabled` セッションを休んでください。四人姉妹の生活
   `disabled`、オーケストラ長
   `MultiSourceError::NoHealthyProviders { last_error, chunk_index }`。
3. **Arrêts déterministes.** Les limites dures remontent sous forme d’erreurs
   構造体 :
   - `MultiSourceError::NoCompatibleProviders` — ファイルのマニフェストには拡張が必要です
     チャンクは、アライメントを調整するために必要な情報を保持します。
   - `MultiSourceError::ExhaustedRetries` — チャンクごとの再試行予算
     コンソメ。
   - `MultiSourceError::ObserverFailed` — ダウンストリームの観察者 (フック
     ストリーミング）チャンク検証を拒否しないでください。

Chaque erreur embarque l’index du chunk fautif et、lorsque disponible、la raison
フルニスールのフィナーレ。 Traitez ces erreurs comme des blqueurs de release
— 再試行の平均的な内容の再現、広告の再試行、広告の再試行
テレメトリーは、日々変化するサンテ・デュ・フォーニサー・スー・ジャセント・パスです。

### 2.1 スコアボードの永続性

Quand `persist_path` est configuré、l’orchestrateur écrit le スコアボードファイナル
アフターチャクラン。ドキュメントの JSON コンテンツ:

- `eligibility` (`eligible` または `ineligible::<reason>`)。
- `weight` (ポイド正規化割り当て割り当て実行)。
- `provider` のメタドン (識別子、エンドポイント、予算の合意)。

スコアボードのスナップショットのアーカイブ、リリース後のアーティファクトのアーカイブ
ブラックリストへの登録と監査対象の保持のロールアウトを選択します。

## 3. テレメトリとデボゲージ

### 3.1 メトリック Prometheus

`iroha_telemetry` 経由の L'orchestrateur émet les métriques suivantes :

|メトリック |ラベル |説明 |
|----------|----------|---------------|
| `sorafs_orchestrator_active_fetches` | `manifest_id`、`region` |ゲージ・デ・フェッチ・オーケストラ・アン Vol. |
| `sorafs_orchestrator_fetch_duration_ms` | `manifest_id`、`region` |試合中のフェッチの遅延に関するヒストグラム登録者。 |
| `sorafs_orchestrator_fetch_failures_total` | `manifest_id`、`region`、`reason` | Compteur des échecs terminaux (再試行 épuisés、aucun fournisseur、échec observateur)。 |
| `sorafs_orchestrator_retries_total` | `manifest_id`、`provider`、`reason` | 4 回の再試行の暫定テストを実行します。 |
| `sorafs_orchestrator_provider_failures_total` | `manifest_id`、`provider`、`reason` |非アクティブ化によるセッションのフルニセールのコンピューティング。 |
| `sorafs_orchestrator_policy_events_total` | `region`、`stage`、`outcome`、`reason` |匿名政治（テニュエ vs ブラウンアウト）の決定とロールアウトとフォールバックのレゾンに関するグループの比較。 |
| `sorafs_orchestrator_pq_ratio` | `region`、`stage` | SoraNet セレクションのリレー PQ の一部のヒストグラム。 |
| `sorafs_orchestrator_pq_candidate_ratio` | `region`、`stage` |スコアボードのスナップショットとリレー PQ の比率のヒストグラム。 |
| `sorafs_orchestrator_pq_deficit_ratio` | `region`、`stage` | Histogramme du déficit de politique (écart entre l’objectif et la part PQ réelle)。 |
| `sorafs_orchestrator_classical_ratio` | `region`、`stage` |リレーの一部のヒストグラムは、チャック セッションで使用されます。 |
| `sorafs_orchestrator_classical_selected` | `region`、`stage` |セッションごとのリレーの古典的な選択のヒストグラム。 |

ダッシュボードのステージングとアクティブなノブの統合
生産。 SF-6 の観察可能性計画を再検討するための処分勧告:

1. **actifs をフェッチ** — モンテサンスの完了対応者をアラートします。
2. **再試行率** — コンピュータ `retry` の無効化を回避する
   ベースラインの歴史。
3. **Échecs fournisseurs** — déclenche les alertes pager quand un fournisseur
   passe `session_failure > 0` dans une fenêtre de 15 分。

### 3.2 丸太構造の聖書

公共機関の構造と聖書の決定を行う組織:- `telemetry::sorafs.fetch.lifecycle` — マーキュール `start` および `complete` avec
  チャンク数、リトライ数、期間の合計。
- `telemetry::sorafs.fetch.retry` — 再試行の実行 (`provider`、`reason`、
  `attempts`) マヌエルを分析してください。
- `telemetry::sorafs.fetch.provider_failure` — 4 つの機能を無効にする
  エラーの繰り返し。
- `telemetry::sorafs.fetch.error` — 検査終了履歴書平均 `reason` et
  メタドンネ・オプションネル・デュ・フルニシュール。

Acheminez の CES フラックスとログのパイプライン Norito の既存の応答
補助的な事件は、ユニークな情報源です。サイクル・ド・ヴィーの生活
`anonymity_effective_policy` 経由でファイル mix PQ/classique を公開、
`anonymity_pq_ratio`、`anonymity_classical_ratio` および leurs compteurs associés、
スクレーパーなしでダッシュボードのカスタマイズを容易にします。ペンダント
GA のロールアウト、`info` のサイクルでのログのブロック
デバイス/再試行および利用 `warn` エラー ターミナルが表示されます。

### 3.3 履歴書 JSON

`sorafs_cli fetch` et le SDK Rust 構造コンテンツの履歴書:

- `provider_reports` 成功/検査および無効化の検査。
- `chunk_receipts` 満足のいくチャックチャンクの詳細を確認します。
- 表 `retry_stats` と `ineligible_providers`。

Archivez le fichier de résumé pour déboguer les fournisseurs defaillants : les
領収書のマッピング指示、ログのメタドンネ、ci-dessus。

## 4. チェックリストの操作方法

1. **CI の設定を準備します。** Lancez `sorafs_fetch` avec la
   構成ケーブル、パスセズ `--scoreboard-out` キャプチャー ラ ビュー
   リリース以前の権利と相違点。観光客
   プロモーションの対象外です。
2. **電話の有効性** 展開の輸出を保証する
   メトリック `sorafs.fetch.*` およびフェッチのアクティブな構造のログ
   マルチソースの活用法。インディケ スヴァンの欠如
   オーケストラのファサードは、上訴者としての役割を果たします。
3. **ドキュメンタ ファイルがオーバーライドされます。** Lors d’un `--deny-provider` ou `--boost-provider`
   至急、変更ログから JSON (呼び出し CLI) を委託してください。レ
   ロールバックは、新しいスナップショットのオーバーライドとキャプチャーを無効にします
   スコアボード。
4. **リランサーのスモークテスト。** 予算の変更後、再試行します。
   Caps de fournisseurs、正規品の再取得
   (`fixtures/sorafs_manifest/ci_sample/`) チャンクの領収書を確認する
   休む決定主義者。

Suivre les étapes ci-dessus garde le comportement de l’orchestrateur
段階的に展開し、必要に応じて再生産可能
ラ・レポンス・オ・インシデント。

### 4.1 政治上のオーバーライド

Les opérateurs peuvent épingler la Phase de Transport/Anonymat active sans
基本設定と定義の修飾子
`policy_override.transport_policy` と `policy_override.anonymity_policy` ダンス
leur JSON `orchestrator` (四分の一
`--transport-policy-override=` / `--anonymity-policy-override=` à
`sorafs_cli fetch`)。 Lorsqu'un は現在を優先し、オーケストラをソテーします
フォールバック ブラウンアウト常習: si le niveau PQ 要求ne peut pas être satisfait,
`no providers` を取得して、沈黙を守ります。ル
シャンプのようなシンプルな構成を維持する
オーバーライドします。