---
lang: ja
direction: ltr
source: docs/portal/docs/soranet/puzzle-service-operations.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id:パズルサービス運営
title: パズル サービスのオペラ座
Sidebar_label: パズル サービスの運用
説明: Operacao はデーモン `soranet-puzzle-service` パラ入場チケット Argon2/ML-DSA を実行します。
---

:::note フォンテ カノニカ
エスペラ `docs/source/soranet/puzzle_service_operations.md`。マンテンハ・アンバスはコピア・シンクロニザダスとして。
:::

# オペラ座のパズルサービス

O デーモン `soranet-puzzle-service` (`tools/soranet-puzzle-service/`) が発行する
入場チケットは Argon2 に関するポリシーを反映しています `pow.puzzle.*`
リレー、Quando configurado、FAZ ブローカーの ML-DSA アドミッション トークンの名前を実行します。
dosエッジリレー。 Ele expoe cinco エンドポイント HTTP:

- `GET /healthz` - 活性プローブ。
- `GET /v1/puzzle/config` - Retorna os parametros efetivos de PoW/パズルエクストライド
JSON を実行してリレーを実行します (`handshake.descriptor_commit_hex`、`pow.*`)。
- `POST /v1/puzzle/mint` - Argon2 チケットを発行します。ええと、本文の JSON はオプションです
  `{ "ttl_secs": <u64>, "transcript_hash_hex": "<32-byte hex>", "signed": true }`
  TTL メナー (ポリシーのクランプ)、チケットのビンキュラ、トランスクリプトを実行します
  ハッシュとレトルナ ウム チケット アシナド ペロ リレー + 指紋 da assinatura quando
  Chaves de assinatura estao configuradas として。
- `GET /v1/token/config` - `pow.token.enabled = true`、ポリシーを返します
  アドミッション トークン (発行者のフィンガープリント、TTL/クロック スキューの制限、リレー ID) の設定
  e o 失効セット メスクラド)。
- `POST /v1/token/mint` - ML-DSA アドミッション トークン vinculado ao 再開ハッシュを発行します
  フォルネシド。 o本体アセタ`{ "transcript_hash_hex": "...", "ttl_secs": <u64>, "flags": <u8> }`。

OS チケットは、検証サービスなしでプロデュージドス ペロ サービスを提供します
`volumetric_dos_soak_preserves_puzzle_and_latency_slo`、問題が発生しました
スロットルは DoS ボリュームの継続的なシナリオをリレーします。
(tools/soranet-relay/tests/adaptive_and_puzzle.rs:337)

## トークンの送信を構成する

Defina os Campos JSON はリレー em `pow.token.*` (veja
`tools/soranet-relay/deploy/config/relay.entry.json` como exemplo) パラハビリタール
ML-DSA トークン。ミニモはありません。発行者と uma 失効リストを公開するための情報
オプション:

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

O パズル サービスは、価値のあるものを再利用し、自動的にアルキーボを再利用します
Norito ランタイムの revogacao の JSON。 CLI `soranet-admission-token` を使用します。
(`cargo run -p soranet-relay --bin soranet_admission_token`)パラエミッター
トークンのオフライン検査、アネクサーエントリ `token_id_hex` ao arquivo de revogacao
監査資格は存在し、公開情報を更新します。

フラグ CLI を介してパズル サービスに発行者の秘密キーを渡します。

```bash
cargo run -p soranet-puzzle-service -- \
  --relay-config /etc/soranet/relay/relay.entry.json \
  --token-secret-path /etc/soranet/relay/token_issuer_secret.hex \
  --token-revocation-file /etc/soranet/relay/token_revocations.json \
  --token-revocation-refresh-secs 60
```

`--token-secret-hex` 秘密の秘密を秘密にします
バンダ用のパイプラインとツール。おお、レボガカオの監視者よ
`/v1/token/config` 正常です。 coordene アップデート com o commando
`soranet-admission-token revoke` パラ エボガカオ デファサード。

Defina `pow.signed_ticket_public_key_hex` は、JSON を使用せずに通知を中継します
publica ML-DSA-44 usada para verificar PoW チケット assinados; `/v1/puzzle/config`
指紋 BLAKE3 を再現 (`signed_ticket_public_key_fingerprint_hex`)
パラケクライアントポッサムフィクサーオーバーベリファイアー。チケット アシナドス サン バリダドス コントラ
o リレー ID e トランスクリプト バインディング e compartilham o mesmo 失効ストア;捕虜
チケット ブルート 74 バイト永続有効期限付き署名付きチケット検証者
構成を変更します。 `--signed-ticket-secret-hex` ou 経由で署名者の秘密を渡します
`--signed-ticket-secret-path` パズル サービスの開始。 o スタートアップ レジータ
keypairs divergentes se o Secret nao valida contra `pow.signed_ticket_public_key_hex`。
`POST /v1/puzzle/mint` アセイタ `"signed": true` (オプションの `"transcript_hash_hex"`) パラ
retornar um 署名済みチケット Norito junto com os bytes do ticket bruto;再投稿として
`signed_ticket_b64` と `signed_ticket_fingerprint_hex` パラ アジュダルとラストレアを含む
指紋の再生。 Requests com `signed = true` sao rejeitadas se o 署名者の秘密
ナオ・エスティバー・コンフィギュラド。

