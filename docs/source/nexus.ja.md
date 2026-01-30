<!-- Japanese translation of docs/source/nexus.md -->

---
lang: ja
direction: ltr
source: docs/source/nexus.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: c8da33b0abb8a6d46dbaaed657c8338a9d723a97f6f28ff29a62caf84c0dbfd6
source_last_modified: "2025-12-27T07:56:34.355655+00:00"
translation_last_reviewed: 2026-01-01
---

#! Iroha 3 - Sora Nexus Ledger: 技術設計仕様

本ドキュメントは、Iroha 3 向けの Sora Nexus Ledger アーキテクチャを提案する。Iroha 2 を、Data Spaces (DS) を中心に組織された単一のグローバルかつ論理的に統合されたレジャーへ進化させることが目的である。Data Spaces は強力なプライバシードメイン（"private data spaces"）とオープン参加（"public data spaces"）を提供する。この設計はグローバルレジャー全体のコンポーザビリティを維持しつつ、プライベート DS データの厳格な分離と機密性を保証し、Kura（ブロックストレージ）と WSV（World State View）での erasure coding によるデータ可用性スケーリングを導入する。

同一リポジトリで Iroha 2（セルフホストネットワーク）と Iroha 3（SORA Nexus）をビルドする。実行は共通の Iroha Virtual Machine (IVM) と Kotodama ツールチェーンによって支えられるため、コントラクトとバイトコードアーティファクトはセルフホスト展開と Nexus グローバルレジャー間で可搬性を保つ。

目標
- 多数の協調バリデータと Data Spaces から構成される単一のグローバル論理レジャー。
- パーミッションド運用（例: CBDC）向けの Private Data Spaces。データは DS 外へ出ない。
- オープン参加の Public Data Spaces。Ethereum 風の permissionless アクセス。
- Data Spaces 間でコンポーザブルなスマートコントラクト（private DS 資産へのアクセスは明示許可が必要）。
- 公開アクティビティが private DS の内部トランザクション性能を損ねない性能分離。
- 大規模データ可用性: erasure-coded Kura と WSV により実質無制限のデータを支えつつ private DS を保護。

非目標（初期フェーズ）
- トークン経済やバリデータインセンティブの定義（スケジューリングとステーキングはプラガブル）。
- 新しい ABI 版の導入や syscalls / pointer-ABI の拡張。ABI v1 は固定で runtime upgrades はホスト ABI を変更しない。

用語
- Nexus Ledger: Data Space (DS) ブロックを単一の順序付き履歴と状態コミットメントに合成したグローバル論理レジャー。
- Data Space (DS): 独自のバリデータ、ガバナンス、プライバシークラス、DA ポリシー、クォータ、fee ポリシーを持つ実行・ストレージドメイン。public と private の 2 クラスがある。
- Private Data Space: パーミッションドバリデータとアクセス制御。トランザクションデータと状態は DS 外に出ない。グローバルには commitments/metadata のみアンカーされる。
- Public Data Space: Permissionless 参加。データと状態は公開される。
- Data Space Manifest (DS Manifest): DS パラメータ（validators/QC keys、privacy class、ISI policy、DA params、retention、quotas、ZK policy、fees）を宣言する Norito エンコードのマニフェスト。ハッシュは nexus チェーンにアンカーされる。特記がない限り DS QC は ML-DSA-87（Dilithium5 クラス）をデフォルトとする。
- Space Directory: DS マニフェスト、バージョン、ガバナンス/ローテーションイベントを追跡する on-chain ディレクトリ契約。
- DSID: Data Space を一意に識別する ID。全オブジェクト/参照の namespace に用いる。
- Anchor: DS ブロック/ヘッダからの暗号コミットメントで、nexus chain に含めて DS 履歴をグローバルレジャーへ束縛する。
- Kura: Iroha ブロックストレージ。ここでは erasure-coded blob と commitments に拡張。
- WSV: Iroha World State View。ここではバージョン化・スナップショット対応の erasure-coded 状態セグメントに拡張。
- IVM: スマートコントラクト実行のための Iroha Virtual Machine（Kotodama バイトコード `.to`）。
 - AIR: Algebraic Intermediate Representation。STARK 形式の証明向け計算の代数的表現で、実行をフィールドベースのトレースと遷移/境界制約で記述する。

Data Spaces モデル
- Identity: `DataSpaceId (DSID)` が DS を識別し、全てを namespace 化する。DS は 2 つの粒度で作成可能:
  - Domain-DS: `ds::domain::<domain_name>` - ドメインに限定された実行/状態。
  - Asset-DS: `ds::asset::<domain_name>::<asset_name>` - 単一アセット定義に限定された実行/状態。
  両者は共存し、トランザクションは複数 DSID を原子的に触れる。
