---
id: chunker-registry
lang: zh-hant
direction: ltr
source: docs/portal/docs/sorafs/chunker-registry.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: SoraFS Chunker Profile Registry
sidebar_label: Chunker Registry
description: Profile IDs, parameters, and negotiation plan for the SoraFS chunker registry.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::注意規範來源
:::

## SoraFS Chunker 配置文件註冊表 (SF-2a)

SoraFS 堆棧通過小型命名空間註冊表協商分塊行為。
每個配置文件分配確定性 CDC 參數、semver 元數據和
預期在清單和 CAR 檔案中使用摘要/多編解碼器。

簡介作者應諮詢
[`docs/source/sorafs/chunker_profile_authoring.md`](./chunker-profile-authoring.md)
之前所需的元數據、驗證清單和提案模板
提交新條目。一旦治理批准變更，請遵循
[註冊表推出清單](./chunker-registry-rollout-checklist.md) 和
[暫存清單劇本](./staging-manifest-playbook) 推廣
通過舞台和生產的固定裝置。

### 個人資料

|命名空間 |名稱 |語義版本 |個人資料 ID |最小（字節）|目標（字節）|最大（字節）|打破面具|多重哈希 |別名 |筆記|
|------------|------|--------|------------|------------|----------------|-------------|------------|------------|---------|--------|
| `sorafs` | `sf1` | `1.0.0` | `1` | 65536 | 262144 | 262144 524288 | 524288 `0x0000ffff` | `0x1f` (BLAKE3-256) | `["sorafs.sf1@1.0.0"]` | SF-1 燈具中使用的規範輪廓 |

註冊表的代碼為 `sorafs_manifest::chunker_registry`（由 [`chunker_registry_charter.md`](./chunker-registry-charter.md) 管理）。每個條目
表示為 `ChunkerProfileDescriptor`，其中：

* `namespace` – 相關配置文件的邏輯分組（例如，`sorafs`）。
* `name` – 人類可讀的配置文件標籤（`sf1`、`sf1-fast`，...）。
* `semver` – 參數集的語義版本字符串。
* `profile` – 實際 `ChunkProfile`（最小值/目標/最大值/掩碼）。
* `multihash_code` – 生成塊摘要時使用的多重哈希 (`0x1f`
  對於 SoraFS 默認值）。

清單通過 `ChunkingProfileV1` 序列化配置文件。結構記錄
註冊表元數據（命名空間、名稱、semver）以及原始 CDC
參數和別名列表如上所示。消費者應首先嘗試
通過 `profile_id` 進行註冊表查找，並在以下情況下回退到內聯參數：
出現未知 ID。註冊機構章程規則需要規範句柄
(`namespace.name@semver`) 是 `profile_aliases` 中的第一個條目。

要通過工具檢查註冊表，請運行幫助器 CLI：

```
$ cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- --list-profiles
[
  {
    "namespace": "sorafs",
    "name": "sf1",
    "semver": "1.0.0",
    "handle": "sorafs.sf1@1.0.0",
    "profile_id": 1,
    "min_size": 65536,
    "target_size": 262144,
    "max_size": 524288,
    "break_mask": "0x0000ffff",
    "multihash_code": 31
  }
]

All of the CLI flags that write JSON (`--json-out`, `--por-json-out`, `--por-proof-out`,
`--por-sample-out`) accept `-` as the path, which streams the payload to stdout instead of
creating a file. This makes it easy to pipe the data into tooling while still keeping the
default behaviour of printing the main report.

To inspect a specific PoR witness, provide chunk/segment/leaf indices and
optionally persist the proof to disk:

```
$ cars run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- ./docs.tar \
    --por-proof=0:0:0 --por-proof-out=leaf.proof.json
```

You can select a profile by numeric id (`--profile-id=1`) or by registry handle
(`--profile=sorafs.sf1@1.0.0`); the handle form is convenient for scripts that
thread namespace/name/semver directly from governance metadata.

Use `--promote-profile=<handle>` to emit a JSON metadata block (including all
registered aliases) that can be pasted into `chunker_registry_data.rs` when
promoting a new default profile:

```
$ 貨物運行 -p sorafs_manifest --bin sorafs_manifest_chunk_store -- \
    --promote-profile=sorafs.sf1@1.0.0
```

The main report (and optional proof file) include the root digest, the sampled
leaf bytes (hex-encoded), and the segment/chunk sibling digests so verifiers can
rehash the 64 KiB/4 KiB layers against the `por_root_hex` value.

To validate an existing proof against a payload, pass the path via
`--por-proof-verify` (the CLI adds `"por_proof_verified": true` when the witness
matches the computed root):

```
$ cars run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- ./docs.tar \
    --por-proof-verify=leaf.proof.json
```

For batch sampling, use `--por-sample=<count>` and optionally provide a seed/
output path. The CLI guarantees deterministic ordering (`splitmix64` seeded)
and will transparently truncate when the request exceeds the available leaves:

```
$ cars run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- ./docs.tar \
    --por-sample=8 --por-sample-seed=0xfeedface --por-sample-out=por.samples.json

清單存根鏡像相同的數據，這在管道中編寫 `--chunker-profile-id` 選擇腳本時很方便。兩個塊存儲 CLI 還接受規範句柄形式 (`--profile=sorafs.sf1@1.0.0`)，因此構建腳本可以避免硬編碼數字 ID：

```
$ cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- --list-chunker-profiles
[
  {
    "profile_id": 1,
    "namespace": "sorafs",
    "name": "sf1",
    "semver": "1.0.0",
    "handle": "sorafs.sf1@1.0.0",
    "min_size": 65536,
    "target_size": 262144,
    "max_size": 524288,
    "break_mask": "0x0000ffff",
    "multihash_code": 31
  }
]
```

`handle` 字段 (`namespace.name@semver`) 與 CLI 接受的內容相匹配
`--profile=…`，可以安全地直接複製到自動化中。

### 談判分塊

網關和客戶端通過提供商廣告宣傳支持的配置文件：

```
ProviderAdvertBodyV1 {
    ...
    chunk_profile: profile_id (implicit via registry)
    capabilities: [...]
}
```

多源塊調度是通過 `range` 功能宣布的。的
CLI 通過 `--capability=range[:streams]` 接受它，其中可選數字
後綴對提供者的首選範圍獲取並發進行編碼（例如，
`--capability=range:64` 公佈 64 流預算）。當省略時，消費者
回到廣告中其他地方發布的一般 `max_streams` 提示。

請求 CAR 數據時，客戶端應發送 `Accept-Chunker` 標頭列表
按優先順序支持的 `(namespace, name, semver)` 元組：

```
Accept-Chunker: sorafs.sf1;version=1.0.0
```

網關選擇相互支持的配置文件（默認為 `sorafs.sf1@1.0.0`）
並通過 `Content-Chunker` 響應標頭反映該決定。艙單
嵌入所選配置文件，以便下游節點可以驗證塊佈局
不依賴 HTTP 協商。

### 一致性

* `sorafs.sf1@1.0.0` 配置文件映射到公共裝置
  `fixtures/sorafs_chunker` 和註冊的語料庫
  `fuzz/sorafs_chunker`。 Rust、Go 和 Node 中執行端到端奇偶校驗
  通過提供的測試。
* `chunker_registry::lookup_by_profile` 斷言描述符參數
  匹配 `ChunkProfile::DEFAULT` 以防止意外發散。
* `iroha app sorafs toolkit pack` 和 `sorafs_manifest_stub` 生成的清單包括註冊表元數據。