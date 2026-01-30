---
lang: ja
direction: ltr
source: docs/portal/i18n/ja/docusaurus-plugin-content-docs/current/nexus/nexus-spec.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 97782472bb55341ea68040437769152530f72112d9e35c8007a640781b7109fc
source_last_modified: "2025-11-14T04:43:20.502188+00:00"
translation_last_reviewed: 2026-01-30
---

:::note 正本ソース
このページは `docs/source/nexus.md` を反映しています。翻訳作業がポータルに反映されるまで、両方のコピーを同期してください。
:::

#! Iroha 3 - Sora Nexus Ledger: 技術設計仕様

本ドキュメントは、Iroha 3 向けの Sora Nexus Ledger アーキテクチャを提案し、Iroha 2 を Data Spaces (DS) を中心とした単一のグローバルかつ論理的に統合された台帳へ進化させる。Data Spaces は強いプライバシードメイン ("private data spaces") と公開参加 ("public data spaces") を提供する。設計はグローバル台帳の composability を維持しつつ private-DS データの厳格な隔離と機密性を確保し、Kura (block storage) と WSV (World State View) に対する消失訂正符号化でデータ可用性のスケールを導入する。

同じリポジトリで Iroha 2 (自社ホスト) と Iroha 3 (SORA Nexus) を構築する。実行は共有の Iroha Virtual Machine (IVM) と Kotodama ツールチェーンによって駆動されるため、コントラクトとバイトコード成果物は自社ホスト展開と Nexus グローバル台帳の間で移植可能なままである。

目標
- 多数の協調バリデータと Data Spaces で構成される単一のグローバル論理台帳。
- permissioned 運用 (例: CBDC) のための Private Data Spaces。データは private DS から外に出ない。
- 参加が開かれた Public Data Spaces。Ethereum 風の permissionless アクセス。
- Data Spaces を跨ぐ composable なスマートコントラクト。private-DS 資産へのアクセスは明示的な許可に従う。
- public 側の活動が private-DS の内部トランザクションを劣化させない性能分離。
- スケールするデータ可用性: Kura と WSV を消失訂正符号化し、private-DS のデータを秘匿したまま事実上無制限に拡張する。

非目標 (初期フェーズ)
- トークン経済やバリデータインセンティブの定義。スケジューリングやステーキングはプラガブル。
- 新しい ABI バージョンの導入。変更は ABI v1 を対象にし、IVM ポリシーに従って syscall と pointer-ABI を拡張する。

用語
- Nexus Ledger: Data Space (DS) のブロックを合成して単一の順序付き履歴と状態コミットメントを構成するグローバル論理台帳。
- Data Space (DS): 固有のバリデータ、ガバナンス、プライバシークラス、DA ポリシー、クォータ、料金ポリシーを持つ実行/ストレージの境界ドメイン。public DS と private DS の 2 クラス。
- Private Data Space: permissioned なバリデータとアクセス制御。トランザクションデータと状態は DS を出ない。グローバルにはコミットメント/メタデータのみをアンカーする。
- Public Data Space: permissionless な参加。全データと状態が公開。
- Data Space Manifest (DS Manifest): DS パラメータ (validators/QC keys、プライバシークラス、ISI ポリシー、DA パラメータ、保持、クォータ、ZK ポリシー、料金) を宣言する Norito エンコードのマニフェスト。マニフェストハッシュは nexus チェーンにアンカーされる。特記がなければ DS QC は ML-DSA-87 (Dilithium5 クラス) を既定のポスト量子署名として用いる。
- Space Directory: DS マニフェスト、バージョン、ガバナンス/ローテーションイベントを追跡するグローバルな on-chain ディレクトリコントラクト。
- DSID: Data Space のグローバルに一意な識別子。全オブジェクト/参照の namespacing に使用。
- Anchor: DS ブロック/ヘッダから nexus チェーンに含められる暗号コミットメント。DS 履歴をグローバル台帳へ拘束する。
- Kura: Iroha のブロックストレージ。ここでは消失訂正符号化された blob ストレージとコミットメントで拡張。
- WSV: Iroha World State View。ここではバージョン付き、スナップショット可能、消失訂正符号化された状態セグメントで拡張。
- IVM: スマートコントラクト実行のための Iroha Virtual Machine (Kotodama bytecode `.to`)。
  - AIR: Algebraic Intermediate Representation。STARK 風の証明のための代数的な計算表現で、遷移/境界制約を持つ field ベースのトレースとして実行を記述する。