- Manifest lifecycle: DS の作成・更新（鍵ローテーション、ポリシー変更）・廃止は Space Directory に記録。各スロットの DS アーティファクトは最新マニフェストハッシュを参照。
- Classes: Public DS（open participation, public DA）と Private DS（permissioned, confidential DA）。マニフェストの flags によるハイブリッドも可。
- Policies per DS: ISI permissions、DA params `(k,m)`、暗号化、retention、quotas（ブロックあたり min/max tx share）、ZK/optimistic proof policy、fees。
- Governance: DS メンバーシップとバリデータローテーションはマニフェストの governance セクションで定義（on-chain 提案、multisig、または nexus トランザクションと attestations でアンカーされる外部ガバナンス）。

Dataspace-aware gossip
- トランザクション gossip バッチは lane catalog 由来の plane tag（public vs restricted）を持つ。restricted バッチは `transaction_gossip_restricted_target_cap` を尊重して現在の commit topology のオンライン・ピアに unicast、public バッチは `transaction_gossip_public_target_cap` を使用（broadcast は `null`）。ターゲット選定は `transaction_gossip_public_target_reshuffle_ms` と `transaction_gossip_restricted_target_reshuffle_ms`（既定: `transaction_gossip_period_ms`）に従い再シャッフルされる。commit topology にオンライン・ピアがいないとき、`transaction_gossip_restricted_public_payload`（既定 `refuse`）で public overlay へ転送するか拒否するかを選べる。テレメトリは fallback 試行、forward/drop カウント、設定ポリシー、および dataspace ごとのターゲット選定を公開する。
- `transaction_gossip_drop_unknown_dataspace` が有効なら未知 dataspace は再キュー、無効なら漏洩回避のため restricted targeting にフォールバック。
- 受信側検証で、lane/dataspace がローカル catalog と一致しないエントリ、plane tag が可視性に合わないエントリ、または advertised route がローカルで再導出した routing 決定と一致しないエントリは破棄される。

Capability manifests と UAID
- Universal accounts: 各参加者に決定論的 UAID（`crates/iroha_data_model/src/nexus/manifest.rs` の `UniversalAccountId`）を付与。Capability manifests（`AssetPermissionManifest`）は UAID を dataspace、activation/expiry epoch、allow/deny `ManifestEntry` ルール（`dataspace`, `program_id`, `method`, `asset`, optional AMX roles）に結び付ける。deny が優先され、評価は `ManifestVerdict::Denied`（監査理由付き）または `Allowed`（該当 allowance metadata 付き）を返す。
- UAID ポートフォリオスナップショットは `GET /v1/accounts/{uaid}/portfolio`（`docs/source/torii/portfolio_api.md`）で公開。`iroha_core::nexus::portfolio` の決定論的アグリゲータがバック。
- Allowances: allow エントリは `AllowanceWindow` バケット（`PerSlot`, `PerMinute`, `PerDay`）と optional `max_amount` を持つ。Hosts と SDK は同じ Norito payload を消費し、実装差を排除する。
- Audit telemetry: Space Directory は `SpaceDirectoryEvent::{ManifestActivated, ManifestExpired, ManifestRevoked}`（`crates/iroha_data_model/src/events/data/space_directory.rs`）を発行。`SpaceDirectoryEventFilter` により Torii/data-event から UAID 更新/失効/deny-wins を監視可能。

### UAID マニフェスト運用

Space Directory 操作は 2 つの形で提供される。1) バイナリ内 CLI（スクリプト展開向け）、2) Torii 直接送信（CI/CD 向け）。どちらも `CanPublishSpaceDirectoryManifest{dataspace}` を executor（`crates/iroha_core/src/smartcontracts/isi/space_directory.rs`）で強制し、world state（`iroha_core::state::space_directory_manifests`）にライフサイクルイベントを記録する。

#### CLI workflow (`iroha app space-directory manifest ...`)

1. **マニフェスト JSON のエンコード** - ポリシー草案を Norito bytes に変換し、レビュー前に再現可能なハッシュを出力:

   ```bash
   iroha app space-directory manifest encode \
     --json dataspace/capability.json \
     --out artifacts/capability.manifest.to \
     --hash-out artifacts/capability.manifest.hash
   ```

   `--json`（raw JSON）または `--manifest`（既存 `.to`）を受け付け、
   `crates/iroha_cli/src/space_directory.rs::ManifestEncodeArgs` と同じロジックを実行。

2. **マニフェストの公開/置換** - Norito/JSON をソースに `PublishSpaceDirectoryManifest` をキュー:

   ```bash
   iroha app space-directory manifest publish \
     --manifest artifacts/capability.manifest.to \
     --reason "Retail wave 4 on-boarding"
   ```

   `--reason` は `entries[*].notes` の欠落を補完する。

