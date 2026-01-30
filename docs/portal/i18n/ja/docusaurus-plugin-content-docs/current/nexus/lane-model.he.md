---
lang: he
direction: rtl
source: docs/portal/i18n/ja/docusaurus-plugin-content-docs/current/nexus/lane-model.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: cb1ab398c4299e5a9a2e4a0b29b3505e2c73b8e85c00350db7521554417f511d
source_last_modified: "2026-01-03T18:08:02+00:00"
translation_last_reviewed: 2026-01-30
---

---
lang: ja
direction: ltr
source: docs/portal/docs/nexus/lane-model.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
id: nexus-lane-model
title: Nexus レーンモデル
description: Sora Nexus の論理レーン分類、設定ジオメトリ、world-state マージ規則。
---

# Nexus レーンモデルと WSV パーティション

> **ステータス:** NX-1 deliverable - レーン分類、設定ジオメトリ、ストレージレイアウトは実装準備完了。  
> **Owners:** Nexus Core WG, Governance WG  
> **Roadmap reference:** `roadmap.md` の NX-1

このポータルページは `docs/source/nexus_lanes.md` の正本ブリーフを反映し、Sora Nexus のオペレーター、SDK オーナー、レビュアーが mono-repo のツリーに潜らずレーンガイダンスを読めるようにしています。対象アーキテクチャは world state の決定性を維持しつつ、個別の data spaces (lanes) が公開または非公開のバリデータセットで隔離された workloads を実行できるようにします。

## Concepts

- **Lane:** Nexus ledger の論理 shard。独自のバリデータセットと実行 backlog を持ち、安定した `LaneId` で識別される。
- **Data Space:** governance が管理するバケットで、compliance/routing/settlement ポリシーを共有する複数 lanes を束ねる。
- **Lane Manifest:** governance 管理の metadata。validators、DA policy、gas token、settlement rules、routing permissions を記述する。
- **Global Commitment:** lane が発行する proof。新しい state roots、settlement data、任意の cross-lane transfers を要約する。global NPoS ring が commitments を順序付ける。

## レーン分類

lane types は可視性、governance surface、settlement hooks を正規に記述する。設定ジオメトリ (`LaneConfig`) がこれらの属性を保持するため、nodes、SDKs、tooling が特別なロジックなしに layout を理解できる。

| Lane type | Visibility | Validator membership | WSV exposure | Default governance | Settlement policy | Typical use |
|-----------|------------|----------------------|--------------|--------------------|-------------------|-------------|
| `default_public` | public | Permissionless (global stake) | Full state replica | SORA Parliament | `xor_global` | Baseline public ledger |
| `public_custom` | public | Permissionless or stake-gated | Full state replica | Stake weighted module | `xor_lane_weighted` | High-throughput public applications |
| `private_permissioned` | restricted | Fixed validator set (governance approved) | Commitments & proofs | Federated council | `xor_hosted_custody` | CBDC, consortium workloads |
| `hybrid_confidential` | restricted | Mixed membership; wraps ZK proofs | Commitments + selective disclosure | Programmable money module | `xor_dual_fund` | Privacy-preserving programmable money |

全ての lane types は次を宣言する必要がある:

- Dataspace alias - 人が読めるグルーピングで compliance ポリシーを結び付ける。
- Governance handle - `Nexus.governance.modules` で解決される識別子。
- Settlement handle - settlement router が XOR buffers をデビットする際に使用する識別子。
- 任意の telemetry metadata (description, contact, business domain)。`/status` と dashboards で露出される。

## レーン設定ジオメトリ (`LaneConfig`)

`LaneConfig` は検証済み lane catalog から導出される runtime ジオメトリである。governance manifests を置き換えるものではなく、各 lane に対して決定論的な storage identifiers と telemetry hints を提供する。

```text
LaneConfigEntry {
    lane_id: LaneId,           // stable identifier
    alias: String,             // human-readable alias
    slug: String,              // sanitised alias for file/metric keys
    kura_segment: String,      // Kura segment directory: lane_{id:03}_{slug}
    merge_segment: String,     // Merge-ledger segment: lane_{id:03}_merge
    key_prefix: [u8; 4],       // Big-endian LaneId prefix for WSV key spaces
    shard_id: ShardId,         // WSV/Kura shard binding (defaults to lane_id)
    visibility: LaneVisibility,// public vs restricted lanes
    storage_profile: LaneStorageProfile,
    proof_scheme: DaProofScheme,// DA proof policy (merkle_sha256 default)
}
```

- `LaneConfig::from_catalog` は設定ロード時 (`State::set_nexus`) にジオメトリを再計算する。
- aliases は小文字の slug にサニタイズされる。連続する非英数字は `_` に畳み込まれる。alias から空の slug が生成された場合は `lane{id}` にフォールバックする。
- `shard_id` は catalog の metadata key `da_shard_id` から導出される (デフォルトは `lane_id`)。永続化された shard cursor journal を駆動し、restarts/resharding 間で DA replay の決定性を維持する。
- key prefixes により、同一 backend を共有しても WSV は lane ごとの key ranges を分離したまま維持する。
- Kura segment 名は host 間で決定論的。監査者は特別な tooling なしに segment directories と manifests を照合できる。
- merge segments (`lane_{id:03}_merge`) は最新の merge-hint roots とグローバル state commitments を保持する。

