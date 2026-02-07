---
lang: zh-hant
direction: ltr
source: docs/source/kaigi_privacy_design.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 6b7ffca7e960376a2959357cd865d8dab5afa1dfcb959adbc688b6db60977c8f
source_last_modified: "2026-01-05T09:28:12.022066+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Kaigi 隱私和中繼設計

本文檔捕捉了引入零知識的以隱私為中心的演變
參與證明和洋蔥式中繼，而不犧牲確定性或
賬本可審計性。

# 概述

該設計跨越三層：

- **名冊隱私** – 在鏈上隱藏參與者身份，同時保持主持人權限和計費一致。
- **使用不透明** – 允許主機記錄計量使用情況，而無需公開披露每個段的詳細信息。
- **覆蓋中繼** – 通過多跳對等點路由傳輸數據包，因此網絡觀察者無法了解哪些參與者進行通信。

所有添加內容均保持 Norito 優先，在 ABI 版本 1 下運行，並且必須跨異構硬件確定性執行。

# 目標

1. 使用零知識證明接納/驅逐參與者，這樣賬本就不會暴露原始賬戶 ID。
2. 保持強大的會計保證：每個加入、離開和使用事件仍然必須確定性地協調。
3. 提供可選的中繼清單，描述控制/數據通道的洋蔥路由，並可以在鏈上進行審核。
4. 對於不需要隱私的部署，保持後備（完全透明的名冊）可操作。

# 威脅模型總結

- **對手：** 網絡觀察者 (ISP)、好奇的驗證者、惡意中繼運營商和半誠實的主機。
- **受保護的資產：** 參與者身份、參與時間、每個分段的使用/計費詳細信息以及網絡路由元數據。
- **假設：** 主機仍然了解鏈外的真實參與者集；賬本節點確定性地驗證證明；覆蓋中繼不受信任，但速率有限； HPKE 和 SNARK 原語已存在於代碼庫中。

# 數據模型更改

所有類型都位於 `iroha_data_model::kaigi` 中。

```rust
/// Commitment to a participant identity (Poseidon hash of account + domain salt).
pub struct KaigiParticipantCommitment {
    pub commitment: FixedBinary<32>,
    pub alias_tag: Option<String>,
}

/// Nullifier unique to each join action, prevents double-use of proofs.
pub struct KaigiParticipantNullifier {
    pub digest: FixedBinary<32>,
    pub issued_at_ms: u64,
}

/// Relay path description used by clients to set up onion routing.
pub struct KaigiRelayManifest {
    pub hops: Vec<KaigiRelayHop>,
    pub expiry_ms: u64,
}

pub struct KaigiRelayHop {
    pub relay_id: AccountId,
    pub hpke_public_key: FixedBinary<32>,
    pub weight: u8,
}
```

`KaigiRecord` 獲得以下字段：

- `roster_commitments: Vec<KaigiParticipantCommitment>` – 啟用隱私模式後，替換公開的 `participants` 列表。經典部署可以在遷移過程中保持兩者均已填充。
- `nullifier_log: Vec<KaigiParticipantNullifier>` – 嚴格僅附加，由滾動窗口限制以保持元數據有限。
- `room_policy: KaigiRoomPolicy` – 選擇會話的查看者身份驗證立場（`Public` 房間鏡像只讀中繼；`Authenticated` 房間在出口轉發數據包之前需要查看者票證）。
- `relay_manifest: Option<KaigiRelayManifest>` – 使用 Norito 編碼的結構化清單，因此躍點、HPKE 密鑰和權重無需 JSON 填充程序即可保持規範。
- `privacy_mode: KaigiPrivacyMode` 枚舉（見下文）。

```rust
pub enum KaigiPrivacyMode {
    Transparent,
    ZkRosterV1,
}
```

`NewKaigi` 接收匹配的可選字段，以便主機可以在創建時選擇隱私。


- 字段使用 `#[norito(with = "...")]` 幫助程序強制執行規範編碼（整數採用小端序，按位置排序躍點）。
- `KaigiRecord::from_new` 將新向量播種為空並複制任何提供的中繼清單。

# 指令表面變化

## 演示快速入門助手