3. **Expire** 寿命到達のマニフェスト、または **Revoke** UAID を要求に応じて実行。どちらも `--uaid uaid:<hex>` または 64 文字の hex digest (LSB=1) と数値 dataspace id を受ける:

   ```bash
   iroha app space-directory manifest expire \
     --uaid uaid:0f4d86b20839a8ddbe8a1a3d21cf1c502d49f3f79f0fa1cd88d5f24c56c0ab11 \
     --dataspace 11 \
     --expired-epoch 4600

   iroha app space-directory manifest revoke \
     --uaid uaid:0f4d86b20839a8ddbe8a1a3d21cf1c502d49f3f79f0fa1cd88d5f24c56c0ab11 \
     --dataspace 11 \
     --revoked-epoch 9216 \
     --reason "Fraud investigation NX-16-R05"
   ```

4. **監査バンドル生成** - `manifest audit-bundle` が JSON、`.to`、ハッシュ、dataspace プロファイル、機械可読メタデータを出力ディレクトリへ書き出し、ガバナンスレビュー用の単一アーカイブを生成:

   ```bash
   iroha app space-directory manifest audit-bundle \
     --manifest-json dataspace/capability.json \
     --profile dataspace/profiles/cbdc_profile.json \
     --out-dir artifacts/capability_bundle
   ```

   バンドルは profile の `SpaceDirectoryEvent` hooks を埋め込み、dataspace が必須監査 webhooks を公開していることを証明する。フィールド構造と証拠要件は `docs/space-directory.md` を参照。

#### Torii APIs

オペレーター/SDK は HTTPS 経由でも同様の操作が可能。Torii は同じ権限チェックを行い、指定 authority のためにトランザクションへ署名する（秘密鍵は Torii セキュアハンドラのメモリ内のみで扱う）。

- `GET /v1/space-directory/uaids/{uaid}` — UAID の現在の dataspace bindings を解決（正規化アドレス、dataspace ids、program bindings）。Sora Name Service 形式には `address_format=compressed` を付与（IH58 推奨、compressed（`sora`）は Sora 専用の次善）。
- `GET /v1/space-directory/uaids/{uaid}/portfolio` — Norito バックの aggregator。`ToriiClient.getUaidPortfolio` をミラーし、wallet が dataspace 横断保有を表示できる。
- `GET /v1/space-directory/uaids/{uaid}/manifests?dataspace={id}` — 正規マニフェスト JSON、ライフサイクル metadata、hash を取得。
- `POST /v1/space-directory/manifests` — JSON から新規/置換マニフェストを送信（`authority`, `private_key`, `manifest`, optional `reason`）。Torii は `202 Accepted` を返す。
- `POST /v1/space-directory/manifests/revoke` — UAID、dataspace id、effective epoch、optional reason を指定して緊急 revoke をキュー。

JS SDK（`javascript/iroha_js/src/toriiClient.js`）は `ToriiClient.getUaidPortfolio` / `.getUaidBindings` / `.getUaidManifests` でこれらの read 面をすでにラップしている。Swift/Python も同じ REST payload を再利用予定。`docs/source/torii/portfolio_api.md` に request/response schema、`docs/space-directory.md` に運用 playbook を記載。

Recent SDK/AMX updates
- **NX-11 (cross-lane relay verification):** SDK helpers は `/v1/sumeragi/status` の lane relay envelope を検証するようになった。Rust クライアントは relay proof の生成/検証と重複 `(lane_id, dataspace_id, height)` 拒否のための `iroha::nexus` helper を提供。Python は `verify_lane_relay_envelope_bytes`/`lane_settlement_hash`、JS SDK は `verifyLaneRelayEnvelope`/`laneRelayEnvelopeSample` を提供し、下流転送前に一貫したハッシュで検証できる。
  【crates/iroha/src/nexus.rs:1】【python/iroha_python/iroha_python_rs/src/lib.rs:666】【crates/iroha_js_host/src/lib.rs:640】【javascript/iroha_js/src/nexus.js:1】
- **NX-17 (AMX budget guardrails):** `ivm::analysis::enforce_amx_budget` が静的解析レポートから dataspace/グループの実行コストを見積もり、30 ms / 140 ms の予算を適用。DS/グループ予算の違反を明確に報告し、unit tests で担保。Nexus scheduler と SDK tooling の AMX スロット予算を決定論的にする。
  【crates/ivm/src/analysis.rs:142】【crates/ivm/src/analysis.rs:241】