## World-state partitioning

- 論理 Nexus world state は lane ごとの state spaces の合成。public lanes は full state を永続化し、private/confidential lanes は Merkle/commitment roots を merge ledger にエクスポートする。
- MV storage は全ての key に `LaneConfigEntry::key_prefix` の 4-byte lane prefix を付与し、`[00 00 00 01] ++ PackedKey` のような keys を生成する。
- Shared tables (accounts, assets, triggers, governance records) は lane prefix でエントリをグループ化し、range scans を決定論的に保つ。
- merge-ledger metadata も同じ layout を反映する: 各 lane が `lane_{id:03}_merge` に merge-hint roots と reduced global state roots を書き込み、lane 退役時に targeted retention/eviction を可能にする。
- Cross-lane indexes (account aliases, asset registries, governance manifests) は明示的な lane prefixes を保存し、オペレーターが素早く entries を照合できる。
- **Retention policy** - public lanes は full block bodies を保持する。commitment-only lanes は checkpoints 後に古い bodies を圧縮可能 (commitments が権威的)。confidential lanes は他 workloads を阻害しないよう専用 segments に ciphertext journals を保持する。
- **Tooling** - 保守ユーティリティ (`kagami`, CLI admin commands) は metrics、Prometheus labels、Kura segments のアーカイブ時に slugged namespace を参照すべき。

## Routing & APIs

- Torii REST/gRPC endpoints は `lane_id` を任意で受け付ける。省略時は `lane_default`。
- SDKs は lane selectors を提供し、lane catalog を用いて user-friendly aliases を `LaneId` にマッピングする。
- Routing rules は検証済み catalog に基づいて lane と dataspace の両方を選択できる。`LaneConfig` は dashboards と logs 向けの telemetry-friendly aliases を提供する。

## Settlement & fees

- 全ての lanes は global validator set に XOR fees を支払う。lanes は native gas tokens を徴収できるが、commitments とともに XOR equivalents を escrow する必要がある。
- Settlement proofs には amount、conversion metadata、escrow の証明が含まれる (例: global fee vault への転送)。
- 統一 settlement router (NX-3) は同じ lane prefixes で buffers をデビットするため、settlement telemetry が storage ジオメトリに揃う。

## Governance

- Lanes は catalog を通じて governance module を宣言する。`LaneConfigEntry` は元の alias と slug を保持し、telemetry と audit trails を読みやすくする。
- Nexus registry は `LaneId`、dataspace binding、governance handle、settlement handle、metadata を含む署名済み lane manifests を配布する。
- Runtime-upgrade hooks は引き続き governance policies (`gov_upgrade_id` デフォルト) を適用し、telemetry bridge (`nexus.config.diff` events) で diffs を記録する。

## Telemetry & status

- `/status` は lane aliases、dataspace bindings、governance handles、settlement profiles を公開する。catalog と `LaneConfig` 由来。
- Scheduler metrics (`nexus_scheduler_lane_teu_*`) は lane aliases/slugs を表示し、オペレーターが backlog と TEU 圧を素早く把握できる。
- `nexus_lane_configured_total` は導出された lane entries 数を数え、設定変更時に再計算される。telemetry は lane geometry 変更時に signed diffs を発行する。
- Dataspace backlog gauges は alias/description metadata を含み、キュー圧とビジネスドメインの関連付けを支援する。

## Configuration & Norito types

- `LaneCatalog`, `LaneConfig`, `DataSpaceCatalog` は `iroha_data_model::nexus` にあり、manifests と SDKs 向けの Norito 互換構造を提供する。
- `LaneConfig` は `iroha_config::parameters::actual::Nexus` にあり、catalog から自動導出される。内部 runtime helper なので Norito encoding は不要。
- ユーザー向け設定 (`iroha_config::parameters::user::Nexus`) は宣言的な lane/dataspace descriptors を引き続き受け付ける。パース時にジオメトリを導出し、無効な aliases や重複 lane IDs を拒否する。

## Outstanding work

- settlement router updates (NX-3) を新しいジオメトリに統合し、XOR buffer のデビット/レシートが lane slug でタグ付けされるようにする。
- 管理 tooling を拡張し、column families の一覧、退役 lane の compaction、slugged namespace による lane 別 block logs の検査を可能にする。
- merge アルゴリズム (ordering, pruning, conflict detection) を最終化し、cross-lane replay の regression fixtures を追加する。
- whitelists/blacklists と programmable-money policies の compliance hooks を追加する (NX-12 で追跡)。

---

*このページは NX-2 から NX-18 の着地に合わせて NX-1 の follow-ups を継続追跡する。未解決の質問は `roadmap.md` または governance tracker に共有し、ポータルが正本ドキュメントと整合するようにする。*
