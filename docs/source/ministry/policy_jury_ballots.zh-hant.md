---
lang: zh-hant
direction: ltr
source: docs/source/ministry/policy_jury_ballots.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: ff3faabda5f1c277f545b7edbbc93f3b58dee65cec943cfd464a026b2984a146
source_last_modified: "2025-12-29T18:16:35.979378+00:00"
translation_last_reviewed: 2026-02-07
title: Policy Jury Sortition & Ballots
translator: machine-google-reviewed
---

路線圖項目 **MINFO-5 — 政策陪審團投票工具包** 需要便攜式格式
用於確定性陪審員選擇以及密封提交 → 揭示選票。  的
`iroha_data_model::ministry::jury` 模塊現在提供三個 Norito 有效負載，
涵蓋整個投票流程：

1. **`PolicyJurySortitionV1`** – 記錄抽獎元數據（提案 ID、
   回合 ID、身份證明快照摘要、隨機性信標），
   委員會規模、選定的陪審員以及用於自動確定的候補名單
   故障轉移。  每個主插槽可能包含 `PolicyJuryFailoverPlan`
   指向候補名單排名，在其寬限後應升級到
   經期流逝。  該結構是有意確定性的，因此審計員
   可以重放抽籤並從同一 POP 重新生成清單
   快照+信標。
2. **`PolicyJuryBallotCommitV1`** – 投票前寫下的密封承諾
   被揭露。  它存儲回合/提案/陪審員標識符、
   Blake2b-256 陪審員 ID + 投票選擇 + 隨機數元組摘要，捕獲
   時間戳和選票模式（`plaintext` 或 `zk-envelope`
   `zk-ballot` 功能已激活）。  `PolicyJuryBallotCommitV1::verify_reveal`
   確保存儲的摘要與顯示有效負載匹配。
3. **`PolicyJuryBallotRevealV1`** – 包含以下內容的公共揭示對象
   投票選擇、提交時使用的隨機數以及可選的 ZK 證明 URI。
   Reveals 需要至少 16 字節的隨機數，以便治理可以處理
   即使陪審員通過不安全的渠道運作，承諾也具有約束力。

`PolicyJurySortitionV1::validate` 幫助程序強制執行委員會規模調整，
重複檢測（評審員不得同時出現在委員會和評審委員會中）
等候名單）、有序的等候名單排名以及有效的故障轉移參考。  選票
當提案或回合 ID 時，驗證例程會引發 `PolicyJuryBallotError`
漂移，當陪審員試圖用不正確的隨機數來揭示時，或者當
`zk-envelope`承諾未能在其承諾中提供匹配的證明參考
揭示。

### 與客戶整合

- 治理工具應保留抽籤清單並將其包含在
  策略數據包，以便觀察者可以重新計算 POP 快照摘要並
  確認隨機性信標加上候選集會導致相同的結果
  陪審員分配。
- 陪審員客戶在之後立即記錄 `PolicyJuryBallotCommitV1`
  為他們的投票生成隨機數。  導出的承諾字節可以是
  作為 base64 值提交給 Torii 或直接嵌入到 Norito 中
  事件。
- 在揭曉階段，陪審員發出 `PolicyJuryBallotRevealV1`。  運營商
  之前將有效負載饋送到 `PolicyJuryBallotCommitV1::verify_reveal`
  接受投票，確保信息不被交換或篡改。
- 當啟用 `zk-ballot` 功能時，陪審員可以附加確定性
  證明 URI（例如，`sorafs://proofs/pj-2026-02/juror-5`），以便下游
  審計員可以檢索引用的零知識見證包
  承諾。所有三個結構都派生出 `Encode`、`Decode` 和 `IntoSchema`，這意味著它們
可用於 ISI 流、CLI 工具、SDK 和治理 REST API。
請參閱 `crates/iroha_data_model/src/ministry/jury.rs` 以了解規範的 Rust
定義和輔助方法。

### CLI 對抽籤清單的支持

路線圖項目 **MINFO-5** 還呼籲使用可重複的工具，以便治理可以
在每次公投數據包發布之前發送可驗證的政策陪審團名冊。
工作區現在公開 `cargo xtask ministry-jury sortition` 命令：

