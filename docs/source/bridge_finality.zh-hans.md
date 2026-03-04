---
lang: zh-hans
direction: ltr
source: docs/source/bridge_finality.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 2e4c6ed5974f623906f51259a634bcad5df703bcec899630ae29f4669b289ab6
source_last_modified: "2026-01-08T21:52:45.509525+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
SPDX-License-Identifier: Apache-2.0
-->

# 桥接最终性证明

本文档描述了 Iroha 的初始桥最终确定性证明表面。
目标是让外部链或轻客户端验证 Iroha 区块
无需链下计算或可信中继即可最终确定。

## 证明格式

`BridgeFinalityProof` (Norito/JSON) 包含：

- `height`：区块高度。
- `chain_id`：Iroha 链标识符，以防止跨链重放。
- `block_header`：规范 `BlockHeader`。
- `block_hash`：标头的哈希值（客户端重新计算以验证）。
- `commit_certificate`：验证器集+最终确定该块的签名。
- `validator_set_pops`：与验证器集对齐的所有权证明字节
  订单（BLS 聚合验证所需）。

证明是独立的；不需要外部清单或不透明的 blob。
保留：Torii 为最近的提交证书窗口提供最终性证明
（受配置的历史上限限制；默认为 512 个条目
`sumeragi.commit_cert_history_cap` / `SUMERAGI_COMMIT_CERT_HISTORY_CAP`）。客户
如果需要更长的视野，应该缓存或锚定证明。
规范元组是 `(block_header, block_hash, commit_certificate)`：
标头的哈希值必须与提交证书内的哈希值匹配，并且
链 ID 将证明绑定到单个分类帐。服务器拒绝并记录
`CommitCertificateHashMismatch` 当证书指向不同的块时
哈希。

## 承诺包

`BridgeFinalityBundle` (Norito/JSON) 通过显式扩展了基本证明
承诺和理由：

- `commitment`: `{ chain_id, authority_set { id, validator_set, validator_set_hash, validator_set_hash_version }, block_height, block_hash, mmr_root?, mmr_leaf_index?, mmr_peaks?, next_authority_set? }`
- `justification`：来自承诺集权威机构的签名
  有效负载（重用提交证书签名）。
- `block_header`，`commit_certificate`：与基本证明相同。

当前占位符：`mmr_root`/`mmr_peaks` 是通过重新计算得出的
内存中的块哈希 MMR；包含证明尚未返回。客户可以
今天仍然通过承诺有效负载验证相同的哈希值。

MMR 峰从左到右排列。通过装袋峰值重新计算 `mmr_root`
从右到左：`root = H(p_n, H(p_{n-1}, ... H(p_1, p_0)))`。

API：`GET /v1/bridge/finality/bundle/{height}`（Norito/JSON）。

验证类似于基本证明：从
header，验证提交证书签名，并检查承诺
字段与证书和块哈希匹配。该捆绑包增加了承诺/
喜欢分离的桥接协议的理由包装器。

## 验证步骤1、从`block_header`重新计算`block_hash`；拒绝不匹配。
2.检查`commit_certificate.block_hash`与重新计算的`block_hash`是否匹配；
   拒绝不匹配的标头/提交证书对。
3. 检查 `chain_id` 是否与预期的 Iroha 链匹配。
4. 根据 `commit_certificate.validator_set` 重新计算 `validator_set_hash` 并
   检查它是否与记录的哈希/版本匹配。
5. 确保 `validator_set_pops` 长度与验证器集匹配并验证
   每个 PoP 都针对其 BLS 公钥。
6. 使用以下命令根据标头哈希验证提交证书中的签名
   引用的验证器公钥和索引；强制执行法定人数
   （当 `n>3` 时为 `2f+1`，否则为 `n`）并拒绝重复/超出范围的索引。
7. 有选择地通过比较验证器集哈希来绑定到受信任的检查点
   到锚定值（弱主观锚定）。
8. 可以选择绑定到预期的纪元锚，以便来自较旧/较新的证明
   纪元被拒绝，直到锚被有意旋转。

`BridgeFinalityVerifier`（在 `iroha_data_model::bridge` 中）应用这些检查，
拒绝链 ID/高度漂移、验证器集哈希/版本不匹配、缺失
或无效的 PoP、重复/超出范围的签名者、无效签名，以及
在计算法定人数之前意外的纪元，以便轻客户端可以重复使用单个
验证者。

## 参考验证器

`BridgeFinalityVerifier` 接受预期的 `chain_id` 以及可选的可信
验证器集和纪元锚。它强制标头/块哈希/
提交证书元组，验证验证器集哈希/版本，检查
根据公布的验证者名册进行签名/法定人数，并跟踪最新的
拒绝陈旧/跳过的校样的高度。当提供锚时，它会拒绝
通过明确的 `UnexpectedEpoch`/ 跨纪元/名册重播
`UnexpectedValidatorSet` 错误；没有锚点，它采用第一个证明
在继续执行重复/超出范围之前验证器设置哈希和纪元
具有确定性错误的范围/不足的签名。

## API 接口

- `GET /v1/bridge/finality/{height}` – 返回 `BridgeFinalityProof`
  请求的块高度。通过 `Accept` 的内容协商支持 Norito 或
  JSON。
- `GET /v1/bridge/finality/bundle/{height}` – 返回 `BridgeFinalityBundle`
  （承诺+理由+标头/证书）所需的高度。

## 注释和后续行动

- 证明当前源自存储的提交证书。有界的
  历史记录遵循提交证书保留窗口；客户端应该缓存
  如果他们需要更长的视野，则锚定证明。窗口外请求返回
  `CommitCertificateNotFound(height)`；暴露错误并回退到
  锚定检查站。
- 重放或伪造的证据，其 `block_hash` 不匹配（标头与标头）
  证书）被拒绝，编号为 `CommitCertificateHashMismatch`；客户应该
  在签名验证之前执行相同的元组检查并丢弃
  有效负载不匹配。
- 未来的工作可以添加 MMR/权威集承诺链以减少证明大小
  更丰富的承诺信封内的提交证书。