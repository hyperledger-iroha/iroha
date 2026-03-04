---
id: address-display-guidelines
lang: zh-hant
direction: ltr
source: docs/portal/docs/sns/address-display-guidelines.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: Sora Address Display Guidelines
sidebar_label: Address display
description: UX and CLI requirements for IH58 vs compressed (`sora`) Sora address presentation (ADDR-6).
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

從 '@site/src/components/ExplorerAddressCard' 導入 ExplorerAddressCard；

:::注意規範來源
此頁面鏡像 `docs/source/sns/address_display_guidelines.md`，現在提供服務
作為規範門戶副本。源文件會保留下來用於翻譯 PR。
:::

錢包、瀏覽器和 SDK 示例必須將帳戶地址視為不可變
有效負載。 Android 零售錢包示例位於
`examples/android/retail-wallet` 現在演示所需的 UX 模式：

- **雙複製目標。 ** 提供兩個顯式複制按鈕 - IH58（首選）和
  僅壓縮的 Sora 形式（`sora…`，第二好）。 IH58 始終安全地對外共享
  並為 QR 有效負載供電。壓縮變體必須包含內聯
  警告，因為它僅適用於 Sora 感知的應用程序。 Android 零售
  錢包示例將 Material 按鈕及其工具提示連接到
  `examples/android/retail-wallet/src/main/res/layout/activity_main.xml`，和
  iOS SwiftUI 演示通過內部 `AddressPreviewCard` 鏡像相同的 UX
  `examples/ios/NoritoDemo/Sources/ContentView.swift`。
- **等寬，可選文本。 ** 使用等寬字體渲染兩個字符串並
  `textIsSelectable="true"`，以便用戶無需調用 IME 即可檢查值。
  避免可編輯字段：IME 可以重寫假名或註入零寬度代碼點。
- **隱式默認域提示。 **當選擇器指向隱式
  `default` 域名，表面有標題提醒操作員無需後綴。
  當選擇器時，瀏覽器還應該突出顯示規範域標籤
  編碼摘要。
- **IH58 QR 有效負載。 ** QR 代碼必須對 IH58 字符串進行編碼。如果二維碼生成
  失敗，顯示明確的錯誤而不是空白圖像。
- **剪貼板消息傳遞。 ** 複製壓縮表單後，發出祝酒詞或
  小吃欄提醒用戶它僅適用於 Sora，並且容易出現 IME 損壞。

遵循這些護欄可以防止 Unicode/IME 損壞並滿足
錢包/瀏覽器用戶體驗的 ADDR-6 路線圖接受標準。

## 截圖裝置

在本地化審查期間使用以下固定裝置以確保按鈕標籤，
工具提示和警告在各個平台上保持一致：

- Android參考：`/img/sns/address_copy_android.svg`

  ![Android雙拷參考](/img/sns/address_copy_android.svg)

- iOS 參考號：`/img/sns/address_copy_ios.svg`

  ![iOS雙副本參考](/img/sns/address_copy_ios.svg)

## SDK 幫助程序

每個 SDK 都公開了一個方便的助手，可返回 IH58（首選）和壓縮的（`sora`，第二好）
與警告字符串一起形成，以便 UI 層可以保持一致：

- JavaScript：`AccountAddress.displayFormats(networkPrefix?: number)`
  （`javascript/iroha_js/src/address.js`）
- JavaScript 檢查器：`inspectAccountId(...)` 返回壓縮警告
  字符串並在調用者提供 `sora…` 時將其附加到 `warnings`
  字面意思，因此瀏覽器/錢包儀表板可以顯示僅限 Sora 的通知
  在粘貼/驗證流程期間而不是僅在它們生成時
  壓縮形式本身。
- Python：`AccountAddress.display_formats(network_prefix: int = 753)`
- 斯威夫特：`AccountAddress.displayFormats(networkPrefix: UInt16 = 753)`
- Java/Kotlin：`AccountAddress.displayFormats(int networkPrefix = 753)`
  （`java/iroha_android/src/main/java/org/hyperledger/iroha/android/address/AccountAddress.java`）

使用這些幫助器而不是在 UI 層中重新實現編碼邏輯。
JavaScript 幫助程序還在 `domainSummary` 上公開了 `selector` 有效負載
（`tag`、`digest_hex`、`registry_id`、`label`），因此 UI 可以指示是否
選擇器是 Local-12 或註冊表支持的，無需重新解析原始有效負載。

## Explorer 儀器演示

<瀏覽器地址卡/>

探索者應該鏡像錢包遙測和可訪問性工作：