高レベルアーキテクチャ
1) Global Composition Layer (Nexus Chain)
- 1 秒の Nexus Blocks を単一の正規順序で維持し、複数 DS に跨る原子トランザクションを確定する。確定トランザクションはグローバル world state（DS root のベクトル）を更新する。
- 最小限の metadata と aggregated proofs/QC を保持し、コンポーザビリティ、ファイナリティ、詐欺検出を担保する（触れた DSID、DS root 前後、DA commitments、DS validity proofs、ML-DSA-87 の DS QC）。プライベートデータは含めない。
- 合意: 22 ノード（3f+1, f=7）の単一グローバル pipeline BFT 委員会。~200k の候補から VRF/stake で epoch 選出し、1 秒以内にブロックを確定。

2) Data Space Layer (Public/Private)
- グローバルトランザクションの DS 断片を実行し、DS ローカル WSV を更新。ブロックごとに DS validity proofs と DA commitments を生成し、1 秒 Nexus Block に集約する。
- Private DS は authorized validators 間でデータを at-rest / in-flight で暗号化し、commitments と PQ proofs のみを外部へ。
- Public DS は DA 経由で完全なデータボディと PQ validity proofs を公開。

3) Atomic Cross-Data-Space Transactions (AMX)
- モデル: 1 つのユーザートランザクションが複数 DS（domain DS + asset DS など）に触れる。1 秒 Nexus Block で原子的に commit されるか abort される。
- 1 秒の Prepare-Commit: 対象 DS は同じ snapshot に対して並列実行し、DS ごとの PQ validity proofs（FASTPQ-ISI）と DA commitments を生成。必要 DS proofs の検証と DA 証明書（<=300 ms 目標）が揃った場合のみ commit。
- Consistency: read-write sets を宣言し、slot 開始 root に対して conflict 検出。DS ごとの lock-free optimistic 実行で全体停止を回避し、atomicity は nexus commit ルールで保証。
- Privacy: Private DS は pre/post root に結び付いた proofs/commitments だけを出力。生データは外に出ない。

4) Data Availability (DA) with Erasure Coding
- Kura はブロックボディと WSV snapshots を erasure-coded blobs として保存。Public blobs は広くシャーディングし、Private blobs は private DS validators 内に暗号化チャンクで保持。
- DA commitments は DS artifacts と Nexus Blocks の両方に記録され、プライバシーを守りながらサンプリング/復旧を可能にする。

Block and Commit Structure
- Data Space Proof Artifact（1 秒スロット、DS ごと）
  - Fields: dsid, slot, pre_state_root, post_state_root, ds_tx_set_hash, kura_da_commitment, wsv_da_commitment, manifest_hash, ds_qc (ML-DSA-87), ds_validity_proof (FASTPQ-ISI).
  - Private DS はデータボディなしの artifact を出力、Public DS は DA 経由で body を取得可能。

- Nexus Block（1 秒 cadence）
  - Fields: block_number, parent_hash, slot_time, tx_list（cross-DS 原子トランザクション + DSIDs）, ds_artifacts[], nexus_qc.
  - Function: 必要 DS artifacts が検証できた原子トランザクションを確定し、DS roots のグローバルベクトルを一括更新。

Consensus and Scheduling
- Nexus Chain Consensus: 22 ノード（3f+1, f=7）の単一グローバル pipeline BFT（Sumeragi 系）。1 秒ブロック/1 秒ファイナリティ。VRF/stake による epoch 選出とローテーション。
- Data Space Consensus: DS ごとに独自 BFT を実行し、proofs/DA commitments/DS QC を生成。lane-relay committees は `fault_tolerance` から `3f+1` で算出し、`(dataspace_id, lane_id)` を束縛した VRF epoch seed で決定論的にサンプリング。Private DS は permissioned、Public DS は anti-Sybil の下で open liveness。Nexus committee は不変。
- Transaction Scheduling: DSIDs と read-write sets を宣言した原子トランザクションを受理。DS をスロット内で並列実行し、DS artifacts と DA certificates が期限内（<=300 ms）なら 1 秒ブロックに含める。
- Performance Isolation: DS ごとの mempools/実行。per-DS quota で block 内の DS 比率を制限し、head-of-line blocking を防ぎ private DS レイテンシを保護。

Data Model and Namespacing
- DS-qualified IDs: ドメイン、アカウント、アセット、ロールは `dsid` で修飾。例: `ds::<domain>::account`, `ds::<domain>::asset#precision`。
- Global References: `(dsid, object_id, version_hint)` のタプル。nexus 層や AMX descriptors に載せて cross-DS 利用。
- Norito Serialization: cross-DS メッセージ（AMX descriptors, proofs）は Norito を使用。production パスで serde は使用しない。

Smart Contracts and IVM Extensions
- Execution Context: IVM 実行コンテキストに `dsid` を追加。Kotodama コントラクトは常に特定 DS 内で実行。
- Atomic Cross-DS Primitives:
  - `amx_begin()` / `amx_commit()` が IVM host で原子 multi-DS トランザクションを区切る。
  - `amx_touch(dsid, key)` が snapshot roots に対する read/write 意図を宣言。
  - `verify_space_proof(dsid, proof, statement)` -> bool
  - `use_asset_handle(handle, op, amount)` -> result（ポリシー許可と handle 有効性が条件）
