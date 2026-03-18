---
lang: ja
direction: ltr
source: docs/portal/docs/sorafs/orchestrator-config.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: オーケストレーター構成
title: SoraFS آرکسٹریٹر کنفیگریشن
サイドバーラベル: ੭ुुल्लुलुल्
説明: ملٹی سورس 取得 آرکسٹریٹر کو کنفیگر کریں، ناکامیوں کی تشریح کریں، اور ٹیلیمیٹری آؤٹ پٹ ڈی بگ کریں۔
---

:::note メモ
یہ صفحہ `docs/source/sorafs/developer/orchestrator.md` کی عکاسی کرتا ہے۔ پرانی ڈاکیومنٹیشن ریٹائر نہ ہو جائے، دونوں نقول کو ہم آہنگ رکھیں۔
:::

# ملٹی سورس fetch آرکسٹریٹر گائیڈ

SoraFS ةا ملٹی سورس 取得 آرکسٹریٹر گورننس سپورٹڈ 広告 میں شائع شدہ
پرووائیڈرز سیٹ سے ڈٹرمنسٹک اور متوازی ڈاؤن لوڈز چلاتا ہے۔ یہ گائیڈ وضاحت کرتی
ہے کہ آرکسٹریٹر کیسے کنفیگر کیا جائے، رول آؤٹس کے دوران کون سی ناکامی سگنلز
متوقع ہیں، اور کون سے ٹیلیمیٹری اسٹریمز صحت کے اشارے ظاہر کرتے ہیں۔

## 1. ありがとうございます

評価:

| और देखेंすごい |にゅう |
|------|------|------|
| `OrchestratorConfig.scoreboard` |重み付けの重み付けの重み付けJSON スコアボード محفوظ کرتا ہے۔ | `crates/sorafs_car::scoreboard::ScoreboardConfig` پر مبنی۔ |
| `OrchestratorConfig.fetch` | (再試行予算、同時実行制限、検証切り替え) | `crates/sorafs_car::multi_fetch` میں `FetchOptions` سے میپ ہوتا ہے۔ |
| CLI / SDK 日本語版 |ピア ステータス テレメトリ リージョン 拒否/ブースト 拒否/ブースト| `sorafs_cli fetch` یہ フラグ براہ راست دیتا ہے؛ SDK انہیں `OrchestratorConfig` کے ذریعے پاس کرتے ہیں۔ |

`crates/sorafs_orchestrator::bindings` JSON ヘルパー پوری کنفیگریشن کو Norito
JSON シリアル化 ہیں، جس سے اسے SDK バインディング اور آٹومیشن میں پورٹیبل
और देखें

### 1.1 JSON の値

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

`iroha_config` 階層化 (`defaults/`、ユーザー、実際)
展開の制限と制限の制限
SNNet-5a ロールアウト 直接のみのフォールバック پروفائل کے لیے
`docs/examples/sorafs_direct_mode_policy.json`
`docs/source/sorafs/direct_mode_pack.md` دیکھیں۔

### 1.2 コンプライアンスのオーバーライド

SNNet-9 セキュリティ セキュリティ セキュリティ セキュリティ セキュリティ セキュリティNorito JSON
کنفیگریشن میں ایک نیا `compliance` آبجیکٹ カーブアウト محفوظ کرتا ہے جو フェッチ
直接のみの対応:

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

- `operator_jurisdictions` ISO‑3166 alpha‑2 کوڈز ڈکلیئر کرتا ہے جہاں یہ
  آرکسٹریٹر انسٹینس آپریٹ کرتا ہے۔解析 دوران کوڈز 大文字の正規化
  ہوتے ہیں۔
- `jurisdiction_opt_outs` ミラー ہے۔ और देखें
  管轄区域 فہرست میں ہو، آرکسٹریٹر `transport_policy=direct-only` نافذ
  フォールバック理由 `compliance_jurisdiction_opt_out` が発行される
- `blinded_cid_opt_outs` マニフェスト ダイジェスト (ブラインド CID、大文字の 16 進数)
  ❁❁❁❁ペイロードの照合 直接のみのスケジューリング
  `compliance_blinded_cid_opt_out` フォールバック دکھاتی ہیں۔
- `audit_contacts` ان URI کو ریکارڈ کرتا ہے جنہیں گورننس آپریٹرز کے GAR
  プレイブック میں شائع کرنے کی توقع کرتی ہے۔