- 應用 `data-copy-mode="ih58|compressed|qr"` 複製按鈕，以便前端可以發出使用計數器
  與 Torii 側 `torii_address_format_total` 指標一起。上面的演示組件調度
  帶有 `{mode,timestamp}` 的 `iroha:address-copy` 事件 - 將其連接到您的分析/遙測中
  管道（例如，推送到 Segment 或 NORITO 支持的收集器），以便儀表板可以關聯服務器
  地址格式與客戶端複製行為的使用。同時鏡像 Torii 域計數器
  (`torii_address_domain_total{domain_kind}`) 在同一個 feed 中，以便 Local-12 退休評論可以
  直接從 `address_ingest` 導出 30 天 `domain_kind="local12"` 零使用證明
  Grafana 板。
- 將每個控件與不同的 `aria-label`/`aria-describedby` 提示配對，以解釋是否
  文字可以安全地共享（`IH58`）或僅限 Sora（壓縮的 `sora`）。將隱式域標題包含在
  輔助技術的描述表面與視覺上顯示的上下文相同。
- 公開實時區域（例如 `<output aria-live="polite">…</output>`），宣布複製結果並
  警告，與現在連接到 Swift/Android 示例中的 VoiceOver/TalkBack 行為相匹配。

該儀器通過證明操作員可以觀察 Torii 攝取和
禁用本地選擇器之前的客戶端複製模式。

## 本地→全局遷移工具包

使用[本地→全局工具包](local-to-global-toolkit.md)來自動化
JSON 審核報告和操作員附加的轉換後的首選 IH58/第二最佳壓縮 (`sora`) 列表
到準備票，而隨附的操作手冊鏈接 Grafana
控制嚴格模式切換的儀表板和 Alertmanager 規則。

## 二進制佈局快速參考 (ADDR-1a)

當 SDK 提供高級地址工具（檢查器、驗證提示、
清單構建器），讓開發人員了解在中捕獲的規範傳輸格式
`docs/account_structure.md`。佈局始終是
`header · selector · controller`，其中標頭位為：

```
bit index:   7        5 4      3 2      1 0
             ┌─────────┬────────┬────────┬────┐
payload bit: │version  │ class  │  norm  │ext │
             └─────────┴────────┴────────┴────┘
```

- 今天 `addr_version = 0`（位 7-5）；非零值被保留並且必須
  提高 `AccountAddressError::InvalidHeaderVersion`。
- `addr_class` 區分單個 (`0`) 與多重簽名 (`1`) 控制器。
- `norm_version = 1` 編碼 Normv1 選擇器規則。未來的規範將被重用
  相同的 2 位字段。
- `ext_flag` 始終為 `0` — 設置位指示不支持的有效負載擴展。

選擇器緊跟在標題後面：

```
┌──────────┬──────────────────────────────────────────────┐
│ tag (u8) │ payload (depends on selector kind)           │
└──────────┴──────────────────────────────────────────────┘
```

UI 和 SDK 表面應該準備好顯示選擇器類型：

- `0x00` = 隱式默認域（無負載）。
- `0x01` = 本地摘要（12 字節 `blake2s_mac("SORA-LOCAL-K:v1", label)`）。
- `0x02` = 全局註冊表項（大端 `registry_id:u32`）。

錢包工具可以鏈接或嵌入文檔/測試的規範十六進制示例：

|選擇器種類 |規範六角 |
|----------------|---------------|
|隱式默認 | `0x02000001203b6a27bcceb6a42d62a3a8d02a6f0d73653215771de243a63ac048a18b59da29` |
|本地摘要 (`treasury`) | `0x0201b18fe9c1abbac45b3e38fc5d0001203b77a042f1de02f6d5f418f36a2a28ea` |
|全局註冊表 (`android`) | `0x020200000059a6a47eb7c9aa415f77b18636a85a57837d5518ff5357ef63c35202` |

請參閱 `docs/source/references/address_norm_v1.md` 了解完整的選擇器/狀態
表和 `docs/account_structure.md` 為完整的字節圖。

## 強制執行規範形式

字符串必須遵循 ADDR-5 下記錄的 CLI 工作流程：

1. `iroha tools address inspect` 現在使用 IH58 發出結構化 JSON 摘要，
   壓縮的、規範的十六進制有效負載。摘要還包括 `domain`
   具有 `kind`/`warning` 字段的對象，並通過以下方式回顯任何提供的域
   `input_domain` 字段。當 `kind` 為 `local12` 時，CLI 會打印一條警告
   stderr 和 JSON 摘要呼應相同的指導，因此 CI 管道和 SDK
   可以將其浮現出來。每當您需要轉換時傳遞 `--append-domain`
   編碼重播為 `<ih58>@<domain>`。