- Asset Handles and Fees:
  - アセット操作は DS の ISI/role ポリシーに従い、fees は DS の gas token で支払う。capability tokens や richer policy（multi-approver、rate-limits、geofencing）は後から追加可能。
- Determinism: syscalls は入力と AMX read/write sets に対して純粋かつ決定論的。時間や環境の隠れ効果はない。

Post-Quantum Validity Proofs (Generalized ISIs)
- FASTPQ-ISI（PQ, trusted setup なし）: transfer 設計を ISI 全般に一般化する hash-based kernelized argument。GPU クラスで 20k スケールをサブ秒で証明することを目標。
  - Operational profile:
    - `fastpq_prover::Prover::canonical` は常に production backend を初期化。deterministic mock は削除。【crates/fastpq_prover/src/proof.rs:126】
    - `zk.fastpq.execution_mode` と `irohad --fastpq-execution-mode` で CPU/GPU 実行を決定論的に固定。observer hook が requested/resolved/backend のトリプルを記録。【crates/iroha_config/src/parameters/user.rs:1357】【crates/irohad/src/main.rs:270】【crates/irohad/src/main.rs:2192】【crates/iroha_telemetry/src/metrics.rs:8887】
- Arithmetization:
  - KV-Update AIR: WSV を Poseidon2-SMT でコミットされた型付き KV マップとして扱う。各 ISI は read-check-write 行に展開。
  - Opcode-gated constraints: selector 列を持つ単一 AIR テーブルで ISI ごとの規則（保存、単調カウンタ、権限、範囲チェック、bounded metadata 更新）を強制。
  - Lookup arguments: permissions/roles、asset precision、policy params 用の hash-committed テーブルでビット制約を軽減。
- State commitments and updates:
  - Aggregated SMT Proof: 触れたキー（pre/post）を `old_root`/`new_root` に対して圧縮 frontier + dedup siblings で証明。
  - Invariants: グローバル不変量（例: asset 総供給）を multiset 等価で保証。
- Proof system:
  - FRI-style polynomial commitments（DEEP-FRI）、arity 8/16、blow-up 8-16。Poseidon2 hash、Fiat-Shamir transcript（SHA-2/3）。
  - Optional recursion: DS ローカルで micro-batches を 1 proof に集約。
- Scope and examples covered:
  - Assets: transfer, mint, burn, asset definitions の登録/解除、precision 設定（上限あり）、metadata 設定。
  - Accounts/Domains: 作成/削除、key/threshold 設定、signatory 追加/削除（署名検証は DS QC で担保）。
  - Roles/Permissions (ISI): roles/permissions 付与/撤回、lookup テーブルと単調ポリシーで強制。
  - Contracts/AMX: AMX begin/commit markers、capability の mint/revoke。状態遷移/ポリシーカウンタとして証明。
- Out-of-AIR checks:
  - ML-DSA など重い署名検証は DS validators が行い DS QC で attest。proof は状態整合性とポリシー遵守のみを対象とする。
- Performance targets:
  - 20k 混在 ISI（キー <=8/ISI）: ~0.4-0.9 s 証明、~150-450 KB proof、~5-15 ms 検証。
  - 重い ISI: micro-batch + recursion でスロットあたり <1 s。
- DS Manifest configuration:
  - `zk.policy = "fastpq_isi"`
  - `zk.hash = "poseidon2"`, `zk.fri = { blowup: 8|16, arity: 8|16 }`
  - `state.commitment = "smt_poseidon2"`
  - `zk.recursion = { none | local }`
  - `attestation.signatures_in_proof = false`
  - `attestation.qc_signature = "ml_dsa_87"`
- Fallbacks:
  - 複雑な ISI は `zk.policy = "stark_fri_general"` を使用可能（証明遅延 + QC attest + slashing）。
  - 非 PQ（Plonk + KZG 等）は trusted setup が必要で、デフォルトビルドでは非対応。

AIR Primer (for Nexus)
- Execution trace: レジスタ列×ステップの行列。各行は ISI 処理の論理ステップ。
- Constraints:
  - Transition constraints: 行間関係を強制（例: `sel_transfer = 1` のとき post_balance = pre_balance - amount）。
  - Boundary constraints: public I/O（old_root/new_root, counters）を最初/最後の行に結び付ける。
  - Lookups/permutations: permissions/asset params のテーブルに対する membership と multiset 等価を保証。
- Commitment and verification:
  - Prover は hash ベースでトレースをコミットし、制約を満たす低次数多項式を構築。
  - Verifier は FRI で低次数をチェックし、Merkle 開口を少数で済ませる。