- `attestations` کمپلائنس کی سائنڈ پیکٹس محفوظ کرتا ہے۔入力項目 オプション
  `jurisdiction` (ISO-3166 alpha-2)、`document_uri`、64 文字 `digest_hex`、
  発行タイムスタンプ `issued_at_ms`、オプション `expires_at_ms`
  アーティファクト 監査チェックリスト ガバナンス ツール
  をオーバーライドします。

重ね着の重ね着 ذریعے دیں تاکہ آپریٹرز کو
ڈٹرمنسٹک は ملیں۔ をオーバーライドします書き込みモードのヒントを確認する _بعد_
バージョン: バージョン SDK `upload-pq-only` バージョン マニフェストの管轄範囲
オプトアウト 直接のみの輸送 準拠
プロバイダー نہ ہو تو فوری فیل کرتے ہیں۔

正規オプトアウト カタログ `governance/compliance/soranet_opt_outs.json`
すごいガバナンス評議会タグ付きリリース کے ذریعے اپڈیٹس شائع کرتی ہے۔
証明書 سمیت مکمل مثال `docs/examples/sorafs_compliance_policy.json` میں
دستیاب ہے، اور آپریشنل پروسیس
[GAR コンプライアンス プレイブック](../../../source/soranet/gar_compliance_playbook.md)
और देखें

### 1.3 CLI および SDK ノブ|フラグ / フィールド |ああ |
|--------------|-----|
| `--max-peers` / `OrchestratorConfig::with_max_providers` |スコアボード فلٹر سے گزر کرتا ہے کہ کتنے پرووائیڈر スコアボード فلٹر سے گزر سکیں۔ `None` پر سیٹ کریں تاکہ تمام 資格あり پرووائیڈرز استعمال ہوں۔ |
| `--retry-budget` / `FetchOptions::per_chunk_retry_limit` | فی-chunk retry حد مقرر کرتا ہے۔ حد سے تجاوز `MultiSourceError::ExhaustedRetries` اٹھاتا ہے۔ |
| `--telemetry-json` |レイテンシ/障害のスナップショット、スコアボード ビルダー、注入機能、 `telemetry_grace_secs` سے پرانی ٹیلیمیٹری پرووائیڈرز کو ineligible کر دیتی ہے۔ |
| `--scoreboard-out` |スコアボード (資格あり + 資格なし) スコアボード (資格あり + 資格なし) スコアボード (資格あり + 資格なし) |
| `--scoreboard-now` |スコアボードのタイムスタンプ (Unix 秒) を上書きする フィクスチャが決定的なものをキャプチャする|
| `--deny-provider` / スコア ポリシー フック |広告を表示する 広告を表示する 決定的な広告を除外する ہے۔ فوری بلیک لسٹنگ کے لیے مفید۔ |
| `--boost-provider=name:delta` |重み付け 重み付け ラウンドロビン クレジット 重み付きラウンドロビン クレジット|
| `--telemetry-region` / `OrchestratorConfig::with_telemetry_region` |構造化ログの記録 ロールアウト ウェーブの展開 ピボットの記録ああ|
| `--transport-policy` / `OrchestratorConfig::with_transport_policy` | ب ڈیفالٹ `soranet-first` ہے کیونکہ ملٹی سورس آرکسٹریٹر ベースライン ہے۔ダウングレード コンプライアンス指令 لیے `direct-only` استعمال کریں، اور PQ 専用パイロット کے لیے `soranet-strict` محفوظ رکھیں؛コンプライアンスは優先されます。 |

SoraNet ファースト出荷デフォルト ロールバック SNNet ブロッカー SNNet ブロッカー
حوالہ دینا چاہیے۔ SNNet-4/5/5a/5b/6a/7/8/12/13 ٩ے گریجویٹ ہونے کے بعد گورننس
必要な姿勢 کو مزید سخت کرے گی (`soranet-strict` کی طرف)؛いいえ、いいえ。
インシデント主導型オーバーライド `direct-only` ロールアウト
ログ میں ریکارڈ کرنا لازمی ہے۔

フラグ `sorafs_cli fetch` 開発者向け `sorafs_fetch` フラグ
میں `--` اسٹائل 構文 قبول کرتے ہیں۔ SDK の型付きビルダー
और देखें