## チャベスの回転戦略1. **新しい記述子のコミット。** ガバナンス公開またはリレー
   記述子はディレクトリ バンドルをコミットしません。 16 進数の文字列をコピーします
   `handshake.descriptor_commit_hex` JSON をリレーコンパートメントで設定する
   com oパズルサービス。
2. **パズルのポリシーの境界を見直します。** 妥当性を確認します。
   `pow.puzzle.{memory_kib,time_cost,lanes}` エステジャム・アリニャドス・アオ・プラノ・デ・リリース。
   オペレータは Argon2 の決定性を考慮したリレーの設定を管理します
   (ミニモ 4 MiB デ メモリア、1 <= レーン <= 16)。
3. **準備または再起動。** システム ディレクトリとコンテナーを一度に再設定します。
   ガバナンス宣言またはカットオーバー・デ・ロータカオ。 O servico nao suporta ホットリロード;
   ええと、新しい記述子のコミットに必要な再起動が必要です。
4. **Validar.** `POST /v1/puzzle/mint` 経由でチケットを送信し、クエリを確認します
   `difficulty` および `expires_at` は nova ポリシーに対応します。オーソークレポート
   (`docs/source/soranet/reports/pow_resilience.md`) 潜在的なキャプチャーの限界
   エスペラードパラレファレンシア。 Quando トークン estiverem habilitados、busque
   `/v1/token/config` 発行者の指紋保証に関する保証
   aos valores esperados の通信者に感染します。

## 緊急事態宣言の手順

1. リレーを実行するために `pow.puzzle.enabled = false` を設定します。
   Mantenha `pow.required = true` se os チケット hashcash フォールバック precisarem
   永続的な義務。
2. オプション、強制エントラダ `pow.emergency` パラレル記述子
   アンティゴス エンクアント オ ゲート Argon2 エスティバーがオフラインです。
3. ムダンカに適用されるパズル サービスのリレー。
4. 困難を保証するために `soranet_handshake_pow_difficulty` を監視する
   caia para o valor hashcash esperado e verifique `/v1/puzzle/config` reportando
   `puzzle = null`。

## アラートの監視

- **レイテンシ SLO:** Acompanhe `soranet_handshake_latency_seconds` e mantenha o P95
  アバイショ デ 300 ミリ秒。 OS オフセットは、校正用のソーク テストを実行します
  ガードスロットル。 (docs/source/soranet/reports/pow_resilience.md:1)
- **クォータ プレッシャー:** `soranet_guard_capacity_report.py` com リレー メトリックを使用します
  `pow.quotas` の調整可能なクールダウン (`soranet_abuse_remote_cooldowns`、
  `soranet_handshake_throttled_remote_quota_total`)。 (docs/source/soranet/relay_audit_pipeline.md:68)
- **パズルの配置:** `soranet_handshake_pow_difficulty` 開発者 a
  `/v1/puzzle/config` による困難な問題。 Divergencia indica リレー構成
  古い、ファルホを再起動します。
- **トークンの準備状況:** アラート `/v1/token/config` または `enabled = false`
  `revocation_source` レポーターのタイムスタンプが古くなっています。オペラドール
  devem rotacionar または arquivo Norito de revogacao (CLI Quando um トークン経由)
  レティラード・パラ・マンター・エッセのエンドポイント精度。
- **サービスの健全性:** `/healthz` の通常の稼働状況とアラートを調査しています
  `/v1/puzzle/mint` 戻り値 HTTP 500 (パラメータ Argon2 ou の不一致
  ファルハス デ RNG)。 HTTP 4xx/5xx em のトークン作成エラー
  `/v1/token/mint`;ページングの繰り返しを繰り返します。

## コンプライアンスと監査ログ

リレーはイベント `handshake` を発行し、スロットルの動機を含む詳細を確認します
クールダウン期間。パイプラインのコンプライアンスの説明に関する説明
`docs/source/soranet/relay_audit_pipeline.md` ingira esses logs para que mudancas
パズルの監査に関するポリシーです。 Quando パズル ゲート エスティバー ハビリタド、
Norito com o のチケット発行時のスナップショットの保存
フューチュラス講堂でのロールアウトチケット。入場トークンは、エミティドス・アンテス・デ・ジャネラス
管理者は、rastreados ペロス valores `token_id_hex` をインセリドスなしで開発します。
レボガドスをすべて有効期限内に保管してください。