Data Spaces モデル
- Identity: `DataSpaceId (DSID)` が DS を識別し全てを namespacing する。DS は 2 つの粒度でインスタンス化可能:
  - Domain-DS: `ds::domain::<domain_name>` - ドメインにスコープされた実行と状態。
  - Asset-DS: `ds::asset::<domain_name>::<asset_name>` - 単一のアセット定義にスコープされた実行と状態。
  両者は共存し、トランザクションは複数 DSID を原子的に触れられる。
- Manifest ライフサイクル: DS の作成、更新 (キー回転/ポリシー変更)、退役は Space Directory に記録される。各 slot の DS 成果物は最新の manifest hash を参照。
- Classes: Public DS (オープン参加、public DA) と Private DS (permissioned、confidential DA)。manifest フラグによりハイブリッドも可能。
- DS ごとのポリシー: ISI 権限、DA パラメータ `(k,m)`、暗号化、保持、クォータ (ブロックあたり tx の min/max シェア)、ZK/楽観的証明ポリシー、料金。
- Governance: DS メンバーシップとバリデータローテーションは manifest の governance セクションで定義 (on-chain 提案、マルチシグ、または nexus 取引とアテステーションによりアンカーされた外部ガバナンス)。

Capability manifests と UAID
- Universal accounts: 各参加者は全 dataspaces に跨る決定的 UAID (`UniversalAccountId` in `crates/iroha_data_model/src/nexus/manifest.rs`) を持つ。Capability manifests (`AssetPermissionManifest`) は UAID を特定の dataspace、activation/expiry epoch、そして `dataspace`, `program_id`, `method`, `asset` とオプションの AMX roles をスコープする allow/deny `ManifestEntry` の順序付きリストに結び付ける。deny ルールが常に優先され、評価器は監査理由付きの `ManifestVerdict::Denied` か、対応する allowance metadata を含む `Allowed` を返す。
- Allowances: 各 allow エントリは決定的な `AllowanceWindow` バケット (`PerSlot`, `PerMinute`, `PerDay`) と任意の `max_amount` を持つ。host と SDK は同一の Norito payload を消費するため、ハードウェアと SDK 間で強制が一致する。
- Audit telemetry: Space Directory は `SpaceDirectoryEvent::{ManifestActivated, ManifestExpired, ManifestRevoked}` (`crates/iroha_data_model/src/events/data/space_directory.rs`) を manifest 状態変化のたびに配信する。新しい `SpaceDirectoryEventFilter` により、Torii/data-event 購読者は UAID manifest 更新、失効、deny-wins の決定をカスタム配管なしで監視できる。

エンドツーエンドのオペレーター証跡、SDK 移行ノート、manifest 公開チェックリストについては、Universal Account Guide (`docs/source/universal_accounts_guide.md`) と併せて更新すること。UAID ポリシーやツールが変わるたびに両方を整合させる。

ハイレベルアーキテクチャ
1) グローバル合成レイヤ (Nexus Chain)
- 1 秒 cadence の Nexus Blocks を単一の canonical order で維持し、1 つ以上の Data Spaces (DS) に跨る原子トランザクションを finalization する。各コミット済みトランザクションは統合されたグローバル world state (DS root のベクトル) を更新する。
- composability/finality/fraud 検出のための最小メタデータと集約 proof/QC を保持 (触れた DSID、各 DS の before/after state root、DA commitments、DS validity proofs、ML-DSA-87 を用いた DS quorum certificate)。private データは含まない。
- Consensus: 1s ブロックを目標に、22 ノードのグローバル BFT committee (3f+1, f=7) を epoch ごとの VRF/stake で最大 ~200k の候補から選出。Nexus committee がトランザクションを順序付けし 1s で finality。

