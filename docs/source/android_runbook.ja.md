---
lang: ja
direction: ltr
source: docs/source/android_runbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: da7119ab99121dbcfc268f5406f43b16ac9149cef6500a45c6717ad16c02ab80
source_last_modified: "2026-01-28T17:01:56.615899+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Android SDK 操作ランブック

このランブックは、Android SDK を管理するオペレーターとサポート エンジニアをサポートします。
AND7 以降の展開。 SLA のための Android サポート プレイブックとペアリングする
定義とエスカレーション パス。

> **注:** インシデント手順を更新するときは、共有された手順も更新してください
> トラブルシューティング マトリックス (`docs/source/sdk/android/troubleshooting.md`)
> シナリオ テーブル、SLA、およびテレメトリの参照は、この Runbook と一致したままになります。

## 0. クイックスタート (ポケベル起動時)

詳細に進む前に、このシーケンスを Sev1/Sev2 アラート用に保管しておいてください。
以下のセクション:

1. **アクティブな構成を確認します。** `ClientConfig` マニフェスト チェックサムをキャプチャします。
   アプリの起動時に発行され、それを固定されたマニフェストと比較します。
   `configs/android_client_manifest.json`。ハッシュが異なる場合は、リリースを停止し、
   テレメトリ/オーバーライドに触れる前に、構成ドリフト チケットをファイルしてください (§1 を参照)。
2. **スキーマ diff ゲートを実行します。** に対して `telemetry-schema-diff` CLI を実行します。
   受け入れられたスナップショット
   (`docs/source/sdk/android/readiness/schema_diffs/android_vs_rust-20260305.json`)。
   `policy_violations` 出力を Sev2 として扱い、エクスポートをブロックします。
   矛盾は理解されています (§2.6 を参照)。
3. **ダッシュボード + ステータス CLI を確認します:** Android テレメトリ リダクションを開き、
   Exporter Health ボードを実行します。
   `scripts/telemetry/check_redaction_status.py --status-url <collector>`。もし
   当局が床下にいるかエクスポートエラーを起こし、スクリーンショットをキャプチャし、
   インシデント ドキュメントの CLI 出力 (§2.4 ～ §2.5 を参照)。
4. **上書きを決定します:** 上記の手順を実行した後、インシデント/所有者とともにのみ決定します。
   記録されている場合は、`scripts/android_override_tool.sh` 経由で制限付きオーバーライドを発行します。
   それを `telemetry_override_log.md` に記録します (§3 を参照)。デフォルトの有効期限: <24 時間。
5. **連絡先リストごとにエスカレーションします:** Android のオンコールとオブザーバビリティ TL をページングします。
   (§8 の連絡先)、§4.1 のエスカレーション ツリーに従います。認証または
   StrongBox 信号が関係しているため、最新のバンドルを取得してハーネスを実行します
   エクスポートを再度有効にする前に、§7 からのチェックを行ってください。

## 1. 構成と展開

- **ClientConfig ソーシング:** Android クライアントが Torii エンドポイント、TLS をロードすることを確認します。
  ポリシー、および `iroha_config` 派生マニフェストの再試行ノブ。検証する
  アプリの起動時の値と、アクティブなマニフェストのログ チェックサム。
  実装リファレンス: `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/ClientConfig.java`
  スレッド `TelemetryOptions` から `java/iroha_android/src/main/java/org/hyperledger/iroha/android/telemetry/TelemetryOptions.java`
  (さらに生成された `TelemetryObserver`) ため、ハッシュ化された権限が自動的に発行されます。
- **ホット リロード:** 構成ウォッチャーを使用して `iroha_config` を取得します
  アプリを再起動せずに更新します。リロードに失敗すると、
  `android.telemetry.config.reload` イベントとトリガーの指数関数的な再試行
  バックオフ (最大 5 回の試行)。
- **フォールバック動作:** 構成が欠落しているか無効な場合は、フォールバックします。
  安全なデフォルト (読み取り専用モード、保留中のキュー送信なし) とユーザーの表示
  プロンプト。フォローアップのためにインシデントを記録します。

### 1.1 構成のリロード診断- 構成ウォッチャーは、次のような `android.telemetry.config.reload` 信号を発行します。
  `source`、`result`、`duration_ms`、およびオプションの `digest`/`error` フィールド (「
  `configs/android_telemetry.json` および
  `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/ConfigWatcher.java`)。
  適用されたマニフェストごとに 1 つの `result:"success"` イベントが発生することが予想されます。繰り返された
  `result:"error"` レコードは、ウォッチャーが 5 回のバックオフ試行を使い果たしたことを示しています
  50msから始まります。
- インシデント中に、コレクターから最新のリロード信号をキャプチャします。
  (OTLP/スパン ストアまたはリダクション ステータス エンドポイント) をログに記録し、`digest` +
  インシデント文書の `source`。ダイジェストを比較する
  `configs/android_client_manifest.json` とリリース マニフェストは次の場所に配布されます。
  オペレーター。
- ウォッチャーが引き続きエラーを出力する場合は、対象のハーネスを実行して再現します。
  疑わしいマニフェストによる解析の失敗:
  `ci/run_android_tests.sh org.hyperledger.iroha.android.client.ConfigWatcherTests`。
  テスト出力と失敗したマニフェストをインシデント バンドルに添付して、SRE が実行できるようにします。
  ベイクされた構成スキーマと比較できます。
- リロード テレメトリが見つからない場合は、アクティブな `ClientConfig` が
  テレメトリ シンクと、OTLP コレクターが引き続き
  `android.telemetry.config.reload` ID;それ以外の場合は、Sev2 テレメトリとして処理されます
  回帰 (§2.4 と同じパス) と一時停止は、信号が戻るまで解除されます。

### 1.2 決定論的なキーのエクスポート バンドル
- ソフトウェア エクスポートでは、エクスポートごとのソルト + ノンス、`kdf_kind`、および `kdf_work_factor` を含む v3 バンドルが生成されるようになりました。
  エクスポーターは Argon2id (64 MiB、3 回の反復、並列処理 = 2) を優先し、次にフォールバックします。
  デバイスで Argon2id が使用できない場合、反復フロアが 350 k の PBKDF2-HMAC-SHA256。バンドル
  AAD は引き続きエイリアスにバインドされます。 v3 エクスポートの場合、パスフレーズは少なくとも 12 文字である必要があります。
  インポーターはすべてゼロのソルト/ナンス シードを拒否します。
  `KeyExportBundle.decode(Base64|bytes)`、元のパスフレーズを使用してインポートし、v3 に再エクスポートします。
  メモリハードフォーマットに移行します。インポーターは、オールゼロまたは再利用されたソルト/ノンスペアを拒否します。いつも
  デバイス間で古いエクスポートを再利用するのではなく、バンドルをローテーションします。
- `ci/run_android_tests.sh --tests org.hyperledger.iroha.android.crypto.export.DeterministicKeyExporterTests` のネガティブパステスト
  拒否。使用後にパスフレーズの文字配列をクリアし、バンドルのバージョンと `kdf_kind` の両方をキャプチャします
  回復が失敗した場合のインシデントメモに記載されます。

## 2. テレメトリーと編集

> クイックリファレンス: を参照してください。
> [`telemetry_redaction_quick_reference.md`](sdk/android/telemetry_redaction_quick_reference.md)
> 有効化中に使用される要約されたコマンド/しきい値チェックリストについては
> セッションとインシデントブリッジ。- **信号インベントリ:** `docs/source/sdk/android/telemetry_redaction.md` を参照
  発行されたスパン/メトリクス/イベントの完全なリストについては、
  `docs/source/sdk/android/readiness/signal_inventory_worksheet.md`
  所有者/検証の詳細と未解決のギャップについては。
- **正規スキーマの差分:** 承認された AND7 スナップショットは次のとおりです。
  `docs/source/sdk/android/readiness/schema_diffs/android_vs_rust-20260305.json`。
  レビュー担当者が確認できるように、新しい CLI を実行するたびにこのアーティファクトと比較する必要があります。
  受け入れられた `intentional_differences` および `android_only_signals` がまだ
  に記載されているポリシー テーブルと一致します。
  `docs/source/sdk/android/telemetry_schema_diff.md` §3。 CLI により追加されるようになりました。
  `policy_violations` 意図的な差異が欠落している場合
  `status:"accepted"`/`"policy_allowlisted"` (または Android のみのレコードが失われた場合)
  受け入れられたステータス)、空でない違反を Sev2 として扱い、停止します。
  輸出。以下の `jq` スニペットは、アーカイブされたファイルの手動健全性チェックとして残ります。
  アーティファクト:
  ```bash
  jq '.intentional_differences[] | select(.status != "accepted" and .status != "policy_allowlisted")' "$OUT"
  jq '.android_only_signals[] | select(.status != "accepted")' "$OUT"
  jq '.field_mismatches[] | {signal, field, android, rust}' "$OUT"
  ```
  これらのコマンドからの出力はすべて、スキーマ回帰として処理してください。
  テレメトリのエクスポートを続行する前の AND7 準備バグ。 `field_mismatches`
  `telemetry_schema_diff.md` §5 に従って空のままにする必要があります。ヘルパーは次のように書きます
  `artifacts/android/telemetry/schema_diff.prom` 自動的に;パスする
  `--textfile-dir /var/lib/node_exporter/textfile_collector` (または設定
  `ANDROID_SCHEMA_DIFF_TEXTFILE_DIR`) ステージング/実稼働ホストで実行する場合
  したがって、`telemetry_schema_diff_run_status` ゲージは `policy_violation` に反転します。
  CLI がドリフトを検出した場合は自動的に実行されます。