對於臨時演示和互操作性測試，CLI 現在公開
`iroha kaigi quickstart`。它：- 重用 CLI 配置（域 `wonderland` + 帳戶），除非通過 `--domain`/`--host` 覆蓋。
- 當省略 `--call-name` 時生成基於時間戳的調用名稱，並針對活動 Torii 端點提交 `CreateKaigi`。
- 可以選擇自動加入主機 (`--auto-join-host`)，以便觀看者可以立即連接。
- 發出一個 JSON 摘要，其中包含 Torii URL、呼叫標識符、隱私/房間策略、準備複製的加入命令以及假脫機路徑測試人員應監視的內容（例如，`storage/streaming/soranet_routes/exit-<relay-id>/kaigi-stream/*.norito`）。使用 `--summary-out path/to/file.json` 持久保存 blob。

此幫助器**不會**取代對正在運行的 `irohad --sora` 節點的需求：隱私路由、假脫機文件和中繼清單仍然由賬本支持。當為外部團體提供臨時房間時，它只是簡單地修剪了樣板文件。

### 單命令演示腳本

為了獲得更快的路徑，有一個配套腳本：`scripts/kaigi_demo.sh`。
它為您執行以下操作：

1. 將捆綁的 `defaults/nexus/genesis.json` 簽名為 `target/kaigi-demo/genesis.nrt`。
2. 啟動帶有簽名塊的 `irohad --sora`（日誌位於 `target/kaigi-demo/irohad.log` 下）並等待 Torii 公開 `http://127.0.0.1:8080/status`。
3. 運行 `iroha kaigi quickstart --auto-join-host --summary-out target/kaigi-demo/kaigi_summary.json`。
4. 打印 JSON 摘要的路徑以及假脫機目錄 (`storage/streaming/soranet_routes/exit-<relay-id>/kaigi-stream/`)，以便您可以與外部測試人員共享。

環境變量：

- `TORII_URL` — 覆蓋 Torii 端點進行輪詢（默認 `http://127.0.0.1:8080`）。
- `RUN_DIR` — 覆蓋工作目錄（默認 `target/kaigi-demo`）。

按 `Ctrl+C` 停止演示；腳本中的陷阱會自動終止 `irohad`。假脫機文件和摘要保留在磁盤上，以便您可以在進程退出後移交工件。

## `CreateKaigi`

- 根據主機權限驗證 `privacy_mode`。
- 如果提供了 `relay_manifest`，則強制執行 ≥3 跳、非零權重、HPKE 密鑰存在和唯一性，以便鏈上清單保持可審核性。
- 驗證來自 SDK/CLI 的 `room_policy` 輸入（`public` 與 `authenticated`）並將其傳播到 SoraNet 配置，以便中繼緩存公開正確的 GAR 類別（`stream.kaigi.public` 與 `stream.kaigi.authenticated`）。主機通過 `iroha kaigi create --room-policy …`、JS SDK 的 `roomPolicy` 字段進行連接，或者在 Swift 客戶端在提交之前組裝 Norito 有效負載時設置 `room_policy` 來連接。
- 存儲空的承諾/無效日誌。

## `JoinKaigi`

參數：

- `proof: ZkProof`（Norito 字節包裝器） - Groth16 證明證明調用者知道 `(account_id, domain_salt)`，其 Poseidon 哈希等於提供的 `commitment`。
- `commitment: FixedBinary<32>`
- `nullifier: FixedBinary<32>`
- `relay_hint: Option<KaigiRelayHop>` – 下一跳的可選每參與者覆蓋。

執行步驟：

1. 如果 `record.privacy_mode == Transparent`，則回退到當前行為。
2. 根據電路註冊表項 `KAIGI_ROSTER_V1` 驗證 Groth16 證明。
3. 確保 `nullifier` 未出現在 `record.nullifier_log` 中。
4. 追加承諾/無效條目；如果提供了 `relay_hint`，則修補該參與者的中繼清單視圖（僅存儲在內存中會話狀態中，而不存儲在鏈上）。## `LeaveKaigi`

透明模式符合當前邏輯。

私有模式需要：

1. 證明調用者知道 `record.roster_commitments` 中的承諾。
2. 無效器更新證明一次性休假。
3. 刪除承諾/無效條目。審核保留固定保留窗口的墓碑，以避免結構洩漏。

## `RecordKaigiUsage`

通過以下方式擴展有效負載：

- `usage_commitment: FixedBinary<32>` – 對原始使用元組的承諾（持續時間、gas、段 ID）。
- 可選的 ZK 證明，驗證增量是否與賬本外提供的加密日誌相匹配。

主辦方仍然可以提交透明的總計；隱私模式僅使承諾字段成為強制性的。

