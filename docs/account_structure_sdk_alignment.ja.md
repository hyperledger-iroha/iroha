# SDK/コーデック担当者向け IH58 展開メモ

対象: Rust SDK、TypeScript/JavaScript SDK、Python SDK、Kotlin SDK、コーデックツール

コンテキスト: `docs/account_structure.md` は出荷される IH58 アカウント ID 実装を反映しました。
SDK の挙動とテストを標準仕様に合わせてください。

主要参照:
- アドレスコーデック + ヘッダー構成 — `docs/account_structure.md` §2
- 曲線レジストリ — `docs/source/references/address_curve_registry.md`
- Norm v1 ドメイン処理 — `docs/source/references/address_norm_v1.md`
- フィクスチャベクトル — `fixtures/account/address_vectors.json`

対応事項:
1. **カノニカル出力:** `AccountId::to_string()`/Display は IH58 のみを出力すること
   （`@domain` サフィックス無し）。カノニカル hex はデバッグ用途（`0x...`）。
2. **Accepted inputs:** parsers MUST accept only canonical IH58 account literals. Reject compressed `sora...`, canonical hex (`0x...`), any `@<domain>` suffix, alias literals, legacy `norito:<hex>`, and `uaid:` / `opaque:` parser forms.
3. **Resolvers:** canonical account parsing has no default-domain binding, scoped inference, or fallback resolver path. Use `ScopedAccountId` only on interfaces that explicitly require `<account>@<domain>`.
4. **IH58 チェックサム:** `IH58PRE || prefix || payload` に対して Blake2b‑512 を計算し、
   先頭 2 バイトを使用。圧縮アルファベットの基数は **105**。
5. **曲線ゲーティング:** SDK は既定で Ed25519 のみ。ML‑DSA/GOST/SM は明示的な opt‑in を提供
   （Swift のビルドフラグ、JS/Android の `configureCurveSupport`）。Rust 以外で secp256k1 が
   既定有効だと仮定しないこと。
6. **CAIP‑10 なし:** まだ CAIP‑10 マッピングは出荷されていないため、CAIP‑10 変換を
   公開・依存しないこと。

コードとテストの更新が完了したら確認してください。未解決事項はアカウントアドレス RFC スレッドで追跡できます。
