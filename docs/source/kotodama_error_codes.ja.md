<!-- Japanese translation of docs/source/kotodama_error_codes.md -->

---
lang: ja
direction: ltr
source: docs/source/kotodama_error_codes.md
status: complete
translator: manual
---

# Kotodama コンパイラエラーコード

Kotodama コンパイラは安定したエラーコードを出力し、ツールや CLI 利用者が失敗原因を即座に把握できるようにしています。`koto_compile --explain <code>` を実行すると対応するヒントが表示されます。

| コード | 説明 | 典型的な対処 |
|-------|------|--------------|
| `E0001` | IVM のジャンプエンコーディングで到達できない分岐先です。 | 巨大な関数を分割するかインライン展開を減らし、基本ブロック間距離を ±1 MiB 以内に収めます。 |
| `E0002` | 呼び出し箇所が定義されていない関数を参照しています。 | スペルミス、可視性修飾子、フィーチャフラグの影響を確認してください。 |
| `E0003` | ABI v1 を有効にせずに耐久状態システムコールを生成しました。 | `CompilerOptions::abi_version = 1` を設定するか、`seiyaku` 内の `meta { abi_version: 1 }` を追加します。 |
| `E0004` | 資産関連のシステムコールにリテラルではないポインターが渡されました。 | `account_id(...)`、`asset_definition(...)` などを使用するか、ホストの既定値に 0 のセンチネルを渡します。 |
| `E0005` | `for` ループの初期化部が現在サポートされる範囲を超えています。 | 複雑なセットアップはループ手前に移動し、シンプルな `let`／式のみを初期化に残してください。 |
| `E0006` | `for` ループのステップ節が現在サポートされる範囲を超えています。 | ループカウンタの更新を `i = i + 1` などの単純な式に変更してください。 |