- **CLI ヘルパー:** `scripts/telemetry/check_redaction_status.py` 検査
  デフォルトでは `artifacts/android/telemetry/status.json`。 `--status-url` をに渡します
  ステージングと `--write-cache` をクエリして、オフライン用にローカル コピーを更新します
  ドリル。 `--min-hashed 214` を使用する (または設定する)
  `ANDROID_TELEMETRY_MIN_HASHED_AUTHORITIES=214`) ガバナンスを強化するため
  すべてのステータスポーリング中にハッシュ化された権限のフロアを確認します。
- **権限ハッシュ:** すべての権限は、Blake2b-256 を使用してハッシュされます。
  四半期ごとのローテーション ソルトは安全な秘密保管庫に保管されます。回転が発生するのは、
  各四半期の最初の月曜日の 00:00 UTC。エクスポーターが受け取りを確認する
  `android.telemetry.redaction.salt_version` メトリクスを確認して、新しいソルトを確認します。
- **デバイス プロファイル バケット:** `emulator`、`consumer`、および `enterprise` のみ
  階層は (SDK メジャー バージョンとともに) エクスポートされます。ダッシュボードでこれらを比較します
  Rust ベースラインに対してカウントされます。差異が 10% を超えるとアラートが発生します。
- **ネットワーク メタデータ:** Android は `network_type` フラグと `roaming` フラグのみをエクスポートします。
  キャリア名は決して発行されません。オペレータは加入者にリクエストをすべきではありません
  インシデントログの情報。サニタイズされたスナップショットは次のように出力されます。
  `android.telemetry.network_context` イベントなので、アプリが必ず
  `NetworkContextProvider` (いずれかの経由)
  `ClientConfig.Builder.setNetworkContextProvider(...)` または利便性
  `enableAndroidNetworkContext(...)` ヘルパー)、Torii 呼び出しが発行される前に。
- **Grafana ポインター:** `Android Telemetry Redaction` ダッシュボードは
  上記の CLI 出力の正規の視覚的チェック - を確認します。
  `android.telemetry.redaction.salt_version` パネルは現在のソルト エポックと一致します
  `android_telemetry_override_tokens_active` ウィジェットはゼロのままです
  訓練やインシデントが実行されていないときはいつでも。どちらかのパネルがずれた場合はエスカレーションする
  CLI スクリプトが回帰を報告する前に。

### 2.1 パイプラインのワークフローのエクスポート1. **構成の配布。** `ClientConfig.telemetry.redaction` は、
   `iroha_config` および `ConfigWatcher` によってホットリロードされます。リロードごとにログが記録されます。
   マニフェスト ダイジェストとソルト エポック - インシデントおよびイベント中にその行をキャプチャします。
   リハーサル。
2. **インストルメンテーション** SDK コンポーネントは、スパン/メトリック/イベントを
   `TelemetryBuffer`。バッファはすべてのペイロードにデバイス プロファイルをタグ付けし、
   現在のソルト エポックを確認できるため、エクスポーターはハッシュ入力を決定的に検証できます。
3. **秘匿化フィルター** `RedactionFilter` ハッシュ `authority`、`alias`、および
   デバイスを離れる前にデバイス識別子を取得します。障害が発生する
   `android.telemetry.redaction.failure` で、エクスポートの試行をブロックします。
4. **エクスポーター + コレクター** サニタイズされたペイロードは Android 経由で出荷されます。
   `android-otel-collector` デプロイメントへの OpenTelemetry エクスポーター。の
   コレクタ ファンのトレース (テンポ)、メトリクス (Prometheus)、および Norito への出力
   ログシンク。
5. **可観測性フック** `scripts/telemetry/check_redaction_status.py` 読み取り
   コレクターカウンター (`android.telemetry.export.status`、
   `android.telemetry.redaction.salt_version`) を生成し、ステータス バンドルを生成します。
   このランブック全体で参照されます。

### 2.2 検証ゲート

- **スキーマの差分:** 実行
  `scripts/telemetry/run_schema_diff.sh --android-config configs/android_telemetry.json --rust-config configs/rust_telemetry.json`
  マニフェストが変化するたびに。実行するたびに、次のことを確認します。
  `intentional_differences[*]` および `android_only_signals[*]` エントリがスタンプされます
  `status:"accepted"` (ハッシュ/バケット化の場合は `status:"policy_allowlisted"`)
  フィールド）を添付する前に、`telemetry_schema_diff.md` §3 で推奨されているように、
  インシデントやカオスラボのレポートに対するアーティファクト。承認されたスナップショットを使用する
  (`android_vs_rust-20260305.json`) ガードレールとして、新たに放出された糸くずとして
  ファイルされる前の JSON:
  ```bash
  LATEST=docs/source/sdk/android/readiness/schema_diffs/$(date -u +%Y%m%d).json
  jq '.intentional_differences[] | select(.status != "accepted" and .status != "policy_allowlisted") | {signal, field, status}' "$LATEST"
  jq '.android_only_signals[] | select(.status != "accepted") | {signal, status}' "$LATEST"
  ```
  `$LATEST` を比較します
  `docs/source/sdk/android/readiness/schema_diffs/android_vs_rust-20260305.json`
  ホワイトリストが変更されていないことを証明するため。欠落または空白 `status`
  エントリ (たとえば、`android.telemetry.redaction.failure` または
  `android.telemetry.redaction.salt_version`) は回帰として扱われるようになり、
  レビューを閉じる前に解決する必要があります。 CLI は受け入れられたものを表示します
  したがって、マニュアル §3.4 の相互参照は次の場合にのみ適用されます。
  `accepted` 以外のステータスが表示される理由を説明します。

  **正規の AND7 シグナル (2026-03-05 スナップショット)**|信号 |チャンネル |ステータス |ガバナンスノート |検証フック |
  |----------|----------|----------|------|------|
  | `android.telemetry.redaction.override` |イベント | `accepted` |ミラーはマニフェストをオーバーライドし、`telemetry_override_log.md` エントリと一致する必要があります。 | `android_telemetry_override_tokens_active` を監視し、§3 に従ってマニフェストをアーカイブします。 |
  | `android.telemetry.network_context` |イベント | `accepted` | Android はキャリア名を意図的に編集します。 `network_type` と `roaming` のみがエクスポートされます。 |アプリが `NetworkContextProvider` を登録していることを確認し、イベント ボリュームが `Android Telemetry Overview` 上の Torii トラフィックに従っていることを確認します。 |
  | `android.telemetry.redaction.failure` |カウンター | `accepted` |ハッシュが失敗するたびに発行されます。ガバナンスでは、スキーマ差分アーティファクトに明示的なステータス メタデータが必要になりました。 | `Redaction Compliance` ダッシュボード パネルと `check_redaction_status.py` からの CLI 出力は、ドリル中を除いてゼロのままでなければなりません。 |
  | `android.telemetry.redaction.salt_version` |ゲージ | `accepted` |エクスポータが現在の四半期ごとのソルト エポックを使用していることを証明します。 | Grafana のソルト ウィジェットを Secrets-Vault エポックと比較し、スキーマ diff の実行で `status:"accepted"` アノテーションが保持されていることを確認します。 |

  上記の表のエントリのいずれかが `status` をドロップする場合、diff アーティファクトは次のとおりである必要があります。
  再生成された ** と ** `telemetry_schema_diff.md` は AND7 の前に更新されました
  ガバナンスパケットが回覧されます。更新された JSON を含めます
  `docs/source/sdk/android/readiness/schema_diffs/` からリンクします。
  再実行のきっかけとなったインシデント、カオス ラボ、またはイネーブルメント レポート。
- **CI/ユニット カバレッジ:** `ci/run_android_tests.sh` は前に合格する必要があります
  ビルドの公開。スイートは、実行することによってハッシュ/オーバーライド動作を強制します。
  サンプル ペイロードを含むテレメトリ エクスポータ。
- **インジェクターの健全性チェック:** を使用します
  リハーサル前 `scripts/telemetry/inject_redaction_failure.sh --dry-run`
  障害挿入が機能し、ガードをハッシュするときに火災を警告することを確認します。
  つまずいている。検証後は常に `--clear` を使用してインジェクターをクリアします
  完了します。

### 2.3 モバイル ↔ Rust テレメトリ パリティ チェックリスト

