---
lang: ja
direction: ltr
source: docs/portal/docs/soranet/puzzle-service-operations.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id:パズルサービス運営
タイトル: パズルの操作ガイド
Sidebar_label: パズルのサービス業務
説明: Argon2/ML-DSA のチケットに対するデーモン `soranet-puzzle-service` の操作。
---

:::ノート フエンテ カノニカ
リフレジャ`docs/source/soranet/puzzle_service_operations.md`。マンテンガンのアンバス バージョンは、すべてのドキュメントを参照して引退する予定です。
:::

# パズルの操作ガイド

El デーモン `soranet-puzzle-service` (`tools/soranet-puzzle-service/`) が発行する
入場チケット、Argon2 que reflejan la ポリシー `pow.puzzle.*`
リレーの削除、設定の確認、ML-DSA のアドミッション メディア トークン
ノンブル・ド・リレーエッジ。 cinco エンドポイント HTTP を公開します。

- `GET /healthz` - ライブネスを調査します。
- `GET /v1/puzzle/config` - PoW/パズルの効果的なパラメータの開発
  tomados デル JSON デル リレー (`handshake.descriptor_commit_hex`、`pow.*`)。
- `POST /v1/puzzle/mint` - アルゴン 2 のチケット。本体 JSON オプション
  `{ "ttl_secs": <u64>, "transcript_hash_hex": "<32-byte hex>", "signed": true }`
  solicita un TTL mas corto (クランプ アル ウィンドウ デ ポリシー)、チケット ア ウン
  トランスクリプト ハッシュ y デブエルブ アン チケット ファームド ポル エル リレー + ラ ウエラ デ
  ラ・ファーム・クアンド・ヘイ・クラベス・デ・ファームド・コンフィギュレーション。
- `GET /v1/token/config` - クアンド `pow.token.enabled = true`、デブエルブ ラ
  アドミッション トークンのポリシー アクティベーション (発行者のフィンガープリント、TTL/クロック スキューの制限、
  リレー ID と取り消しの組み合わせを設定します)。
- `POST /v1/token/mint` - ML-DSA の承認トークンの履歴書
  ハッシュ但し書き。エルボディアセプタ`{ "transcript_hash_hex": "...", "ttl_secs": <u64>, "flags": <u8> }`。

ロスチケットは、統合されたプルエバでの検証を保証します
`volumetric_dos_soak_preserves_puzzle_and_latency_slo`、緊急事態宣言
DoS ボリュームトリコの継続的なスロットル デル リレーのシナリオを失います。【tools/soranet-relay/tests/adaptive_and_puzzle.rs:337】

## トークンの発行を構成する

ロス カンポス JSON デル リレー バジョ `pow.token.*` (ver) を構成する
`tools/soranet-relay/deploy/config/relay.entry.json` como ejemplo) パラハビリタール
トークン ML-DSA を失います。コモミニモ、発行者とリストの発行者を証明するクラーベ
取り消しのオプション:

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

パズル サービスの再利用と価値の自動取得、アーカイブの自動化
Norito 実行時の JSON の取り消し。米国エル CLI `soranet-admission-token`
(`cargo run -p soranet-relay --bin soranet_admission_token`) 症状
トークンのオフライン検査、アネクサーエントリ `token_id_hex` のアーカイブ
監査資格の失効と公開更新の既存の情報
生産。

フラグ CLI を介した発行者アル クラーベの秘密のパズル サービス:

```bash
cargo run -p soranet-puzzle-service -- \
  --relay-config /etc/soranet/relay/relay.entry.json \
  --token-secret-path /etc/soranet/relay/token_issuer_secret.hex \
  --token-revocation-file /etc/soranet/relay/token_revocations.json \
  --token-revocation-refresh-secs 60
```

`--token-secret-hex` 安全な秘密を確認してください
帯域外のツールのパイプライン。取り消しのアーカイブを監視する
マンティエン `/v1/token/config` 実際の状態。エルコマンドの実際の座標
`soranet-admission-token revoke` は、取り消しの無効化を解除します。

通知用の JSON リレーによる `pow.signed_ticket_public_key_hex` の構成
la clave publica ML-DSA-44 usada para verificar ticket PoW farmados; `/v1/puzzle/config`
レプリカ ラ クラーベ イ ス フエラ BLAKE3 (`signed_ticket_public_key_fingerprint_hex`)
パラ・ケ・ロス・クライアント・プエダン・フィハル・エル・ベリフィカドール。ロスチケットは有効です
対照リレー ID が失われ、トランスクリプト バインディングが失われ、取り消しのミスモ ストアが失われます。
Los ticket PoW crudos de 74 bytes siguen siendo validos cuando el verificador de
署名済みチケットの esta 設定。 `--signed-ticket-secret-hex` 経由の会社秘密情報
o `--signed-ticket-secret-path` 最初のパズル サービス。エル・アランク・レチャザ
クラベスのペアは偶然の一致ではなく、秘密の検証もありません `pow.signed_ticket_public_key_hex`。
`POST /v1/puzzle/mint` acepta `"signed": true` (オプションの `"transcript_hash_hex"`) パラ
Devolver un ticket farmado Norito junto con los bytes del ticket en bruto;ラス
`signed_ticket_b64` y `signed_ticket_fingerprint_hex` パラ アユダールを含む応答
ラストレアの指紋のリプレイ。 `signed = true` に対するお願いです
会社の秘密は設定されません。

