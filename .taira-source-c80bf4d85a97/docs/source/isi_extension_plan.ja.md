<!-- Auto-generated stub for Japanese (ja) translation. Replace this content with the full translation. -->

---
lang: ja
direction: ltr
source: docs/source/isi_extension_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 9648381ac7cc1716ffd3c48aca425ed17a6afe1ac73bdeff866ebbbd9147cf68
source_last_modified: "2026-03-30T18:22:55.972718+00:00"
translation_last_reviewed: 2026-04-02
translator: machine-google-reviewed
---

# ISI 拡張プラン (v1)

このメモは、新しい Iroha の特別な指示とキャプチャの優先順位を決定します。
実装前の各命令に対する交渉不可能な不変条件。順序が一致している
セキュリティと操作性のリスクが第一、UX スループットが二番目です。

## 優先スタック

1. **RotateAccountSignatory** – 破壊的な移行を行わずに、衛生的なキーのローテーションを行うために必要です。
2. **DeactivateContractInstance** / **RemoveSmartContractBytes** – 確定的なコントラクトを提供します
   侵害された展開に対するキルスイッチとストレージの再利用。
3. **SetAssetKeyValue** / **RemoveAssetKeyValue** – メタデータ パリティを具体的なアセットに拡張します
   可観測性ツールが保有物にタグを付けることができるようにバランスを調整します。
4. **BatchMintAsset** / **BatchTransferAsset** – ペイロード サイズを維持するための確定的なファンアウト ヘルパー
   VM フォールバック プレッシャーは管理可能です。

## 命令の不変条件

### SetAssetKeyValue / RemoveAssetKeyValue
- `AssetMetadataKey` 名前空間 (`state.rs`) を再利用して、正規の WSV キーを安定させます。
- JSON サイズとスキーマ制限をアカウント メタデータ ヘルパーと同様に適用します。
- 影響を受ける `AssetId` とともに `AssetEvent::MetadataInserted` / `AssetEvent::MetadataRemoved` を発行します。
- 既存のアセット メタデータ編集と同じ権限トークンが必要です (定義所有者または
  `CanModifyAssetMetadata` スタイルの許可)。
- アセット レコードが見つからない場合 (暗黙的な作成がない場合) は中止されます。### RotateAccountSignatory
- アカウントのメタデータとリンクを保持しながら、`AccountId` での署名者のアトミック スワップ
  リソース (アセット、トリガー、ロール、権限、保留中のイベント)。
- 現在の署名者が呼び出し元 (または明示的なトークンを介して委任された権限) と一致することを確認します。
- 新しい公開キーがすでに別の正規アカウントを裏付けている場合は拒否します。
- アカウント ID を埋め込むすべての正規キーを更新し、コミット前にキャッシュを無効にします。
- 監査証跡用に新旧キーを使用して専用の `AccountEvent::SignatoryRotated` を発行します。
- 移行足場: `AccountAlias` + `AccountRekeyRecord` (`account::rekey` を参照) に依存するため
  既存のアカウントは、ハッシュを中断することなく、ローリング アップグレード中に安定したエイリアス バインディングを維持できます。

### DeactivateContractInstance
- 来歴データを保持しながら、`(namespace, contract_id)` バインディングを削除または廃棄する
  (誰が、いつ、理由コード) トラブルシューティング用。
- アクティベーションと同じガバナンス権限セットが必要で、ポリシーフックを使用して禁止します
  高い承認を得ずにコア システムの名前空間を非アクティブ化する。
- イベント ログを確定的に保つために、インスタンスがすでに非アクティブである場合は拒否します。
- ダウンストリーム ウォッチャーが消費できる `ContractInstanceEvent::Deactivated` を発行します。### RemoveSmartContractBytes
- マニフェストまたはアクティブなインスタンスがない場合にのみ、`code_hash` による保存されたバイトコードのプルーニングを許可します。
  成果物を参照します。それ以外の場合は、説明的なエラーが発生して失敗します。
- 許可ゲートミラー登録 (`CanRegisterSmartContractCode`) とオペレーターレベル
  ガード (例: `CanManageSmartContractStorage`)。
- 削除の直前に、提供された `code_hash` が保存されている本文ダイジェストと一致することを確認してください。
  古くなったハンドル。
- ハッシュと呼び出し元のメタデータを含む `ContractCodeEvent::Removed` を送信します。

### BatchMintAsset / BatchTransferAsset
- 全か無かのセマンティクス: すべてのタプルが成功するか、サイドなしで命令が中止されるかのどちらかです
  効果。
- 入力ベクトルは決定論的に順序付けされ (暗黙的な並べ替えなし)、構成によって制限される必要があります。
  (`max_batch_isi_items`)。
- 品目ごとの資産イベントを発行して、下流の会計処理の一貫性を維持します。バッチ コンテキストは追加的です。
  代替品ではありません。
- 権限チェックでは、ターゲットごとに既存の単一項目ロジックを再利用します (アセット所有者、定義所有者、
  または付与された機能) 状態の突然変異の前。
- アドバイザリー アクセス セットは、オプティミスティック同時実行性を正しく保つために、すべての読み取り/書き込みキーを結合する必要があります。

## 実装の足場- データ モデルには、バランス メタデータ用の `SetAssetKeyValue` / `RemoveAssetKeyValue` スキャフォールドが含まれるようになりました。
  編集 (`transparent.rs`)。
- エグゼキューターの訪問者は、ホスト配線が確立されるとアクセス許可をゲートするプレースホルダーを公開します
  (`default/mod.rs`)。
- プロトタイプのキー再生成 (`account::rekey`) は、ローリング移行のためのランディング ゾーンを提供します。
- ワールド状態には、`AccountAlias` をキーとする `account_rekey_records` が含まれるため、エイリアスをステージングできます →
  過去の `AccountId` エンコーディングに触れることなく、署名者の移行を行うことができます。

## IVM システムコールの製図

- `DeactivateContractInstance` / `RemoveSmartContractBytes` 用のホスト シムは次のように出荷されます。
  `SYSCALL_DEACTIVATE_CONTRACT_INSTANCE` (0x43) および
  `SYSCALL_REMOVE_SMART_CONTRACT_BYTES` (0x44)、どちらもミラーリングする Norito TLV を消費します。
  正規の ISI 構造体。
- ホスト ハンドラーが `iroha_core` 実行パスをミラーリングして保持した後のみ、`abi_syscall_list()` を拡張します。
  ABI ハッシュは開発中安定しています。
- システムコール番号が安定したら、Kotodama を低く更新します。拡張されたものにゴールデン カバレッジを追加する
  同時に表面化します。

## ステータス

上記の順序付けと不変条件を実装する準備ができました。フォローアップブランチは参照する必要があります
実行パスとシステムコールの公開を配線する場合は、このドキュメントを参照してください。