Android エクスポーターと Rust ノード サービスの連携を維持しながら、
に文書化されたさまざまな編集要件
`docs/source/sdk/android/telemetry_redaction.md`。以下の表は、
AND7 ロードマップ エントリで参照されているデュアル ホワイトリスト。
スキーマ diff はフィールドを導入または削除します。|カテゴリー | Android エクスポーター | Rustサービス |検証フック |
|----------|-------------------|--------------|---------------|
|権限/ルートコンテキスト | Blake2b-256 経由で `authority`/`alias` をハッシュし、エクスポート前に生の Torii ホスト名を削除します。ソルト回転を証明するために `android.telemetry.redaction.salt_version` を発行します。 |相関のために完全な Torii ホスト名とピア ID を出力します。 | `readiness/schema_diffs/` の最新のスキーマ差分で `android.torii.http.request` エントリと `torii.http.request` エントリを比較し、`scripts/telemetry/check_redaction_status.py` を実行して `android.telemetry.redaction.salt_version` がクラスタ ソルトと一致することを確認します。 |
|デバイスと署名者の ID |バケット `hardware_tier`/`device_profile`、ハッシュ コントローラー エイリアス、シリアル番号は決してエクスポートしないでください。 |デバイスのメタデータはありません。ノードはバリデーター `peer_id` とコントローラー `public_key` をそのまま出力します。 | `docs/source/sdk/mobile_device_profile_alignment.md` のマッピングをミラーリングし、ラボ中に `PendingQueueInspector` 出力を監査し、`ci/run_android_tests.sh` 内のエイリアス ハッシュ テストが緑色のままであることを確認します。 |
|ネットワークメタデータ | `network_type` + `roaming` ブール値のみをエクスポートします。 `carrier_name` は削除されます。 | Rust はピアのホスト名と完全な TLS エンドポイント メタデータを保持します。 |最新の差分 JSON を `readiness/schema_diffs/` に格納し、Android 側でまだ `carrier_name` が省略されていることを確認します。 Grafana の「ネットワーク コンテキスト」ウィジェットにキャリア文字列が表示された場合に警告します。 |
|オーバーライド / カオス証拠 |マスクされたアクターの役割を使用して `android.telemetry.redaction.override` および `android.telemetry.chaos.scenario` イベントを発行します。 | Rust サービスは、ロール マスキングやカオス固有のスパンを使用せずにオーバーライド承認を発行します。 |毎回のドリル後に `docs/source/sdk/android/readiness/and7_operator_enablement.md` をクロスチェックして、オーバーライド トークンとカオス アーティファクトがマスクされていない Rust イベントとともにアーカイブされていることを確認します。 |

パリティのワークフロー:

1. マニフェストまたはエクスポーターを変更するたびに、次のコマンドを実行します。
   `scripts/telemetry/run_schema_diff.sh --android-config configs/android_telemetry.json --rust-config configs/rust_telemetry.json --out docs/source/sdk/android/readiness/schema_diffs/$(date -u +%Y%m%d).json --textfile-dir /var/lib/node_exporter/textfile_collector`
   そのため、JSON アーティファクトとミラーリングされたメトリクスの両方が証拠バンドルに含まれます。
   (ヘルパーはデフォルトで `artifacts/android/telemetry/schema_diff.prom` を書き込みます)。
2. 上の表との差分を確認します。 Android が次のフィールドを出力すると、
   Rust でのみ許可されている (またはその逆) 場合は、AND7 対応バグを報告して更新してください
   編集計画。
3. 毎週のチェック中に、次のコマンドを実行します。
   `scripts/telemetry/check_redaction_status.py --status-url https://android-telemetry-stg/api/redaction/status`
   ソルト エポックが Grafana ウィジェットと一致することを確認し、
   オンコール日記。
4. デルタを記録します。
   `docs/source/sdk/android/readiness/signal_inventory_worksheet.md` だから
   ガバナンスは同等性の決定を監査できます。

### 2.4 可観測性ダッシュボードとアラートしきい値

ダッシュボードとアラートを AND7 スキーマの差分承認に合わせて維持する
`scripts/telemetry/check_redaction_status.py` の出力を確認します。

- `Android Telemetry Redaction` — ソルト エポック ウィジェット、トークン ゲージをオーバーライドします。
- `Redaction Compliance` — `android.telemetry.redaction.failure` カウンタおよび
  インジェクタートレンドパネル。
- `Exporter Health` — `android.telemetry.export.status` レートの内訳。
- `Android Telemetry Overview` — デバイス プロファイル バケットとネットワーク コンテキスト ボリューム。

次のしきい値はクイック リファレンス カードを反映しており、強制する必要があります。
インシデント対応中およびリハーサル中:|メトリック / パネル |しきい値 |アクション |
|-----|----------|----------|
| `android.telemetry.redaction.failure` (`Redaction Compliance` ボード) >0 ローリング 15 分ウィンドウ |障害のある信号を調査し、インジェクター クリアを実行し、CLI 出力 + Grafana スクリーンショットをログに記録します。 |
| `android.telemetry.redaction.salt_version` (`Android Telemetry Redaction` ボード) Secrets-Vault Salt epoch とは異なります。リリースを停止し、シークレットのローテーションを調整し、AND7 のメモをファイルします。 |
| `android.telemetry.export.status{status="error"}` (`Exporter Health` ボード)輸出の >1% |コレクターの状態を検査し、CLI 診断を取得し、SRE にエスカレーションします。 |
| `android.telemetry.device_profile{tier="enterprise"}` 対 Rust パリティ (`Android Telemetry Overview`) | Rust ベースラインからの差異 >10% |ファイルガバナンスのフォローアップ、フィクスチャプールの検証、スキーマ差分アーティファクトの注釈付け。 |
| `android.telemetry.network_context` ボリューム (`Android Telemetry Overview`) | Torii トラフィックが存在する間はゼロに低下します。 `NetworkContextProvider` 登録を確認し、スキーマ diff を再実行してフィールドが変更されていないことを確認します。 |
| `android.telemetry.redaction.override` / `android_telemetry_override_tokens_active` (`Android Telemetry Redaction`) |承認されたオーバーライド/ドリル ウィンドウの外側にゼロ以外 |トークンをインシデントに関連付け、ダイジェストを再生成し、§3 のワークフロー経由で取り消します。 |

### 2.5 オペレーターの準備状況とイネーブルメントのトレイル

ロードマップ項目 AND7 では、サポート、SRE、および
リリース関係者は、Runbook が公開される前に上記のパリティ テーブルを理解する必要があります。
GA。でアウトラインを使用します
正規ロジスティックス用の `docs/source/sdk/android/telemetry_readiness_outline.md`
(議題、発表者、タイムライン) および `docs/source/sdk/android/readiness/and7_operator_enablement.md`
詳細なチェックリスト、証拠リンク、アクション ログについては、こちらをご覧ください。以下を守ってください
テレメトリ計画が変更されるたびにフェーズが同期されます。|フェーズ |説明 |証拠の束 |主要所有者 |
|------|-----------|------|---------------|
|先読み配信｜事前に読んだポリシー、`telemetry_redaction.md`、およびクイック リファレンス カードを説明会の少なくとも 5 営業日前までに送信してください。アウトラインの通信ログで確認応答を追跡します。 | `docs/source/sdk/android/telemetry_readiness_outline.md` (セッション ロジスティックス + 通信ログ) および `docs/source/sdk/android/readiness/archive/<YYYY-MM>/` のアーカイブされた電子メール。 |ドキュメント/サポート マネージャー |
|ライブ準備セッション | 60 分間のトレーニング (ポリシーの詳細、ランブックのウォークスルー、ダッシュボード、カオス ラボのデモ) を提供し、非同期視聴者向けに録画を継続します。 |記録とスライドは、概要の §2 でキャプチャされた参照とともに `docs/source/sdk/android/readiness/archive/<YYYY-MM>/` の下に保存されます。 | LLM (AND7 オーナー代理) |
|カオスラボの実行 |ライブ セッションの直後に `docs/source/sdk/android/readiness/labs/telemetry_lab_01.md` から少なくとも C2 (オーバーライド) + C6 (キュー リプレイ) を実行し、ログ/スクリーンショットを有効化キットに添付します。 | `docs/source/sdk/android/readiness/labs/reports/<YYYY-MM>/` および `/screenshots/<YYYY-MM>/` 内のシナリオ レポートとスクリーンショット。 | Android の可観測性 TL + SRE オンコール |
|知識チェック＆出席 |クイズの提出を収集し、90% 未満のスコアを出した人を修正し、出席/クイズの統計を記録します。クイック リファレンスの質問をパリティ チェックリストと一致させておいてください。 | `docs/source/sdk/android/readiness/forms/responses/` でのクイズのエクスポート、`scripts/telemetry/generate_and7_quiz_summary.py` 経由で生成された要約マークダウン/JSON、および `and7_operator_enablement.md` 内の出席表。 |サポートエンジニアリング |
|アーカイブとフォローアップ |イネーブルメント キットのアクション ログを更新し、アーティファクトをアーカイブにアップロードし、完了を `status.md` に記録します。セッション中に発行された修復トークンまたはオーバーライド トークンは、`telemetry_override_log.md` にコピーする必要があります。 | `docs/source/sdk/android/readiness/and7_operator_enablement.md` §6 (アクション ログ)、`.../archive/<YYYY-MM>/checklist.md`、および§3 で参照されるオーバーライド ログ。 | LLM (AND7 オーナー代理) |

カリキュラムが再実行されるとき（四半期ごと、または主要なスキーマ変更前）、更新します。
新しいセッションの日付を含む概要、出席者名簿を最新の状態に保ち、
クイズの概要の JSON/Markdown アーティファクトを再生成して、ガバナンス パケットが
一貫した証拠を参照します。 AND7 の `status.md` エントリは、
各有効化スプリントが終了すると、最新のアーカイブ フォルダーに保存されます。

### 2.6 スキーマ diff ホワイトリストとポリシー チェック

