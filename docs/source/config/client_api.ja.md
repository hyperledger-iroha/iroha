---
lang: ja
direction: ltr
source: docs/source/config/client_api.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: fa548ec31fe928decc5c23719472618ff97f4eb45b084f9f9084df82b96cfac6
source_last_modified: "2026-01-03T18:07:57.683798+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

## クライアント API 構成リファレンス

この文書では、Torii クライアント側の構成ノブを追跡します。
`iroha_config::parameters::user::Torii` を通じて表面化します。以下のセクション
NRPC-1 に導入された Norito-RPC トランスポート制御に焦点を当てます。未来
クライアント API 設定では、このファイルを拡張する必要があります。

### `torii.transport.norito_rpc`

|キー |タイプ |デフォルト |説明 |
|-----|------|----------|---------------|
| `enabled` | `bool` | `true` |バイナリ Norito デコードを有効にするマスター スイッチ。 `false` の場合、Torii は、`403 norito_rpc_disabled` によるすべての Norito-RPC 要求を拒否します。 |
| `stage` | `string` | `"disabled"` |ロールアウト層: `disabled`、`canary`、または `ga`。ステージは、許可の決定と `/rpc/capabilities` の出力を駆動します。 |
| `require_mtls` | `bool` | `false` | Norito-RPC トランスポートに mTLS を適用します。`true` の場合、Torii は、mTLS マーカー ヘッダー (例: `X-Forwarded-Client-Cert`) を含まない Norito-RPC リクエストを拒否します。このフラグは `/rpc/capabilities` 経由で表示されるため、SDK は設定が間違っている環境について警告できます。 |
| `allowed_clients` | `array<string>` | `[]` |カナリアの許可リスト。 `stage = "canary"` の場合、このリストに存在する `X-API-Token` ヘッダーを運ぶリクエストのみが受け入れられます。 |

構成例:

```toml
[torii.transport.norito_rpc]
enabled = true
require_mtls = true
stage = "canary"
allowed_clients = ["alpha-canary-token", "beta-canary-token"]
```

ステージのセマンティクス:

- **無効** - Norito-RPC は、`enabled = true` であっても使用できません。クライアント
  `403 norito_rpc_disabled`を受信します。
- **canary** — リクエストには、次のいずれかに一致する `X-API-Token` ヘッダーが含まれている必要があります。
  `allowed_clients`の。他のすべてのリクエストは `403 を受け取ります
  ノリト_rpc_canary_denied`。
- **ga** — Norito-RPC は、認証されたすべての発信者が利用できます (
  通常のレートと認証前の制限）。

オペレーターは、`/v2/config` を通じてこれらの値を動的に更新できます。それぞれの変化
`/rpc/capabilities` に即座に反映され、SDK と可観測性が可能になります。
ダッシュボードには実際の輸送姿勢が表示されます。