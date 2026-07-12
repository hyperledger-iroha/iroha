---
lang: zh-hant
direction: ltr
source: docs/source/bridge_finality.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 1cbd248fe14e63d00f002f09e1663181f3ab9bd99124ffeb89c56763b784046b
source_last_modified: "2026-07-12"
translation_last_reviewed: 2026-07-12
translator: machine-google-reviewed
---

<!--
SPDX-License-Identifier: Apache-2.0
-->

# 橋接最終性證明

本文定義首次發佈的橋接最終性格式。證明攜帶 Sumeragi v2 產生並持久化的精確最終性證據。
證明外層的綱要版本為 `1`，其中的共識協定版本為 `2`。不存在 Sumeragi v1 憑證投影、
解碼器或回退路徑。

## 精確證明格式

採用 Norito 或 Norito JSON 編碼的 `BridgeFinalityProof` 只有三個欄位：

```text
{ version, block_header, finality_artifact }
```

- `version` 必須為 `1`；
- `block_header` 是要求高度的規範 `BlockHeader`；
- `finality_artifact` 是該區塊持久化的精確 `V2FinalityArtifact`。它按高度上下文中的
  驗證器名冊順序，持久地內嵌每個驗證器的 BLS-normal PoP（`validator_set_pops`）。

該製品包含完整且不可變的 `HeightContext`、精確的 `BlockSubject`、區塊雜湊、CommitQC
及與名冊對齊的 PoP。高度上下文凍結鏈、epoch、名冊、`DualQuorum`、DA 佈局和 leader
seed 等共識資料。結束 epoch 的父區塊上下文還包含可選的 `next_epoch_snapshot`；該欄位
參與 context id，因此父 CommitQC 會先認證它，之後它才能授權子名冊。最終快照還會認證
`epoch_end_height`、下一名冊對齊的 `validator_set_pops` 以及下一 epoch 的參數。

## 持久化與驗證

Kura 在發布 finality 或淘汰 block body 之前，先將精確的 canonical header 和由 root
認證的 SCCP archive 寫入 immutable retained-block record，再把 exact V2 artifact
保存到單獨的 immutable finality record。兩次寫入都具有 idempotent、no-clobber
語意，並拒絕同一高度的衝突。`build_finality_proof` 只讀取 retained header 和已驗證的
finality record，絕不讀取 historical block body，也不會用 mutable world state 取代
PoP。重新啟動時會再次驗證 header/archive/artifact/hash association。淘汰 block body
不會使有效 proof 無法取得；record 缺失、損壞、衝突或無法驗證時一律 fail closed。

無狀態驗證器嚴格核對版本、鏈、高度、header 雜湊、規範前驅和 view、上下文、subject 和
CommitQC，並驗證製品中的全部 PoP。簽名者索引必須嚴格遞增且在範圍內；CommitQC 必須同時滿足驗證器數量
和投票權重兩個 quorum，且針對精確 Sumeragi v2 投票 preimage 的 BLS 聚合簽名必須有效。

## 信任錨與後繼驗證

單一證明只能說明它在自身攜帶的名冊下內部一致。`BridgeFinalityVerifier` 在接受第一個證明前
必須取得明確信任的 `HeightContextId`。此後它只接受緊鄰的下一高度，並使用前一個凍結名冊
及其 PoP 驗證子上下文的父 CommitQC。epoch 內的子製品複製前一製品的 PoP；在 epoch 邊界，
epoch、名冊、quorum、seed 和 PoP 必須符合前一個父上下文中由其 CommitQC 認證的
`next_epoch_snapshot`，包括已認證的 `epoch_end_height`。舊高度、跳躍高度和未連結後繼都會被拒絕。

SCCP 使用同一個 `BridgeFinalityProof`。不能只信任訊息自帶名冊下的簽名；必須從治理固定的
checkpoint context/artifact 開始，驗證到訊息製品為止的每個緊鄰後繼。

## Bundle 與 API

`BridgeFinalityBundle` 恰好為 `{ commitment, finality_proof }`。commitment 為
`{ chain_id, height_context_id, block_height, block_hash }`。

- `GET /v1/bridge/finality/{height}` 傳回 `BridgeFinalityProof`；
- `GET /v1/bridge/finality/bundle/{height}` 傳回 `BridgeFinalityBundle`。

若 retained canonical header 或精確的持久 v2 製品缺失或無效，兩個端點都會關閉失敗。
淘汰 historical block body 不會使有效 proof 無法取得。消費者必須拒絕未知欄位、不支援的版本和已廢棄的證明形狀。