### 1.4 ガード キャッシュ

CLI による SoraNet ガード セレクター 有線接続 SNNet-5 接続
トランスポートロールアウト、エントリーリレー、決定論的アクセス、ピンチェック。
フラグのフラグ:

|旗 |すごい |
|------|------|
| `--guard-directory <PATH>` | ایک JSON فائل کی طرف اشارہ کرتا ہے جو تازہ ترینリレーコンセンサス بیان کرتی ہے (نیچے サブセット دکھایا گیا ہے)۔ディレクトリ پاس کرنے سے フェッチ سے پہلے ガード キャッシュ リフレッシュ ہوتی ہے۔ |
| `--guard-cache <PATH>` | Norito エンコードされた `GuardSet` محفوظ کرتا ہے۔ディレクトリ ディレクトリ キャッシュの再利用 キャッシュの再利用|
| `--guard-target <COUNT>` / `--guard-retention-days <DAYS>` |エントリガード (ڈیفالٹ 3) 保持ウィンドウ (ڈیفالٹ 30 دن) ٩ی لیے オプションのオーバーライド|
| `--guard-cache-key <HEX>` |オプションの 32 バイト キー、Blake3 MAC、ガード キャッシュ、タグ、再利用、検証、検証|

ディレクトリ ペイロードのコンパクト スキーマの保護:

`--guard-directory` フラグ Norito でエンコードされた `GuardDirectorySnapshotV2` ペイロード
期待してくださいバイナリ スナップショット:

- `version` — スキーマ (فی الحال `2`)。
- `directory_hash`、`published_at_unix`、`valid_after_unix`、`valid_until_unix` —
  コンセンサスメタデータ 埋め込まれた証明書 一致する証明書
- `validation_phase` — 証明書ポリシー ゲート (`1` = 単一の Ed25519 署名)
  許可（`2` = 二重署名を優先します）、`3` = 二重署名が必要です）。
- `issuers` — ガバナンス発行者 `fingerprint`、`ed25519_public`、`mldsa65_public`。
  指紋認証:
  `BLAKE3("soranet.src.v2.issuer" || ed25519 || u32(len(ml-dsa)) || ml-dsa)`。
- `relays` — SRCv2 バンドル (`RelayCertificateBundleV2::to_cbor()` 出力)。
  バンドル リレー記述子、機能フラグ、ML-KEM ポリシー、Ed25519/ML-DSA-65
  二重署名 رکھتا ہے۔

CLI バンドル 宣言された発行者キー 確認する ディレクトリ ディレクトリ
スナップショット

`--guard-directory` CLI のコンセンサス キャッシュ
マージ ٩یا سکے۔セレクター固定ガード
保存期間 ディレクトリ 対象となる期間リレーの有効期限が切れました
エントリを置き換える ہیں۔フェッチ キャッシュを更新しました `--guard-cache`
パス پر لکھ دی جاتی ہے، جس سے اگلی سیشنز 決定的 رہتی ہیں۔ SDK ⌌ہی
動作 `GuardSelector::select(&RelayDirectory, existing_guard_set, now_unix_secs)`
پال کر کے اور نتیجہ `GuardSet` کو `SorafsGatewayFetchOptions` میں پاس کر کے
再現する`ml_kem_public_hex` セレクター PQ 対応ガード
SNNet-5 のロールアウトが完了しましたステージ切り替え (`anon-guard-pq`、`anon-majority-pq`、
`anon-strict-pq`) 古典的なリレーの降格: جب PQ
ガード دستیاب ہو تو セレクター اضافی クラシックピン ہٹا دیتا ہے تاکہ اگلی سیشنز
ハイブリッド ハンドシェイクCLI/SDK の概要と概要
`anonymity_status`/`anonymity_reason`、`anonymity_effective_policy`、
`anonymity_pq_selected`、`anonymity_classical_selected`、`anonymity_pq_ratio`、
`anonymity_classical_ratio` 候補/不足/供給デルタ فیلڈز کے
電圧低下 古典的なフォールバック واضح ہو جاتے ہیں۔