2) Data Space レイヤ (Public/Private)
- グローバル取引の DS ごとのフラグメントを実行し、DS ローカル WSV を更新し、1 秒 Nexus Block へ集約される DS ごとの validity artifacts (集約 proofs と DA commitments) を生成する。
- Private DS は承認済みバリデータ間でデータを暗号化し、コミットメントと PQ validity proof のみを外部へ出す。
- Public DS は DA 経由でフルデータボディと PQ validity proof を公開する。

3) 原子 cross-Data-Space トランザクション (AMX)
- モデル: ユーザートランザクションは複数 DS (例: domain DS と asset DS) を触れる。単一の Nexus Block で原子的に commit されるか abort され、部分的効果はない。
- 1s 内の Prepare-Commit: 各候補トランザクションに対し、触れた DS が同一スナップショット (slot 開始時 DS roots) で並列実行し、DS ごとの PQ validity proof (FASTPQ-ISI) と DA commitments を生成する。Nexus committee は全 DS proofs が検証され、DA certificates が期限内 (目標 <=300 ms) に到着した場合のみ commit し、失敗時は次スロットに再スケジュールする。
- Consistency: read/write セットを宣言し、slot 開始 root に対して commit 時に競合検知。DS ごとの lock-free optimistic 実行により global stall を回避し、atomicity は nexus commit ルール (全 DS での all-or-nothing) で保証。
- Privacy: Private DS は pre/post DS roots に結び付く proof/commitment のみを外部へ出し、private データは出ない。

4) 消失訂正符号による DA (Data Availability)
- Kura は block body と WSV snapshot を消失訂正符号化された blob として保存。public blobs は広く shard され、private blobs は private-DS バリデータ内のみで暗号化 chunk として保存。
- DA commitments は DS artifacts と Nexus Blocks の双方に記録され、private 内容を明かさずに sampling と復旧保証を可能にする。

Block/Commit 構造
- Data Space Proof Artifact (1s slot, per DS)
  - フィールド: dsid, slot, pre_state_root, post_state_root, ds_tx_set_hash, kura_da_commitment, wsv_da_commitment, manifest_hash, ds_qc (ML-DSA-87), ds_validity_proof (FASTPQ-ISI)。
  - Private-DS はデータ本体なしで artifacts を輸出し、Public DS は DA で body を取得可能。

- Nexus Block (1s cadence)
  - フィールド: block_number, parent_hash, slot_time, tx_list (触れた DSID を含む原子 cross-DS トランザクション), ds_artifacts[], nexus_qc。
  - 機能: 必要な DS artifacts が検証される原子トランザクションを全て finalization し、DS roots のグローバルベクトルを一括更新する。

Consensus と scheduling
- Nexus Chain Consensus: 1s ブロック/1s finality を目標とする 22 ノード (3f+1, f=7) のグローバル BFT (Sumeragi クラス)。committee は epoch ごとに VRF/stake で ~200k 候補から選出され、ローテーションで分散性と耐検閲性を維持する。
- Data Space Consensus: 各 DS は独自の BFT を実行し、slot ごとに artifacts (proofs, DA commitments, DS QC) を生成する。lane-relay committee は dataspace の `fault_tolerance` に基づき `3f+1` でサイズ決定され、VRF epoch seed を `(dataspace_id, lane_id)` に結び付けて epoch ごとに決定的にサンプリングされる。Private DS は permissioned、Public DS は anti-Sybil ポリシー下で open liveness。グローバル nexus committee は不変。
- Transaction Scheduling: ユーザーは触れる DSID と read/write セットを宣言して原子トランザクションを送る。DS が slot 内で並列実行し、全 DS artifacts が検証され DA certificates が期限内 (<=300 ms) に届いた場合に 1s block に含める。
- Performance Isolation: 各 DS は独立した mempool と実行を持ち、DS ごとの quota によりブロックあたりの DS 関連 tx 数を制限し、head-of-line blocking を回避して private DS のレイテンシを保護する。

