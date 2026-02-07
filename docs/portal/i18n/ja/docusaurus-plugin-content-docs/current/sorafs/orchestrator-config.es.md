---
lang: ja
direction: ltr
source: docs/portal/docs/sorafs/orchestrator-config.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: オーケストレーター構成
タイトル: SoraFS の構成
Sidebar_label: オルケスタドールの構成
説明: 複数のオリジンを取得するための設定、テレメトリの解釈、および解釈。
---

:::ノート フエンテ カノニカ
エスタページナリフレジャ`docs/source/sorafs/developer/orchestrator.md`。満天は、コピアス・シンクロニザダス・ハスタ・ケ・セ・リタイア・ロス・ドキュメント・ヘレダドスです。
:::

# 複数オリジンを取得するためのツール

SoraFS のインパルスを決定する複数のオリジンを取得するための命令
証明されたデータを公開し、広告を表示します
ラ・ゴベルナンザ。オルケスタドール、セナーレスの設定を説明します
エスペラール デュランテ ロスのロールアウトとテレメトリ指数の指標のフルホスの影響
デ・サルード。

## 1. 設定の再開

最適な構成を組み合わせたオルケスタドール:

|フエンテ |プロポジト |メモ |
|----------|-----------|----------|
| `OrchestratorConfig.scoreboard` |正常なペソス デ プローベドア、テレメトリアのフレスコラ、および永続的なスコアボード JSON を聴覚的に検証します。 | `crates/sorafs_car::scoreboard::ScoreboardConfig` によるレスパルダード。 |
| `OrchestratorConfig.fetch` |排出時の制限の適用 (reintentos の事前制限、同意の制限、検証の切り替え)。 | `FetchOptions` と `crates/sorafs_car::multi_fetch` を参照してください。 |
| CLI / SDK のパラメータ |ピアの数を制限し、テレメトリの補助地域と拒否/ブーストの政治を制限します。 | `sorafs_cli fetch` エストスフラグの指示を説明します。 los SDK los pasan（`OrchestratorConfig`経由）。 |

Los helpers JSON en `crates/sorafs_orchestrator::bindings` シリアル番号ラ
Norito JSON の構成が完了しました。ポータブルなエントリのバインディングを確認できます。
SDK と自動化。

### 1.1 JSON の構成

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

`iroha_config` (`defaults/`,
ユーザー、実際) パラケロスデスリーグ決定論、ここにロスミスモスの限界
エントレノドス。フォールバックの直接実行のみのロールアウトの実行
SNNet-5a、`docs/examples/sorafs_direct_mode_policy.json` とラグアを参照してください。
コンプリメンタリア en `docs/source/sorafs/direct_mode_pack.md`。

### 1.2 絶頂宣言

SNNet-9 は、オルケスタドールの制御を完全に統合します。アン
新しいオブジェクト `compliance` 設定 Norito JSON キャプチャ ロス カーブアウト
Modo を直接フェッチするパイプラインの実行のみ:

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

- `operator_jurisdictions` ISO-3166 alpha-2 ドンデ オペラ エスタ宣言
  オルケスタドールのインスタンシア。ロス・コディゴス・セ・ノーマル・ア・マユスキュラス・デュランテ・エル
  パーセオ。
- `jurisdiction_opt_outs` ゴベルナンザの登録情報。クアンド アルグナ
  オペラの司法管轄区域、エル・オルケスタドール・アプリケーション
  `transport_policy=direct-only` はフォールバックを発行します
  `compliance_jurisdiction_opt_out`。
- `blinded_cid_opt_outs` マニフェストのダイジェスト リスト (CID のコード、コードのコード)
  ヘックスマユスキュラス）。ラスカーガスはタンビエンフエルザンラプラニフィカシオンと一致します
  直接のみの y 指数 el フォールバック `compliance_blinded_cid_opt_out` en la
  テレメトリア。
- `audit_contacts` registra las URI que la gobernanza espera que los operadores
  publiqueen en sus プレイブック GAR。
