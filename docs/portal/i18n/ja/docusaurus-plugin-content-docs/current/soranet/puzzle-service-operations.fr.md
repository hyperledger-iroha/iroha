---
lang: ja
direction: ltr
source: docs/portal/docs/soranet/puzzle-service-operations.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id:パズルサービス運営
タイトル: パズルのサービス操作ガイド
Sidebar_label: パズルのサービスに関する操作
説明: デーモン `soranet-puzzle-service` による Argon2/ML-DSA 入場チケットの悪用。
---

:::note ソースカノニク
:::

# パズルのサービス操作ガイド

デーモン `soranet-puzzle-service` (`tools/soranet-puzzle-service/`) のイベント
入場券ベース sur Argon2 qui refletent lapolicy `pow.puzzle.*` du
リレー、ロルスク構成、入場 ML-DSA プールのトークンのオーケストラ
リレーエッジ。 cinq エンドポイント HTTP を公開します。

- `GET /healthz` - ライブネスを調査します。
- `GET /v2/puzzle/config` - PoW/パズルの効果に関するパラメータの報告
  JSON リレー (`handshake.descriptor_commit_hex`、`pow.*`)。
- `POST /v2/puzzle/mint` - Argon2 のチケットを入手しました。本体の JSON オプションネル
  `{ "ttl_secs": <u64>, "transcript_hash_hex": "<32-byte hex>", "signed": true }`
  TTL プラス裁判所 (ポリシーの適用窓口) を要求し、国連のチケットを請求する
  リレー + チケット署名の記録ハッシュと記録
  署名を設定するには、署名を設定します。
- `GET /v2/token/config` - `pow.token.enabled = true`、ポリシーの返却
  d'admission-token active (発行者のフィンガープリント、TTL/クロックスキューの制限、リレー ID、
  取り消しの融合など）。
- `POST /v2/token/mint` - トークンの承認 ML-DSA 嘘と履歴書のハッシュを取得
  フォーニ。ファイル本体は `{ "transcript_hash_hex": "...", "ttl_secs": <u64>, "flags": <u8> }` を受け入れます。

サービスのチケット製品は統合テストで検証されます
`volumetric_dos_soak_preserves_puzzle_and_latency_slo`、オーストラリアで練習してください
リレーとシナリオのスロットル DoS ボリュームトリック。【tools/soranet-relay/tests/adaptive_and_puzzle.rs:337】

## トークンの発行の構成者

Definissez les Champs JSON デュ リレー スー `pow.token.*` (voir)
`tools/soranet-relay/deploy/config/relay.entry.json` 例) afin
d'activer les トークン ML-DSA。最小限の発行者としての発行者
失効オプションのリスト:

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

パズル サービスは ces valeurs と recharge automatiquement le fichier を再利用します
Norito 実行時の JSON の取り消し。 CLI `soranet-admission-token` を利用する
(`cargo run -p soranet-relay --bin soranet_admission_token`) 注ぎます
オフラインのトークンの検査、メインの検査 `token_id_hex` の検査
失効、および更新情報の存在する資格情報の監査
生産。

フラグ CLI 経由でパズル サービスの発行者秘密を渡す:

```bash
cargo run -p soranet-puzzle-service -- \
  --relay-config /etc/soranet/relay/relay.entry.json \
  --token-secret-path /etc/soranet/relay/token_issuer_secret.hex \
  --token-revocation-file /etc/soranet/relay/token_revocations.json \
  --token-revocation-refresh-secs 60
```

`--token-secret-hex` オーストラリアで最も秘密にされている情報
パイプラインの帯域外。失効ガードの監視者 `/v2/token/config`
1時間。コーディネート レ ミゼス ア ジュール アベック ラ コマンド `soranet-admission-token revoke`
取り消しを遅らせてください。

JSON 中継アナウンサーによる `pow.signed_ticket_public_key_hex` の定義
la cle public ML-DSA-44 は、検証者による PoW チケットの署名を利用します。 `/v2/puzzle/config`
BLAKE3 (`signed_ticket_public_key_fingerprint_hex`) を繰り返します
クライアントの投稿者は検証者です。チケットは有効ではなく署名されます
リレー ID とトランスクリプト バインディングとミーム ストアの一部を制御します
取り消し; 74 オクテットの PoW チケットを保持し、検証者を検証します
署名付きチケットの構成。 `--signed-ticket-secret-hex`経由で秘密の署名を渡します
ou `--signed-ticket-secret-path` パズル サービスの提供。ル・デマラージュ
キーペアの矛盾を解消し、秘密を有効にします。
`pow.signed_ticket_public_key_hex`。 `POST /v2/puzzle/mint` 受け入れます `"signed": true`
(オプションネル `"transcript_hash_hex"`) チケット署名 Norito en を送信してください
プラス・デ・バイト・デュ・チケット・ブリュット。 `signed_ticket_b64` などを含む応答
`signed_ticket_fingerprint_hex` 指紋を再生してください。レ
avec `signed = true` は次のパスの秘密の署名を拒否します
設定します。