ロードマップでは、デュアル許可リスト ポリシー (モバイルのリダクションとモバイルのリダクション) を明示的に呼び出しています。
Rust 保持）は、以下に格納されている `telemetry-schema-diff` CLI によって強制されます。
`tools/telemetry-schema-diff`。に記録されたすべての差分アーティファクト
`docs/source/sdk/android/readiness/schema_diffs/` は、どのフィールドがどのようなものであるかを文書化する必要があります。
Android ではハッシュ化/バケット化されるか、Rust ではどのフィールドがハッシュ化されずに残るか、
ホワイトリストに登録されていない信号がビルドに紛れ込んでいた。それらの決定を把握する
以下を実行して JSON 内で直接実行します。

```bash
cargo run -p telemetry-schema-diff -- \
  --android-config configs/android_telemetry.json \
  --rust-config configs/rust_telemetry.json \
  --format json \
  > "$LATEST"

if jq -e '.policy_violations | length > 0' "$LATEST" >/dev/null; then
  jq '.policy_violations[]' "$LATEST"
  exit 1
fi
```レポートがクリーンな場合、最後の `jq` は noop と評価されます。あらゆる出力を処理する
Sev2 対応バグとしてそのコマンドから: 入力された `policy_violations`
配列は、Android 専用リストにない信号を CLI が検出したことを意味します
に記載されているRustのみの例外リストにも載っていません。
`docs/source/sdk/android/telemetry_schema_diff.md`。このような場合は停止してください
エクスポートし、AND7 チケットをファイルし、ポリシー モジュールの後でのみ差分を再実行します。
マニフェストのスナップショットが修正されました。結果の JSON を次の場所に保存します
`docs/source/sdk/android/readiness/schema_diffs/` に日付の接尾辞とメモを付けたもの
インシデントまたはラボレポート内のパス。ガバナンスがチェックを再実行できるようにします。

**ハッシュと保持マトリックス**

|シグナルフィールド | Androidの取り扱い |錆びの処理 |ホワイトリストタグ |
|--------------|--------------|--------------|--------------|
| `torii.http.request.authority` | Blake2b-256 ハッシュ化 (`representation: "blake2b_256"`) |トレーサビリティのためにそのまま保存 | `policy_allowlisted` (モバイルハッシュ) |
| `attestation.result.alias` | Blake2b-256 ハッシュ |プレーン テキスト エイリアス (構成証明アーカイブ) | `policy_allowlisted` |
| `attestation.result.device_tier` |バケット化 (`representation: "bucketed"`) |プレーン層文字列 | `policy_allowlisted` |
| `hardware.profile.hardware_tier` |不在 — Android エクスポータはフィールドを完全に削除します。編集せずに提示 | `rust_only` (`telemetry_schema_diff.md` の §3 に記載) |
| `android.telemetry.redaction.override.*` |マスクされたアクターの役割を持つ Android 専用シグナル |同等の信号は発せられない | `android_only` (`status:"accepted"` のままでなければなりません) |

新しいシグナルが表示された場合は、それらをスキーマ diff ポリシー モジュールに追加し、**
上の表にあるように、Runbook は CLI に同梱されている適用ロジックを反映しています。
Android 専用シグナルで明示的な `status` が省略されている場合、または次の場合にスキーマの実行が失敗するようになりました。
`policy_violations` 配列は空ではないため、このチェックリストを常に同期してください。
`telemetry_schema_diff.md` §3 およびで参照されている最新の JSON スナップショット
`telemetry_redaction_minutes_*.md`。

## 3. ワークフローを上書きする

オーバーライドは、回帰やプライバシーをハッシュするときの「ガラス破り」オプションです
アラートは顧客をブロックします。完全な意思決定証跡を記録した後にのみ適用してください
インシデントドキュメントで。1. **ドリフトとスコープを確認します。** PagerDuty アラートまたはスキーマ差分を待ちます。
   ゲートに火をつけてから走ります
   `scripts/telemetry/check_redaction_status.py --status-url <collector>`から
   権限が一致していないことを証明します。 CLI 出力と Grafana のスクリーンショットを添付します。
   事件記録に。
2. **署名されたリクエストを準備します。** データを入力します。
   `docs/examples/android_override_request.json` チケット ID、リクエスタ、
   有効期限と正当化。ファイルをインシデント アーティファクトの隣に保存します。
   コンプライアンスは入力を監査できます。
3. **オーバーライドを発行します。** 呼び出します。
   ```bash
   scripts/android_override_tool.sh apply \
     --request docs/examples/android_override_request.json \
     --log docs/source/sdk/android/telemetry_override_log.md \
     --out artifacts/android/telemetry/override-$(date -u +%Y%m%dT%H%M%SZ).json \
     --event-log docs/source/sdk/android/readiness/override_logs/override_events.ndjson \
     --actor-role <support|sre|docs|compliance|program|other>
   ```
   ヘルパーはオーバーライド トークンを出力し、マニフェストを書き込み、行を追加します。
   Markdown 監査ログに記録されます。チャットにトークンを投稿しないでください。直接届ける
   Torii 演算子にオーバーライドを適用します。
4. **効果を監視します。** 5 分以内に単一の効果を確認します。
   `android.telemetry.redaction.override` イベントが発行されました、コレクター
   ステータス エンドポイントは `override_active=true` を示し、インシデント ドキュメントには
   有効期限。 Android Telemetry の概要ダッシュボードの「トークンのオーバーライド」をご覧ください。
   「アクティブ」パネル (`android_telemetry_override_tokens_active`) も同様です
   トークンのカウントを確認し、次のステータスが表示されるまで 10 分ごとにステータス CLI を実行し続けます。
   ハッシュが安定します。
5. **取り消してアーカイブします。** 緩和策が適用されたらすぐに実行します。
  `scripts/android_override_tool.sh revoke --token <token>` なので監査ログ
  失効時刻を取得して実行します
  `scripts/android_override_tool.sh digest --out docs/source/sdk/android/readiness/override_logs/override_digest_$(date -u +%Y%m%dT%H%M%SZ).json`
  ガバナンスが期待するサニタイズされたスナップショットを更新します。を添付します。
  マニフェスト、ダイジェスト JSON、CLI トランスクリプト、Grafana スナップショット、NDJSON ログ
  `--event-log` 経由で生成され、
  `docs/source/sdk/android/readiness/screenshots/<date>/` とクロスリンクします。
  `docs/source/sdk/android/telemetry_override_log.md` からのエントリ。

24 時間を超えるオーバーライドには、SRE ディレクターとコンプライアンスの承認が必要です。
次回の毎週の AND7 レビューで強調される必要があります。

### 3.1 エスカレーション マトリックスの上書き

|状況 |最大持続時間 |承認者 |必要な通知 |
|----------|--------------|----------|--------------------------|
|シングルテナント調査 (ハッシュ化された権限の不一致、顧客 Sev2) | 4時間 |サポート エンジニア + SRE オンコール |チケット `SUP-OVR-<id>`、`android.telemetry.redaction.override` イベント、インシデント ログ |
|フリート全体のテレメトリの停止または SRE が要求した再現 | 24時間 | SRE オンコール + プログラム リード | PagerDuty のメモ、ログ エントリを上書き、`status.md` で更新 |
|コンプライアンス/フォレンジック要求または 24 時間を超えるケース |明示的に取り消されるまで | SRE ディレクター + コンプライアンス リード |ガバナンス メーリング リスト、オーバーライド ログ、AND7 週次ステータス |

#### 役割の責任|役割 |責任 | SLA / 注記 |
|-----|---------------------|---------------|
| Android テレメトリ オンコール (インシデント コマンダー) |検出を推進し、上書きツールを実行し、承認をインシデント文書に記録し、有効期限が切れる前に確実に取り消しを行います。 | 5 分以内に PagerDuty を確認し、15 分ごとに進行状況を記録します。 |
| Android 可観測性 TL (山本 遥) |ドリフト信号を検証し、エクスポーター/コレクターの状態を確認し、オーバーライド マニフェストをオペレーターに渡す前にサインオフします。 | 10分以内に橋に合流してください。利用できない場合は、ステージング クラスターの所有者に委任します。 |
| SRE リエゾン (リアム・オコナー) |マニフェストをコレクターに適用し、バックログを監視し、Torii 側の緩和策についてリリース エンジニアリングと調整します。 |すべての `kubectl` アクションを変更リクエストに記録し、コマンドのトランスクリプトをインシデント ドキュメントに貼り付けます。 |
|コンプライアンス (ソフィア・マーティンズ / ダニエル・パーク) | 30 分を超えるオーバーライドを承認し、監査ログの行を確認し、規制当局/顧客へのメッセージについてアドバイスします。 | `#compliance-alerts` でのポスト確認応答。運用イベントの場合は、オーバーライドが発行される前にコンプライアンスに関するメモをファイルします。 |
|ドキュメント/サポート マネージャー (Priya Deshpande) |マニフェスト/CLI 出力を `docs/source/sdk/android/readiness/…` でアーカイブし、オーバーライド ログを整理して、ギャップが表面化した場合はフォローアップ ラボをスケジュールします。 |証拠の保持 (13 か月) を確認し、インシデントを解決する前に AND7 のフォローアップをファイルします。 |

オーバーライド トークンの有効期限が近づいた場合、ただちにエスカレーションします。
文書化された失効計画。

## 4. インシデント対応