- `attestations` レスパルダン ラ の最高のパケットをキャプチャ
  政治。 Cada entrada 定義 una `jurisdiction` オプション (codigo ISO-3166)
  alpha-2)、un `document_uri`、el `digest_hex` canonico de 64 文字、el
  タイムスタンプの発行 `issued_at_ms` および `expires_at_ms` はオプションです。エストス
  健康食品のチェックリストの芸術品
  ツーリング・デ・ゴベルナンサ・プエダ・ヴィンキュラー・ラス・アヌラシオネス・コン・ラ・ドキュメンタシオン
  フィルマダ。

コンプライアンスの中央値と習慣性のブロック
オペラドールの設定を決定する必要があります。エル
orquestador アプリケーションのコンプライアンス _despues_ の書き込みモードのヒント: si を含む
UN SDK solicita `upload-pq-only`、法的解釈による除外は禁止されています
シグエン・フォルツァンド・トランスポート・ダイレクトのみ、ファラン・ラピド・クアンドは存在しません
プルーベドアス・アプトス。

オプトアウト時にカタログを失う
`governance/compliance/soranet_opt_outs.json`;エル・コンセホ・デ・ゴベルナンサの出版
actualizaciones mediante はエチケットをリリースします。完全に完了します
設定 (証明書を含む) は、必要な電子メールを送信します。
`docs/examples/sorafs_compliance_policy.json`、操作を開始します
エルのキャプチャ
[GAR コンプリミエントのプレイブック](../../../source/soranet/gar_compliance_playbook.md)。

### 1.3 CLI と SDK の調整|旗 / カンポ |効果 |
|--------------|----------|
| `--max-peers` / `OrchestratorConfig::with_max_providers` |スコアボードに表示される制限を確認します。 Ponlo en `None` para usar todos los proveedores の資格があります。 |
| `--retry-budget` / `FetchOptions::per_chunk_retry_limit` |チャンクの再表示を制限します。スーパーエルリミテ属 `MultiSourceError::ExhaustedRetries`。 |
| `--telemetry-json` |スコアボードのコンストラクターでのレイテンシア/ファロのスナップショット。 La telemetria obsoleta mas alla de `telemetry_grace_secs` marca proveedores como は対象外です。 |
| `--scoreboard-out` |事後検査によるスコアボードの計算 (適格者 + 不適格者を証明) を維持します。 |
| `--scoreboard-now` |スコアボードのタイムスタンプ (Unix のセグンドス) を使用して、試合結果のキャプチャを確認したり、決定したりできます。 |
| `--deny-provider` / フック・デ・ポリティカ・デ・スコア |広告の計画に関する決定的な証明を除外します。ネグラ・デ・レスプエスタ・ラピダを使用します。 |
| `--boost-provider=name:delta` |ラウンドロビン ポンデラード パラ アン プルーフ マンテニエンド インタクトス ロス ペソス デ ゴベルナンザの審査。 |
| `--telemetry-region` / `OrchestratorConfig::with_telemetry_region` |ダッシュボードの構造やロールアウトの地理情報を記録するための指標を記録します。 |
| `--transport-policy` / `OrchestratorConfig::with_transport_policy` |欠陥 `soranet-first` は、複数の起源を持つオルケスタドールを攻撃します。米国 `direct-only` はダウングレードの準備をしており、事前に指示する必要があります。`soranet-strict` はパイロット PQ のみです。コンプライアンスに関する遵守事項は、すべて電子手帳に記載されています。 |

SoraNet ファースト エス アホラ エル ヴァロール ポル 欠陥、ロス ロールバック デベン シタール エル
ブロックアドールSNNet特派員。 Tras 段階的 SNNet-4/5/5a/5b/6a/7/8/12/13、
la gobernanza endercera la postura requerida (hacia `soranet-strict`);ハスタ
エントンセス、ソロ ラス アヌラシオネス モティバダス ポル インシデント デベン プライオリザール
`direct-only`、ロールアウトのログに関するデベントレジストラ。

