<!-- Japanese translation of docs/source/runtime_upgrades.md -->

---
lang: ja
direction: ltr
source: docs/source/runtime_upgrades.md
status: complete
translator: manual
---

# ランタイムアップグレード（IVM + ホスト）— 無停止・ノーハードフォーク

本ドキュメントは、ネットワーク停止やハードフォークなしに IVM/ホスト機能（新しいシステムコールやポインター ABI 型など）を導入するための決定論的でガバナンス制御の仕組みを定義します。ノードは事前にバイナリを展開し、オンチェーンでのアクティベーションをブロック高さのウィンドウ内で調整します。既存コントラクトはそのまま動作し、新機能は ABI バージョンとポリシーでゲートされます。

## ゴール
- スケジュールされた高さウィンドウでの決定論的アクティベーションと冪等適用。
- 複数 ABI バージョンの共存。既存バイナリを壊さない。
- アクティベーション前に新機能を有効化させない受理・実行ガードレール。
- 機能可視化と明確な失敗モードを備えた運用者向けロールアウト。

## 非ゴール
- 既存システムコール番号やポインター型 ID の変更（禁止）。
- バイナリアップデート無しでノードをライブパッチすること。

## 定義
- **ABI バージョン**: `ProgramMetadata.abi_version` に宣言される小整数。`SyscallPolicy` とポインター型の許可リストを選択。
- **ABI ハッシュ**: 指定バージョンの ABI 表面（システムコール一覧、ポインター型 ID/許可リスト、ポリシーフラグ）を `ivm::syscalls::compute_abi_hash` で計算した決定論的ダイジェスト。
- **システムコールポリシー**: 指定 ABI バージョンおよびホストポリシーで、システムコール番号の許可／不許可を判断するマッピング。
- **アクティベーションウィンドウ**: 半開区間 `[start, end)`。`start` で一度だけアクティベーションが有効。

## ステートオブジェクト（データモデル）
- `RuntimeUpgradeId`: マニフェスト正規 Norito バイト列の Blake2b-256。
- `RuntimeUpgradeManifest`:
  - `name: String`
  - `description: String`
  - `abi_version: u16`
  - `abi_hash: [u8; 32]`
  - `added_syscalls: Vec<u16>`
  - `added_pointer_types: Vec<u16>`
  - `start_height: u64`
  - `end_height: u64`
- `RuntimeUpgradeRecord`:
  - `manifest: RuntimeUpgradeManifest`
  - `status: RuntimeUpgradeStatus`
  - `proposer: AccountId`
  - `created_height: u64`

**不変条件**: `end_height > start_height`、`abi_version` は既存より大きく、`added_*` は既存集合と不交差。既存番号/ID の削除・再割り当ては禁止。

## ストレージレイアウト
`world.runtime_upgrades` に `RuntimeUpgradeId` をキーとした MVCC マップとして格納。Norito でエンコードされた `RuntimeUpgradeRecord` を持ち、リプレイに安全。

## 命令（ISI）
- `ProposeRuntimeUpgrade { manifest }`
  - 効果: `RuntimeUpgradeRecord{ status=Proposed }` を挿入（存在する場合は不変）。
  - ウィンドウ重複や不変条件違反を拒否。正規バイト列のみ受理。
- `ActivateRuntimeUpgrade { id }`
  - 前提: Proposed 記録が存在し、`current_height == start_height`、かつ `current_height < end_height`。
  - 効果: `ActivatedAt(current_height)` に更新し、`abi_version` をアクティブ集合へ追加。
  - 冪等: 同高さのリプレイは無操作。異なる高さは拒否。
- `CancelRuntimeUpgrade { id }`
  - 前提: Proposed 状態で `current_height < start_height`。
  - 効果: ステータスを `Canceled` に変更。

## イベント
`RuntimeUpgradeEvent::{Proposed, Activated, Canceled}`。

## 受理ルール
- コントラクト受理: `ProgramMetadata.abi_version = v` のマニフェストは、アクティベーション前なら `AbiVersionNotActive` で拒否。アクティベーション後は計算した `abi_hash` と一致しなければ `ManifestAbiHashMismatch`。
- トランザクション受理: 各命令は適切な権限（root/sudo）とウィンドウ不重複性のチェックが必要。

## 実行ルール
- VM ホストポリシー: `ProgramMetadata.abi_version` に基づき `SyscallPolicy` を選択。許可外のシステムコールは `VMError::UnknownSyscall`。
- ポインター ABI: バージョンごとの許可リストで TLV を検証。
- ホスト切替: 各ブロックでアクティブ ABI 集合を再計算。アクティベーション後の同一ブロック内トランザクションも新ポリシーを観測します。

## 決定論と安全性
- アクティベーションは `start_height` で一度のみ。リオーガも決定論的に再適用される。
- 既存 ABI バージョンは継続して有効。新バージョンは集合に追加。
- 合意や実行順序に影響する動的交渉は存在しない。

## 運用ロールアウト
1. 新 ABI (`v+1`) をサポートするバイナリを事前配布。
2. テレメトリで対応率を確認。
3. `ProposeRuntimeUpgrade` を十分先のウィンドウで提出。
4. `start_height` でアクティベート。旧バイナリは従来 ABI のみ受理・実行。
5. アクティベーション後に `v+1` 対応コントラクトを再コンパイル／デプロイ。

## Torii & CLI
- Torii: `GET /v1/runtime/abi/active`, `GET /v1/runtime/abi/hash`, `GET /v1/runtime/upgrades`, `POST /v1/runtime/upgrades/{propose|activate|cancel}`。
- CLI: `iroha runtime abi active/hash`, `iroha runtime upgrade {list, propose, activate, cancel}`。

## コアクエリ API
`FindActiveAbiVersions` が `{ active_versions, default_compile_target }` を返却。サンプルは `docs/source/samples/find_active_abi_versions.md`。

## 必要なコード変更
- `iroha_data_model`: 型／命令／イベントと Norito/JSON コーデック、往復テスト。
- `iroha_core`: レジストリ追加、ハンドラー実装、受理ゲート、テスト。
- `ivm`: 新 ABI 定義とポリシー、ゴールデンテスト再生成。
- `iroha_cli` / `iroha_torii`: エンドポイント／コマンド追加。
- Kotodama コンパイラ: 新 ABI バージョンと `abi_hash` をサポート。

## テレメトリ
`runtime.active_abi_versions` ゲージと `runtime.upgrade_events_total{kind}` カウンタを追加。

## セキュリティ
- root/sudo のみが命令を実行可能。マニフェストは署名必須。
- アクティベーションウィンドウが先行実行を防ぎ、アクティベーションを決定論的に。
- `abi_hash` がインターフェースを固定し、バイナリ差異を吸収。

## 受け入れ基準
- アクティベーション前は `v+1` コードを拒否。
- アクティベーション後は `v+1` を受理・実行し、旧プログラムは影響なし。
- ABI ハッシュ／システムコール一覧のゴールデンテストが x86-64/ARM64 で成功。
- アクティベーションが冪等であり、リオーガ下で安全。 