- **アラート:** PagerDuty サービス `android-telemetry-primary` は編集をカバーします
  障害、エクスポータの停止、バケットのドリフトなどです。 SLAウィンドウ内で承認する
  (サポート プレイブックを参照)。
- **診断:** `scripts/telemetry/check_redaction_status.py` を実行して収集します
  現在のエクスポータの健全性、最近のアラート、およびハッシュ化された権限メトリクス。含める
  インシデント タイムライン (`incident/YYYY-MM-DD-android-telemetry.md`) に出力されます。
- **ダッシュボード:** Android テレメトリの編集、Android テレメトリを監視します
  概要、墨消しコンプライアンス、およびエクスポーターの健全性ダッシュボード。キャプチャ
  インシデント レコードのスクリーンショットと、ソルト バージョンまたはオーバーライドに注釈を付ける
  インシデントをクローズする前のトークンの逸脱。
- **調整:** 輸出者の問題、コンプライアンスに関してリリース エンジニアリングを担当します。
  オーバーライド/PII の質問については、重大度 1 インシデントについてはプログラム リードが担当します。

### 4.1 エスカレーションの流れ

Android インシデントは、Android と同じ重大度レベルを使用して優先順位付けされます。
サポート プレイブック (§2.1)。以下の表は、誰にどのようにページングする必要があるかをまとめたものです
すぐに各応答者がブリッジに参加することが期待されます。|重大度 |影響 |プライマリレスポンダー (≤5 分) |二次エスカレーション (≤10 分) |追加のお知らせ |メモ |
|----------|----------|----------------------------|----------------------------|--------------------------|----------|
|セクション 1 |顧客側の機能停止、プライバシー侵害、データ漏洩 | Android テレメトリ オンコール (`android-telemetry-primary`) | Torii オンコール + プログラム リード |コンプライアンス + SRE ガバナンス (`#sre-governance`)、ステージング クラスター所有者 (`#android-staging`) |すぐにウォー ルームを開始し、コマンド ログ用の共有ドキュメントを開きます。 |
|セクション2 |フリートの劣化、オーバーライドの誤用、または長期にわたるリプレイ バックログ | Android テレメトリ オンコール | Android 基盤 TL + ドキュメント/サポート マネージャー |プログラム リード、リリース エンジニアリング担当 |オーバーライドが 24 時間を超える場合は、コンプライアンスにエスカレーションします。 |
|セクション 3 |シングルテナントの問題、ラボのリハーサル、または勧告アラート |サポートエンジニア | Android オンコール (オプション) |ドキュメント/意識向上のためのサポート |範囲が拡大する場合、または複数のテナントが影響を受ける場合は、Sev2 に変換します。 |

|ウィンドウ |アクション |所有者 |証拠/メモ |
|----------|----------|----------|-----|
| 0 ～ 5 分 | PagerDuty を認識し、インシデント コマンダー (IC) を割り当て、`incident/YYYY-MM-DD-android-telemetry.md` を作成します。 `#android-sdk-support` のリンクと 1 行のステータスをドロップします。 |オンコール SRE / サポート エンジニア | PagerDuty ACK と他のインシデント ログの横にコミットされたインシデント スタブのスクリーンショット。 |
| 5 ～ 15 分 | `scripts/telemetry/check_redaction_status.py --status-url https://android-telemetry-stg/api/redaction/status` を実行し、インシデント ドキュメントに概要を貼り付けます。 Android Observability TL (山本遥) とサポート リード (Priya Deshpande) に ping を送信し、リアルタイムのハンドオフを行います。 | IC + Android 可観測性 TL | CLI 出力 JSON を添付し、開かれたダッシュボード URL をメモし、診断の所有者をマークします。 |
| 15 ～ 25 分 |ステージング クラスターの所有者 (可観測性については山本遥、SRE についてはリアム・オコナー) に協力して、`android-telemetry-stg` で再現してもらいます。 `scripts/telemetry/generate_android_load.sh --cluster android-telemetry-stg` でロードをシードし、Pixel + エミュレータからキュー ダンプをキャプチャして、症状の同等性を確認します。 |ステージング クラスターの所有者 |サニタイズされた `pending.queue` + `PendingQueueInspector` 出力をインシデント フォルダーにアップロードします。 |
| 25～40分 |オーバーライド、Torii スロットリング、または StrongBox フォールバックを決定します。 PII の漏洩または非決定的ハッシュが疑われる場合は、`#compliance-alerts` 経由でコンプライアンス (Sofia Martins、Daniel Park) にページングし、同じインシデント スレッドでプログラム リードに通知してください。 | IC + コンプライアンス + プログラム リード |オーバーライド トークン、Norito マニフェスト、および承認コメントをリンクします。 |
| ≥40分 | 30 分間のステータス更新を提供します (PagerDuty メモ + `#android-sdk-support`)。まだアクティブになっていない場合は作戦室ブリッジをスケジュールし、緩和 ETA を文書化し、リリース エンジニアリング (Alexei Morozov) がコレクター/SDK アーティファクトをロールするためにスタンバイしていることを確認します。 | ＩＣ |タイムスタンプ付きの更新と決定ログはインシデント ファイルに保存され、次の週次更新時に `status.md` に要約されます。 |- すべてのエスカレーションは、Android サポート プレイブックの「所有者 / 次回更新時刻」テーブルを使用して、インシデント ドキュメントに反映されたままにする必要があります。
- 別のインシデントがすでにオープンしている場合は、新しいインシデントを作成するのではなく、既存の作戦室に参加して Android コンテキストを追加します。
- インシデントがランブックのギャップに該当する場合は、AND7 JIRA エピックにフォローアップ タスクを作成し、`telemetry-runbook` タグを付けます。

## 5. カオス&レディネス演習

- で詳しく説明されているシナリオを実行します。
  `docs/source/sdk/android/telemetry_chaos_checklist.md` 四半期およびそれ以前
  メジャーリリース。ラボ レポート テンプレートを使用して結果を記録します。
- 証拠（スクリーンショット、ログ）を以下に保存します。
  `docs/source/sdk/android/readiness/screenshots/`。
- AND7 エピック内のラベル `telemetry-lab` で修復チケットを追跡します。
- シナリオ マップ: C1 (リダクション フォールト)、C2 (オーバーライド)、C3 (エクスポーター ブラウンアウト)、C4
  (ドリフト構成で `run_schema_diff.sh` を使用するスキーマ diff ゲート)、C5
  (`generate_android_load.sh` 経由でシードされたデバイス プロファイル スキュー)、C6 (Torii タイムアウト)
  + キューリプレイ)、C7 (認証拒否)。この番号を次と揃えてください
  `telemetry_lab_01.md` とドリル追加時のカオス チェックリスト。

### 5.1 リダクションドリフトとオーバーライドドリル (C1/C2)

1. ハッシュ失敗を挿入します。
   `scripts/telemetry/inject_redaction_failure.sh` し、PagerDuty を待ちます
   アラート (`android.telemetry.redaction.failure`)。 CLI 出力をキャプチャします。
   `scripts/telemetry/check_redaction_status.py --status-url <collector>` 用
   事件の記録。
2. `--clear` で障害をクリアし、アラートが以内に解決されることを確認します。
   10分。ソルト/権限パネルの Grafana スクリーンショットを添付してください。
3. 次を使用して、署名付きオーバーライド リクエストを作成します。
   `docs/examples/android_override_request.json`、次のように適用します
   `scripts/android_override_tool.sh apply` を実行し、ハッシュ化されていないサンプルを次のように検証します。
   ステージングでエクスポーターのペイロードを検査します (
   `android.telemetry.redaction.override`)。
4. `scripts/android_override_tool.sh revoke --token <token>` を使用してオーバーライドを取り消します。
   オーバーライド トークン ハッシュとチケット参照を追加します
   `docs/source/sdk/android/telemetry_override_log.md`、ダイジェスト JSON を作成する
   `docs/source/sdk/android/readiness/override_logs/`の下。これで閉じます
   C2 シナリオはカオス チェックリストに含まれており、ガバナンスの証拠を最新の状態に保ちます。

### 5.2 エクスポーターのブラウンアウトとキューのリプレイ訓練 (C3/C6)1. ステージング コレクターをスケールダウンします (`kubectlscale)
   deploy/android-otel-collector --replicas=0`) エクスポーターをシミュレートします
   ブラウンアウト。ステータス CLI を介してバッファ メトリクスを追跡し、次の時点でアラートが発生することを確認します。
   15分経過。
2. コレクターを復元し、バックログの排出を確認し、コレクターのログをアーカイブします。
   リプレイの完了を示すスニペット。
3. ステージング Pixel とエミュレーターの両方で、ScenarioC6: インストールに従います。
   `examples/android/operator-console`、機内モードを切り替え、デモを送信します
   転送してから機内モードを無効にし、キューの深さのメトリクスを監視します。
4. 各保留キューをプルします (`adb shell run-as  cat files/pending.queue >
   /tmp/.queue`), compile the inspector (`gradle -p java/iroha_android
   :core:classes >/dev/null`), and run `java -cp build/classes
   org.hyperledger.iroha.android.tools.PendingQueueInspector --file
   /tmp/.queue --json > キュー-リプレイ-.json`。デコードされた添付ファイル
   エンベロープとラボ ログへのリプレイ ハッシュ。
5. エクスポータの停止期間、前後のキューの深さ、カオス レポートを更新します。
   そして`android_sdk_offline_replay_errors`が0のままであることを確認。

### 5.3 ステージングクラスターカオススクリプト (android-telemetry-stg)

ステージング クラスターの所有者 山本遥 (Android Observability TL) と Liam O’Connor
(SRE) リハーサルの実行がスケジュールされている場合は常に、このスクリプトに従います。シーケンスは維持されます
参加者はテレメトリ カオス チェックリストに沿って調整し、次のことを保証します。
アーティファクトはガバナンスのために収集されます。

**参加者**

|役割 |責任 |お問い合わせ |
|------|-------|-----------|
| AndroidオンコールIC |ドリルを推進し、PagerDuty のメモを調整し、コマンド ログを所有します。 PagerDuty `android-telemetry-primary`、`#android-sdk-support` |
|ステージングクラスター所有者 (Haruka、Liam) |ゲート変更ウィンドウ、`kubectl` アクションの実行、クラスター テレメトリのスナップショット | `#android-staging` |
|ドキュメント/サポート マネージャー (Priya) |証拠を記録し、ラボのチェックリストを追跡し、フォローアップ チケットを発行します | `#docs-support` |