Todos los flags anteriores aceptan sintaxis estilo `--` Tanto en
`sorafs_cli fetch` コモエンエルビナリオ `sorafs_fetch` オリエンタド
デサロラドール。 Los SDK exponen las missmas opciones mediante builderstipados。

### 1.4 キャッシュガードの取得

LA CLI アホラ インテグラ エル セレクター デ ガード デ ソラネット パラ ケ ロス オペラドーレス
プエダン・フィハルは、正式な決定を下すための中継を開始し、展開を完了する
SNNet-5 の輸送。トレス・ヌエボス・フラグがエル・フルホを制御:

|旗 |プロポジト |
|------|-----------|
| `--guard-directory <PATH>` | JSON のアーカイブを参照して、リレーのコンセンサスを説明します (se muestra un subconjunto abajo)。安全なキャッシュと取り出し用のディレクトリを参照できます。 |
| `--guard-cache <PATH>` | `GuardSet` コードを Norito に保存します。後から再利用できるキャッシュには、新しい管理者による管理が含まれます。 |
| `--guard-target <COUNT>` / `--guard-retention-days <DAYS>` |フィジャール (3 番目の欠陥) および 30 番目の欠陥の保持に関する保護オプションをオーバーライドします。 |
| `--guard-cache-key <HEX>` | MAC Blake3 のアーカイブを再利用する前に、32 バイトのセキュリティ キャッシュを保護する必要があります。 |

安全な警備のためのラスカーガス:

El フラグ `--guard-directory` アホラ エスペラ アン ペイロード `GuardDirectorySnapshotV2`
Norito のコード。スナップショットのバイナリコンティエン:

- `version` — バージョン デル エスケマ (実際の `2`)。
- `directory_hash`、`published_at_unix`、`valid_after_unix`、`valid_until_unix` —
  メタデータはコンセンサスと一致し、認証を取得します。
- `validation_phase` — 政治的証明書の計算 (`1` = 許可
  una sola farma Ed25519、`2` = 企業ダブルを優先、`3` = 企業を要求
  ダブルス）。
- `issuers` — `fingerprint`、`ed25519_public` y に対するエミソール デ ゴベルナンザ
  `mldsa65_public`。計算上の指紋の紛失
  `BLAKE3("soranet.src.v2.issuer" || ed25519 || u32(len(ml-dsa)) || ml-dsa)`。
- `relays` — SRCv2 バンドルのリスト (サリダ)
  `RelayCertificateBundleV2::to_cbor()`)。 Cada バンドルには記述子デルが含まれています
  リレー、フラグ デ キャパシダード、ポリティカ ML-KEM および企業ダブル Ed25519/ML-DSA-65。

LA CLI 検証証明書バンドル コントラ ラス クラベス デ 大臣宣言
キャッシュ・デ・ガードと融合したディレクトリ。 Los esquemas JSON はここにあります
いいえ、受け入れられません。スナップショット SRCv2 が必要です。CLI コン `--guard-directory` を呼び出して、コンセンサスの一致を確認します
キャッシュが存在します。エル セレクター コンセルバ ロス ガード フィハドス ケ アウン エスタン デントロ
息子の記憶を保存するためのファイルをディレクトリに保存します。ロスリレーヌエボス
レンプラザ・エントラダス・エクスピラダス。取り出して終了し、実際にキャッシュを取得します
`--guard-cache` による新しい指標の記述、マンテニエンドセッション
その後の決定論者。 Los SDK は、互換性のある再現性を備えています
ラマー `GuardSelector::select(&RelayDirectory, existing_guard_set, now_unix_secs)`
y pasar el `GuardSet` の結果は `SorafsGatewayFetchOptions` になります。