# 驗證和電路

- `iroha_core::smartcontracts::isi::kaigi::privacy` 現在執行完整名單
  默認驗證。它解析 `zk.kaigi_roster_join_vk` （連接）和
  `zk.kaigi_roster_leave_vk`（離開）來自配置，
  在WSV中查找對應的`VerifyingKeyRef`（確保記錄是
  `Active`，後端/電路標識符匹配，並且承諾一致），費用
  字節計費，並調度到配置的ZK後端。
- `kaigi_privacy_mocks` 功能保留了確定性存根驗證器，因此
  單元/集成測試和受限 CI 作業可以在沒有 Halo2 後端的情況下運行。
  生產版本必須禁用該功能以強制執行真實的證明。
- 如果在某個設備上啟用了 `kaigi_privacy_mocks`，則該包會發出編譯時錯誤
  非測試、非 `debug_assertions` 構建，防止意外發布二進製文件
  隨存根一起運輸。
- 運營商需要（1）註冊通過治理設置的名冊驗證者，以及
  (2) 設置 `zk.kaigi_roster_join_vk`、`zk.kaigi_roster_leave_vk`，以及
  `zk.kaigi_usage_vk` 位於 `iroha_config` 中，以便主機可以在運行時解析它們。
  在密鑰出現之前，隱私加入、離開和使用調用都會失敗
  確定性地。
- `crates/kaigi_zk` 現在提供用於名冊加入/離開和使用的 Halo2 電路
  與可重複使用壓縮機一起的承諾（`commitment`、`nullifier`、
  `usage`）。花名冊電路暴露了 Merkle 根（四個小端字節序）
  64 位肢體）作為額外的公共輸入，以便主機可以交叉檢查證明
  在驗證之前針對存儲的名冊根。使用承諾是
  由 `KaigiUsageCommitmentCircuit` 強制執行，它與 `(duration、gas、
  段）`到賬本哈希。
- `Join` 電路輸入：`(commitment, nullifier, domain_salt)` 和專用
  `(account_id)`。公共輸入包括 `commitment`、`nullifier` 和
  名冊承諾樹的 Merkle 根的四個分支（名冊
  仍然處於鏈外，但根被綁定到轉錄本中）。
- 確定性：我們修復了 Poseidon 參數、電路版本和索引
  註冊表。任何更改都會使 `KaigiPrivacyMode` 變為 `ZkRosterV2` 且匹配
  測試/黃金文件。

# 洋蔥路由覆蓋

## 中繼註冊- 中繼自註冊為域元數據條目 `kaigi_relay::<relay_id>`，包括 HPKE 密鑰材料和帶寬類別。
- `RegisterKaigiRelay` 指令將描述符保留在域元數據中，發出 `KaigiRelayRegistered` 摘要（具有 HPKE 指紋和帶寬類別），並且可以重新調用以確定性地輪換密鑰。
- 治理通過域元數據 (`kaigi_relay_allowlist`) 管理許可名單，並在接受新路徑之前中繼註冊/清單更新強制執行成員資格。

## 清單創建

- 主機從可用中繼構建多跳路徑（最小長度為 3）。清單對 AccountId 序列和加密分層信封所需的 HPKE 公鑰進行編碼。
- 鏈上存儲的 `relay_manifest` 包含跳描述符和過期時間（Norito 編碼的 `KaigiRelayManifest`）；實際的臨時密鑰和每個會話的偏移量使用 HPKE 在賬本外進行交換。

## 信令與媒體

- SDP/ICE 交換通過 Kaigi 元數據繼續進行，但逐跳加密。驗證器只能看到 HPKE 密文和標頭索引。
- 媒體數據包使用帶有密封有效負載的 QUIC 通過中繼傳輸。每跳解密一層，獲知下一跳地址；最終接收者在剝離所有層後獲得媒體流。

## 故障轉移

- 客戶端通過 `ReportKaigiRelayHealth` 指令監控中繼運行狀況，該指令在域元數據 (`kaigi_relay_feedback::<relay_id>`) 中保留簽名反饋，廣播 `KaigiRelayHealthUpdated`，並允許治理/主機推斷當前可用性。當中繼失敗時，主機會發出更新的清單並記錄 `KaigiRelayManifestUpdated` 事件（見下文）。
- 主機通過 `SetKaigiRelayManifest` 指令在賬本上應用清單更改，該指令會替換存儲的路徑或完全清除它。 Clearing 使用 `hop_count = 0` 發出摘要，以便操作員可以觀察返回到直接路由的轉換。
- Prometheus 指標（`kaigi_relay_registered_total`、`kaigi_relay_registration_bandwidth_class`、`kaigi_relay_manifest_updates_total`、`kaigi_relay_manifest_hop_count`、`kaigi_relay_health_reports_total`、`kaigi_relay_health_state`、`kaigi_relay_failover_total`、 `kaigi_relay_failover_hop_count`）現在可以在操作員儀表板上顯示繼電器流失、運行狀況和故障轉移節奏。

