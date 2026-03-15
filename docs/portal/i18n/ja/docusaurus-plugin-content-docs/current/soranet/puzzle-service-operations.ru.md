---
lang: ja
direction: ltr
source: docs/portal/docs/soranet/puzzle-service-operations.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id:パズルサービス運営
タイトル: Руководство по эксплуатации パズル サービス
サイドバー_ラベル: パズル サービス運用
説明: Эксплуатация デーモン `soranet-puzzle-service` для Argon2/ML-DSA 入場チケット。
---

:::note Канонический источник
:::

# Руководство по эксплуатации パズルサービス

デーモン `soranet-puzzle-service` (`tools/soranet-puzzle-service/`) の詳細
Argon2 がサポートする入場チケット、которые отражают ポリシー `pow.puzzle.*` уリレー
つまり、ML-DSA アドミッション トークンはエッジ リレーに相当します。
HTTP エンドポイントの詳細:

- `GET /healthz` - 活性プローブ。
- `GET /v2/puzzle/config` - PoW/パズル、
  リレー JSON (`handshake.descriptor_commit_hex`、`pow.*`) を参照してください。
- `POST /v2/puzzle/mint` - Argon2 チケット。オプションの JSON 本文
  `{ "ttl_secs": <u64>, "transcript_hash_hex": "<32-byte hex>", "signed": true }`
  запразывает более короткий TTL (クランプされた к ポリシー ウィンドウ)、チケット
  記録ハッシュとリレー署名チケット + 署名指紋
  署名キー。
- `GET /v2/token/config` - когда `pow.token.enabled = true`, возвращает активную
  アドミッション トークン ポリシー (発行者のフィンガープリント、TTL/クロック スキュー境界、リレー ID、
  и マージされた失効セット)。
- `POST /v2/token/mint` - ML-DSA アドミッション トークン、提供された
  ハッシュを再開します。本体は `{ "transcript_hash_hex": "...", "ttl_secs": <u64>, "flags": <u8> }` です。

チケット, выпущенные сервисом, проверяются в интеграционном тесте
`volumetric_dos_soak_preserves_puzzle_and_latency_slo`、 который также
ボリューム DoS を使用してリレー スロットルを設定します。[tools/soranet-relay/tests/adaptive_and_puzzle.rs:337]

## Настройка выпуска токенов

リレー JSON の `pow.token.*` (最低。
`tools/soranet-relay/deploy/config/relay.entry.json` как пример)、чтобы
ML-DSA トークンを取得します。発行者の公開鍵を指定するか、オプションで指定します。
失効リスト:

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

パズル サービスは、最高のサービスを提供します。
Norito JSON の取り消しとランタイム。 CLI `soranet-admission-token` を参照してください。
(`cargo run -p soranet-relay --bin soranet_admission_token`) Їтобы выпускать и
オフラインのトークン、失効後の `token_id_hex` エントリ
認証情報を監査し、本番環境を更新します。

発行者の秘密キーとパズル サービスの CLI フラグ:

```bash
cargo run -p soranet-puzzle-service -- \
  --relay-config /etc/soranet/relay/relay.entry.json \
  --token-secret-path /etc/soranet/relay/token_issuer_secret.hex \
  --token-revocation-file /etc/soranet/relay/token_revocations.json \
  --token-revocation-refresh-secs 60
```

`--token-secret-hex` доступен、когда Secret управляется アウトオブバンド ツール
パイプライン。失効ファイル ウォッチャー `/v2/token/config` актуальным;
координируйте обновления с командой `soranet-admission-token revoke`, чтобы
失効状態。

`pow.signed_ticket_public_key_hex` とリレー JSON、メッセージ
ML-DSA-44 公開キーと署名済み PoW チケット。 `/v2/puzzle/config` эхо
キーと BLAKE3 指紋 (`signed_ticket_public_key_fingerprint_hex`)、
クライアントのピン認証検証者。サイン入りチケット、リレーID、
トランスクリプト バインディングと失効ストア。生の74バイトのPoWチケット
署名付きチケットの検証者。署名者の秘密
`--signed-ticket-secret-hex` または `--signed-ticket-secret-path` は、
パズルサービス。キーペア、シークレットを確認する
`pow.signed_ticket_public_key_hex`。 `POST /v2/puzzle/mint` です
`"signed": true` (またはオプションの `"transcript_hash_hex"`)
Norito エンコードされた署名付きチケット - 生のチケット バイト。 ответы включают
`signed_ticket_b64` および `signed_ticket_fingerprint_hex` は、指紋を再生します。
Запросы с `signed = true` отклоняются、署名者の秘密を保護します。