データモデルと namespacing
- DS-qualified IDs: 全てのエンティティ (domains, accounts, assets, roles) は `dsid` で修飾される。例: `ds::<domain>::account`, `ds::<domain>::asset#precision`。
- Global References: グローバル参照は `(dsid, object_id, version_hint)` のタプルで、nexus 層や AMX 記述子に置ける。
- Norito Serialization: 全ての cross-DS メッセージ (AMX 記述子、proofs) は Norito コーデックを使用する。production パスで serde は使わない。

スマートコントラクトと IVM 拡張
- Execution Context: IVM 実行コンテキストに `dsid` を追加。Kotodama コントラクトは常に特定の Data Space 内で実行される。
- Atomic Cross-DS Primitives:
  - `amx_begin()` / `amx_commit()` が IVM host で原子 multi-DS トランザクションを区切る。
  - `amx_touch(dsid, key)` が read/write 意図を宣言し、slot snapshot roots に対する競合検知に使用。
  - `verify_space_proof(dsid, proof, statement)` -> bool
  - `use_asset_handle(handle, op, amount)` -> result (ポリシー許可かつ handle 有効時のみ許可)
- Asset Handles and Fees:
  - アセット操作は DS の ISI/role ポリシーで認可され、手数料は DS のガストークンで支払われる。capability tokens や高度なポリシー (multi-approver, rate-limits, geofencing) は後で追加可能で、原子モデルは維持される。
- Determinism: 新規 syscalls は入力と宣言済み AMX read/write セットのみに依存する純粋かつ決定的なもの。時間や環境に依存しない。

ポスト量子の validity proofs (一般化 ISI)
- FASTPQ-ISI (PQ, trusted setup なし): transfer 設計を全 ISI ファミリーに一般化する hash-based な引数で、GPU クラスのハードウェアで 20k 規模のバッチに対してサブ秒証明を目標とする。
  - 運用プロファイル:
    - production ノードは `fastpq_prover::Prover::canonical` で prover を構築し、常に production backend を初期化する。決定的 mock は削除済み。 [crates/fastpq_prover/src/proof.rs:126]
    - `zk.fastpq.execution_mode` (config) と `irohad --fastpq-execution-mode` により、CPU/GPU 実行を決定的に固定でき、observer hook が requested/resolved/backend の triple を記録してフリート監査に使える。 [crates/iroha_config/src/parameters/user.rs:1357] [crates/irohad/src/main.rs:270] [crates/irohad/src/main.rs:2192] [crates/iroha_telemetry/src/metrics.rs:8887]
- Arithmetization:
  - KV-Update AIR: WSV を Poseidon2-SMT でコミットされた型付き key-value map として扱う。各 ISI は key (accounts, assets, roles, domains, metadata, supply) に対する read-check-write の小さな行集合に展開。
  - Opcode-gated constraints: selector 列を持つ単一 AIR テーブルで ISI ごとのルール (保存則、単調カウンタ、権限、範囲チェック、制限付き metadata 更新) を強制。
  - Lookup arguments: permissions/roles, asset precisions, policy parameters のための透明な hash-commit テーブルにより、重い bitwise 制約を回避。
- State commitments and updates:
  - Aggregated SMT Proof: 触れた key (pre/post) を `old_root`/`new_root` に対して圧縮 frontier と重複 sibling 排除で証明。
  - Invariants: グローバル不変条件 (例: 資産ごとの total supply) を effect 行と追跡カウンタの multiset equality で強制。
- Proof system:
  - FRI 系多項式コミットメント (DEEP-FRI) を high arity (8/16) と blow-up 8-16 で使用。Poseidon2 ハッシュ、Fiat-Shamir transcript には SHA-2/3。
  - Optional recursion: DS ローカルの再帰集約で micro-batch を 1 proof/slot に圧縮可能。