2. SDK 可以通過 JavaScript 幫助程序顯示相同的警告/摘要：

   ```js
   import { inspectAccountId } from "@iroha/iroha-js";

   const summary = inspectAccountId("sora...");
   if (summary.domain.warning) {
     console.warn(summary.domain.warning);
   }
   console.log(summary.ih58.value, summary.compressed);
   ```
  幫助器保留從文字中檢測到的 IH58 前綴，除非您
  明確提供 `networkPrefix`，因此非默認網絡的摘要不會
  不會使用默認前綴默默地重新渲染。

3. 通過重用 `ih58.value` 或 `compressed` 轉換規範負載
   摘要中的字段（或通過 `--format` 請求其他編碼）。這些
   字符串已經可以安全地與外部共享。
4. 使用以下內容更新清單、註冊表和麵向客戶的文檔
   規範形式並通知交易對手本地選擇器將是
   切換完成後被拒絕。
5. 對於批量數據集，運行
   `iroha tools address audit --input addresses.txt --network-prefix 753`。命令
   讀取換行符分隔的文字（以 `#` 開頭的註釋將被忽略，並且
   `--input -` 或無標誌使用 STDIN），發出 JSON 報告
   每個條目的規範/首選 IH58/第二最佳壓縮 (`sora`) 摘要，並對兩個解析進行計數
   包含垃圾行的轉儲以及帶有 `--fail-on-warning` 的門自動化
   一旦操作員準備好阻止 CI 中的本地選擇器。
6. 當需要換行符到換行符重寫時，使用
  對於本地選擇器修復電子表格，請使用
  導出 `input,status,format,…` CSV，一次性突出顯示規範編碼、警告和解析失敗。
   默認情況下，助手會跳過非本地行，轉換每個剩餘的條目
   到請求的編碼（IH58首選/壓縮（`sora`）第二好/十六進制/JSON），並保留
   設置 `--append-domain` 時的原始域。與 `--allow-errors` 配對
   即使轉儲包含格式錯誤的文字也可以繼續掃描。
7. CI/lint 自動化可以運行 `ci/check_address_normalize.sh`，它提取
   來自 `fixtures/account/address_vectors.json` 的本地選擇器，轉換
   通過 `iroha tools address normalize` 進行回放
   `iroha tools address audit --fail-on-warning` 證明版本不再發出
   本地摘要。`torii_address_local8_total{endpoint}`加
`torii_address_collision_total{endpoint,kind="local12_digest"}`，
`torii_address_collision_domain_total{endpoint,domain}`，以及
Grafana 板 `dashboards/grafana/address_ingest.json` 提供執行
信號：一旦生產儀表板顯示零合法的本地提交和
連續 30 天零 Local-12 衝突，Torii 將翻轉 Local-8
主網上出現硬故障，一旦全局域出現故障，則 Local-12 緊隨其後
匹配的註冊表項。將 CLI 輸出視為面向操作員的通知
對於此凍結 - SDK 工具提示使用相同的警告字符串，並且
自動化以與路線圖退出標准保持一致。 Torii 現在默認為
當診斷回歸時。保持鏡像 `torii_address_domain_total{domain_kind}`
進入 Grafana (`dashboards/grafana/address_ingest.json`) 所以 ADDR-7 證據包
可以證明 `domain_kind="local12"` 在之前所需的 30 天窗口內保持為零
(`dashboards/alerts/address_ingest_rules.yml`)增加了三個護欄：

- 每當上下文報告新的 Local-8 時，`AddressLocal8Resurgence` 頁面
  增量。停止嚴格模式推出，在
  直到信號返回到零，然後恢復默認值 (`true`)。
- 當兩個 Local-12 標籤散列到相同的值時，`AddressLocal12Collision` 會觸發
  消化。暫停清單促銷，運行本地 → 全局工具包進行審核
  摘要映射，並在重新發布之前與 Nexus 治理進行協調
  註冊表項或重新啟用下游部署。
- `AddressInvalidRatioSlo` 當車隊範圍內的無效比率（不包括
  本地 8/嚴格模式拒絕）超過 0.1% SLO 十分鐘。使用
  `torii_address_invalid_total` 查明負責任的背景/原因和
  在重新啟用嚴格模式之前與所屬 SDK 團隊協調。

### 發行說明片段（錢包和瀏覽器）

發貨時，請在錢包/瀏覽器發行說明中包含以下項目符號
切換：

> **地址：** 添加了 `iroha tools address normalize --only-local --append-domain`
> helper 並將其連接到 CI (`ci/check_address_normalize.sh`) 所以錢包/資源管理器
> 在主網上阻止 Local-8/Local-12 之前。將任何自定義導出更新為
> 運行命令並將規範化列表附加到發布證據包中。