- Example (Transfer): pre_balance, amount, post_balance, nonce, selectors を含み、非負/範囲、保存則、nonce 単調性を enforced。SMT multi-proof が old/new roots を連結。

ABI Stability (ABI v1)
- ABI v1 のサーフェスは固定で、新規 syscalls / pointer-ABI 型はこのリリースでは追加しない。
- runtime upgrades は `abi_version = 1` かつ `added_syscalls`/`added_pointer_types` は空であること。
- ABI goldens（syscall list/ABI hash/pointer type ID）は固定で変更しない。

プライバシーモデル
- Private Data Containment: private DS の transaction bodies、state diffs、WSV snapshots は DS 外に出ない。
- Public Exposure: headers、DA commitments、PQ validity proofs のみ公開。
- Optional ZK Proofs: private DS は ZK proofs を生成し、内部状態を晒さずに cross-DS を可能にする。
- Access Control: ISI/role ポリシーで認可。capability tokens はオプション。

パフォーマンス分離と QoS
- DS ごとに consensus/mempool/storage を分離。
- per-DS quotas で anchor inclusion 時間を制御し head-of-line blocking を回避。
- DS ごとの contract resource budgets（compute/memory/IO）を IVM host が強制。public DS の混雑が private DS 予算を消費しない。
- 非同期 cross-DS 呼び出しで private DS 実行内の同期待ちを避ける。

データ可用性とストレージ設計
1) Erasure Coding
- Reed-Solomon（例: GF(2^16)）を Kura ブロックと WSV snapshots の blob に適用。`(k, m)`、`n = k + m`。
- Public DS の提案値: `k=32, m=16`（n=48）。最大 16 shard 損失を ~1.5x 拡張で復元。Private DS は `k=16, m=8`（n=24）。いずれも DS Manifest で調整可能。
- Public Blobs: 多数の DA ノードに分散し、サンプリングで可用性チェック。DA commitments は light clients に検証材料を提供。
- Private Blobs: 暗号化された shards を private DS バリデータ内にのみ配置。グローバルチェーンには commitments のみ。

2) Commitments and Sampling
- 各 blob で shards の Merkle root を計算し `*_da_commitment` に含める。楕円曲線 commitments を避け PQ を維持。
- DA Attesters: VRF サンプルされた regional attesters（例: 64/region）が ML-DSA-87 証明書を発行。目標レイテンシ <=300 ms。

3) Kura Integration
- ブロックボディを erasure-coded blobs と Merkle commitments で保存。
- ヘッダに blob commitments を含め、public DS は DA ネットワーク、private DS は私設チャネルから取得。

4) WSV Integration
- WSV Snapshotting: 定期的に chunked erasure-coded snapshots を作成し commitments をヘッダに記録。間は change logs を維持。
- Proof-carrying Access: Merkle/Verkle proofs を snapshot commitments に紐付け、private DS は ZK attestations を提供可能。

5) Retention and Pruning
- Public DS は pruning なし。Kura bodies と WSV snapshots を DA で保持。
- Private DS は内部 retention を設定可能だが、外部 commitments は不変。nexus 層は全 Nexus Blocks と DS artifacts を保持。

Networking and Node Roles
- Global Validators: nexus consensus に参加し、Nexus Blocks と DS artifacts を検証、public DS の DA checks を実施。
- Data Space Validators: DS consensus を実行し、契約を実行、ローカル Kura/WSV を管理、DS の DA を扱う。
- DA Nodes（任意）: public blobs を保存/配布しサンプリングを支援。private DS では validators または trusted custodians と共置。

System-Level Improvements and Considerations
- Sequencing/mempool 分離: Narwhal 型 DAG mempool + pipeline BFT を採用し、レイテンシとスループットを改善。
- DS quotas と fairness: per-DS per-block quotas と weight caps により head-of-line blocking を回避し private DS の遅延を予測可能に。
- DS attestation (PQ): DS QC は ML-DSA-87 をデフォルト。DS は ML-DSA-65/44 や EC signatures を明示的に選択可能。public DS は ML-DSA-87 を推奨。
- DA attesters: public DS は VRF サンプルの regional attesters を使用。nexus は証明書を検証し、private DS は内部で attestation。
- Recursion and epoch proofs: micro-batches を DS 内で再帰的に集約し、proof サイズ/検証時間を一定化。
- Lane scaling: 単一 committee がボトルネックなら K 並列 lanes を導入し deterministic merge。
- Deterministic acceleration: SIMD/CUDA の feature-gated kernel と bit-exact CPU fallback を提供。
- Lane activation thresholds: p95 finality >1.2 s が 3 分超、occupancy >85% が 5 分超、または tx rate が >1.2x capacity を要求する場合に 2-4 lanes を有効化。DSID hash で bucket 化し nexus block で merge。