```bash
cargo xtask ministry-jury sortition \
  --roster docs/examples/ministry/policy_jury_roster_example.json \
  --proposal AC-2026-042 \
  --round PJ-2026-02 \
  --beacon 22b1e48d47123f5c9e3f0cc0c8e34aa3c5f9c49a2cbb70559d3cb0ddc1a6ef01 \
  --committee-size 3 \
  --waitlist-size 2 \
  --drawn-at 2026-01-15T09:00:00Z \
  --waitlist-ttl-hours 72 \
  --out artifacts/ministry/policy_jury_sortition.json
```

- `--roster` 接受確定性 PoP 名冊（JSON 示例：
  `docs/examples/ministry/policy_jury_roster_example.json`）。  每個條目
  聲明 `juror_id`、`pop_identity`、重量和可選
  `grace_period_secs`。  不符合條件的條目將被自動過濾。
- `--beacon` 注入治理中捕獲的 32 字節隨機信標
  分鐘。  CLI 將信標直接連接到 ChaCha20 RNG，以便審核員
  可以逐字節重放繪製。
- `--committee-size`、`--waitlist-size` 和 `--waitlist-ttl-hours` 控制
  就座陪審員數量、故障轉移緩衝區以及應用的到期時間戳
  到候補名單條目。  當某個插槽存在故障轉移等級時，該命令
  記錄指向匹配的候補名單排名的 `PolicyJuryFailoverPlan`。
- `--drawn-at` 記錄抽籤的掛鐘時間戳；工具
  將其轉換為清單中的 Unix 毫秒。

生成的清單是經過充分驗證的 `PolicyJurySortitionV1` 有效負載。
大型部署通常將輸出保存在 `artifacts/ministry/` 下，因此
可以與審查小組一起直接捆綁到公投包中
總結。  說明性輸出可在
`docs/examples/ministry/policy_jury_sortition_example.json`，以便 SDK 團隊可以
運行其 Norito 解碼器，而無需在本地重播整個繪製過程。

### 投票提交/顯示助手

陪審員客戶也需要用於提交 → 揭示流程的確定性工具。
相同的 `cargo xtask ministry-jury` 命令現在公開以下幫助程序：

```bash
cargo xtask ministry-jury ballot commit \
  --proposal AC-2026-042 \
  --round PJ-2026-02 \
  --juror citizen:ada \
  --choice approve \
  --nonce-hex aabbccddeeff00112233445566778899aabbccddeeff00112233445566778899 \
  --committed-at 2026-02-01T13:00:00Z \
  --out artifacts/ministry/policy_jury_commit_ada.json \
  --reveal-out artifacts/ministry/policy_jury_reveal_ada.json

cargo xtask ministry-jury ballot verify \
  --commit artifacts/ministry/policy_jury_commit_ada.json \
  --reveal artifacts/ministry/policy_jury_reveal_ada.json
```

- `ballot commit` 發出 `PolicyJuryBallotCommitV1` JSON 負載。  當
  `--out` 被省略，該命令將承諾打印到標準輸出。  如果
  `--reveal-out` 提供的工具還寫入匹配的
  `PolicyJuryBallotRevealV1`，重用提供的隨機數並應用
  可選 `--revealed-at` 時間戳（默認為 `--committed-at` 或
  當前時間）。
- `--nonce-hex` 接受任何長度≥16 字節的偶數十六進製字符串。  當省略時
  helper 使用 `OsRng` 生成 32 字節隨機數，從而可以輕鬆編寫腳本
  無需自定義隨機性管道的陪審員工作流程。
- `--choice` 不區分大小寫，接受 `approve`、`reject` 或 `abstain`。

`ballot verify` 通過交叉檢查承諾/揭示對
`PolicyJuryBallotCommitV1::verify_reveal`，保證輪次id，
提案 ID、陪審員 ID、隨機數和投票選擇在揭曉之前全部對齊
考入Torii。  驗證時助手以非零狀態退出
失敗，從而可以安全地連接到 CI 或當地陪審員門戶。