**飛行前の調整**

- 訓練の 48 時間前に、計画内容をリストした変更リクエストを提出します。
  シナリオ (C1 ～ C7) を選択し、`#android-staging` にリンクを貼り付けて、クラスター所有者がアクセスできるようにします。
  衝突するデプロイメントをブロックできます。
- 最新の `ClientConfig` ハッシュを収集し、「kubectl --context staging get pods」を実行します。
  -n android-telemetry-stg` 出力を使用してベースライン状態を確立し、保存します
  どちらも `docs/source/sdk/android/readiness/labs/reports/<date>/` の下にあります。
- デバイスのカバレッジ (Pixel + エミュレータ) を確認し、
  `ci/run_android_tests.sh` は、ラボで使用したツールをコンパイルしました
  (`PendingQueueInspector`、テレメトリー インジェクター)。

**実行チェックポイント**

- `#android-sdk-support`で「カオススタート」をアナウンスし、ブリッジレコーディングを開始します。
  `docs/source/sdk/android/telemetry_chaos_checklist.md` を表示したままにしておきます。
  すべての命令は書記のためにナレーションされます。
- ステージング所有者に各インジェクター アクションをミラーリングしてもらいます (`kubectl scale`、エクスポーター)
  再起動、ロード ジェネレーターなど）、オブザーバビリティと SRE の両方がステップを確認します。
- `scripts/telemetry/check_redaction_status.py から出力をキャプチャします。
  それぞれの後に --status-url https://android-telemetry-stg/api/redaction/status`
  シナリオを作成し、インシデントのドキュメントに貼り付けます。

**回復**- すべてのインジェクターがクリアされるまでブリッジを離れないでください (`inject_redaction_failure.sh --clear`、
  `kubectl scale ... --replicas=1`) および Grafana ダッシュボードには緑色のステータスが表示されます。
- ドキュメント/サポートは、キュー ダンプ、CLI ログ、およびスクリーンショットをアーカイブします。
  `docs/source/sdk/android/readiness/screenshots/<date>/` とアーカイブにチェックを入れます
  変更リクエストが終了する前にチェックリストを作成します。
- 次のようなシナリオについて、フォローアップ チケットを `telemetry-chaos` ラベルで記録します。
  失敗したか、予期しないメトリクスが生成され、`status.md` で参照されます。
  次の週次レビュー中に。

|時間 |アクション |所有者 |アーティファクト |
|------|----------|----------|----------|
| T−30分 | `android-telemetry-stg` の健全性: `kubectl --context staging get pods -n android-telemetry-stg` を確認し、保留中のアップグレードがないことを確認し、コレクターのバージョンをメモします。 |はるか | `docs/source/sdk/android/readiness/screenshots/<date>/cluster-health.png` |
| T−20分 |ベースラインロード (`scripts/telemetry/generate_android_load.sh --cluster android-telemetry-stg --duration 20m`) をシードし、stdout をキャプチャします。 |リアム | `readiness/labs/reports/<date>/load-generator.log` |
| T−15分 | `docs/source/sdk/android/readiness/incident/telemetry_chaos_template.md` から `docs/source/sdk/android/readiness/incident/<date>-telemetry-chaos.md` をコピーし、実行するシナリオ (C1 ～ C7) をリストし、スクライブを割り当てます。 |プリヤ・デシュパンデ (サポート) |リハーサル開始前に犯されたインシデントのマークダウン。 |
| T−10分 | Pixel + エミュレータをオンラインで確認し、最新の SDK がインストールされていること、および `ci/run_android_tests.sh` が `PendingQueueInspector` をコンパイルしていることを確認します。 |ハルカ、リアム | `readiness/screenshots/<date>/device-checklist.png` |
| T−5分 | Zoom ブリッジを開始し、画面録画を開始し、`#android-sdk-support` で「カオス スタート」をアナウンスします。 | IC / ドキュメント/サポート |録音は `readiness/archive/<month>/` の下に保存されました。 |
| +0分 | `docs/source/sdk/android/readiness/labs/telemetry_lab_01.md` から選択したシナリオを実行します (通常は C2 + C6)。ラボ ガイドを表示したままにし、コマンドの呼び出しが発生したときに呼び出します。 |ハルカはドライブ、リアムは結果を反映 |インシデント ファイルにリアルタイムで添付されるログ。 |
| +15分 |一時停止してメトリクス (`scripts/telemetry/check_redaction_status.py --status-url https://android-telemetry-stg/api/redaction/status`) を収集し、Grafana のスクリーンショットを取得します。 |はるか | `readiness/screenshots/<date>/status-<scenario>.png` |
| +25分 |挿入された障害 (`inject_redaction_failure.sh --clear`、`kubectl scale ... --replicas=1`) を復元し、キューを再生し、アラートが閉じていることを確認します。 |リアム | `readiness/labs/reports/<date>/recovery.log` |
| +35分 |報告: シナリオごとに合格/不合格を示すインシデント ドキュメントを更新し、フォローアップをリストし、アーティファクトを git にプッシュします。アーカイブ チェックリストを完了できることをドキュメント/サポートに通知してください。 | ＩＣ |インシデントのドキュメントが更新され、`readiness/archive/<month>/checklist.md` にチェックが入りました。 |

- エクスポータが正常になり、すべてのアラートが解除されるまで、ステージング オーナーをブリッジに留めておきます。
- 生のキュー ダンプを `docs/source/sdk/android/readiness/labs/reports/<date>/queues/` に保存し、インシデント ログでそのハッシュを参照します。
- シナリオが失敗した場合は、すぐに `telemetry-chaos` というラベルの JIRA チケットを作成し、`status.md` からクロスリンクします。
- 自動化ヘルパー: `ci/run_android_telemetry_chaos_prep.sh` は、ロード ジェネレーター、ステータス スナップショット、キュー エクスポート プラミングをラップします。ステージング アクセスが利用可能な場合は `ANDROID_TELEMETRY_DRY_RUN=false` を設定し、スクリプトが各キュー ファイルをコピーし、`<label>.sha256` を発行し、`PendingQueueInspector` を実行して `<label>.json` を生成するように `ANDROID_PENDING_QUEUE_EXPORTS=pixel8=/tmp/pixel.queue,emulator=/tmp/emulator.queue` を設定します。 `ANDROID_PENDING_QUEUE_INSPECTOR=false` は、JSON の発行をスキップする必要がある場合 (JDK が利用できない場合など) にのみ使用してください。 **`ANDROID_TELEMETRY_EXPECTED_SALT_EPOCH=<YYYYQ#>` および `ANDROID_TELEMETRY_EXPECTED_SALT_ROTATION=<id>` を設定して、ヘルパーを実行する前に常に予期されるソルト識別子をエクスポートします**。これにより、キャプチャされたテレメトリが Rust ベースラインから逸脱した場合、埋め込み `check_redaction_status.py` 呼び出しがすぐに失敗します。

## 6. ドキュメントと有効化- **オペレーター有効化キット:** `docs/source/sdk/android/readiness/and7_operator_enablement.md`
  ランブック、テレメトリ ポリシー、ラボ ガイド、アーカイブ チェックリスト、およびナレッジをリンクします。
  単一の AND7 対応パッケージにチェックインします。 SREを準備する際の参考にしてください
  ガバナンスの事前読み取りまたは四半期ごとの更新のスケジュール設定。
- **有効化セッション:** 60 分間の有効化録画が 2026 年 2 月 18 日に実行されます。
  四半期ごとの更新あり。マテリアルは以下に存在します
  `docs/source/sdk/android/readiness/`。
- **知識チェック:** スタッフは準備フォームで 90% 以上のスコアを獲得する必要があります。ストア
  結果は `docs/source/sdk/android/readiness/forms/responses/` となります。
- **更新:** テレメトリ スキーマ、ダッシュボード、またはポリシーを上書きするたびに
  この Runbook、サポート Playbook、および `status.md` を同じ内で変更、更新します
  広報。
- **週次レビュー:** Rust リリース候補ごとに (または少なくとも毎週) 検証します。
  `java/iroha_android/README.md` とこの Runbook には現在の自動化が反映されています。
  フィクスチャのローテーション手順、およびガバナンスの期待。レビューをキャプチャします
  `status.md` なので、Foundations マイルストーン監査でドキュメントの鮮度を追跡できます。

