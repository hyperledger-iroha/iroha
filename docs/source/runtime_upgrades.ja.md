---
lang: ja
direction: ltr
source: docs/source/runtime_upgrades.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: ee7142a82f21e646d4d71844adbf779d180e5647
source_last_modified: "2025-12-04T06:31:08.260928+00:00"
translation_last_reviewed: 2026-01-01
---

# ランタイムアップグレード (IVM + Host) — 無停止・ハードフォーク無し

本ドキュメントは、ネットワーク停止やハードフォーク無しでランタイムアップグレードを展開するための、
決定論的かつガバナンス制御の仕組みを規定します。ノードは事前にバイナリを展開し、アクティベーションは
高さウィンドウ内でオンチェーン調整されます。既存コントラクトは変更なしで動作し、ホスト ABI サーフェスは v1 に固定されます。

注記(初回リリース): ABI v1 は固定で、ABI バージョンの増加は予定していません。runtime upgrade manifest は `abi_version = 1` を設定し、`added_syscalls`/`added_pointer_types` は空である必要があります。

目的
- 予定された高さウィンドウでの決定論的アクティベーション (冪等適用)。
- ABI v1 の安定性を維持し、ランタイムアップグレードはホスト ABI サーフェスを変更しない。
- 事前アクティベーションの payload が新挙動を有効化できない入場/実行ガード。
- 機能可視性と明確な失敗モードを備えた運用フレンドリーなロールアウト。

非目的
- 新しい ABI バージョンの導入や syscall/pointer 型サーフェスの拡張 (このリリースの対象外)。
- 既存の syscall 番号や pointer 型 ID の変更 (禁止)。
- 更新済みバイナリを配布せずにノードをライブパッチすること。

定義
- ABI Version: `ProgramMetadata.abi_version` に宣言される小さな整数。`SyscallPolicy` と pointer 型の allowlist を選択する。初回リリースでは `1` に固定。
- ABI Hash: あるバージョンの ABI サーフェス (syscall リスト/番号/形状、pointer 型 ID/allowlist、ポリシーフラグ) の決定論的 digest。`ivm::syscalls::compute_abi_hash` で算出。
- Syscall Policy: ABI バージョンと host ポリシーに対して syscall 番号の許可/禁止を判断する host マッピング。
- Activation Window: ブロック高の半開区間 `[start, end)` で、`start` で一度だけ有効化される。

状態オブジェクト (データモデル)
<!-- BEGIN RUNTIME UPGRADE TYPES -->
- `RuntimeUpgradeId`: manifest の canonical Norito bytes の Blake2b-256。
- `RuntimeUpgradeManifest` フィールド:
  - `name: String` — 人が読めるラベル。
  - `description: String` — 運用向けの短い説明。
  - `abi_version: u16` — 有効化する対象 ABI バージョン (初回リリースでは 1 固定)。
  - `abi_hash: [u8; 32]` — 対象ポリシーの canonical ABI hash。
  - `added_syscalls: Vec<u16>` — このバージョンで有効になる syscall 番号。
  - `added_pointer_types: Vec<u16>` — アップグレードで追加される pointer 型 ID。
  - `start_height: u64` — アクティベーションが許可される最初のブロック高。
  - `end_height: u64` — アクティベーションウィンドウの排他的上限。
  - `sbom_digests: Vec<RuntimeUpgradeSbomDigest>` — アップグレード成果物の SBOM digest。
  - `slsa_attestation: Vec<u8>` — SLSA attestation 生バイト (JSON では base64)。
  - `provenance: Vec<ManifestProvenance>` — canonical payload への署名。
- `RuntimeUpgradeRecord` フィールド:
  - `manifest: RuntimeUpgradeManifest` — canonical 提案 payload。
  - `status: RuntimeUpgradeStatus` — 提案のライフサイクル状態。
  - `proposer: AccountId` — 提案を提出した権限主体。
  - `created_height: u64` — 提案が台帳に入ったブロック高。
- `RuntimeUpgradeSbomDigest` フィールド:
  - `algorithm: String` — digest アルゴリズム ID。
  - `digest: Vec<u8>` — digest 生バイト (JSON では base64)。
<!-- END RUNTIME UPGRADE TYPES -->
  - 不変条件: `end_height > start_height`; `abi_version` は `1` であること; `abi_hash` は `ivm::syscalls::compute_abi_hash(ivm::SyscallPolicy::AbiV1)` と一致すること; `added_*` は空であること; 既存の番号/ID を削除または再番号付けしてはならない。