ガード ディレクトリ `certificate_base64` セキュリティ SRCv2 バンドルの埋め込み
और देखेंバンドルのデコード、Ed25519/ML-DSA 署名の確認
解析された証明書を再検証し、ガード キャッシュを再検証します。
❁❁❁❁証明書、PQ キー、ハンドシェイク設定、重み付け
正規の情報源期限切れの証明書は破棄します。
管理 ذریعے 伝播 ہوتے ہیں اور `telemetry::sorafs.guard` اور
`telemetry::sorafs.circuit` 表面の有効期間
握手スイート 二重署名 مشاہدے کو ریکارڈ کرتے ہیں۔

CLI ヘルパー、スナップショット、パブリッシャー、同期機能:

```bash
sorafs_cli guard-directory fetch \
  --url https://directory.soranet.dev/mainnet_snapshot.norito \
  --output ./state/guard_directory.norito \
  --expected-directory-hash <directory-hash-hex>

sorafs_cli guard-directory verify \
  --path ./state/guard_directory.norito \
  --expected-directory-hash <directory-hash-hex>
```

`fetch` SRCv2 スナップショットを確認します。 `verify`
検証パイプライン 検証パイプライン 検証パイプライン 検証パイプライン
CLI/SDK ガード セレクターの出力、一致、JSON 概要の出力、

### 1.5 回路ライフサイクルマネージャー

リレー ディレクトリ ガード キャッシュ サーキット ライフサイクル
マネージャー フェッチ フェッチ SoraNet 回路 プリビルド
更新します`OrchestratorConfig`
(`crates/sorafs_orchestrator/src/lib.rs:305`) میں دو نئے فیلڈز کے ذریعے ہوتی ہے:

- `relay_directory`: SNNet-3 ディレクトリのスナップショット 中間/出口ホップ
  決定的 طریقے سے منتخب ہوں۔
- `circuit_manager`: オプションの回線 TTL (有効)
  ٩و کنٹرول کرتی ہے۔

Norito JSON 番号 `circuit_manager` 番号:

```json
"circuit_manager": {
  "enabled": true,
  "circuit_ttl_secs": 900
}
```

SDK ディレクトリ データ
`SorafsGatewayFetchOptions::relay_directory`
(`crates/iroha/src/client.rs:320`) フォワード کرتی ہیں، اور CLI اسے اس وقت
有線接続 `--guard-directory` 接続
(`crates/iroha_cli/src/commands/sorafs.rs:365`)。

マネージャー回路の更新、ガード メタデータ (エンドポイント、PQ キー)
ピン留めされたタイムスタンプ) بدل جائے یا TTL ختم ہو۔ ہر フェッチ سے پہلے 呼び出し ہونے والا
ヘルパー `refresh_circuits` (`crates/sorafs_orchestrator/src/lib.rs:1346`) `CircuitEvent`
ライフサイクル فیصلوں کو ٹریس کر سکیں۔ を放出します。浸漬試験
`circuit_manager_latency_soak_remains_stable_across_rotations`
(`crates/sorafs_orchestrator/src/soranet.rs:1479`) ガードローテーションを強化します
レイテンシー دکھاتا ہے؛ `docs/source/soranet/reports/circuit_stability.md:1` すごい
और देखें

### 1.6 QUIC プロキシ

QUIC プロキシ アプリケーション ブラウザ拡張機能 SDK
アダプタ 証明書 ガード キャッシュ キーの管理プロキシ ループバック
バインド QUIC 接続 Norito
マニフェスト 証明書 オプションのガード キャッシュ キー オプションのガード キャッシュ キー
プロキシはトランスポート イベントを発行します `sorafs_orchestrator_transport_events_total`
میں شمار کیا جاتا ہے۔