- Scope と例:
  - Assets: transfer, mint, burn, register/unregister asset definitions, set precision (bounded), set metadata。
  - Accounts/Domains: create/remove, set key/threshold, add/remove signatories (状態のみ。署名検証は DS validator が行い、AIR では証明しない)。
  - Roles/Permissions (ISI): grant/revoke roles と permissions。lookup table と単調ポリシー check で強制。
  - Contracts/AMX: AMX begin/commit マーカー、capability mint/revoke (有効時)。状態遷移とポリシーカウンタとして証明。
- AIR 外チェック (低レイテンシ維持):
  - 署名や重い暗号 (例: ML-DSA ユーザ署名) は DS validator が検証し DS QC でアテストする。validity proof は状態整合性とポリシー遵守のみをカバー。これにより PQ かつ高速。
- Performance targets (例: 32-core CPU + modern GPU):
  - 20k の mixed ISI (key-touch <=8 keys/ISI): 約 0.4-0.9 s の証明、約 150-450 KB、検証約 5-15 ms。
  - 重い ISI: micro-batch (例: 10x2k) + recursion で per-slot <1 s を維持。
- DS Manifest 設定:
  - `zk.policy = "fastpq_isi"`
  - `zk.hash = "poseidon2"`, `zk.fri = { blowup: 8|16, arity: 8|16 }`
  - `state.commitment = "smt_poseidon2"`
  - `zk.recursion = { none | local }`
  - `attestation.signatures_in_proof = false` (署名は DS QC で検証)
  - `attestation.qc_signature = "ml_dsa_87"` (既定。代替は明示宣言が必要)
- Fallbacks:
  - 複雑/カスタム ISI は汎用 STARK (`zk.policy = "stark_fri_general"`) を利用可能。証明は遅延し、QC アテスト + 不正証明の slashing により 1s finality を維持。
  - 非 PQ (例: Plonk + KZG) は trusted setup が必要で、デフォルトビルドでは非対応。

AIR Primer (Nexus)
- Execution trace: 幅 (レジスタ列) と長さ (ステップ) を持つ行列。各行が ISI 処理の論理ステップで、列は pre/post 値、selector、flags を保持する。
- Constraints:
  - Transition constraints: 行間関係 (例: `sel_transfer = 1` のとき debit 行で post_balance = pre_balance - amount) を強制。
  - Boundary constraints: 公開 I/O (old_root/new_root, counters) を先頭/末尾行に束縛。
  - Lookups/permutations: permissions や asset params のテーブルに対する membership/multiset equality を保証し、bit-heavy 回路を回避。
- Commitment and verification:
  - Prover は hash-based エンコードでトレースにコミットし、制約が満たされるときのみ有効な低次数多項式を構成。
  - Verifier は FRI (hash-based, post-quantum) で低次数性をチェックし、少数の Merkle 開示で済む。コストはステップ数に対して対数。
- Example (Transfer): レジスタは pre_balance, amount, post_balance, nonce, selectors を含む。制約で非負/範囲、保存、nonce 単調性を保証し、SMT の集約マルチプルーフが pre/post leaves を old/new roots に結び付ける。

ABI と syscall の進化 (ABI v1)
- 追加予定 syscalls (例):
  - `SYS_AMX_BEGIN`, `SYS_AMX_TOUCH`, `SYS_AMX_COMMIT`, `SYS_VERIFY_SPACE_PROOF`, `SYS_USE_ASSET_HANDLE`。
- 追加予定 pointer-ABI types:
  - `PointerType::DataSpaceId`, `PointerType::AmxDescriptor`, `PointerType::AssetHandle`, `PointerType::ProofBlob`。