`ml_kem_public_hex` セレクターの優先ガードと容量 PQ を許可します
SNNet-5 を使用できます。ロストグルデエタパ (`anon-guard-pq`,
`anon-majority-pq`、`anon-strict-pq`) アホラ デグラダン自動リレー
クラシコ: クアンド・ヘイ・アン・ガード PQ ディスポニブル・エル・セレクター エリミナ・ロス・ピネス
クラシコ ソブランテス パラ ケ ラス セシオネス ポストオリレス 好意的な握手
ヒブリドス。 CLI/SDK エキスポネンの履歴書を介してメズクラの結果を失う
`anonymity_status`/`anonymity_reason`、`anonymity_effective_policy`、
`anonymity_pq_selected`、`anonymity_classical_selected`、`anonymity_pq_ratio`、
`anonymity_classical_ratio` ロスカンポス補完候補/赤字/
供給の変動、停電やフォールバックのクラシコの危険性。

セキュリティ ディレクトリをアホラ プエデンに組み込み、SRCv2 を完全にバンドルする
`certificate_base64`。 El orquestador decodifica cada バンドル、revalida las farmas
Ed25519/ML-DSA は、セキュリティ保護のための安全な証明書を保持します。
Cuando hay un certificado presente se convierte en la fuente canonica para
クラベス PQ、握手とポンデラシオンの優先順位。ロス証明書エクスピラドス
デスカルタンとセレクタは、記述子を参照します。ロス
サーキットでの活動を宣伝する証明書
`telemetry::sorafs.guard` y `telemetry::sorafs.circuit`、レジストラン経由の指数
有効なイベントを開催し、ハンドシェイクとイベントを監視してください。
パラカダガード。

米国ロスヘルパーデラCLIパラマンテナーロススナップショットシンクロニザドスコンロス
広報担当者:

```bash
sorafs_cli guard-directory fetch \
  --url https://directory.soranet.dev/mainnet_snapshot.norito \
  --output ./state/guard_directory.norito \
  --expected-directory-hash <directory-hash-hex>

sorafs_cli guard-directory verify \
  --path ./state/guard_directory.norito \
  --expected-directory-hash <directory-hash-hex>
```

`fetch` ディスコの検証スナップショット SRCv2 のダウンロード、
mientras que `verify` パイプラインのアーティファクト オブテニドスの検証を繰り返します
装備を開始し、JSON クエリを参照してセレクターを送信します
CLI/SDK を保護します。

### 1.5 回路図のゲスト

警備員や警備員とリレーの監督を行うことができます
事前構築用の回路のオルケスタドール アクティバ エル ゲストール デ シクロ デ ヴィーダ
SoraNet の回路を改修し、キャッシュを取得します。生存環境の設定
`OrchestratorConfig` (`crates/sorafs_orchestrator/src/lib.rs:305`) ドス・カンポ経由
ヌエボス:

- `relay_directory`: SNNet-3 パラケロスホップディレクトリのスナップショットのディレクトリ
  中間/出口の形式決定の選択。
- `circuit_manager`: オプションの構成 (欠陥に対する修正) 要求
  TTL 回路を制御します。

Norito JSON アホラ アセプタ アン ブロック `circuit_manager`:

```json
"circuit_manager": {
  "enabled": true,
  "circuit_ttl_secs": 900
}
```

Los SDK reenfian ロス ダトス デル ディレクトリ経由
`SorafsGatewayFetchOptions::relay_directory`
(`crates/iroha/src/client.rs:320`)、CLI に接続して自動接続します
QUE SE SUMINISTRE `--guard-directory` (`crates/iroha_cli/src/commands/sorafs.rs:365`)。

エル ゲストル レヌエバ サーキットス クアンド カンビアン ロス メタデータス デル ガード (エンドポイント、
クラーベ PQ o タイムスタンプ fijado) o cuando vence el TTL。エルヘルパー `refresh_circuits`
事前にフェッチを実行する (`crates/sorafs_orchestrator/src/lib.rs:1346`)
`CircuitEvent` パラケ ロス オペラドールのラストリーンの決定のログを出力します
シクロ・デ・ヴィーダ。エルソークテスト
`circuit_manager_latency_soak_remains_stable_across_rotations`
(`crates/sorafs_orchestrator/src/soranet.rs:1479`) デモストラテンシアの安定性
トラベス・デ・トレス・ロタシオネス・デ・ガード。レポートを参照してください
`docs/source/soranet/reports/circuit_stability.md:1`。