# 活動

擴展 `DomainEvent` 變體：

- `KaigiRosterSummary` – 發布匿名計數和當前名單
  每當花名冊發生變化時，root（透明模式下 root 為 `None`）。
- `KaigiRelayRegistered` – 每當創建或更新中繼註冊時發出。
- `KaigiRelayManifestUpdated` – 當中繼清單更改時發出。
- `KaigiRelayHealthUpdated` – 當主機通過 `ReportKaigiRelayHealth` 提交中繼運行狀況報告時發出。
- `KaigiUsageSummary` – 在每個使用段後發出，僅公開總計。

事件以 Norito 序列化，僅公開承諾哈希值和計數。CLI 工具 (`iroha kaigi …`) 包裝每個 ISI，以便操作員可以註冊會話，
提交名冊更新、報告中繼運行狀況並記錄使用情況，無需手工製作交易。
中繼清單和隱私證明從傳遞的 JSON/hex 文件中加載
CLI 的正常提交路徑，使編寫合約腳本變得簡單
進入暫存環境。

# 天然氣核算

- `crates/iroha_core/src/gas.rs` 中的新常量：
  - `BASE_KAIGI_JOIN_ZK`、`BASE_KAIGI_LEAVE_ZK` 和 `BASE_KAIGI_USAGE_ZK`
    根據 Halo2 驗證時間進行校準（名冊的 ≈1.6 毫秒）
    加入/離開，在 Apple M2 Ultra 上使用時約為 1.2 毫秒）。附加費繼續
    通過 `PER_KAIGI_PROOF_BYTE` 縮放證明字節大小。
- `RecordKaigiUsage` 承諾根據承諾規模和證明驗證支付額外費用。
- 校準工具將重用具有固定種子的機密資產基礎設施。

# 測試策略

- 單元測試驗證 Norito 編碼/解碼 `KaigiParticipantCommitment`、`KaigiRelayManifest`。
- JSON 視圖的黃金測試確保規範排序。
- 集成測試旋轉迷你網絡（參見
  當前覆蓋範圍為 `crates/iroha_core/tests/kaigi_privacy.rs`）：
  - 使用模擬證明的私有加入/離開週期（功能標誌 `kaigi_privacy_mocks`）。
  - 通過元數據事件傳播的中繼清單更新。
- Trybuild UI 測試涵蓋主機配置錯誤（例如，隱私模式下缺少中繼清單）。
- 在受限環境中運行單元/集成測試時（例如 Codex
  沙箱），導出 `NORITO_SKIP_BINDINGS_SYNC=1` 以繞過 Norito 綁定
  由 `crates/norito/build.rs` 強制執行同步檢查。

# 遷移計劃

1. ✅ 在 `KaigiPrivacyMode::Transparent` 默認值後面添加數據模型。
2. ✅ 線控雙路驗證：量產禁用`kaigi_privacy_mocks`，
   解析`zk.kaigi_roster_vk`，並運行真實的信封驗證；測試可以
   仍然啟用確定性存根的功能。
3. ✅ 推出專用 `kaigi_zk` Halo2 板條箱、校準氣體和有線
   集成覆蓋率以端到端運行真實的證明（模擬現在僅供測試）。
4. ⬜ 一旦所有消費者都理解承諾，就棄用透明的 `participants` 向量。

# 開放式問題

- 定義 Merkle 樹持久化策略：鏈上 vs 鏈下（當前傾向：具有鏈上根承諾的鏈下樹）。 *（在 KPG-201 中跟踪。）*
- 確定中繼清單是否應支持多路徑（同時冗餘路徑）。 *（在 KPG-202 中跟踪。）*
- 澄清中繼聲譽的治理——我們需要削減還是只是軟禁令？ *（在 KPG-203 中跟踪。）*

在生產中啟用 `KaigiPrivacyMode::ZkRosterV1` 之前應解決這些問題。