ストレージレイアウト
- `world.runtime_upgrades`: `RuntimeUpgradeId.0` (生の 32 バイトハッシュ) をキーとする MVCC マップ。値は canonical Norito の `RuntimeUpgradeRecord` payload。エントリはブロックを跨いで保持され、commit は冪等で replay 安全。

命令 (ISI)
- ProposeRuntimeUpgrade { manifest: RuntimeUpgradeManifest }
  - 効果: 既存でなければ `RuntimeUpgradeId` をキーに `RuntimeUpgradeRecord { status: Proposed }` を挿入。
  - 提案/有効化済み record とウィンドウが重なる場合、または不変条件が破られる場合は拒否。
  - 冪等: 同一 canonical manifest bytes の再送は no-op。
  - Canonical encoding: manifest bytes は `RuntimeUpgradeManifest::canonical_bytes()` と一致する必要があり、非 canonical は拒否。
- ActivateRuntimeUpgrade { id: RuntimeUpgradeId }
  - 事前条件: 対応する Proposed record が存在; `current_height` は `manifest.start_height` に一致; `current_height < manifest.end_height`。
  - 効果: record を `ActivatedAt(current_height)` に更新し、アクティブ ABI セットは初回リリースでは `{1}` のまま。
  - 冪等: 同じ高さでの replay は no-op。他の高さは決定論的に拒否。
- CancelRuntimeUpgrade { id: RuntimeUpgradeId }
  - 事前条件: status が Proposed かつ `current_height < manifest.start_height`。
  - 効果: `Canceled` に更新。

イベント (Data Events)
- RuntimeUpgradeEvent::{Proposed { id, manifest }, Activated { id, abi_version, at_height }, Canceled { id }}

入場ルール
- コントラクト入場: 初回リリースでは `ProgramMetadata.abi_version = 1` のみ許可され、他の値は `IvmAdmissionError::UnsupportedAbiVersion` で拒否。
  - ABI v1 の場合は `abi_hash(1)` を再計算し payload/manifest と一致させる。不一致は `IvmAdmissionError::ManifestAbiHashMismatch`。
- トランザクション入場: `ProposeRuntimeUpgrade`/`ActivateRuntimeUpgrade`/`CancelRuntimeUpgrade` は適切な権限 (root/sudo) が必要。ウィンドウ重複制約を満たすこと。

プロベナンス強制
- Runtime-upgrade manifest は SBOM digest (`sbom_digests`)、SLSA attestation bytes (`slsa_attestation`)、署名メタデータ (`provenance` signatures) を持てる。署名は canonical `RuntimeUpgradeManifestSignaturePayload` (manifest の全フィールド、`provenance` の署名リストを除く) を対象とする。
- ガバナンス設定 `governance.runtime_upgrade_provenance` で強制:
  - `mode`: `optional` (欠落を許可、存在時は検証) / `required` (欠落時は拒否)。
  - `require_sbom`: `true` の場合 SBOM digest が最低 1 つ必要。
  - `require_slsa`: `true` の場合 SLSA attestation が非空で必要。
  - `trusted_signers`: 承認済み署名者の公開鍵リスト。
  - `signature_threshold`: 必要な信頼署名の最小数。
- プロベナンス拒否は命令失敗に安定コードを付与 (prefix `runtime_upgrade_provenance:`):
  - `missing_provenance`, `missing_sbom`, `invalid_sbom_digest`, `missing_slsa_attestation`
  - `missing_signatures`, `invalid_signature`, `untrusted_signer`, `signature_threshold_not_met`
- テレメトリ: `runtime_upgrade_provenance_rejections_total{reason}` が拒否理由を集計。

実行ルール
- VM Host Policy: 実行時に `ProgramMetadata.abi_version` から `SyscallPolicy` を導出。未知の syscall は `VMError::UnknownSyscall`。
- Pointer-ABI: `ProgramMetadata.abi_version` に由来する allowlist。該当バージョンの allowlist 外型は decode/validation で拒否。
- Host Switching: 各ブロックでアクティブ ABI セットを再計算。初回リリースでは `{1}` のままだが、アクティベーションは記録され冪等 ( `runtime_upgrade_admission::activation_allows_v1_in_same_block` で検証)。
  - Syscall policy binding: `CoreHost` はトランザクションの ABI バージョンを読み、`ivm::syscalls::is_syscall_allowed`/`is_type_allowed_for_policy` をブロック単位 `SyscallPolicy` に適用。ホストはトランザクション単位の VM インスタンスを再利用するため、ブロック途中の有効化は安全。後続トランザクションは更新ポリシーを観測し、先行トランザクションは元のバージョンのまま。

