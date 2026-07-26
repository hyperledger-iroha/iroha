<!-- Japanese translation of docs/source/error_mapping.md -->

---
lang: ja
direction: ltr
source: docs/source/error_mapping.md
status: complete
translator: manual
---

# エラーマッピングガイド

最終更新: 2025-08-21

本ガイドは、Iroha で発生しがちな失敗パターンをデータモデルが公開する安定したエラーカテゴリへ対応付けます。テスト設計やクライアント側のエラーハンドリングを予測可能にするために活用してください。

原則
- 命令系とクエリ系はいずれも構造化した列挙型を返します。パニックは避け、可能な限り具体的なカテゴリを報告します。
- カテゴリは安定し、メッセージ本文は進化し得ます。クライアントは自由形式の文字列ではなく、列挙型バリアントで分岐してください。

カテゴリ
- `InstructionExecutionError::Find`: エンティティ欠如（資産、アカウント、ドメイン、NFT、ロール、トリガー、権限、公開鍵、ブロック、トランザクション）。例: 存在しないメタデータキーの削除で `Find(MetadataKey)`。
- `InstructionExecutionError::Repetition`: 登録の重複や ID 競合。命令種別と重複した `IdBox` を含みます。
- `InstructionExecutionError::Mintability`: 発行回数の不変条件違反（`Once` 超過、`Limited(n)` 超過、`Infinitely` 禁止など）。例: `Once` 定義の資産を 2 度鋳造すると `Mintability(MintUnmintable)`、`Limited(0)` を設定すると `Mintability(InvalidMintabilityTokens)`。
- `InstructionExecutionError::Math`: 数値領域のエラー（オーバーフロー、ゼロ除算、負値、数量不足）。例: 保有量を超えてバーンすると `Math(NotEnoughQuantity)`。
- `InstructionExecutionError::InvalidParameter`: 命令パラメータや構成が不正（例: 過去時刻のタイムトリガー）。契約ペイロードが不正な場合にも使用します。
- `InstructionExecutionError::Evaluate`: DSL／仕様と命令の形や型が一致しない。例: 資産値の数値仕様が誤っていると `Evaluate(Type(AssetNumericSpec(..)))`。
- `InstructionExecutionError::InvariantViolation`: 他カテゴリで表現できないシステム不変条件の違反。例: 最後の署名者を削除しようとするケース。
- `InstructionExecutionError::Query`: 命令実行中にクエリが失敗した際の `QueryExecutionFail` ラップ。

`QueryExecutionFail`
- `Find`: クエリコンテキスト内のエンティティ欠如。
- `Conversion`: クエリで期待された型と一致しない。
- `NotFound`: ライブクエリカーソルが存在しない。
- `CursorMismatch` / `CursorDone`: カーソルプロトコルの不整合。
- `FetchSizeTooBig`: サーバが設定した上限を超過。
- `GasBudgetExceeded`: クエリ実行のガス／マテリアライズ予算を超過。
- `InvalidSingularParameters`: 単一クエリでサポートされないパラメータ。
- `CapacityLimit`: ライブクエリストアの容量に到達。

テストのヒント
- エラー発生源に近いレイヤでユニットテストを書くことを推奨します。例えば資産数値仕様の不一致はデータモデルのテストで再現できます。
- 代表的なケース（重複登録、削除対象の欠如、権限なしの転送など）は統合テストでエンドツーエンドのマッピングを確認します。
- 列挙型バリアントにマッチさせ、メッセージ文字列への依存を避けることでアサーションの頑健性を保ってください。