## 7. StrongBox 認証ハーネス- **目的:** デバイスを
  StrongBox プール (AND2/AND6)。ハーネスは、取得した証明書チェーンを使用して検証します。
  運用コードが実行するものと同じポリシーを使用して、信頼されたルートに対して。
- **参考:** 詳細については、`docs/source/sdk/android/strongbox_attestation_harness_plan.md` を参照してください。
  API、エイリアスのライフサイクル、CI/Buildkite の接続、所有権マトリックスをキャプチャします。その計画を
  新しい検査技術者を採用したり、財務/コンプライアンスの成果物を更新したりする際に信頼できる情報源となります。
- **ワークフロー:**
  1. デバイス上の証明書バンドル (エイリアス `challenge.hex` および `chain.pem`) を収集します。
     リーフ→ルートの順序で)、それをワークステーションにコピーします。
  2. `scripts/android_keystore_attestation.sh --bundle-dir  --trust-root  を実行します。
     [--trust-root-dir ] --require-strongbox --output ` を適切な形式で使用します
     Google/Samsung ルート (ディレクトリを使用すると、ベンダー バンドル全体をロードできます)。
  3. JSON 概要を生の証明書資料と一緒にアーカイブします。
     `artifacts/android/attestation/<device-tag>/`。
- **バンドル形式:** `docs/source/sdk/android/readiness/android_strongbox_attestation_bundle.md`に従ってください
  必要なファイル レイアウト (`chain.pem`、`challenge.hex`、`alias.txt`、`result.json`)。
- **信頼されたルート:** デバイス ラボ シークレット ストアからベンダー提供の PEM を取得します。複数を渡す
  `--trust-root` 引数、または `--trust-root-dir` にアンカーを保持するディレクトリを指定する
  チェーンは Google 以外のアンカーで終了します。
- **CI ハーネス:** `scripts/android_strongbox_attestation_ci.sh` を使用してアーカイブされたバンドルをバッチ検証する
  ラボマシンまたは CI ランナー上で。スクリプトは `artifacts/android/attestation/**` をスキャンし、
  文書化されたファイルを含むすべてのディレクトリをハーネスし、更新された `result.json` を書き込みます
  概要を記載します。
- **CI レーン:** 新しいバンドルを同期した後、で定義された Buildkite ステップを実行します。
  `.buildkite/android-strongbox-attestation.yml` (`buildkite-agent pipeline upload --pipeline .buildkite/android-strongbox-attestation.yml`)。
  ジョブは `scripts/android_strongbox_attestation_ci.sh` を実行し、次の概要を生成します。
  `scripts/android_strongbox_attestation_report.py`、レポートを `artifacts/android_strongbox_attestation_report.txt` にアップロードします、
  そして、ビルドに `android-strongbox/report` という注釈を付けます。障害があればすぐに調査し、
  デバイス マトリックスからビルド URL をリンクします。
- **レポート:** JSON 出力をガバナンス レビューに添付し、デバイス マトリックスのエントリを更新します。
  `docs/source/sdk/android/readiness/android_strongbox_device_matrix.md` と認証日。
- **模擬リハーサル:** ハードウェアが利用できない場合は、`scripts/android_generate_mock_attestation_bundles.sh` を実行します。
  (`scripts/android_mock_attestation_der.py` を使用します) 決定論的テスト バンドルと共有モック ルートを作成し、CI とドキュメントがエンドツーエンドでハーネスを実行できるようにします。
- **コード内ガードレール:** `ci/run_android_tests.sh --tests
  org.hyperledger.iroha.android.crypto.keystore.KeystoreKeyProviderTests は空とチャレンジをカバーします
  構成証明の再生成 (StrongBox/TEE メタデータ) と `android.keystore.attestation.failure` の発行
  チャレンジの不一致により、新しいバンドルを出荷する前にキャッシュ/テレメトリのリグレッションが検出されます。

## 8. 連絡先

- **サポート エンジニアリング オンコール:** `#android-sdk-support`
- **SRE ガバナンス:** `#sre-governance`
- **ドキュメント/サポート:** `#docs-support`
- **エスカレーション ツリー:** Android サポート ハンドブック §2.1 を参照してください。

## 9. トラブルシューティングのシナリオロードマップ項目 AND7-P2 は、繰り返しページングする 3 つのインシデント クラスを呼び出します。
Android オンコール: Torii/ネットワーク タイムアウト、StrongBox 構成証明の失敗、および
`iroha_config` マニフェスト ドリフト。提出する前に関連するチェックリストに目を通す
Sev1/2 のフォローアップと証拠のアーカイブは `incident/<date>-android-*.md` にあります。

### 9.1 Torii とネットワーク タイムアウト

**信号**

- `android_sdk_submission_latency`、`android_sdk_pending_queue_depth`、に関するアラート
  `android_sdk_offline_replay_errors`、および Torii `/v2/pipeline` エラー率。
- `operator-console` ウィジェット (例/Android) でキュー ドレインの停止または停止が表示される
  再試行が指数関数的バックオフで停止しました。

**即時対応**

1. PagerDuty (`android-networking`) を認識し、インシデント ログを開始します。
2.
   最後の30分。
3. デバイス ログからアクティブな `ClientConfig` ハッシュを記録します (`ConfigWatcher`)
   リロードが成功または失敗するたびにマニフェスト ダイジェストを出力します)。

**診断**

- **キューの健全性:** ステージング デバイスまたは
  エミュレータ (`adb shell run-as  cat files/pending.queue >
  /tmp/pending.queue`)。封筒をデコードします
  `OfflineSigningEnvelopeCodec` で説明されているとおり
  `docs/source/sdk/android/offline_signing.md#4-queueing--replay` を確認します。
  バックログはオペレーターの期待と一致します。デコードされたハッシュを
  事件。
- **インベントリのハッシュ:** キュー ファイルをダウンロードした後、インスペクター ヘルパーを実行します。
  インシデントアーティファクトの正規ハッシュ/エイリアスをキャプチャするには:

  ```bash
  gradle -p java/iroha_android :core:classes >/dev/null  # compiles classes if needed
  java -cp build/classes org.hyperledger.iroha.android.tools.PendingQueueInspector \
    --file /tmp/pending.queue --json > queue-inspector.json
  ```

  `queue-inspector.json` ときれいに出力された標準出力をインシデントに添付します
  シナリオ D の AND7 ラボ レポートからリンクします。
- **Torii 接続:** HTTP トランスポート ハーネスをローカルで実行して SDK を除外します
  回帰: `ci/run_android_tests.sh` 演習
  `HttpClientTransportTests`、`HttpClientTransportHarnessTests`、および
  `ToriiMockServerTests`。ここでの失敗は、問題ではなくクライアントのバグを示しています。
  Torii 障害。
- **フォールト挿入のリハーサル:** ステージング Pixel (StrongBox) と AOSP 上
  エミュレータ、接続を切り替えて保留中のキューの増加を再現します。
  `adb shell cmd connectivity airplane-mode enable` → 2 つのデモを送信
  オペレーターコンソール経由のトランザクション → `adb Shell cmd connectivity 機内モード
  disable` → verify the queue drains and `android_sdk_offline_replay_errors`
  0 のままです。再生されたトランザクションのハッシュを記録します。
- **アラート パリティ:** しきい値を調整するとき、または Torii が変更された後に実行します。
  `scripts/telemetry/test_torii_norito_rpc_alerts.sh` なので Prometheus ルールはそのままです
  ダッシュボードと合わせて。

**回復**

1. Torii のパフォーマンスが低下した場合は、Torii をオンコールにし、再生を続けます。
   `/v2/pipeline` がトラフィックを受け入れるとキューになります。
2. 影響を受けるクライアントは、署名された `iroha_config` マニフェストを介してのみ再構成します。の
   `ClientConfig` ホットリロード ウォッチャーはインシデントの前に成功ログを出力する必要があります
   閉じることができます。
3. リプレイ前後のキュー サイズと次のハッシュを使用してインシデントを更新します。
   ドロップされたトランザクション。

### 9.2 StrongBox と認証の失敗

**信号**- `android_sdk_strongbox_success_rate` または
  `android.keystore.attestation.failure`。
- `android.keystore.keygen` テレメトリは、要求されたデータを記録するようになりました。
  `KeySecurityPreference` と使用されるルート (`strongbox`、`hardware`、
  `software`)、StrongBox 設定が到達したときの `fallback=true` フラグ
  TEE/ソフトウェア。 STRONGBOX_REQUIRED リクエストはサイレントではなく高速に失敗するようになりました
  TEE キーを返します。
- `KeySecurityPreference.STRONGBOX_ONLY` デバイスを参照するサポート チケット
  ソフトウェア キーに戻ります。

**即時対応**

1. PagerDuty (`android-crypto`) を確認し、影響を受けるエイリアス ラベルをキャプチャします
   (ソルテッドハッシュ) とデバイスプロファイルバケット。
2. デバイスの認証マトリックス エントリを確認します。
   `docs/source/sdk/android/readiness/android_strongbox_device_matrix.md` および
   最後に確認された日付を記録します。

**診断**