- 必要な更新:
  - `ivm::syscalls::abi_syscall_list()` に追加 (順序維持)、ポリシーでゲート。
  - 未知番号は hosts で `VMError::UnknownSyscall` にマップ。
  - テスト更新: syscall list golden, ABI hash, pointer type ID goldens, policy tests。
  - Docs: `crates/ivm/docs/syscalls.md`, `status.md`, `roadmap.md`。

プライバシーモデル
- Private Data Containment: private DS のトランザクションボディ、状態差分、WSV snapshot は private validator 集合を出ない。
- Public Exposure: headers, DA commitments, PQ validity proofs のみを公開。
- Optional ZK Proofs: private DS は ZK proof (例: 残高十分、ポリシー遵守) を生成し、内部状態を公開せずに cross-DS 行為を可能にする。
- Access Control: 認可は DS 内の ISI/role ポリシーで行う。capability tokens は任意で後から導入可能。

性能分離と QoS
- DS ごとに独立した consensus, mempool, storage。
- DS ごとの nexus scheduling quota で anchor inclusion 時間を制限し、head-of-line blocking を回避。
- DS ごとの契約リソース予算 (compute/memory/IO) を IVM host が強制。public DS の競合が private DS 予算を消費しない。
- 非同期 cross-DS 呼び出しにより private-DS 実行内の長い同期待ちを回避。

データ可用性とストレージ設計
1) Erasure Coding
- Kura block と WSV snapshot の blob に対して systematic Reed-Solomon (例: GF(2^16)) を使用。パラメータ `(k, m)` で `n = k + m` shards。
- デフォルト (提案, public DS): `k=32, m=16` (n=48)。最大 16 shard 損失を ~1.5x 拡張で復旧。private DS は `k=16, m=8` (n=24)。いずれも DS Manifest で設定可能。
- Public blobs: 多数の DA nodes/validators に shard 分散し、sampling ベースの可用性チェック。DA commitments を headers に入れて light clients が検証可能。
- Private blobs: shard を暗号化して private-DS validators (または custodian) のみへ配布。グローバルチェーンには DA commitments のみ (shard 位置/鍵は含まない)。

2) Commitments と sampling
- 各 blob の shard に対して Merkle root を計算し `*_da_commitment` に含める。楕円曲線コミットメントを避け、PQ を維持。
- DA Attesters: VRF サンプルの地域 attesters (例: 地域ごと 64) が shard sampling 成功を ML-DSA-87 で証明。目標 DA attestation latency <=300 ms。Nexus committee は shard を引かず証明書を検証する。

3) Kura 統合
- ブロックはトランザクションボディを erasure-coded blobs として保存し Merkle commitments を持つ。
- headers は blob commitments を持ち、public DS は DA ネットワークから、private DS は private チャネルから body を取得。

4) WSV 統合
- WSV Snapshotting: DS 状態を定期的に chunked erasure-coded snapshots にチェックポイントし、commitments を headers に記録。snapshot 間は change logs を維持。public snapshots は広く shard 化され、private snapshots は private validators に残る。
- Proof-Carrying Access: コントラクトは snapshot commitments にアンカーされた状態証明 (Merkle/Verkle) を提供/要求できる。private DS は raw proofs の代わりに ZK アテステーションを提供可能。

5) Retention と pruning
- public DS は pruning なし: Kura bodies と WSV snapshots を DA 経由で保持 (水平スケール)。private DS は内部 retention を定義できるが、エクスポートされた commitments は不変。nexus レイヤは全 Nexus Blocks と DS artifact commitments を保持。

ネットワーキングとノードロール
- Global Validators: nexus consensus に参加し、Nexus Blocks と DS artifacts を検証、public DS の DA チェックを行う。
- Data Space Validators: DS consensus を実行し、コントラクト実行、ローカル Kura/WSV 管理、DS 用 DA を処理。
- DA Nodes (optional): public blobs を保存/配布し sampling を支援。private DS では validators か trusted custodians に同居。