## クラーベスの回転戦略1. **新しい記述子のコミットを再収集します。** ガバナンス広報リレー
   記述子はディレクトリのバンドルをコミットします。コピア ラ カデナ ヘックス アン
   `handshake.descriptor_commit_hex` リレーの JSON 構成をデントロ
   コンパルティド コンエル パズル サービス。
2. **パズルのポリシーの制限の改訂。** 価値を失うことを確認する
   `pow.puzzle.{memory_kib,time_cost,lanes}` 現実的な安全保障計画
   デリリース。アルゴン 2 の設定を決定するロス オペラドール
   エントリリレー (ミニモ 4 MiB デメモリア、1 <= レーン <= 16)。
3. **再設定を準備します。** システム デバイスとコンテンツを再確認します。
   ガバナンスは、ローテーションの切り替えを発表します。ホットリロードを必要としないサービス。
   トマールエルヌエボ記述子のコミットを要求します。
4. **Validar.** `POST /v1/puzzle/mint` 経由でチケットを発行し、確認を行ってください。
   値 `difficulty` と `expires_at` は、新しいポリシーと一致します。お知らせします
   デソーク (`docs/source/soranet/reports/pow_resilience.md`) キャプチャーロスリミット
   デ・レイテンシア・エスペラードス・コモ・レファレンシア。クアンド・ロス・トークン・エスタン・ハビリタドス、
   `/v1/token/config` を参照して発行者の指紋を確認してください
   取り消しの状況は、損失のエスペラードと偶然に一致します。

## 緊急時の避難指示手順

1. リレー比較設定で `pow.puzzle.enabled = false` を設定します。
   Mantiene `pow.required = true` si los チケット hashcash フォールバック デベン セギル
   シエンド・オブリガトリオス。
2. Rechazar 記述子に関する `pow.emergency` アプリケーションのオプション
   オフラインのオブソレトス ミエントラ ラ プエルタ Argon2 エスタ。
3. カンビオでのリレーとパズルのサービス。
4. Monitorea `soranet_handshake_pow_difficulty` の困難な状況を監視する
   cae al valor hashcash esperado、y verifica que `/v1/puzzle/config` reporte
   `puzzle = null`。

## 監視とアラート

- **レイテンシ SLO:** Rastrea `soranet_handshake_latency_seconds` およびマンテン エル P95
  300 ミリ秒で送信されます。損失オフセットはソークテストで証明された校正データです
  ガードのスロットル。【docs/source/soranet/reports/pow_resilience.md:1】
- **クォータプレッシャー:** 米国 `soranet_guard_capacity_report.py` コンメトリクス デ リレー
  `pow.quotas` の調整可能なクールダウン (`soranet_abuse_remote_cooldowns`、
  `soranet_handshake_throttled_remote_quota_total`).【docs/source/soranet/relay_audit_pipeline.md:68】
- **パズルの配置:** `soranet_handshake_pow_difficulty` デベ コンシディル コン ラ
  `/v1/puzzle/config` の難しい問題。インドの構成の相違
  古いリレーと不規則な状況です。
- **トークンの準備状況:** `/v1/token/config` から `enabled = false` へのアラート
  `revocation_source` レポートのタイムスタンプが古くなっています。ロス オペラドーレス
  CLI 経由で deben rotar el archive de revocaciones Norito cuando se restart un token
  パラマンテナーエステエンドポイントの精度。
- **サービスの健全性:** Sondea `/healthz` en la cadencia 習慣的な活性とアラート
  si `/v1/puzzle/mint` HTTP 500 の応答を取得 (パラメータの不一致
  Argon2 は RNG の影響を受けます)。トークン鋳造時のエラーの損失
  HTTP 4xx/5xx en `/v1/token/mint`;ページングの条件を繰り返します。

## コンプライアンスと監査ログ

イベント `handshake` を含むロス リレーの構造
スロットルとデュラシオンのクールダウン。パイプラインのコンプライアンス記述を確保する
en `docs/source/soranet/relay_audit_pipeline.md` estos ログをパラケロスで取り込む
監査可能なポリシーとパズルの組み合わせ。クアンド・ラ・プエルタ・デ・パズル
エステハビリタダ、チケット発行のアーカイブ、および設定のスナップショット
Norito フューチュラス オーディトリアスのロールアウト チケットを確認します。入場トークンの紛失
エミドス・アンテス・デ・ベンタナス・デ・マンテニミエント・デベン・ラストレアス・コン・サス・ヴァロレス
`token_id_hex` 有効期限が切れていない取り消しアーカイブを挿入します
ショーン・レボカドス。