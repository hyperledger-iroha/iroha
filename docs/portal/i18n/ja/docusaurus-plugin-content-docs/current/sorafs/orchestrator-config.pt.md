---
lang: ja
direction: ltr
source: docs/portal/docs/sorafs/orchestrator-config.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: オーケストレーター構成
title: オルケストラドールの設定 SoraFS
Sidebar_label: オルケストラドールの設定
説明: マルチオリジェムを取得するための命令を設定し、テレメトリの説明を解釈します。
---

:::note フォンテ カノニカ
エスタ・ページナ・エスペルハ`docs/source/sorafs/developer/orchestrator.md`。マンテンハ・アンバスは、コピー・シンクロニザダスとして、別の文書を食べました。
:::

# マルチオリジェムをフェッチするためのツール

マルチオリジェムを取得するための命令は、SoraFS のダウンロードを決定します。
プローベドールとの連携によるパルティールの公開広告のレスパルダドス
ペラ・ガバナンカ。エステ ガイアは、オルケストラドールの構成を説明し、南アフリカの
ファルハ エスペラール デュランテ ロールアウトとテレメトリア エキスポの最新情報
インディカドール デ サウデ。

## 1. 設定を変更する

設定を組み合わせてください:

|フォンテ |プロポジト |メモ |
|------|-----------|------|
| `OrchestratorConfig.scoreboard` |検証ペソを正規化して、テレメトリーのフレスコラを検証し、スコアボードの JSON を使用して聴覚を維持します。 |アポイアド ポル `crates/sorafs_car::scoreboard::ScoreboardConfig`。 |
| `OrchestratorConfig.fetch` |アプリケーションの実行時間の制限 (再試行の予算、合意の制限、検証の切り替え)。 |マペイアは `FetchOptions` と `crates/sorafs_car::multi_fetch` です。 |
| CLI / SDK のパラメータ |ピアの数、テレメトリーの地域、および拒否/ブーストの政治活動の制限。 | `sorafs_cli fetch` フラグを立てます。 OS SDK は `OrchestratorConfig` 経由で宣伝します。 |

OS ヘルパー JSON em `crates/sorafs_orchestrator::bindings` シリアル化
Norito JSON、トルナンド、SDK バインディングのポート設定を完了しました
eオートマカオ。

### 1.1 JSON の構成例

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

Persista o arquivo atraves do empilhamento いつものように `iroha_config` (`defaults/`,
ユーザー、実際の) パラケデプロイメントは、全体的な運用管理の制限を決定します。
うなずきます。フォールバック直接専用のパラメータ SNNet-5a ロールアウト、
`docs/examples/sorafs_direct_mode_policy.json` または方向性を参照してください
`docs/source/sorafs/direct_mode_pack.md` に対応します。

### 1.2 適合性の無効化

SNNet-9 は、秩序を維持するための完全な管理を行っています。えーっと
新しいオブジェクト `compliance` の構成 Norito JSON キャプチャ オス カーブアウト クエリ
forcam o パイプライン de fetch ao modo 直接のみ:

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

- `operator_jurisdictions` は ISO-3166 alpha-2 をオンデエスタで宣言します
  オルケストラドール・オペラの瞬間。 Os codigos saonormalizados para maiusculas
  デュランテ o 解析。
- `jurisdiction_opt_outs` 行政上の登録。クアンド・クアルケル
  オペラ座のパリスタ、オルケストラドールの申請を行う司法裁判所
  `transport_policy=direct-only` フォールバックの動機を出力する
  `compliance_jurisdiction_opt_out`。
- `blinded_cid_opt_outs` マニフェストのダイジェスト リスト (CID のコード、コードのコード)
  ヘックスマイウスクロ）。直接のみのカメラアジェンダメントのペイロード通信者
  テレメトリのフォールバック `compliance_blinded_cid_opt_out` を説明します。
- `audit_contacts` レジストラは、政府機関のオペランドの URI として登録されています
  パブリック nos プレイブック GAR。
- `attestations` 完全に一致する情報をキャプチャし、継続的な情報を取得します
  政治。 Cada entrada 定義 uma `jurisdiction` オプション (codigo ISO-3166)
  alpha-2)、um `document_uri`、o `digest_hex` canonico de 64 文字、o
  `issued_at_ms` のタイムスタンプ、`expires_at_ms` はオプションです。エセス
  芸術的な芸術品のチェックリスト、オルケストラドールを演奏するためのチェックリスト
  ferramentas de Governmenta possam vincular は documentacao assinada をオーバーライドします。