JSON シンボル `local_proxy` プロキシ有効化:

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
```- `bind_addr` ٩نٹرول کرتا ہے کہ proxy کہاں listen کرے (پورٹ `0` سے ephemeral)
  پورٹ مانگیں)۔
- `telemetry_label` میٹرکس میں 伝播 ہوتا ہے تاکہ ダッシュボード プロキシ اور
  セッションを取得する میں فرق کر سکیں۔
- `guard_cache_key_hex` (اختیاری) プロキシ کو وہی キー付きガード キャッシュ دکھانے دیتا ہے
  CLI/SDK とブラウザ拡張機能の同期
- `emit_browser_manifest` ハンドシェイク マニフェストの切り替え
  واپس کرے جسے 拡張機能 ذخیرہ/verify کر سکیں۔
- `proxy_mode` طے کرتا ہے کہ プロキシ مقامی طور پر ブリッジ کرے (`bridge`) یا صرف
  メタデータは SDK と SoraNet 回路 (`metadata-only`) を生成します。
  `bridge` ہے؛マニフェスト دینا ہو تو `metadata-only` سیٹ کریں۔
- `prewarm_circuits`、`max_streams_per_circuit`、`circuit_ttl_hint_secs`
  ヒント セキュリティ ゲートウェイ パラレル ストリーム セキュリティ プロキシ 回路
  再利用する
- `car_bridge` (اختیاری) مقامی CAR アーカイブ キャッシュ کی طرف اشارہ کرتا ہے۔ `extension`
  اس suffix کو کنٹرول کرتا ہے جو `*.car` غائب ہونے پر لگایا جاتا ہے؛ `allow_zst = true`
  سے `*.car.zst` براہ راست サーブ ہو سکتا ہے۔
- `kaigi_bridge` (اختیاری) Kaigi ルートがプロキシを公開する`room_policy`
  ブリッジ `public` ہے یا `authenticated` ブラウザ クライアント
  GAR ラベル ラベル
- `sorafs_cli fetch` `--local-proxy-mode=bridge|metadata-only`
  `--local-proxy-norito-spool=PATH` は、JSON をオーバーライドします。
  ランタイム モード スプール ランタイム モード スプール ランタイム モード スプール ランタイム モード スプール ランタイム モード スプール ランタイム モード
- `downgrade_remediation` (اختیاری) ダウングレード フック کو کنفیگر کرتا ہے۔
  有効化 リレー テレメトリ ダウングレード バースト サポート
  `window_secs` `threshold` ローカル プロキシ `target_mode`
  (ڈیفالٹ `metadata-only`) پر مجبور کرتا ہے۔プロキシをダウングレードします
  `cooldown_secs` ٩ے بعد `resume_mode` پر واپس آتا ہے۔ `modes` 配列固有
  リレーの役割 تک محدود کرنے کے لیے استعمال کریں (ڈیفالٹ エントリーリレー)۔

プロキシ ブリッジ モードの説明: アプリケーション サービスの説明:

- **`norito`** — ストリーム ターゲット `norito_bridge.spool_dir` 解決
  ہوتا ہے۔ターゲットのサニタイズ جاتا ہے (絶対パスのトラバース)
  اور اگر فائل میں 拡張子 نہیں تو 設定されたサフィックス لگایا جاتا ہے، پھر ペイロード
  ストリームストリーム جاتا ہے۔
- **`car`** — ストリーム ターゲット `car_bridge.cache_dir` を解決します。
  デフォルトの拡張子は、圧縮ペイロードを継承します。
  `allow_zst` فعال نہ ہو۔ کامیاب ブリッジ `STREAM_ACK_OK` کے ساتھ جواب دیتے ہیں
  アーカイブ バイトの確認 ہیں の確認 パイプラインの確認

プロキシ キャッシュ タグ HMAC (ガード キャッシュ キー ハンドシェイク)
دوران موجود ہو) اور `norito_*` / `car_*` テレメトリ理由コード ریکارڈ کرتا ہے
ダッシュボードの欠落ファイル、サニタイゼーションの失敗、ダッシュボードの欠落など
और देखें

`Orchestrator::local_proxy().await` ハンドルを操作し、公開する
証明書 PEM پڑھ سکیں، ブラウザマニフェスト حاصل کر سکیں، یا ایپلیکیشن
正常なシャットダウンを完了します。

プロキシが有効になっています **マニフェスト v2** が有効です سرور کرتا ہے۔証明書
ガード キャッシュ キー v2 のバージョン:

- `alpn` (`"sorafs-proxy/1"`) `capabilities` 配列ストリーム
  پروٹوکول کی تصدیق کر سکیں۔
- セッションごとのハンドシェイク `session_id` `cache_tagging` ブロック
  ガード アフィニティ HMAC タグ مشتق کیے جا سکیں۔
- 回路ガード選択ヒント (`circuit`、`guard_selection`、`route_hints`)
  ブラウザ統合ストリームの強化 UI の強化
- `telemetry_v2` 計測器のサンプリング/プライバシー ノブ
- ہر `STREAM_ACK_OK` میں `cache_tag_hex` شامل ہوتا ہے۔ありがとうございます
  `x-sorafs-cache-tag` ヘッダー ミラーリング キャッシュされたガード選択
  残りは暗号化されています

v1 サブセット پر انحصار جاری رکھ سکتے ہیں۔

## 2. 失敗のセマンティクス

予算チェックの能力
❁❁❁❁ और देखें1. **適格性の失敗 (飛行前)。** 航続可能距離が期限切れになりました
   広告、古いテレメトリ、プロバイダー、スコアボード アーティファクト、ログ、
   スケジュール管理 スケジュール管理 スケジュール管理CLI の概要 `ineligible_providers`
   配列 理由 理由 オペレータ ログ スクレイピング ガバナンス
   ドリフト دیکھ سکیں۔
2. **実行時の枯渇。** プロバイダーの障害、トラックの障害。ああ
   `provider_failure_threshold` پہنچ جائے تو プロバイダー کو سیشن کے باقی حصے کے لیے
   `disabled` دیا جاتا ہے۔プロバイダー `disabled` ہو جائیں تو آرکسٹریٹر
   `MultiSourceError::NoHealthyProviders { last_error, chunk_index }` واپس کرتا ہے۔
3. **決定的な中止** 構造化エラー、エラー、エラー:
   - `MultiSourceError::NoCompatibleProviders` — マニフェスト チャンク スパン
     アラインメント مانگتا ہے جسے باقی プロバイダーの名誉 نہیں کر سکتے۔
   - `MultiSourceError::ExhaustedRetries` — チャンクごとの再試行予算
   - `MultiSourceError::ObserverFailed` — ダウンストリーム オブザーバー (ストリーミング フック)
     検証済みチャンク دیا۔

問題のあるチャンク インデックスでエラーが発生しました。 最終的なプロバイダー障害の理由が発生しました。
ساتھ آتا ہے۔ブロッカーを解放する - 入力を再試行する
障害の発生、広告の発生、テレメトリの発生、プロバイダーの健康状態の確認、および障害の発生

### 2.1 スコアボードの永続性

جب `persist_path` کنفیگر ہو تو آرکسٹریٹر ہر رن کے بعد 最終スコアボード لکھتا ہے۔
JSON メッセージ:

- `eligibility` (`eligible` یا `ineligible::<reason>`)。
- `weight` (正規化された重みが割り当てられています)。
- `provider` メタデータ (識別子、エンドポイント、同時実行バジェット)。

スコアボードのスナップショット、アーティファクトのリリース、アーカイブ、ブラックリスト
ロールアウトの監査可能性

## 3. テレメトリとデバッグ

### 3.1 Prometheus メトリクス

آرکسٹریٹر `iroha_telemetry` کے ذریعے درج ذیل میٹرکس が放射する کرتا ہے:

|メトリック |ラベル |説明 |
|----------|----------|---------------|
| `sorafs_orchestrator_active_fetches` | `manifest_id`、`region` |機内で調整されたフェッチのゲージ|
| `sorafs_orchestrator_fetch_duration_ms` | `manifest_id`、`region` |エンドツーエンドのフェッチ レイテンシのヒストグラム|
| `sorafs_orchestrator_fetch_failures_total` | `manifest_id`、`region`、`reason` |端末障害カウンタ (再試行が尽きた、プロバイダーがない、オブザーバー障害) |
| `sorafs_orchestrator_retries_total` | `manifest_id`、`provider`、`reason` |プロバイダーの再試行回数 カウンタ|
| `sorafs_orchestrator_provider_failures_total` | `manifest_id`、`provider`、`reason` |セッション レベルのプロバイダーの障害、カウンタの無効化、およびエラーの発生|
| `sorafs_orchestrator_policy_events_total` | `region`、`stage`、`outcome`、`reason` |匿名性ポリシー カウント (ミート vs ブラウンアウト) 段階 フォールバック理由 مطابق۔ |
| `sorafs_orchestrator_pq_ratio` | `region`、`stage` | SoraNet セット PQ リレー シェア ヒストグラム|
| `sorafs_orchestrator_pq_candidate_ratio` | `region`、`stage` |スコアボードのスナップショット PQ リレー供給率のヒストグラム|
| `sorafs_orchestrator_pq_deficit_ratio` | `region`、`stage` |保険契約不足額 (目標と実際の PQ シェア)、ヒストグラム|
| `sorafs_orchestrator_classical_ratio` | `region`、`stage` |クラシック リレー シェアのヒストグラム|
| `sorafs_orchestrator_classical_selected` | `region`、`stage` |古典的なリレーのヒストグラム|

プロダクション ノブの管理 メトリクス ステージング ダッシュボードの管理
レイアウト SF-6 可観測性計画をフォローしてください:

1. **アクティブなフェッチ** — ゲージの完了数
2. **再試行率** — `retry` カウンター ベースライン 警告 警告
3. **プロバイダーの障害** — 15 منٹ میں کسی Provider کے `session_failure > 0` پر
   ポケベルアラート

### 3.2 構造化ログのターゲット

決定的目標と構造化されたイベント:

- `telemetry::sorafs.fetch.lifecycle` — `start` および `complete` ライフサイクル マーカー
  チャンク数、再試行数、合計継続時間
- `telemetry::sorafs.fetch.retry` — イベントを再試行します (`provider`、`reason`、`attempts`)。
- `telemetry::sorafs.fetch.provider_failure` — 繰り返されるエラーが無効になりました
  プロバイダー
- `telemetry::sorafs.fetch.error` — `reason` オプションのプロバイダー メタデータ
  端末の故障

ストリーム ストリーム Norito ログ パイプライン フォワード トラフィック インシデント対応
真実の情報源 ملے۔ライフサイクル イベント PQ/クラシック ミックス
`anonymity_effective_policy`、`anonymity_pq_ratio`、`anonymity_classical_ratio`
カウンターの管理 ダッシュボードのメトリクス収集
بغیر ワイヤー کرنا آسان ہوتا ہے۔ GA ロールアウトのライフサイクル/再試行イベント
ログ レベル `info` ターミナル エラー `warn` استعمال کریں۔

### 3.3 JSON の概要`sorafs_cli fetch` Rust SDK の構造化概要:

- `provider_reports` 成功/失敗のカウント、プロバイダーの無効化。
- `chunk_receipts` جو دکھاتے ہیں کہ کس プロバイダー نے کون سا chunk پورا کیا۔
- `retry_stats` 配列 `ineligible_providers` 配列

プロバイダー デバッグ 概要ファイル アーカイブ — 領収書
ログ メタデータ ログ メタデータ ログ メタデータ ログ メタデータ ログ メタデータ ログ メタデータ

## 4. 動作チェックリスト

1. **CI ステータス ステージ ステータス** `sorafs_fetch` ターゲット設定 ステータス ステータス
   資格ビュー `--scoreboard-out` دیں، اور پچھلے release سے diff کریں۔
   不適格なプロバイダーのプロモーション دیتا ہے۔
2. **テレメトリ** マルチソースフェッチにより、複数のソースの取得が可能になります。
   導入 `sorafs.fetch.*` メトリクス 構造化ログのエクスポート
   ❁❁❁❁メトリクス عدم موجودگی عموماً اس بات کا اشارہ ہے کہ オーケストレーター ファサード
   نہیں ہوئی۔ を呼び出します
3. **オーバーライド دستاویزی بنائیں۔** ہنگامی `--deny-provider` یا `--boost-provider`
   JSON (CLI 呼び出し) 変更ログのコミットロールバック
   オーバーライド 元に戻す スコアボード スナップショット キャプチャ スコアボード スナップショット キャプチャ
4. **スモークテスト、リトライバジェット、プロバイダーキャップ、テスト。
   正規フィクスチャ (`fixtures/sorafs_manifest/ci_sample/`) フェッチ
   チャンクレシートを確定的に検証する

段階的ロールアウトの再現性の向上
インシデント対応 فیے ضروری テレメトリ ہیں۔

### 4.1 ポリシーの上書き

オペレーターのトランスポート/匿名性ステージと基本構成のピン
سکتے ہیں، بس `policy_override.transport_policy` اور
`policy_override.anonymity_policy` کو اپنے `orchestrator` JSON میں سیٹ کریں (یا
`sorafs_cli fetch` `--transport-policy-override=` / `--anonymity-policy-override=`
پاس کریں)۔オーバーライド موجود ہو تو آرکسٹریٹر 通常の電圧低下フォールバック کو スキップ
ステータス: PQ 層のステータス `no providers` ステータス フェッチ `no providers`
ہوتا ہے بجائے خاموشی سے ダウングレード ہونے کے۔デフォルトの動作
フィールドを上書きする