## プレイブック ротации ключей

1. **Соберите новый 記述子 commit.** ガバナンス リレー記述子
   ディレクトリバンドルをコミットします。 16 進文字列 `handshake.descriptor_commit_hex`
   внутри リレー JSON конфигурации、которой пользуется パズル サービス。
2. **境界ポリシー パズルです。** パズルを解いてみましょう。
   `pow.puzzle.{memory_kib,time_cost,lanes}` がリリースされました。 Операторы
   должны держать Argon2 конфигурацию детерминированной между リレー (минимум 4 MiB)
   1 <= レーン <= 16)。
3. **Подготовьте рестарт.** systemd ユニットとコンテナーの組み合わせ
   ガバナンスによるローテーションの切り替え。ホットリロードを実行します。日
   применения нового 記述子コミット требуется 再起動。
4. **Провалидируйте.** チケット через `POST /v2/puzzle/mint` および подтвердите、
   что `difficulty` および `expires_at` は、ポリシーを適用します。ソークレポート
   (`docs/source/soranet/reports/pow_resilience.md`) レイテンシーの限界
   для справки. Когда トークン включены、запросите `/v2/token/config`、чтобы убедиться、
   公開された発行者のフィンガープリントと失効数を確認できます。

## 緊急無効化機能1. Установите `pow.puzzle.enabled = false` в общей リレー конфигурации. Оставьте
   `pow.required = true`、hashcash フォールバック チケットが表示されます。
2. Опционально примените `pow.emergency` エントリ、чтобы отклонять устаревлие
   記述子は Argon2 ゲートをオフラインにします。
3. リレーとパズル サービスを利用できます。
4. Мониторьте `soranet_handshake_pow_difficulty`、чтобы убедиться、что сложность
   ハッシュキャッシュ значения, и проверьте, что `/v2/puzzle/config`
   `puzzle = null` です。

## モニタリングとアラート

- **レイテンシ SLO:** Отслеживайте `soranet_handshake_latency_seconds` および держите P95
  300ミリ秒。ソークテストオフセットとキャリブレーションデータとガードスロットル。
  【docs/source/soranet/reports/pow_resilience.md:1】
- **クォータ プレッシャー:** Используйте `soranet_guard_capacity_report.py` с リレー メトリクス
  `pow.quotas` クールダウン (`soranet_abuse_remote_cooldowns`、
  `soranet_handshake_throttled_remote_quota_total`).【docs/source/soranet/relay_audit_pipeline.md:68】
- **パズルの配置:** `soranet_handshake_pow_difficulty` должен совпадать с
  難易度は`/v2/puzzle/config`です。リレー構成が古いです
  再起動です。
- **トークンの準備状況:** アラート、`/v2/token/config` неожиданно падает до
  `enabled = false` または `revocation_source` は、古いタイムスタンプを返します。 Операторы
  должны ротировать Norito 失効ファイル через CLI при выводе токена, чтобы
  エンドポイント оставался точным。
- **サービスの健全性:** Пробуйте `/healthz` と liveness cadence および alert、
  `/v2/puzzle/mint` HTTP 500 (Argon2 パラメータの不一致)
  または RNG の失敗)。トークンの鋳造 проявляются как HTTP 4xx/5xx на
  `/v2/token/mint`;ページング条件を確認します。

## コンプライアンスと監査ログ

リレーの表示 `handshake` イベント、スロットルの理由
и クールダウン期間。コンプライアンス パイプラインの管理
`docs/source/soranet/relay_audit_pipeline.md` ログ、ログ、ログなど
パズルポリシーは監査可能です。 Когда パズル ゲート включен, архивируйте
作成されたチケットと Norito 構成スナップショットのロールアウト チケット
監査。入場トークン、メンテナンス期間、
`token_id_hex` と失効を確認してください。
истечения или отзыва.