- **バンドル検証:** 実行
  `scripts/android_keystore_attestation.sh --bundle-dir <bundle> --trust-root <root.pem>`
  アーカイブされた証明書を使用して、失敗の原因がデバイスにあるのかどうかを確認します
  構成ミスまたはポリシーの変更。生成された `result.json` を添付します。
- **チャレンジの再生成:** チャレンジはキャッシュされません。チャレンジリクエストごとに新しいチャレンジリクエストが再生成されます
  `(alias, challenge)` による認証とキャッシュ。チャレンジレス呼び出しではキャッシュが再利用されます。サポートされていません
- **CI スイープ:** `scripts/android_strongbox_attestation_ci.sh` を実行して、
  保存されたバンドルが再検証されます。これにより、システム上の問題が発生するのを防ぐことができます
  新しいトラストアンカーによって。
- **デバイスドリル:** StrongBox のないハードウェア上で (またはエミュレータを強制的に使用して)、
  StrongBox のみを必要とするように SDK を設定し、デモ トランザクションを送信して確認します。
  テレメトリ エクスポータは `android.keystore.attestation.failure` イベントを発行します
  予想通りの理由で。 StrongBox 対応ピクセルでこれを繰り返して、
  幸せな道は緑のままです。
- **SDK 回帰チェック:** `ci/run_android_tests.sh` を実行して支払います
  証明に重点を置いたスイート (`AndroidKeystoreBackendDetectionTests`、
  `AttestationVerifierTests`、`IrohaKeyManagerDeterministicExportTests`、
  キャッシュ/チャレンジ分離の場合は `KeystoreKeyProviderTests`)。ここでの失敗
  クライアント側の回帰を示します。

**回復**

1. ベンダーが証明書をローテーションした場合、または
   デバイスは最近メジャーな OTA を受け取りました。
2. 更新されたバンドルを `artifacts/android/attestation/<device>/` にアップロードし、
   マトリックスのエントリを新しい日付で更新します。
3. StrongBox が運用環境で使用できない場合は、次のオーバーライド ワークフローに従います。
   セクション 3 とフォールバック期間を文書化します。長期的な緩和には必要なもの
   デバイスの交換またはベンダーの修正。

### 9.2a 決定的な輸出回復

- **形式:** 現在のエクスポートは v3 (エクスポートごとのソルト/ノンス + Argon2id、として記録されます)
- **パスフレーズ ポリシー:** v3 では 12 文字以上のパスフレーズが強制されます。ユーザーが短く供給した場合
  パスフレーズについては、準拠したパスフレーズを使用して再エクスポートするように指示します。 v0/v1 インポートは
  免除されますが、インポート直後に v3 として再ラップする必要があります。
- **改ざん/再利用ガード:** デコーダは、ゼロ/短いソルトまたはノンスの長さを拒否し、繰り返されます。
  Salt/nonce ペアは `salt/nonce reuse` エラーとして表面化します。エクスポートを再生成してクリアします
  警備員。強制的に再利用しようとしないでください。
  `SoftwareKeyProvider.importDeterministic(...)` キーをリハイドレートしてから、
  `exportDeterministic(...)` は、デスクトップ ツールが新しい KDF を記録できるように v3 バンドルを発行します
  パラメータ。### 9.3 マニフェストと構成の不一致

**信号**

- `ClientConfig` リロードの失敗、Torii ホスト名の不一致、またはテレメトリ
  AND7 diff ツールによってフラグが付けられたスキーマ diff。
- オペレーターが同じデバイス間で異なる再試行/バックオフ ノブを報告する
  艦隊。

**即時対応**

1. Android ログに出力された `ClientConfig` ダイジェストをキャプチャし、
   リリースマニフェストから予想されるダイジェスト。
2. 比較のために実行中のノード構成をダンプします。
   `iroha_cli config show --actual > /tmp/iroha_config.actual.json`。

**診断**

- **スキーマ diff:** `scripts/telemetry/run_schema_diff.sh --android-config を実行します。
   --rust-config  --textfile-dir /var/lib/node_exporter/textfile_collector`
  Norito diff レポートを生成するには、Prometheus テキストファイルを更新し、
  JSON アーティファクトとインシデントのメトリクス証拠および AND7 テレメトリ準備ログ。
- **マニフェスト検証:** `iroha_cli runtime capabilities` (またはランタイムを使用)
  Audit コマンド) を使用して、ノードのアドバタイズされた暗号化/ABI ハッシュを取得し、
  それらはモバイルマニフェストと一致します。不一致によりノードがロールバックされたことが確認されます
  Android マニフェストを再発行する必要はありません。
- **SDK 回帰チェック:** `ci/run_android_tests.sh` はカバーします
  `ClientConfigNoritoRpcTests`、`ClientConfig.ValidationTests`、および
  `HttpClientTransportStatusTests`。障害は、出荷された SDK が使用できないことを示します。
  現在デプロイされているマニフェスト形式を解析します。

**回復**

1. 承認されたパイプライン経由でマニフェストを再生成します (通常は
   `iroha_cli runtime Capabilities` → 署名済み Norito マニフェスト → 構成バンドル)、および
   オペレータ チャネルを通じて再展開します。 `ClientConfig` は決して編集しないでください
   デバイス上でオーバーライドされます。
2. 修正されたマニフェストが到着したら、`ConfigWatcher`「reload ok」を監視します。
   フリート層ごとにメッセージを送信し、テレメトリ後にのみインシデントをクローズします。
   スキーマ diff はパリティを報告します。
3. マニフェスト ハッシュ、スキーマ差分アーティファクト パス、およびインシデント リンクを次の場所に記録します。
   監査可能性については、Android セクションの下の `status.md`。

## 10. オペレーター対応カリキュラム

ロードマップ項目 **AND7** には反復可能なトレーニング パッケージが必要です。
サポート エンジニアと SRE は、テレメトリ/秘匿化の更新を何もせずに導入できます。
推測。このセクションと組み合わせる
`docs/source/sdk/android/readiness/and7_operator_enablement.md`、これには以下が含まれます
詳細なチェックリストとアーティファクトのリンク。

### 10.1 セッションモジュール (60 分間のブリーフィング)

1. **テレメトリ アーキテクチャ (15 分)** エクスポーター バッファーを確認します。
   リダクションフィルターとスキーマ差分ツール。デモ
   `scripts/telemetry/run_schema_diff.sh --textfile-dir /var/lib/node_exporter/textfile_collector`プラス
   `scripts/telemetry/check_redaction_status.py` 出席者がパリティを確認できるようにする
   施行された。
2. **ランブック + カオス ラボ (20 分)。** このランブックのセクション 2 ～ 9 を強調表示します。
   `readiness/labs/telemetry_lab_01.md` の 1 つのシナリオをリハーサルし、その方法を示します
   `readiness/labs/reports/<stamp>/` の下にアーティファクトをアーカイブします。
3. **オーバーライド + コンプライアンスのワークフロー (10 分)。** セクション 3 のオーバーライドを確認します。
   `scripts/android_override_tool.sh` (適用/取り消し/ダイジェスト) のデモンストレーション、および
   `docs/source/sdk/android/telemetry_override_log.md` と最新のアップデート
   JSONをダイジェストします。
4. **Q&A / 知識チェック (15 分)** クイックリファレンス カードを使用してください。
   `readiness/cards/telemetry_redaction_qrc.md` で質問を固定し、その後
   `readiness/and7_operator_enablement.md` でフォローアップをキャプチャします。### 10.2 資産のケイデンスと所有者

|資産 |ケイデンス |所有者 |アーカイブの場所 |
|----------|-----------|----------|----------|
|録画されたウォークスルー (Zoom/Teams) |四半期ごと、または毎回のソルトローテーションの前 | Android オブザーバビリティ TL + ドキュメント/サポート マネージャー | `docs/source/sdk/android/readiness/archive/<YYYY-MM>/` (記録 + チェックリスト) |
|スライドデッキとクイックリファレンスカード |ポリシー/ランブックが変更されるたびに更新 |ドキュメント/サポート マネージャー | `docs/source/sdk/android/readiness/deck/` および `/cards/` (PDF + マークダウンのエクスポート) |
|知識チェック+出席シート |各ライブセッションの後 |サポートエンジニアリング | `docs/source/sdk/android/readiness/forms/responses/` および `and7_operator_enablement.md` 出席ブロック |
| Q&A バックログ / アクション ログ |ローリング;セッションごとに更新 | LLM (DRI 代理) | `docs/source/sdk/android/readiness/and7_operator_enablement.md` §6 |

### 10.3 証拠とフィードバックのループ

- セッションの成果物 (スクリーンショット、インシデント訓練、クイズのエクスポート) を
  カオスのリハーサルに同じ日付のディレクトリが使用されるため、ガバナンスは両方を監査できます
  準備状況が一緒に追跡されます。
- セッションが完了したら、`status.md` (Android セクション) へのリンクを更新します。
  アーカイブ ディレクトリに移動し、未解決のフォローアップがあるかどうかをメモします。
- ライブ Q&A からの未解決の質問は、問題またはドキュメントに変換する必要があります
  1 週間以内のプル リクエスト。のロードマップ エピック (AND7/AND8) を参照してください。
  チケットの説明を記載することで、所有者との認識を一致させることができます。
- SRE 同期は、アーカイブ チェックリストと、にリストされているスキーマ差分アーティファクトを確認します。
  セクション 2.3 を参照してから、その四半期のカリキュラムの終了を宣言します。