Fees and Economics (Initial Defaults)
- Gas unit: DS ごとのガストークン。fees は DS のネイティブ gas asset で支払う。DS 間の変換はアプリ側の関心事。
- Inclusion priority: DS 間 round-robin と per-DS quotas。DS 内は fee bidding でタイブレーク。
- Future: グローバル fee market や MEV 最小化ポリシーは原子性や PQ 設計を崩さずに検討可能。

Cross-Data-Space Workflow (Example)
1) ユーザーが Public DS P と Private DS S をまたぐ AMX トランザクションを提出。S から P の受取人 B に asset X を移動。
2) スロット内で P と S が snapshot に対し並列実行。S は認可/可用性を確認し、内部状態を更新し PQ validity proof と DA commitment を生成。P は対応する状態更新（例: mint/burn/locking）と proof を生成。
3) Nexus committee が DS proofs と DA certificates を検証。両方がスロット内に揃えば 1 s Nexus Block で原子 commit、両 DS root を更新。
4) proof/DA certificate が欠落または不正なら abort。クライアントは次スロットに再送可能。S のプライベートデータは外部に出ない。

- Security Considerations
- Deterministic Execution: IVM syscalls は決定論的。cross-DS 結果は AMX commit と finality に依存し、壁時計やネットワーク時間に依存しない。
- Access Control: private DS の ISI/role ポリシーで送信者/操作を制御。capability tokens で細粒度権限を表現可能。
- Confidentiality: private DS データは end-to-end 暗号化。erasure-coded shards は権限者のみ保有。外部向け ZK proofs は任意。
- DoS Resistance: mempool/consensus/storage の分離で public 混雑が private DS に影響しない。

Changes to Iroha Components
- iroha_data_model: `DataSpaceId`、DS-qualified IDs、AMX descriptors（read/write sets）、proof/DA commitment types を追加。Norito のみ。
- ivm: ABI v1 サーフェスは固定（新規 syscalls/pointer-ABI なし）。AMX/runtime upgrades は既存 v1 primitives を使用し、ABI goldens を維持。
- iroha_core: nexus scheduler、Space Directory、AMX routing/validation、DS artifact verification、DA sampling/quotas の policy enforcement を実装。
- Space Directory & manifest loaders: DS manifest parsing に FMS endpoint metadata（他の common-good service descriptors も）を通し、参加時にローカル endpoints を自動発見。
- kura: erasure coding blob store、commitments、private/public ポリシーに従う retrieval APIs。
- WSV: snapshotting、chunking、commitments、proof APIs、AMX conflict detection/verification への統合。
- irohad: ノード役割、DA ネットワーク、private DS membership/authentication、`iroha_config` による設定（production で env toggle 禁止）。

Configuration and Determinism
- すべての runtime 振る舞いは `iroha_config` で構成し constructors/hosts にスレッドする。production で env toggle 禁止。
- SIMD/NEON/METAL/CUDA は feature-gated。deterministic fallback はハードウェア差を埋める。
- - Post-Quantum default: 全 DS は PQ validity proofs（STARK/FRI）と ML-DSA-87 をデフォルトとする。代替は DS Manifest で明示し、ポリシー承認が必要。

### Runtime Lane Lifecycle Control

- **Admin endpoint:** `POST /v1/nexus/lifecycle`（Torii）が `additions`（`LaneConfig` 完全オブジェクト）と `retire`（lane ids）を受け取り、再起動なしで lane を追加/削除。`nexus.enabled=true` で gated し、同じ Nexus configuration/state view を使用。
- **Behaviour:** 成功時に WSV/Kura metadata を更新し、queue routing/limits/manifests を再構築し、`{ ok: true, lane_count: <u32> }` を返す。検証失敗（未知の retire ids、重複 alias/id、Nexus 無効）時は `400 Bad Request` と `lane_lifecycle_error` を返す。
- **Safety:** shared state view lock を使い読み手との競合を回避。外部で lifecycle 更新のシリアライズは必要。
- **伝播:** キューのルーティング/リミットと lane マニフェストは更新済みカタログから再構築され、consensus/DA/RBC ワーカーは状態スナップショット経由で最新の lane config を読み取るため、スケジューリングとバリデータ選定が再起動なしで切り替わる（進行中の作業は旧設定で完了）。
- **ストレージのクリーンアップ:** Kura と tiered WSV のジオメトリを整合（作成/退役/リラベル）し、DA shard cursor マッピングを同期/永続化。退役 lane は lane relay キャッシュと DA commitment/confidential-compute/pin-intent ストアから剪定される。