通常の設定を介したブロックの適合性
que os operadores recebam は deterministas をオーバーライドします。オー オルケストラドール アプリカ
_depois_ 書き込みモードのヒント: SDK 要求に関するメッセージ
`upload-pq-only`、交通機関のマニフェストの法的オプトアウト
パラ直接のみ、ファルハム・ラピダメンテ・クアンド・ナオ存在証明が適合します。

オプトアウト時のカタログ
`governance/compliance/soranet_opt_outs.json`; o コンセリョ・デ・ガベルナンカの広報
atualizacoes は tagueadas をリリースします。設定の完了例
(証明書を含む) esta disponivel em
`docs/examples/sorafs_compliance_policy.json`、プロセス操作のエスタ
キャプチャーラドいいえ
[GAR 準拠のプレイブック](../../../source/soranet/gar_compliance_playbook.md)。

### 1.3 CLI と SDK の調整|旗 / カンポ |エフェイト |
|--------------|----------|
| `--max-peers` / `OrchestratorConfig::with_max_providers` |制限された量の証明は、スコアボードを実行するためのフィルターを実行します。 Defina `None` は、ユーザーの Todos OS を証明します。 |
| `--retry-budget` / `FetchOptions::per_chunk_retry_limit` | Limita はチャンクごとに再試行します。エクシーダー・オー・リミテ・ジェラ `MultiSourceError::ExhaustedRetries`。 |
| `--telemetry-json` |インジェタのラテンシア/ファルハのスナップショットにはスコアボードの作成者はいません。 Telemetria obsoleta alem de `telemetry_grace_secs` marca provedores como inelegiveis。 |
| `--scoreboard-out` |実行中のスコアボード計算 (provedores elegiveis + inelegiveis) を維持します。 |
| `--scoreboard-now` |スコアボードのタイムスタンプ (セグンドス Unix) は、試合結果を決定するために必要な情報を記録します。 |
| `--deny-provider` / フック・デ・ポリティカ・デ・スコア |削除広告の形式的決定性の証明を除外します。ブラックリストに登録する Rapid を利用します。 |
| `--boost-provider=name:delta` | Ajusta os Creditos は、ラウンドロビンでポンデラドを証明し、完全な統治期間を維持します。 |
| `--telemetry-region` / `OrchestratorConfig::with_telemetry_region` | Rotula のメトリクスは、ダッシュボードのパラメタを記録し、地理情報やロールアウトのフィルターを記録します。 |
| `--transport-policy` / `OrchestratorConfig::with_transport_policy` |パドラオ アゴラ `soranet-first` ジャケ オ オルケストラドール マルチオリジェム ベース。 `direct-only` を使用してダウングレードを準備し、安全性を確保し、パイロット PQ のみで `soranet-strict` を予約します。コンフィミダードを継続して送信または固定したものをオーバーライドします。 |

SoraNet ファースト、アゴラ、パドラオ、エンヴィオ、ロールバック開発、シタール、ブロケアドール
SNNet 関連。 SNNet-4/5/5a/5b/6a/7/8/12/13 の卒業生を預けます。
ガバナンカ・エンデュラセラ・ア・ポストラ・レケリダ (rumo a `soranet-strict`);ラを食べた、
アペナスは、インシデントに関する動機を優先して開発し、`direct-only`、エレレスをオーバーライドします。
開発者登録者にはロールアウトのログはありません。

Todos os フラグ acima aceitam a sintaxe `--` Tanto em `sorafs_cli fetch` quanto no
ビナリオ `sorafs_fetch` ボルタード・ア・デセンボルベドーレス。 OS SDK はメスマス オペレーションとして機能します
ビルダーのヒントを教えてください。

### 1.4 キャッシュガードの取得

オペラドールのポッサムとSoraNetの警備員のセレクターを統合するCLI
フィクサーリレーは、展開を完了する前に形式を確定する必要があります
SNNet-5 をトランスポートします。 Tres novos フラグが Fluxo を制御します:

