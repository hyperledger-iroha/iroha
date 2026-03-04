<!-- Japanese translation of docs/source/isi_extension_plan.md -->

---
lang: ja
direction: ltr
source: docs/source/isi_extension_plan.md
status: complete
translator: manual
---

# ISI 拡張計画 (v1)

このメモでは、新しい Iroha Special Instructions の優先順位を確定し、実装前に各命令で守るべき必須インバリアントをまとめます。優先順はまずセキュリティと運用リスク、次に UX スループットの順です。

## 優先スタック

1. **RotateAccountSignatory** – 破壊的な移行を伴わない衛生的な鍵ローテーションに必須。
2. **DeactivateContractInstance** / **RemoveSmartContractBytes** – 被害を受けたデプロイメントに対し、決定的なキルスイッチとストレージ再利用を提供。
3. **SetAssetKeyValue** / **RemoveAssetKeyValue** – 実際のアセット残高にメタデータパリティを拡張し、可観測性ツールが保有資産をタグ付けできるようにする。
4. **BatchMintAsset** / **BatchTransferAsset** – 決定的なファンアウト支援でペイロードサイズと VM フォールバック負荷を抑える。

## 命令インバリアント

### SetAssetKeyValue / RemoveAssetKeyValue
- `AssetMetadataKey` 名前空間（`state.rs`）を再利用し、WSV の正準キーを安定させる。
- アカウントメタデータと同じ JSON サイズとスキーマ制約を適用する。
- 影響した `AssetId` とともに `AssetEvent::MetadataInserted` / `AssetEvent::MetadataRemoved` を発行する。
- 既存のアセットメタデータ編集と同じパーミッション（定義オーナーまたは `CanModifyAssetMetadata` 相当の権限）を要求する。
- アセットレコードが存在しない場合は即座に中止（暗黙作成は禁止）。

### RotateAccountSignatory
- アカウントメタデータや関連リソース（アセット、トリガ、ロール、パーミッション、保留イベント）を保持したまま `AccountId` の署名者をアトミックに入れ替える。
- 現在の署名者が呼び出し元に一致すること（または権限トークンによる委譲）を検証する。
- 新しい公開鍵が同一ドメイン内の別アカウントで既に使われていないかチェックする。
- アカウント ID を埋め込む正準キーをすべて更新し、コミット前にキャッシュを無効化する。
- 監査用に古い鍵と新しい鍵を含む `AccountEvent::SignatoryRotated` を発行する。
- 移行足場: ローリングアップグレードでハッシュを壊さずに安定ラベルへ写像できるよう、`AccountLabel` と `AccountRekeyRecord`（`account::rekey` を参照）を導入する。

### DeactivateContractInstance
- 監査用の情報（実行者、日時、理由コード）を保持しつつ、`(namespace, contract_id)` のバインディングを削除またはトゥームストーン化する。
- アクティベーションと同じガバナンスパーミッションを要求し、コアシステムネームスペースの無効化には昇格承認が必要となるようポリシーフックを用意する。
- 既に非アクティブなインスタンスに対しては拒否し、イベントログの決定性を保つ。
- 下流ウォッチャが消費できる `ContractInstanceEvent::Deactivated` を発行する。

### RemoveSmartContractBytes
- マニフェストやアクティブインスタンスが参照していない場合に限り、`code_hash` をキーに保存済みバイトコードを削除可能とする。それ以外は説明的なエラーで失敗する。
- パーミッションは登録時 (`CanRegisterSmartContractCode`) と同水準で、さらにオペレータ向けガード（例 `CanManageSmartContractStorage`）を追加する。
- 削除直前に提出された `code_hash` が保存されている本体のダイジェストと一致することを検証し、陳腐化したハンドルを防ぐ。
- ハッシュと呼び出し元メタデータを含む `ContractCodeEvent::Removed` を発行する。

### BatchMintAsset / BatchTransferAsset
- オールオアナッシングのセマンティクス: すべてのタプルが成功するか、命令全体が副作用なく失敗する。
- 入力ベクタは決定的順序（暗黙ソート不可）で、設定値（`max_batch_isi_items`）による上限を守る。
- 下流アカウンティングを保つため、アイテムごとにアセットイベントを発行する（バッチコンテキストは付加情報であり置き換えではない）。
- 状態変更前に、各ターゲットで既存の単体ロジックと同じパーミッションチェック（アセットオーナー、定義オーナー、または権限付与）を再利用する。
- 楽観的同時実行性を保つため、アドバイザリアクセスセットには読取り／書き込みキーをすべて統合する。

## 実装足場

- データモデルには、残高メタデータ編集向けの `SetAssetKeyValue` / `RemoveAssetKeyValue` スケルトン（`transparent.rs`）が追加済み。
- エグゼキュータ訪問者はホスト配線が揃った際にパーミッションを制御するプレースホルダを公開済み（`default/mod.rs`）。
- リキー用プロトタイプ（`account::rekey`）がローリング移行の受け皿を提供。
- ワールドステートには `AccountLabel` をキーにした `account_rekey_records` があり、歴史的な `AccountId` エンコーディングに触れずラベル→署名者移行を段階的に進められる。

## IVM Syscall 起草

- IVM に `DeactivateContractInstance` / `RemoveSmartContractBytes` を提供するホストシムは、`SYSCALL_DEACTIVATE_CONTRACT_INSTANCE` (0x43) と `SYSCALL_REMOVE_SMART_CONTRACT_BYTES` (0x44) として公開され、Norito TLV で ISI 構造体のエンコードを受け付ける。
- ホストハンドラが `iroha_core` の実行経路に揃うまでは `abi_syscall_list()` を拡張せず、開発中の ABI ハッシュを安定させる。
- syscall 番号が固まったら Kotodama のローアリングを更新し、拡張されたインターフェース向けのゴールデンテストを同時に追加する。

## ステータス

上記の優先順とインバリアントは実装の準備が整っています。実装ブランチでは、本書を参照しながら実行経路と syscall 公開を配線してください。