決定論性と安全性の不変条件
- アクティベーションは `start_height` のみで発生し冪等。`start_height` より下の reorg はブロックが再着地した際に決定論的に再適用。
- アクティブ ABI セットは初回リリースでは `{1}` に固定。
- 動的交渉はコンセンサスや実行順序に影響しない。能力 gossip は情報提供のみ。

運用ロールアウト (無停止)
1) ABI v1 を維持したまま、新しい runtime 成果物を含むノードバイナリを配布。
2) テレメトリでフリートの準備状況を確認。
3) 十分に先のウィンドウ (例: `H+N`) で `ProposeRuntimeUpgrade` を提出。
4) `start_height` で `ActivateRuntimeUpgrade` がブロックに含まれて実行され、アクティベーションを記録。ABI は v1 のまま。

Torii と CLI
- Torii
  - `GET /v2/runtime/abi/active` -> `{ active_versions: [u16], default_compile_target: u16 }` (implemented)
  - `GET /v2/runtime/abi/hash` -> `{ policy: "V1", abi_hash_hex: "<64-hex>" }` (implemented)
  - `GET /v2/runtime/upgrades` -> record 一覧 (implemented)。
  - `POST /v2/runtime/upgrades/propose` -> `ProposeRuntimeUpgrade` をラップ (instruction skeleton を返す; implemented)。
  - `POST /v2/runtime/upgrades/activate/:id` -> `ActivateRuntimeUpgrade` をラップ (instruction skeleton を返す; implemented)。
  - `POST /v2/runtime/upgrades/cancel/:id` -> `CancelRuntimeUpgrade` をラップ (instruction skeleton を返す; implemented)。
- CLI
  - `iroha runtime abi active` (implemented)
  - `iroha runtime abi hash` (implemented)
  - `iroha runtime upgrade list` (implemented)
  - `iroha runtime upgrade propose --file <manifest.json>` (implemented)
  - `iroha runtime upgrade activate --id <id>` (implemented)
  - `iroha runtime upgrade cancel --id <id>` (implemented)

Core Query API
- Norito 単発クエリ (署名付き):
  - `FindActiveAbiVersions` は Norito 構造体 `{ active_versions: [u16], default_compile_target: u16 }` を返す。
  - サンプル: `docs/source/samples/find_active_abi_versions.md` (型/フィールドと JSON 例)。

実装ノート (v1 固定)
- iroha_data_model
  - `RuntimeUpgradeManifest`, `RuntimeUpgradeRecord`, 命令 enum, イベント, JSON/Norito codec と roundtrip テストを追加。
- iroha_core
  - WSV: `runtime_upgrades` レジストリと重複チェック/ゲッターを追加。
  - Executors: ISI handlers を実装; イベントを発行; 入場ルールを適用。
  - Admission: `abi_version` の有効性と `abi_hash` 一致でプログラム manifest をゲート。
  - Syscall policy mapping: アクティブ ABI セットを VM host コンストラクタに渡し、実行開始時のブロック高を用いて決定論性を担保。
  - Tests: アクティベーションウィンドウの冪等性、重複拒否、入場の pre/post 振る舞い。
- ivm
  - ABI サーフェスは v1 に固定; syscall リストと ABI hash は golden tests で固定。
- iroha_cli / iroha_torii
  - 上記エンドポイント/コマンドを追加; manifest 用 Norito JSON helper; 基本的な統合テスト。
- Kotodama compiler
  - `abi_version = 1` を出力し、canonical v1 の `abi_hash` を `.to` manifest に埋め込む。

Telemetry
- `runtime.active_abi_versions` gauge と `runtime.upgrade_events_total{kind}` counter を追加。

Security Considerations
- root/sudo のみが propose/activate/cancel 可能; manifest は適切に署名されていること。
- アクティベーションウィンドウが front-running を防ぎ、決定論的適用を保証。
- `abi_hash` が interface surface を固定し、バイナリ間のサイレント drift を防止。

Acceptance Criteria (Conformance)
- `abi_version != 1` のコードは常に決定論的に拒否。
- ランタイムアップグレードは ABI ポリシーを変更しない; 既存プログラムは v1 で無変更のまま動作。
- ABI hash と syscall リストの golden tests が x86-64/ARM64 で通る。
- アクティベーションは冪等で reorg に対して安全。