|旗 |プロポジト |
|------|-----------|
| `--guard-directory <PATH>` |最近のリレーのコンセンサスに関する JSON の説明 (サブセット abaixo)。実行者がフェッチする前に、ディレクトリを設定してキャッシュを保護します。 |
| `--guard-cache <PATH>` | `GuardSet` コードを Norito として保持します。後続の再利用は、キャッシュ メモリ sem novo ディレクトリを実行します。 |
| `--guard-target <COUNT>` / `--guard-retention-days <DAYS>` |フィクサー (パドラオ 3) と保持している番号 (パドラオ 30 ディアス) のオプションをオーバーライドします。 |
| `--guard-cache-key <HEX>` | 32 バイトのセキュリティ メッセージ キャッシュを使用して、MAC Blake3 パラメータを再利用できるかどうかを確認できます。 |

ディレクトリを保護するためのファイルとして、次のようにコンパクトにします。

O フラグ `--guard-directory` アゴラ エスペラウム ペイロード `GuardDirectorySnapshotV2`
Norito のコード。スナップショット バイナリオ コンテム:

- `version` — エスケーマを実行します (実質 `2`)。
- `directory_hash`、`published_at_unix`、`valid_after_unix`、`valid_until_unix` —
  メタダドス デ コンセンサス QUE DEVEM 特派員と証明書認証を取得します。
- `validation_phase` — 証明書の政治ゲート (`1` = 許可証)
  assinatura Ed25519、`2` = preirir assinaturas duplas、`3` = exigir assinaturas
  デュプラス）。
- `issuers` — 行政長官コム `fingerprint`、`ed25519_public` e
  `mldsa65_public`。 OS の指紋 sao calculados como
  `BLAKE3("soranet.src.v2.issuer" || ed25519 || u32(len(ml-dsa)) || ml-dsa)`。
- `relays` — SRCv2 バンドルの uma リスト (サイダ
  `RelayCertificateBundleV2::to_cbor()`)。 Cada バンドル カレガ o 記述子
  リレー、大容量フラグ、政治 ML-KEM e assinaturas duplas Ed25519/ML-DSA-65。

CLI 検証証明書バンドルは、提出者宣言と反対です
mesclar ディレクトリ com キャッシュガード。 Esbocos JSON 代替ファイル
アセイトス。スナップショット SRCv2 SAO オブリガトリオス。CLI com `--guard-directory` para mesclar o consenso mais recente com o を呼び出します。
キャッシュは存在します。おお、セレトール・プリザーバ・ガードズ・フィサドス・ケ・アインダ・エスタオ・デントロ・ダ
Janela de retencao e sao elegive にはディレクトリがありません。ノボスリレーの代替エントラダ
エクスピラダ。フェッチ・ベム・スセディドのデポジット、および自動キャッシュおよびボルタ・エスクリットのキャッシュ
`--guard-cache` 経由のカミーニョ フォルネシドはありません。その後のマンテンド セッションはありません
基準。 OS SDK のポデム再現またはメスモ コンポルタメント シャマンド
`GuardSelector::select(&RelayDirectory, existing_guard_set, now_unix_secs)` e
パスサンド o `GuardSet` 結果の `SorafsGatewayFetchOptions`。

`ml_kem_public_hex` 許可セレクターは、容量性 PQ のガードを優先します
SNNet-5 の展開はすぐに開始されます。 OS の切り替え (`anon-guard-pq`、
`anon-majority-pq`、`anon-strict-pq`) アゴラ リバイサム リレー クラシコス
自動修正: Quando um Guard PQ esta disponivel o selector ピンの削除
古典的な優れたパフォーマンスで、その後のセッションで好意的なカメラのハンドシェイクを行います。
CLI/SDK は、`anonymity_status`/ 経由で結果のミスを解決します。
`anonymity_reason`、`anonymity_effective_policy`、`anonymity_pq_selected`、
`anonymity_classical_selected`、`anonymity_pq_ratio`、`anonymity_classical_ratio`
カンポス補完候補/赤字/供給デルタ、トルナンド クラロス
ブラウンアウトとフォールバックのクラシコ。

バンドル SRCv2 を完全に保護するためのディレクトリ
`certificate_base64`。 O orquestrador decodifica cada バンドル、revalida として
assinaturas Ed25519/ML-DSA e retém o certificado Analisado junto Ao Cache de
警備員。 Quando um certificado esta presente ele se torna a fonte canonica para
チャベス PQ、握手とポンダラソンの優先権。 SAO認証期限
descartados e o seletor retorna aos Campos alternativos do 記述子。証明書
サーキットやサオの展示を宣伝します。
`telemetry::sorafs.guard` および `telemetry::sorafs.circuit`、ジャネラの登録
検証、ハンドシェイクおよび一連の動作の監視
カダガード。

