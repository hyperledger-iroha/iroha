---
lang: zh-hant
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

# 橋接最終性證明

本文檔描述了 Iroha 的初始橋最終確定性證明表面。
目標是讓外部鍊或輕客戶端驗證 Iroha 區塊
無需鏈下計算或可信中繼即可最終確定。

## 證明格式

`BridgeFinalityProof` (Norito/JSON) 包含：

- `height`：區塊高度。
- `chain_id`：Iroha 鏈標識符，以防止跨鏈重放。
- `block_header`：規範 `BlockHeader`。
- `block_hash`：標頭的哈希值（客戶端重新計算以驗證）。
- `commit_certificate`：驗證器集+最終確定該塊的簽名。
- `validator_set_pops`：與驗證器集對齊的所有權證明字節
  訂單（BLS 聚合驗證所需）。

證明是獨立的；不需要外部清單或不透明的 blob。
保留：Torii 為最近的提交證書窗口提供最終性證明
（受配置的歷史上限限制；默認為 512 個條目
`sumeragi.commit_cert_history_cap` / `SUMERAGI_COMMIT_CERT_HISTORY_CAP`）。客戶
如果需要更長的視野，應該緩存或錨定證明。
規范元組是 `(block_header, block_hash, commit_certificate)`：
標頭的哈希值必須與提交證書內的哈希值匹配，並且
鏈 ID 將證明綁定到單個分類帳。服務器拒絕並記錄
`CommitCertificateHashMismatch` 當證書指向不同的塊時
哈希。

## 承諾包

`BridgeFinalityBundle` (Norito/JSON) 通過顯式擴展了基本證明
承諾和理由：

- `commitment`: `{ chain_id, authority_set { id, validator_set, validator_set_hash, validator_set_hash_version }, block_height, block_hash, mmr_root?, mmr_leaf_index?, mmr_peaks?, next_authority_set? }`
- `justification`：來自承諾集權威機構的簽名
  有效負載（重用提交證書籤名）。
- `block_header`，`commit_certificate`：與基本證明相同。

當前佔位符：`mmr_root`/`mmr_peaks` 是通過重新計算得出的
內存中的塊哈希 MMR；包含證明尚未返回。客戶可以
今天仍然通過承諾有效負載驗證相同的哈希值。

MMR 峰從左到右排列。通過裝袋峰值重新計算 `mmr_root`
從右到左：`root = H(p_n, H(p_{n-1}, ... H(p_1, p_0)))`。

API：`GET /v1/bridge/finality/bundle/{height}`（Norito/JSON）。

驗證類似於基本證明：從
header，驗證提交證書籤名，並檢查承諾
字段與證書和塊哈希匹配。該捆綁包增加了承諾/
喜歡分離的橋接協議的理由包裝器。

## 驗證步驟1、從`block_header`重新計算`block_hash`；拒絕不匹配。
2.檢查`commit_certificate.block_hash`與重新計算的`block_hash`是否匹配；
   拒絕不匹配的標頭/提交證書對。
3. 檢查 `chain_id` 是否與預期的 Iroha 鏈匹配。
4. 根據 `commit_certificate.validator_set` 重新計算 `validator_set_hash` 並
   檢查它是否與記錄的哈希/版本匹配。
5. 確保 `validator_set_pops` 長度與驗證器集匹配並驗證
   每個 PoP 都針對其 BLS 公鑰。
6. 使用以下命令根據標頭哈希驗證提交證書中的簽名
   引用的驗證器公鑰和索引；強制執行法定人數
   （當 `n>3` 時為 `2f+1`，否則為 `n`）並拒絕重複/超出範圍的索引。
7. 有選擇地通過比較驗證器集哈希來綁定到受信任的檢查點
   到錨定值（弱主觀錨定）。
8. 可以選擇綁定到預期的紀元錨，以便來自較舊/較新的證明
   紀元被拒絕，直到錨被有意旋轉。

`BridgeFinalityVerifier`（在 `iroha_data_model::bridge` 中）應用這些檢查，
拒絕鏈 ID/高度漂移、驗證器集哈希/版本不匹配、缺失
或無效的 PoP、重複/超出範圍的簽名者、無效簽名，以及
在計算法定人數之前意外的紀元，以便輕客戶端可以重複使用單個
驗證者。

## 參考驗證器

`BridgeFinalityVerifier` 接受預期的 `chain_id` 以及可選的可信
驗證器集和紀元錨。它強制標頭/塊哈希/
提交證書元組，驗證驗證器集哈希/版本，檢查
根據公佈的驗證者名冊進行簽名/法定人數，並跟踪最新的
拒絕陳舊/跳過的校樣的高度。當提供錨時，它會拒絕
通過明確的 `UnexpectedEpoch`/ 跨紀元/名冊重播
`UnexpectedValidatorSet` 錯誤；沒有錨點，它採用第一個證明
在繼續執行重複/超出範圍之前驗證器設置哈希和紀元
具有確定性錯誤的範圍/不足的簽名。

## API 接口

- `GET /v1/bridge/finality/{height}` – 返回 `BridgeFinalityProof`
  請求的塊高度。通過 `Accept` 的內容協商支持 Norito 或
  JSON。
- `GET /v1/bridge/finality/bundle/{height}` – 返回 `BridgeFinalityBundle`
  （承諾+理由+標頭/證書）所需的高度。

## 註釋和後續行動

- 證明當前源自存儲的提交證書。有界的
  歷史記錄遵循提交證書保留窗口；客戶端應該緩存
  如果他們需要更長的視野，則錨定證明。窗口外請求返回
  `CommitCertificateNotFound(height)`；暴露錯誤並回退到
  錨定檢查站。
- 重放或偽造的證據，其 `block_hash` 不匹配（標頭與標頭）
  證書）被拒絕，編號為 `CommitCertificateHashMismatch`；客戶應該
  在簽名驗證之前執行相同的元組檢查並丟棄
  有效負載不匹配。
- 未來的工作可以添加 MMR/權威集承諾鏈以減少證明大小
  更豐富的承諾信封內的提交證書。