### 1.6 プロキシ QUIC ローカル

El orquestador puede iniciar opcionalmente un proxy QUIC local para que las
拡張機能 del navegador と losadadares SDK にテンガン クエリはありません
クラーベ・デ・キャッシュ・デ・ガードの証明書。指示を与えるためのプロキシ
ループバック、ターミナル接続 マニフェスト Norito クエリの説明を迅速に実行します
任意のクライアントのキャッシュとガードの証明書。ロスイベントスデ
トランスポート・エミドス・ポート・エル・プロキシ・コンタビリザン
`sorafs_orchestrator_transport_events_total`。

新しいブロック `local_proxy` の JSON デルでのプロキシの使用
オルケスタドール:

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
```- `bind_addr` 代理人ドンデ エスクチャ エル コントロール (米国プエルト `0` パラ弁護士)
  ウン・プエルト・エフィメロ）。
- `telemetry_label` ダッシュボード パラケロス メトリクスを宣伝します
  プロキシのセッションとフェッチを区別します。
- `guard_cache_key_hex` (オプション) プロキシのエクスポンガ ラ ミスマ キャッシュを許可します
  CLI/SDK の警備員、拡張機能の管理
  デル・ナベガドール。
- `emit_browser_manifest` オルタナティブ SI EL ハンドシェイク開発、マニフェスト ケ ラス
  拡張子は pueden almacenar y validar です。
- `proxy_mode` ローカルの交通機関のプロキシ選択 (`bridge`) o
  SDK アブラン回路の SoraNet からのソロ送信メタデータ
  (`metadata-only`)。欠陥のあるプロキシ `bridge`;安定性 `metadata-only`
  cuando una ワークステーション deba exponer el マニフェスト罪 reenviar ストリーム。
- `prewarm_circuits`、`max_streams_per_circuit`、`circuit_ttl_hint_secs`
  指数ヒント アディシオナレス アル ナベガドール パラ ケ プエダ プレスプエスター ストリーム
  パラレロスと入札者は、再利用回路の代理人としての同意を得ることができます。
- `car_bridge` (オプション) CAR のローカル キャッシュをサポートします。エル・カンポ
  `extension` ストリームを省略してオブジェクトを制御します
  `*.car`;サーバーペイロード `*.car.zst` に設定された `allow_zst = true`
  プレコンプリミドス ディレクタメンテ。
- `kaigi_bridge` (オプション) スプール・アル・プロキシでの rutas Kaigi の説明。エル・カンポ
  `room_policy` アヌンシア シエル ブリッジ オペラ アン モード `public` または `authenticated`
  パラ ケ ロス クライアント デル ナベガドール プレセレクショネン ラス エチケット GAR
  修正します。
- `sorafs_cli fetch` エクスポネは `--local-proxy-mode=bridge|metadata-only` をオーバーライドします
  y `--local-proxy-norito-spool=PATH`、代替モードを許可
  政治的な JSON を変更するための代替スプールの出力。
- `downgrade_remediation` (オプション) 自動ダウングレードフックの設定。
  リレーパラテレメトリー観測を行うことができる場所
  ダウングレードの検出、`threshold` の設定の検出
  `window_secs`、ローカル プロキシ ローカル `target_mode` (欠陥あり)
  `metadata-only`)。セサンロスのダウングレードを解除し、プロキシをダウンロードしてください
  `resume_mode` は `cooldown_secs` を表します。米国エルアレグロ `modes` パラリミッター
  特定のリレーの役割をトリガーします (内部リレーの欠陥)。

アプリケーションのサービスを提供するモードブリッジのプロキシ設定:

- **`norito`** – クライアントからのストリームに関するオブジェクトの相対的な結果
  `norito_bridge.spool_dir`。 Los objetivos se sanitizan (罪の横断
  absolutas) y、クアンド エル アーカイブ、ティエン拡張なし、セ アプリカ エル スーフィホ
  ペイロードの送信前にリテラル ナベガドールを設定します。
- **`car`** – ストリームのオブジェクトを削除
  `car_bridge.cache_dir`、構成不良による拡張機能が含まれています
  rechazan ペイロードは、メノス クエリ `allow_zst` エステ アクティバドを含んでいます。ロス
  ブリッジ exitosos 応答 con `STREAM_ACK_OK` 転送前にバイトが失われます
  クライアントの検証のためのアーカイブ。

キャッシュ タグで HMAC のプロキシをアンボス (クアンド存在ウナ クラーベ)
キャッシュ デ ガード デュランテ エル ハンドシェイク) とテレメトリアの登録コード
`norito_*` / `car_*` パラケロスダッシュボードの出口、アーカイブの違い
ファルタンテスと衛生施設を訪問してください。

`Orchestrator::local_proxy().await` 排出時の処理を説明します
ラマドール プエダンは PEM の証明書を取得し、マニフェストを取得します
あなたの弁護士は、最終的なアプリケーションを作成する必要があります。

Cuando se habilita、el proxy ahora sirve registros **マニフェスト v2**。アデマス デル
キャッシュガードガードの証明書が存在します、v2 集約:

- `alpn` (`"sorafs-proxy/1"`) は、`capabilities` パラケロスクライアントを無視しています
  ストリームのプロトコルを確認してください。
- ハンドシェイクおよびブロック デ サル `cache_tagging` パラの派生に対する `session_id`
  HMAC タグを保護します。
- 回路選択のヒント (`circuit`、`guard_selection`、
  `route_hints`) パラケ ラス インテグラシオネス デル ナベガドール expongan una UI mas
  リカ・アンテス・デ・アブリルストリーム。
- `telemetry_v2` はローカル機器の管理およびプライバシーを制御します。
- Cada `STREAM_ACK_OK` には `cache_tag_hex` が含まれます。ロス クライアント リフレジャン エル ヴァロール エン
  ヘッダー `x-sorafs-cache-tag` の送信者への要求 HTTP または TCP パラケラス
  リポジトリの永続的なキャッシュの保護を選択します。Estos Campos preservan el formato;ロス・クライアント・アンティグオス・プエデン・イノラー
ラス ヌエバス クラベスと継続的なウサンド エル サブコンフント v1。

## 2. ファロスの意味

安全性と安全性を確保するために必要な規制を適用するエル・オルケスタドール
ソロバイトでトランスフィエラをやりましょう。カテゴリー別の損失:

1. **Fallos de elegibilidad (飛行前).** Proveedores sin capacidad de rango、
   広告の期限切れまたはテレメトリアの古い登録に関する登録
   スコアボードは計画を省略します。 CLI レナンの履歴書
   エル アレグロ `ineligible_providers` コン ラゾネス パラ ケ ロス オペラドーレス プエダン
   ラスパログのドリフトデゴベルナンザを検査します。
2. **Agotamiento entiempo de ejecucion.** Cada proveedor registra fallos
   連続。ウナ ベス ケ セ アルカンサ エル `provider_failure_threshold`
   構成、マルカ コモ `disabled` の復元を確認してください。
   `disabled` を実行し、テストを完了します。
   `MultiSourceError::NoHealthyProviders { last_error, chunk_index }`。
3. **Abortos deterministas.** Los limites estrictos se presentan como errores
   構造化:
   - `MultiSourceError::NoCompatibleProviders` — トラモ要求をマニフェストします
     デ・チャンク・オ・アラインアシオン・ケ・ロス・プロベドアレス・レストンテス・プエデン・クンプリルはありません。
   - `MultiSourceError::ExhaustedRetries` — 事前に設定した内容を確認してください
     再インテントスポルチャンク。
   - `MultiSourceError::ObserverFailed` — ロスオブザーバドレスダウンストリーム（フックデ
     ストリーミング) 再チャザロンとチャンク検証。

Cada エラーには、チャンクのインデックスに関する問題、責任のある処理、
ラ・ラゾン・ファイナル・デ・ファロ・デル・プルーヴェドール。 Trata estos fallos como bloqueadores デ
リリース: los reintentos con la missma entrada reproduciran el fallo hasta que
カンビエン エル アドバタイズ、ラ テレメトリー、ラ サルッド デル プルーベドール サブヤセンテ。

### 2.1 永続的なスコアボード

`persist_path` を設定し、決勝戦のスコアボードを記述します
トラスカダ排出。 JSON コンテンツのドキュメント:

- `eligibility` (`eligible` または `ineligible::<reason>`)。
- `weight` (ペソ・ノーマル・アシグナド・パラ・エスタ・エジェクシオン)。
- `provider` のメタデータ (識別子、エンドポイント、事前設定)
  同意）。

スコアボードのスナップショットとリリース時の成果物のアーカイブ
ブラックリストと監査対象のロールアウトに関する決定。

## 3. テレメトリアと堕落

### 3.1 メトリカス Prometheus

オルケスタドールは `iroha_telemetry` 経由で las siguientes metricas を発行します。

|メトリカ |ラベル |説明 |
|----------|----------|---------------|
| `sorafs_orchestrator_active_fetches` | `manifest_id`、`region` |ゲージ ド フェッチ オルケスタドス アン ブエロ。 |
| `sorafs_orchestrator_fetch_duration_ms` | `manifest_id`、`region` |極端なフェッチの遅延を記録するヒストグラム。 |
| `sorafs_orchestrator_fetch_failures_total` | `manifest_id`、`region`、`reason` |コンタドール デ ファロス ターミナルス (reintentos agotados、sin proveedores、fallo del observador)。 |
| `sorafs_orchestrator_retries_total` | `manifest_id`、`provider`、`reason` |コンタドール・デ・インテントス・デ・レインテント・ポル・プロヴェドール。 |
| `sorafs_orchestrator_provider_failures_total` | `manifest_id`、`provider`、`reason` |コンタドール・デ・ファロス、ニベル・デ・セッション・ケ・レヴァン、デシャビリタール・プロベドール。 |
| `sorafs_orchestrator_policy_events_total` | `region`、`stage`、`outcome`、`reason` |政治的意思決定 (クンプリダ vs ブラウンアウト) の展開とフォールバックに関する決定を報告します。 |
| `sorafs_orchestrator_pq_ratio` | `region`、`stage` | SoraNet の選択リレー PQ デントロ デル セットのヒストグラム。 |
| `sorafs_orchestrator_pq_candidate_ratio` | `region`、`stage` |スコアボードのスナップショットとリレー PQ の比率のヒストグラム。 |
| `sorafs_orchestrator_pq_deficit_ratio` | `region`、`stage` |政治的赤字のヒストグラム (実際のオブジェクトと現実の PQ)。 |
| `sorafs_orchestrator_classical_ratio` | `region`、`stage` |米国とのセッションにおけるリレーのクラシコのヒストグラム。 |
| `sorafs_orchestrator_classical_selected` | `region`、`stage` |リレーのクラシコス選択のヒストグラム。 |

ダッシュボードのメトリクスをステージング前にアクティベートし、ノブを統合します。
生産。 SF-6 の観察計画に推奨されるレイアウト:

1. **アクティビティをフェッチします** — 完了に相当するものをアラートします。
2. **頻度の割合** — アヴィサ クアンド ロス コンタドレス `retry` スーパーン
   ベースラインの履歴。
3. **Fallos de proveedor** — 確認メッセージを表示するページャー cuando cualquier proveedor
   スーパー `session_failure > 0` デントロ デ 15 分。

### 3.2 ログ構造のターゲット

ターゲットを決定するためのイベントの構造を公開する方法:- `telemetry::sorafs.fetch.lifecycle` — マルカス・デ・シクロ・デ・ヴィーダ `start` y
  `complete` チャンクの内容、再生時間、持続時間の合計。
- `telemetry::sorafs.fetch.retry` — 定期的なイベント (`provider`、`reason`、
  `attempts`) パラ食品トリアージマニュアル。
- `telemetry::sorafs.fetch.provider_failure` — 証明されたデシャビリタドス
  エラーが繰り返される。
- `telemetry::sorafs.fetch.error` — ファロス ターミナル レスミドス コン `reason` y
  メタデータス・オプシオナレス・デル・プロヴェードール。

ログ Norito が存在するパイプラインのログを取得します
事件はテンガ・ウナ・ユニカ・フエンテ・デ・ベルダド。ロス・イベントトス・デ・シクロ・デ・ヴィダ
`anonymity_effective_policy` 経由の exponen la mezcla PQ/clasica、
`anonymity_pq_ratio`、`anonymity_classical_ratio` y sus contadores asociados、
Raspar メトリクスのケーブルカー ダッシュボードを簡単に確認できます。 Durante が GA でロールアウト、
フィジャ エル ニベル デ ログ en `info` パラ イベント デ シクロ デ ヴィダ/再放送 アメリカ
`warn` パラファロスターミナル。

### 3.3 履歴書の JSON

Tanto `sorafs_cli fetch` は SDK を使用して Rust 開発を再開し、構造化を再開しました
ケコンティエン:

- `provider_reports` 出口/フラカソの状況は証明されていません
  デシャビリタド。
- `chunk_receipts` デタランド クエスト プローブドール レゾルビオ カダ チャンク。
- arreglos `retry_stats` e `ineligible_providers`。

問題を証明する履歴書のアーカイブ: 領収書の紛失
事前にログを記録するための指示。

## 4. チェックリスト操作

1. **CI の構成を準備します。** Ejecuta `sorafs_fetch` con la
   オブジェクトの構成、パス `--scoreboard-out` のキャプチャー・ラ・ヴィスタ
   elegibilidad y 比較対照リリース前方。 Cualquier が不適格であることが証明されました
   inesperado detiene la promocion。
2. **Validar テレメトリア** メトリカスをエクスポートする
   `sorafs.fetch.*` y ログ構造体が複数のオリジンをフェッチします
   パラユースアリオス。ファサード デルのインディカル メトリックスを使用する
   オルケスタドールのフエ・インボカド。
3. **ドキュメンタリーはオーバーライドされます。** 緊急事態を調整する `--deny-provider`
   o `--boost-provider`、JSON (CLI 呼び出し) と変更ログを確認します。
   ロスロールバックデベンリバーティルエルオーバーライドとキャプチャーと新しいスナップショットデル
   スコアボード。
4. **煙テストを繰り返します。** 制限を変更する前に、再指示を行います。
   デ・プローベドア、ハーサー・フェッチ・デル・カノニコを実行します
   (`fixtures/sorafs_manifest/ci_sample/`) 領収書の紛失を確認します
   チャンクはシガンシエンドデターミニスタ。

オルケスタドールの指揮官であるマンティエン・エル・ロス・パソス・アンテリオレス
必要なテレメトリのロールアウトで再現可能
ラレスペスタ事件。

### 4.1 政治のアヌラシオネス

ロス オペラドーレス プエデン フィハル ラ エタパ アクティバ デ トランスポート / アノニマト シン エディター
構成ベースのestableciendo `policy_override.transport_policy` y
`policy_override.anonymity_policy` `orchestrator` の JSON (o スミミニストランド)
`--transport-policy-override=` / `--anonymity-policy-override=`
`sorafs_cli fetch`)。クアンド・クアルキエラ・デ・ロスはエスタ・プレゼンテ・エルを上書きします
オルケスタドールのフォールバック ブラウンアウト習慣: si el nivel PQ solicitado no se
満足のいくものを見つけて、`no providers` を劣化版で取得します
サイレンシオ。 Volver al comportamiento pordefeto es tan simple como limpiar los
カンポス デ オーバーライド。