sincronia com publicadores の CLI パラメータ スナップショットに OS ヘルパーを使用します。

```bash
sorafs_cli guard-directory fetch \
  --url https://directory.soranet.dev/mainnet_snapshot.norito \
  --output ./state/guard_directory.norito \
  --expected-directory-hash <directory-hash-hex>

sorafs_cli guard-directory verify \
  --path ./state/guard_directory.norito \
  --expected-directory-hash <directory-hash-hex>
```

`fetch` スナップショット SRCv2 の事前検証、ディスコ、エンカント
`verify` パイプラインの検証用のアーティファト ビンドス デ アウトラス装備の再現、
CLI/SDK を保護するために、JSON を使用して、セレクターを実行します。

### 1.5 回路図のゲスト

Quando um リレー ディレクトリ、fornecidos、orquestrador のキャッシュを保護します
建設前およびリノバーの回路での動作を開始します。
回路の SoraNet からデータを取得します。 `OrchestratorConfig` を有効にするための設定
(`crates/sorafs_orchestrator/src/lib.rs:305`) ドイス・ノボス・カンポス経由:

- `relay_directory`: ディレクトリ SNNet-3 パラケ ホップのスナップショットを実行します
  中間/出口は決定的な形式を選択します。
- `circuit_manager`: オプションの設定 (パドラオの使用) 制御機能
  TTLはサーキットで行います。

Norito JSON アゴラ アセイタ ウム ブロコ `circuit_manager`:

```json
"circuit_manager": {
  "enabled": true,
  "circuit_ttl_secs": 900
}
```

OS SDK encaminham gotos do ディレクトリ経由
`SorafsGatewayFetchOptions::relay_directory`
(`crates/iroha/src/client.rs:320`)、CLI による自動接続
`--guard-directory` フォルネシド (`crates/iroha_cli/src/commands/sorafs.rs:365`)。

O gestor renova circuito semper que metadados do Guard mudam (エンドポイント、チャベ)
PQ またはタイムスタンプ修正) または Quando または TTL 有効期限。 O ヘルパー `refresh_circuits`
事前にフェッチを実行する (`crates/sorafs_orchestrator/src/lib.rs:1346`)
ログを出力 `CircuitEvent` パラ ケ オペラドール ポッサム ラスター デ シクロ
デ・ヴィダ。 O浸漬試験 `circuit_manager_latency_soak_remains_stable_across_rotations`
(`crates/sorafs_orchestrator/src/soranet.rs:1479`) デモンストラ ラテンシア エスタベル
アトラベス・デ・トレス・ロタコス・デ・ガード。ヴェジャ・オ・リラトリオ・エム
`docs/source/soranet/reports/circuit_stability.md:1`。

### 1.6 プロキシ QUIC ローカル

プロキシ QUIC ローカル パラケ エクステンソの開始オプションを指定します
デナベガドールと適応 SDK の正確な一般証明書とチャベス
キャッシュガード。 O プロキシ リガ、エンデレコ ループバック、エンセラ コネクソ QUIC e
復元マニフェスト Norito キャッシュ保護証明書の記述
オプションのaoクライアント。輸送機関のイベントがペロプロキシ経由で送信されます
`sorafs_orchestrator_transport_events_total`。

