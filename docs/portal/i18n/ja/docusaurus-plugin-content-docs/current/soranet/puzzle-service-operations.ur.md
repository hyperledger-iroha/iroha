---
lang: ja
direction: ltr
source: docs/portal/docs/soranet/puzzle-service-operations.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id:パズルサービス運営
title: パズルサービス運用ガイド
サイドバー_ラベル: パズル サービス運用
説明: Argon2/ML-DSA 入場チケット `soranet-puzzle-service` デーモン آپریشنز۔
---

:::note 正規ソース
`docs/source/soranet/puzzle_service_operations.md` ٩ی عکاسی کرتا ہے۔ドキュメント セットの引退と同期
:::

# パズルサービス運営ガイド

`tools/soranet-puzzle-service/` デーモン `soranet-puzzle-service` デーモン
Argon2 が支援する入場チケット جاری کرتا ہے جو リレー کی `pow.puzzle.*` ポリシー
ミラーの設定 エッジ リレーの設定 ML-DSA
入場トークンブローカーHTTP エンドポイントは次の情報を公開します。

- `GET /healthz` - 活性プローブ。
- `GET /v2/puzzle/config` - リレー JSON (`handshake.descriptor_commit_hex`、`pow.*`)
  PoW/パズルパラメータの説明
- `POST /v2/puzzle/mint` - Argon2 チケット ミントオプションの JSON 本文
  `{ "ttl_secs": <u64>, "transcript_hash_hex": "<32-byte hex>", "signed": true }`
  TTL チェック (ポリシー ウィンドウ クランプ) チケット トランスクリプト ハッシュ
  バインド、署名キーの設定、リレー署名済みチケット +
  署名指紋 واپس کرتا ہے۔
- `GET /v2/token/config` - `pow.token.enabled = true` アクティブなアドミッション トークン
  ポリシー、発行者のフィンガープリント、TTL/クロック スキュー境界、リレー ID、および
  マージされた失効セット)。
- `POST /v2/token/mint` - ML-DSA アドミッション トークン ミント、提供された履歴書ハッシュ
  結合された ہوتا ہے؛リクエストボディ `{ "transcript_hash_hex": "...", "ttl_secs": <u64>, "flags": <u8> }`
  قبول کرتا ہے۔

サービス チケット 統合テスト
`volumetric_dos_soak_preserves_puzzle_and_latency_slo` میں verify کیا جاتا ہے، جو
ボリューム DoS シナリオ リレー スロットルの演習
【tools/soranet-relay/tests/adaptive_and_puzzle.rs:337】

## トークン発行の設定

`pow.token.*` کے تحت リレー JSON フィールド セット کریں (مثال کے لئے
`tools/soranet-relay/deploy/config/relay.entry.json` دیکھیں) ML-DSA トークン
有効にする発行者の公開鍵とオプションの失効リスト:

```json
"pow": {
  "token": {
    "enabled": true,
    "issuer_public_key_hex": "<ML-DSA-44 public key>",
    "revocation_list_hex": [],
    "revocation_list_path": "/etc/soranet/relay/token_revocations.json"
  }
}
```

パズル サービスの値、再利用、ランタイム、Norito JSON の失効
فائل کو خودکار طور پر reload کرتا ہے۔ CLI `soranet-admission-token`
(`cargo run -p soranet-relay --bin soranet_admission_token`) パスワードを忘れてください。
オフラインのトークンのミント/検査 ہوں، 失効 فائل میں `token_id_hex` エントリの追加 ہوں،
生産の更新情報 認証情報の監査 پہل۔

発行者の秘密鍵、CLI フラグ、パズル サービス、およびサービス:

```bash
cargo run -p soranet-puzzle-service -- \
  --relay-config /etc/soranet/relay/relay.entry.json \
  --token-secret-path /etc/soranet/relay/token_issuer_secret.hex \
  --token-revocation-file /etc/soranet/relay/token_revocations.json \
  --token-revocation-refresh-secs 60
```

`--token-secret-hex` 秘密のアウトオブバンド ツール パイプラインの管理
失効ファイル ウォッチャー `/v2/token/config` 現在の値更新情報
`soranet-admission-token revoke` 失効状態の座標
遅れます

リレー JSON میں `pow.signed_ticket_public_key_hex` set کریں تاکہ 署名付き PoW チケット
ML-DSA-44 公開キーを検証する アドバタイズする`/v2/puzzle/config` یہ キー
BLAKE3 フィンガープリント (`signed_ticket_public_key_fingerprint_hex`) エコー
クライアント検証者ピン署名付きチケット リレー ID トランスクリプト バインディング
検証 ہوتے ہیں اور اسی 失効ストア کو 共有 کرتے ہیں؛生の74バイトのPoWチケット
署名済みチケット検証者が構成されました ہونے پر بھی 有効な رہتے ہیں۔署名者の秘密
`--signed-ticket-secret-hex` یا `--signed-ticket-secret-path` ذریعے サービス開始
パス パス起動時に一致しないキーペアがシークレットを拒否します
`pow.signed_ticket_public_key_hex` ٩ے خلاف validate نہ ہو۔ `POST /v2/puzzle/mint`
`"signed": true` (オプションの `"transcript_hash_hex"`) Norito でエンコードされた
署名付きチケット生チケットバイト کے ساتھ واپس ہو؛回答 `signed_ticket_b64`
اور `signed_ticket_fingerprint_hex` شامل ہوتے ہیں تاکہ 指紋トラックを再生する ہوں۔
` signed = true` والی リクエストが拒否されました ہوتی ہیں اگر 署名者秘密が構成されました نہ ہو۔