## ローテーションのプレイブック1. **コレクター le nouveau 記述子コミット。** ガバナンス パブリック ル リレー
   ファイルディレクトリバンドルの記述子コミット。コピーラチェーンヘックスダンス
   `handshake.descriptor_commit_hex` 構成 JSON リレー参加者
   avec leパズルサービス。
2. **政策パズルの検証者。** 価値を確認する
   `pow.puzzle.{memory_kib,time_cost,lanes}` は 1 時間の平均計画を達成できません
   デリリース。 Argon2 の設定を決定する操作者
   リレー全体 (最小 4 MiB メモワール、1 <= レーン <= 16)。
3. **再婚の準備をします。** システム デバイスとコンテナーを再充電します
   ガバナンスはローテーションによるカットオーバーを発表します。 Le service ne supporte
   ホットリロードだけでなく。再婚を要求する場合は、プレンドル ル ヌーボーの記述子を注ぐ必要があります
   コミットします。
4. **有効者** Emettez が `POST /v2/puzzle/mint` 経由でチケットを発行し、確認します
   `difficulty` および `expires_at` 特派員の新しいポリシー。ル・ラポール
   ソーク (`docs/source/soranet/reports/pow_resilience.md`) キャプチャ デ ボーンズ
   遅延出席者注ぐ参照。 Lorsque les tokens ソントアクティブ、リセズ
   `/v2/token/config` 検証者が発行者指紋通知などを送信します
   compte de revocation特派員aux valeurs出席者。

## 緊急の非活性化手順

1. 構成リレー参加者による `pow.puzzle.enabled = false` の定義。
   Gardez `pow.required = true` シレス チケット ハッシュキャッシュ フォールバック ドイベント レストレーター
   義務者。
2. オプション、メインディッシュ `pow.emergency` を拒否する
   記述子はペンダント ケ ラ ポート Argon2 est をオフラインにします。
3. リレーやパズルサービス、アップリケなどのレデマレッツ
   変化。
4. Surveillez `soranet_handshake_pow_difficulty` 検証が困難な場合
   tombe a la valeur hashcash 出席、有効性確認 `/v2/puzzle/config`
   ラポール `puzzle = null`。

## モニタリングとアラート

- **レイテンシ SLO:** Suivez `soranet_handshake_latency_seconds` および Gardez le P95
  300ミリ秒程度。 4 回の浸漬テストでのオフセットとキャリブレーションの必要性
  スロットルをガードしてください。【docs/source/soranet/reports/pow_resilience.md:1】
- **クォータ プレッシャー:** `soranet_guard_capacity_report.py` 平均値を使用
  メトリクス リレー プール アジャスタ レ クールダウン `pow.quotas` (`soranet_abuse_remote_cooldowns`、
  `soranet_handshake_throttled_remote_quota_total`).【docs/source/soranet/relay_audit_pipeline.md:68】
- **パズルの配置:** `soranet_handshake_pow_difficulty` はアラに対応します
  `/v2/puzzle/config` による帰還は困難です。構成に関する相違点
  リレーの失効率と再婚率。
- **トークンの準備状況:** `/v2/token/config` から `enabled = false` へのアラート
  `revocation_source` タイムスタンプの報告が古くなりました。
  CLI 経由で失効 Norito を操作するフェア ツアーナーの操作
  トークンの復元は、ガーダー セットのエンドポイントの精度を維持します。
- **サービスの正常性:** Probez `/healthz` ライブの習慣などの平均
  alertez si `/v2/puzzle/mint` renvoie des reponses HTTP 500 (インデックスの不一致
  Argon2 または RNG のパラメータ)。トークン鋳造時のエラー
  レスポンス HTTP 4xx/5xx sur `/v2/token/mint` 経由で明示的。トレイテズ・レ
  echecs はページング条件を繰り返します。

## コンプライアンスと監査ログ

Les emettent desevenements `handshake` 構造がレゾンに影響を与える
スロットルとクールダウンのデュレ。パイプラインのコンプライアンス遵守を保証する
`docs/source/soranet/relay_audit_pipeline.md` の入力ログの説明
ポリシーパズルの変更は監査可能です。クアンド・ラ・ポート・パズル
最もアクティブな、チケットのアーカイブとスナップショットの保存
構成 Norito 将来の監査のロールアウト チケットの平均。レ
入場トークン mintes avant les fenetres de メンテナンス doivent etre suivis
avec leurs valeurs `token_id_hex` et inseres dans le fichier de revocation une
期限切れまたは取り消し。