Habilite または proxy por meio do novo bloco `local_proxy` no JSON do orquestrador:

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
```- `bind_addr` 代理窓口を制御 (ポータルを使用) `0` 弁護士ポータルを使用
  エフェメラ）。
- `telemetry_label` ダッシュボードの指標としての情報の伝達
  プロキシとセッソのフェッチ。
- `guard_cache_key_hex` (オプション) プロキシの公開メッセージキャッシュの許可
  警備員は CLI/SDK を使用し、拡張機能を使用してナベガドール アリンハダを実行します。
- `emit_browser_manifest` 代替ハンドシェイク デボルブ、マニフェスト キュー
  アルマゼナールと有効性を拡張します。
- `proxy_mode` プロキシ ファズ ブリッジ ローカル (`bridge`) を選択し、アペナスを送信します
  SDK に関するメタダド、Abram 回路、SoraNet と独自のコンテンツ
  (`metadata-only`)。 O プロキシ パドラオ e `bridge`; `metadata-only` Quando um を使用してください
  ワークステーションは、マニフェストの再送信ストリームをエクスポートします。
- `prewarm_circuits`、`max_streams_per_circuit`、`circuit_ttl_hint_secs`
  expõemヒントは、パラレロスeをストリームするか、またはナベガドールパラケポッサをアディショナします
  エンター、クアント、プロキシ再利用回路。
- `car_bridge` (オプション) ローカル CAR のパラメータ キャッシュ。オー・カンポ
  `extension` コントロールはサフィックス アネクサド クアンド オールボ デ ストリームを省略します `*.car`;
  Defina `allow_zst = true` パラ Servir ペイロード `*.car.zst` プリコンプライミド。
- `kaigi_bridge` (オプション) スプール AO プロキシの回転会議を実行します。オー・カンポ
  `room_policy` アヌンシア・セ・オ・ブリッジ・オペラ・モード `public` ou `authenticated`
  クライアントは GAR コレトスの OS ラベルを事前に選択します。
- `sorafs_cli fetch` expõe は `--local-proxy-mode=bridge|metadata-only` e をオーバーライドします
  `--local-proxy-norito-spool=PATH`、ランタイムモードの代替モードを許可します
  政治的な JSON を変更する代替スプールをサポートします。
- `downgrade_remediation` (オプション) 自動ダウングレードフックの設定。
  Quando habilitado、o orquestrador observa a telemetry de Relays para rajadas
  ダウングレード、`threshold` の構成、`window_secs`、força o
  プロキシローカルパラメータ `target_mode` (パドラオ `metadata-only`)。 Quando OSのダウングレード
  cessam、o プロキシ レトルナ ao `resume_mode` apos `cooldown_secs`。配列を使用する
  `modes` リレー固有の機能に関する制限事項 (パドラオ リレー)
  デ・エントラーダ）。

アプリケーション サービスを提供するためのモード ブリッジのプロキシを実行します:

- **`norito`** – クライアントとの相対的なストリームを実行します
  `norito_bridge.spool_dir`。 Os alvos sao sanitizados (sem traversal、sem caminhos)
  absolutos)、拡張機能、拡張子設定、アプリケーションの拡張機能
  ナビゲーターの送信時にペイロードを実行する必要があります。
- **`car`** – `car_bridge.cache_dir` のストリームを解決します、ヘルダム
  拡張パドラオ構成と再構成ペイロードの互換性
  `allow_zst` エステジャ・ハビリタド。ブリッジ bem-sucedidos 応答 com `STREAM_ACK_OK`
  転送前の OS バイトは、クライアントからのポッサム ファザー パイプラインへの転送を実行します
  ダ・ベリフィカオ。

キャッシュ タグを実行するか、プロキシを実行するか、HMAC を実行します (キャッシュ タグを使用する必要があります)
キャッシュ デ ガード デュランテまたはハンドシェイク) テレメトリのコード登録
`norito_*` / `car_*` パラケ ダッシュボードの違い、正しい情報
迅速な消毒を行ってください。

`Orchestrator::local_proxy().await` 問題を解決するための処理を実行します
possam ler、PEM do certificado、buscar、manifest do navegador、または solicitar
最後の準備を完了します。

Quando habilitado、またはプロキシ アゴラがレジストリを提供します **マニフェスト v2**。アレム・ドゥ
キャッシュガードガードに存在する証明書、v2 の付加条件:

- `alpn` (`"sorafs-proxy/1"`) 配列 `capabilities` パラケクライアント
  ストリームのプロトコルを確認してください。
- ええと `session_id` のハンドシェイクとブロコ デ サル `cache_tagging` の派生版
  HMAC タグを保護します。
- 回路とガードの選択に関するヒント (`circuit`、`guard_selection`、
  `route_hints`) パラ ケ インテグラコエス ド ナベガドール エクスポナム UM UI mais rica antes
  デアブリルストリーム。
- `telemetry_v2` com ノブはローカル機器の管理とプライバシーを提供します。
- `cache_tag_hex` を含む Cada `STREAM_ACK_OK`。クライアントのエスペルハムはヘッダーなしで勝利を収めた
  `x-sorafs-cache-tag` HTTP または TCP パラメータの送信要求がガードされます
  永久にキャッシュを保存し、記録を保存します。

Esses は実際のスキーマを実現します。クライアントは、ユーティリティを開発し、接続します
ネゴシアルストリームを完了します。

## 2. ファルハスの意味

予算を事前に確認して、容量を確認してください。
ユニコバイトセハトランスファーイド。ファルハス・セ・エンクアドラム・エム・トレス・カテゴリーとして:1. **Falhas de elegibilidade (飛行前).** Provedores sem capacidade de range、
   広告の有効期限とテレメトリアの廃止、登録は不要です
   スコアボードとアジェンダメントを実行します。 CLI 事前エンケムまたはアレイの再開
   `ineligible_providers` ドリフト オペラドールの検査を行う
   ガバナンカ sem raspar ログ。
2. **実行時のエスゴタメント** Cada は、連続した rastreia falhas を証明しました。クアンド
   `provider_failure_threshold` 活動、証明されたマルカド コモ `disabled`
   ペロ・レスタンテ・ダ・セッサオ。 `disabled` の手順に従って、transicionarem を実行します。
   o オルケストラドール レトルナ
   `MultiSourceError::NoHealthyProviders { last_error, chunk_index }`。
3. **決定的な機能を放棄します。** 正常な動作を制限します。
   - `MultiSourceError::NoCompatibleProviders` — マニフェストはすべての範囲に適用されます
     チャンクとアリンハメント、クエリ、プローベドア、レストラン、ナオ・コンセゲム・ホンラー。
   - `MultiSourceError::ExhaustedRetries` — チャンクごとの再試行予算
     コンスミド。
   - `MultiSourceError::ObserverFailed` — 下流の観測点 (フック
     ストリーミング) レジェイタラム ウム チャンク ベリフィカド。

チャンク問題のインデックスを取り込み、完全に削除します。
最後のデ・ファルハ・ド・プルードール。リリース時にエラーを報告します —
ファルハが広告を表示した後、テレメトリを再試行して再試行します。
あなたは、下位のムデムを証明しています。

### 2.1 持続的なスコアボード

Quando `persist_path` の構成、オルケストラドールのエスケープ、スコアボードの決勝
アポス・カダ・ラン。 O ドキュメント JSON コンテンツ:

- `eligibility` (`eligible` または `ineligible::<reason>`)。
- `weight` (ペソ・ノーマル・アトリブイド・パラ・エステ・ラン)。
- メタデータは `provider` (識別子、エンドポイント、予算の調整) を実行します。

決定事項をリリースするためのスコアボードのスナップショットをアーカイブする
監査とロールアウトの永続的な監査。

## 3. テレメトリアとデプラカオ

### 3.1 メトリカス Prometheus

O orquestrador は、`iroha_telemetry` 経由でセギンテス メトリクスとして出力します。

|メトリカ |ラベル |説明 |
|----------|----------|----------|
| `sorafs_orchestrator_active_fetches` | `manifest_id`、`region` |ゲージは、すべてのブーを取得します。 |
| `sorafs_orchestrator_fetch_duration_ms` | `manifest_id`、`region` |ポンタから取得した潜在的なヒストグラムを記録します。 |
| `sorafs_orchestrator_fetch_failures_total` | `manifest_id`、`region`、`reason` | Contador de falhas terminais (再試行エスゴタード、sem プローベドール、ファルハ デ オブザーバードール)。 |
| `sorafs_orchestrator_retries_total` | `manifest_id`、`provider`、`reason` |コンタドール・デ・テンタティバス・デ・リトライ・ポル・プロヴェドール。 |
| `sorafs_orchestrator_provider_failures_total` | `manifest_id`、`provider`、`reason` | Contador de falhas de provedor na sessao que levam a desabilitacao. |
| `sorafs_orchestrator_policy_events_total` | `region`、`stage`、`outcome`、`reason` |政治的判断 (クンプリダ vs ブラウンアウト) の展開とフォールバックの動機を決定します。 |
| `sorafs_orchestrator_pq_ratio` | `region`、`stage` | SoraNet 選択に関連するリレー PQ の参加者のヒストグラム。 |
| `sorafs_orchestrator_pq_candidate_ratio` | `region`、`stage` |リレー PQ の比率のヒストグラム、スコアボードのスナップショットなし。 |
| `sorafs_orchestrator_pq_deficit_ratio` | `region`、`stage` |政治的赤字のヒストグラム (PQ 実質参加者間のギャップ)。 |
| `sorafs_orchestrator_classical_ratio` | `region`、`stage` |古典的なリレーの参加者のヒストグラム。 |
| `sorafs_orchestrator_classical_selected` | `region`、`stage` |リレーの古典的な選択のヒストグラム。 |

ステージングの準備や操作ノブのメトリクスとしてダッシュボードとして統合
プロデュースする。 SF-6 の平面上の推奨レイアウト:

1. **アティボスを取得** — sem の完了状況を確認するためにアラートを送信します。
2. **ラザオ デ リトライ** — avisa quando contadores `retry` がベースラインを超えています
   ヒストリクス。
3. **ファルハス デ 証明** — ポケベルなしで警告が表示されます。
   クルーズ `session_failure > 0` デントロ デ 15 分。

### 3.2 攻撃対象のターゲット

ターゲット決定者向けの公開イベントの決定版:

- `telemetry::sorafs.fetch.lifecycle` — マルカドール `start` e `complete` com
  チャンクの感染、再試行、デュラソーの合計。
- `telemetry::sorafs.fetch.retry` — 再試行イベント (`provider`、`reason`、
  `attempts`) 消化器トリアージマニュアル。
- `telemetry::sorafs.fetch.provider_failure` — 安全性の証明
  エラーの繰り返し。
- `telemetry::sorafs.fetch.error` — falhas terminais resumidas com `reason` e
  メタダドス・オプシオネイスは証明されています。Encaminhe は、パイプラインのログ Norito が存在することを確認します。
事件が発生したら、ウニカ・フォンテ・デ・ベルダーデに報告します。イベント・デ・シクロ・デ・ヴィダ
`anonymity_effective_policy` 経由でミスチュラ PQ/classica を説明します。
`anonymity_pq_ratio`、`anonymity_classical_ratio` コンタドール アソシアドを使用し、
Tornando シンプルなインテグラル ダッシュボード sem raspar メトリカ。デュランテのロールアウト
GA、ログファイル `info` パライベントのシクロデヴィダ/再試行の使用を修正
`warn` パラファルハス終了。

### 3.3 履歴書 JSON

タント `sorafs_cli fetch` Quanto SDK Rust レポートと詳細な内容:

- `provider_reports` com contagens de sucesso/falha e se o Provedor foi
  デサビリタード。
- `chunk_receipts` デタルハンド品質証明アテンデウ カダ チャンク。
- 配列 `retry_stats` e `ineligible_providers`。

問題を解決するための証拠の保管 — OS の領収書
ログの記録に関するメタデータの作成。

## 4. 運用チェックリスト

1. **CI なしの設定を準備します。** `sorafs_fetch` com a configuracao を実行します。
   アルヴォ、パス `--scoreboard-out` ビザを取得してエレジビリダードを取得します
   com o リリース前と比較してください。 Qualquer は、無資格の無資格者であることを証明しました
   プロモーションを中断します。
2. **Validar テレメトリア。** エクスポート メトリクスを展開する必要があります `sorafs.fetch.*`
   e ログは、使用状況に応じて複数のオリジェムをフェッチします。あ
   オーセンシア・デ・メトリカス・ノーマルメント・インディカ・ケ・ア・ファチャダ・ド・オルケストラドール・ナオ・フォイ
   インボカダ。
3. **Documentar オーバーライド。** Ao aplicar `--deny-provider` ou `--boost-provider`
   緊急です。JSON (呼び出し CLI) をコミットします。変更ログはありません。ロールバックの開発
   リバータ、オーバーライド、キャプチャ、新しいスナップショット、スコアボード。
4. **再実行スモークテスト** 再試行または制限の変更予算の保管
   プローベドール、フィクスチャ カノニコを取得するための再ファサ
   (`fixtures/sorafs_manifest/ci_sample/`) チャンクの受信確認を確認する
   永続的決定主義者。

オーケストラの再現を行うための重要な役割を果たします
インシデントに応じてテレメトリが必要な場合に備えてロールアウトします。

### 4.1 政治上のオーバーライド

Operadores podem fixar o estagio ativo de Transporte/anonimato sem editar a
ベース定義 `policy_override.transport_policy` e を設定します
`policy_override.anonymity_policy` は `orchestrator` の JSON を使用します (このため
`--transport-policy-override=` / `--anonymity-policy-override=` あお
`sorafs_cli fetch`)。 Quando qualquer override esta presente、o orquestrador pula
o フォールバック ブラウンアウト 通常: se o nivel PQ solicitado nao puder serSatisfeito、o
falha com `no providers` を取得して、劣化した沈黙を守ります。 O ロールバック
パラオコンポルタメントパドラオとタオは、オーバーライドのクォントリンパーオスカンポスをシンプルにします。