## キーローテーションプレイブック

1. ** 記述子コミット جمع کریں۔** ガバナンス ディレクトリ バンドル リレー
   記述子コミット公開16 進文字列リレー JSON 構成
   `handshake.descriptor_commit_hex` コピー パズル サービス 共有 ہے۔
2. **パズル ポリシーの境界の確認** 更新されました
   `pow.puzzle.{memory_kib,time_cost,lanes}` 値リリース計画 طابق ہیں۔演算子
   Argon2 構成リレーは、決定的なデータを保持します (メモリ 4 MiB)
   1 <= レーン <= 16)。
3. **再起動ステージ** ガバナンス ローテーションの切り替えを発表、システム ユニットを発表
   コンテナのリロードサービス ホットリロード記述子コミット لینے کے لئے
   再起動 ضروری ہے۔
4. **検証 کریں۔** `POST /v2/puzzle/mint` کے ذریعے チケット発行 کریں اور تصدیق کریں کہ
   `difficulty` اور `expires_at` ポリシーの一致と一致ソークレポート
   (`docs/source/soranet/reports/pow_resilience.md`) 予想されるレイテンシの限界を参照してください
   キャプチャートークンにより、宣伝された発行者の `/v2/token/config` フェッチが可能になります
   指紋認証失効数期待値一致一致## 緊急無効化手順

1. 共有リレー構成 `pow.puzzle.enabled = false` セット
   `pow.required = true` رکھیں اگر hashcash フォールバック チケット لازمی رہنے چاہئیں۔
2. オプションの `pow.emergency` エントリは、Argon2 ゲートをオフラインに強制します。
   古い記述子は ہوں۔ を拒否します
3. リレーパズルサービス再起動 申請 アプリ
4. `soranet_handshake_pow_difficulty` モニター難易度予想ハッシュキャッシュ値
   ドロップ ہو، اور verify کریں کہ `/v2/puzzle/config` `puzzle = null` رپورٹ کرے۔

## 監視およびアラート

- **レイテンシ SLO:** `soranet_handshake_latency_seconds` トラック ٩ریں اور P95 کو 300 ms سے نیچے رکھیں۔
  ソークテストオフセットガードスロットルキャリブレーションデータ
  【docs/source/soranet/reports/pow_resilience.md:1】
- **クォータ プレッシャー:** `soranet_guard_capacity_report.py` リレー メトリクスのエラー
  `pow.quotas` クールダウン (`soranet_abuse_remote_cooldowns`、`soranet_handshake_throttled_remote_quota_total`) 曲 ہوں۔
  【docs/source/soranet/relay_audit_pipeline.md:68】
- **パズルの配置:** `soranet_handshake_pow_difficulty` کو `/v2/puzzle/config` سے واپس ہونی والی
  難易度 高い マッチ 高いリレー構成の相違が失われ、再起動に失敗しました。
- **トークンの準備状況:** اگر `/v2/token/config` غیر متوقع طور پر `enabled = false` ہو جائے یا
  `revocation_source` タイムスタンプが古いです。アラートが発生します。演算子 CLI の操作 Norito
  失効ファイルの回転 ٩رنا چاہئے جب کوئی トークンの引退 ہو تاکہ یہ エンドポイント درست رہے۔
- **サービス正常性:** `/healthz` ライブネス ケイデンス プローブ アラート アラート
  `/v2/puzzle/mint` HTTP 500 応答 (Argon2 パラメーターの不一致、RNG エラー、およびエラー)。
  トークン鋳造エラー `/v2/token/mint` HTTP 4xx/5xx 応答繰り返される失敗
  ページング条件の確認

## コンプライアンス監査ログ

リレー構造化 `handshake` イベントは、スロットルの理由とクールダウン期間を生成します。
コンプライアンス パイプライン ログ 取り込み `docs/source/soranet/relay_audit_pipeline.md` コンプライアンス パイプライン
パズルポリシーの変更が監査可能になりますパズル ゲートにより、チケットのサンプルが作成され、Norito 構成が有効になります。
スナップショット ロールアウト チケット アーカイブ セキュリティ 監査 セキュリティ ہوں۔メンテナンス時間枠
ミント、入場トークン、`token_id_hex` 値、追跡、有効期限、取り消し、
失効ファイル میں insert کیا جانا چاہئے۔