Migration Path (Iroha 2 -> Iroha 3)
1) dataspace-qualified IDs と nexus block/global state composition を data model に導入し、Iroha 2 互換用 feature flags を追加。
2) Kura/WSV erasure coding backends を feature flags の背後に実装し、初期は既存 backends をデフォルトとする。
3) ABI v1 サーフェスを固定し、AMX は新規 syscalls/pointer types なしで実装。テスト/ドキュメントは ABI を変えず更新。
4) 単一 public DS と 1 秒ブロックの最小 nexus chain を提供し、その後 private DS pilot を追加。
5) DS-local FASTPQ-ISI proofs と DA attesters を備えた AMX を完成させ、全 DS で ML-DSA-87 QCs を有効化。

Testing Strategy
- data model 型、Norito roundtrip、AMX syscalls、proof encode/decode の unit tests。
- ABI v1 の goldens（syscall list/ABI hash/pointer type ID）を固定する IVM tests。
- cross-DS 原子トランザクション（正/負）、DA attester latency（<=300 ms）、性能分離の統合テスト。
- DS QC 検証（ML-DSA-87）、conflict detection/abort semantics、機密 shards 漏洩防止のセキュリティテスト。

### NX-18 Telemetry & Runbook Assets

- **Grafana board:** `dashboards/grafana/nexus_lanes.json` が NX-18 要求の "Nexus Lane Finality & Oracles" をエクスポート。`iroha_slot_duration_ms` の `histogram_quantile()`、`iroha_da_quorum_ratio`、DA availability warnings（`sumeragi_da_gate_block_total{reason="missing_local_data"}`）、オラクル価格/鮮度/TWAP/haircut gauges、`iroha_settlement_buffer_xor` バッファなどを包含。
- **CI gate:** `scripts/telemetry/check_slot_duration.py` が Prometheus snapshots を解析し、p50/p95/p99 を出力、NX-18 しきい値（p95 <= 1000 ms, p99 <= 1100 ms）を適用。`scripts/telemetry/nx18_acceptance.py` は DA quorum、オラクル指標、settlement buffers、slot quantiles を一括チェックし、`ci/check_nexus_lane_smoke.sh` 内で実行。
- **Evidence bundler:** `scripts/telemetry/bundle_slot_artifacts.py` が metrics snapshot + JSON summary を `artifacts/nx18/` にコピーし、`slot_bundle_manifest.json` に SHA-256 digests を記録。
- **Release automation:** `scripts/run_release_pipeline.py` が `ci/check_nexus_lane_smoke.sh` を呼び出し（`--skip-nexus-lane-smoke` で回避）、`artifacts/nx18/` を release 出力へコピー。
- **Runbook:** `docs/source/runbooks/nexus_lane_finality.md` が on-call ワークフロー（しきい値、インシデント手順、証拠取得、chaos drills）を文書化。
- **Telemetry helpers:** `scripts/telemetry/compare_dashboards.py` でダッシュボード差分を監視し、`scripts/telemetry/check_nexus_audit_outcome.py` で routed-trace/chaos リハを記録。

Open Questions (Clarification Needed)
1) トランザクション署名: エンドユーザーは DS が宣言する任意の署名アルゴリズム（Ed25519, secp256k1, ML-DSA など）を選択可能。hosts は multisig/curve capability を強制し、決定論的 fallback を提供し、混在時のレイテンシを文書化する必要がある。未解決: Torii/SDKs での capability negotiation flow と admission tests。
2) Gas economics: DS はローカルトークンで gas を定義可能だが、グローバル settlement fees は SORA XOR で支払う。標準的な変換経路（public-lane DEX など）や会計フック、ゼロ価格 DS への safeguards が未確定。
3) DA attesters: region ごとの目標数と threshold（例: 64 sampled, 43-of-64 ML-DSA-87）をどう設定するか。初日から含めるべき region はあるか。
4) Default DA params: public DS `k=32, m=16`、private DS `k=16, m=8` を提案。特定 DS クラスにより高い冗長性（`k=30, m=20`）は必要か。
5) DS granularity: ドメインとアセットの両方を DS にできる。階層 DS（domain DS が asset DS の親）とポリシー継承を導入するか、v1 はフラットに保つか。
6) Heavy ISIs: サブ秒証明できない複雑 ISI を (a) 拒否、(b) 原子的ステップに分割、(c) 旗付き遅延包含のどれにするか。
7) Cross-DS conflicts: クライアント宣言の read/write set で十分か、ホストが自動推論・拡張すべきか。

Appendix: Repository Policy 準拠
- Norito を全 wire 形式と JSON シリアライズに使用。
- ABI v1 のみ。ABI ポリシーの runtime toggle は不可。syscalls/pointer-types サーフェスは固定で golden tests により固定。
- ハードウェア間の決定論性を維持し、加速はオプションで gated。
- production パスに serde を使用しない。production で環境変数による設定は行わない。