システムレベル改善と考慮点
- Sequencing/mempool decoupling: DAG mempool (例: Narwhal 風) を導入し、nexus レイヤの pipelined BFT に供給して低レイテンシ/高スループットを実現。
- DS quotas と fairness: DS ごとの per-block quotas と weight caps で head-of-line blocking を避け、private DS の予測可能なレイテンシを確保。
- DS attestation (PQ): DS QC は既定で ML-DSA-87 (Dilithium5 クラス) を使用。EC より大きいが 1 slot あたり 1 QC なら許容。DS は明示的に ML-DSA-65/44 や EC 署名を選択可能。public DS では ML-DSA-87 を推奨。
- DA attesters: public DS では VRF サンプルの regional attesters が DA certificates を発行。Nexus committee は raw shard sampling ではなく証明書を検証。private DS は内部 DA attestations を維持。
- Recursion と epoch proofs: DS 内の micro-batch を 1 proof/slot/epoch に集約して、証明サイズと検証時間を高負荷でも一定にする。
- Lane scaling (必要なら): 単一グローバル committee がボトルネック化した場合、K 本の並列 sequencing lanes と決定的 merge を導入し、単一グローバル順序を保ちながら水平スケールする。
- Deterministic acceleration: hashing/FFT 向け SIMD/CUDA kernels を feature flag で提供し、bit-exact CPU fallback でハードウェア差による非決定性を回避。
- Lane activation thresholds (提案): (a) p95 finality が 1.2 s を >3 分連続で超過、(b) ブロック占有率が 85% を >5 分超過、(c) incoming tx rate が持続的にブロック容量の >1.2x を要求する場合に 2-4 lanes を有効化。DSID hash により決定的にトランザクションを bucket し、nexus block で merge する。

料金と経済 (初期デフォルト)
- Gas unit: DS ごとの gas token に compute/IO を計測し、料金は DS のネイティブ gas asset で支払う。DS 間の変換はアプリ側責務。
- Inclusion priority: DS 間の round-robin と DS ごとの quotas で fairness と 1s SLO を維持。DS 内は fee bidding でタイブレーク可能。
- Future: グローバル fee market や MEV 最小化ポリシーは atomicity/PQ proof 設計を変えずに検討可能。

Cross-Data-Space ワークフロー (例)
1) ユーザーが public DS P と private DS S に触れる AMX トランザクションを送る: S の asset X を P のアカウント B へ移動。
2) slot 内で P と S がスナップショットに対してフラグメントを実行。S は認可/可用性を検証し内部状態を更新、PQ validity proof と DA commitment を生成 (private データは漏れない)。P は対応する状態更新 (例: P 側の mint/burn/locking) と proof を準備。
3) Nexus committee が両 DS proofs と DA certificates を検証し、slot 内に両方成功すれば 1s Nexus Block に原子的に commit し、グローバル world state ベクトルの DS roots を更新。
4) proof または DA certificate が欠落/無効ならトランザクションは abort (効果なし)。クライアントは次 slot で再送可能。S の private データはどの段階でも外部に出ない。

- セキュリティ考慮
- Deterministic Execution: IVM syscalls は決定的で、cross-DS の結果は AMX commit と finality により決まり、壁時計やネットワークタイミングには依存しない。
- Access Control: private DS の ISI 権限で誰がトランザクションを送れるか/どの操作が許可されるかを制御。capability tokens で細粒度権限を付与。
- Confidentiality: private-DS データの end-to-end 暗号化、消失訂正 shard を許可メンバーのみが保持、外部アテステーションに ZK proof を選択可能。
- DoS Resistance: mempool/consensus/storage レイヤの分離で public 渋滞が private DS の進行を阻害しない。

Iroha コンポーネントの変更
- iroha_data_model: `DataSpaceId`、DS 修飾 ID、AMX 記述子 (read/write セット)、proof/DA commitment 型を導入。Norito のみでシリアライズ。
- ivm: AMX (`amx_begin`, `amx_commit`, `amx_touch`) と DA proofs の syscalls/pointer-ABI types を追加し、v1 ポリシーに従って ABI tests/docs を更新。
