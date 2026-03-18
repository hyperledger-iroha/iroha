---
lang: zh-hans
direction: ltr
source: docs/portal/docs/sorafs/chunker-registry.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: d43c9ac18ba6b9e4e7941325b2ebdec672b01627747e3272927557faf82957af
source_last_modified: "2026-01-22T14:35:36.798331+00:00"
translation_last_reviewed: 2026-02-07
id: chunker-registry
title: SoraFS Chunker Profile Registry
sidebar_label: Chunker Registry
description: Profile IDs, parameters, and negotiation plan for the SoraFS chunker registry.
translator: machine-google-reviewed
---

:::注意规范来源
:::

## SoraFS Chunker 配置文件注册表 (SF-2a)

SoraFS 堆栈通过小型命名空间注册表协商分块行为。
每个配置文件分配确定性 CDC 参数、semver 元数据和
预期在清单和 CAR 档案中使用摘要/多编解码器。

简介作者应咨询
[`docs/source/sorafs/chunker_profile_authoring.md`](./chunker-profile-authoring.md)
之前所需的元数据、验证清单和提案模板
提交新条目。一旦治理批准变更，请遵循
[注册表推出清单](./chunker-registry-rollout-checklist.md) 和
[暂存清单剧本](./staging-manifest-playbook) 推广
通过舞台和生产的固定装置。

### 个人资料

|命名空间 |名称 |语义版本 |个人资料 ID |最小（字节）|目标（字节）|最大（字节）|打破面具|多重哈希 |别名 |笔记|
|------------|------|--------|------------|------------|----------------|-------------|------------|------------|---------|--------|
| `sorafs` | `sf1` | `1.0.0` | `1` | 65536 | 262144 | 262144 524288 | 524288 `0x0000ffff` | `0x1f` (BLAKE3-256) | `["sorafs.sf1@1.0.0"]` | SF-1 灯具中使用的规范轮廓 |

注册表的代码为 `sorafs_manifest::chunker_registry`（由 [`chunker_registry_charter.md`](./chunker-registry-charter.md) 管理）。每个条目
表示为 `ChunkerProfileDescriptor`，其中：

* `namespace` – 相关配置文件的逻辑分组（例如，`sorafs`）。
* `name` – 人类可读的配置文件标签（`sf1`、`sf1-fast`，...）。
* `semver` – 参数集的语义版本字符串。
* `profile` – 实际 `ChunkProfile`（最小值/目标/最大值/掩码）。
* `multihash_code` – 生成块摘要时使用的多重哈希 (`0x1f`
  对于 SoraFS 默认值）。

清单通过 `ChunkingProfileV1` 序列化配置文件。结构记录
注册表元数据（命名空间、名称、semver）以及原始 CDC
参数和别名列表如上所示。消费者应首先尝试
通过 `profile_id` 进行注册表查找，并在以下情况下回退到内联参数：
出现未知 ID。注册机构章程规则需要规范句柄
(`namespace.name@semver`) 是 `profile_aliases` 中的第一个条目。

要通过工具检查注册表，请运行帮助器 CLI：

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
$ 货物运行 -p sorafs_manifest --bin sorafs_manifest_chunk_store -- \
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

清单存根镜像相同的数据，这在管道中编写 `--chunker-profile-id` 选择脚本时很方便。两个块存储 CLI 还接受规范句柄形式 (`--profile=sorafs.sf1@1.0.0`)，因此构建脚本可以避免硬编码数字 ID：

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

`handle` 字段 (`namespace.name@semver`) 与 CLI 接受的内容相匹配
`--profile=…`，可以安全地直接复制到自动化中。

### 谈判分块

网关和客户端通过提供商广告宣传支持的配置文件：

```
ProviderAdvertBodyV1 {
    ...
    chunk_profile: profile_id (implicit via registry)
    capabilities: [...]
}
```

多源块调度是通过 `range` 功能宣布的。的
CLI 通过 `--capability=range[:streams]` 接受它，其中可选数字
后缀对提供者的首选范围获取并发进行编码（例如，
`--capability=range:64` 公布 64 流预算）。当省略时，消费者
回到广告中其他地方发布的一般 `max_streams` 提示。

请求 CAR 数据时，客户端应发送 `Accept-Chunker` 标头列表
按优先顺序支持的 `(namespace, name, semver)` 元组：

```
Accept-Chunker: sorafs.sf1;version=1.0.0
```

网关选择相互支持的配置文件（默认为 `sorafs.sf1@1.0.0`）
并通过 `Content-Chunker` 响应标头反映该决定。舱单
嵌入所选配置文件，以便下游节点可以验证块布局
不依赖 HTTP 协商。

### 一致性

* `sorafs.sf1@1.0.0` 配置文件映射到公共装置
  `fixtures/sorafs_chunker` 和注册的语料库
  `fuzz/sorafs_chunker`。 Rust、Go 和 Node 中执行端到端奇偶校验
  通过提供的测试。
* `chunker_registry::lookup_by_profile` 断言描述符参数
  匹配 `ChunkProfile::DEFAULT` 以防止意外发散。
* `iroha app sorafs toolkit pack` 和 `sorafs_manifest_stub` 生成的清单包括注册表元数据。