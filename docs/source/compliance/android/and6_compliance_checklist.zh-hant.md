---
lang: zh-hant
direction: ltr
source: docs/source/compliance/android/and6_compliance_checklist.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 2a0ce1be46f9c468915f50de5e38e2f34657b26bf4243fb5ea45dab175789393
source_last_modified: "2026-01-05T09:28:12.002460+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Android AND6 合規性檢查表

該清單跟踪實現里程碑的合規性交付成果 **AND6 -
CI 和合規性強化**。它整合了所要求的監管工件
在 `roadmap.md` 中並定義了下的存儲佈局
`docs/source/compliance/android/` 所以發布工程、支持和法律
在批准 Android 版本之前可以參考相同的證據集。

## 範圍和所有者

|面積 |可交付成果 |主要所有者 |備份/審閱|
|------|--------------|------------------------|--------------------|
|歐盟監管捆綁| ETSI EN 319 401 安全目標、GDPR DPIA 摘要、SBOM 證明、證據日誌 |合規與法律（索菲亞·馬丁斯）|發布工程（Alexei Morozov）|
|日本監管捆綁| FISC 安全控制清單、雙語 StrongBox 證明包、證據日誌 |合規與法律（丹尼爾·帕克）| Android 項目負責人 |
|設備實驗室準備情況|容量跟踪、應急觸發器、升級日誌 |硬件實驗室負責人 | Android 可觀察性 TL |

## 人工製品矩陣|神器|描述 |存儲路徑 |刷新節奏 |筆記|
|----------|-------------|--------------|-----------------|--------|
| ETSI EN 319 401 安全目標 |描述 Android SDK 二進製文件的安全目標/假設的敘述。 | `docs/source/compliance/android/eu/security_target.md` |重新驗證每個 GA + LTS 版本。 |必須引用發布序列的構建來源哈希。 |
| GDPR DPIA 摘要 |涵蓋遙測/日誌記錄的數據保護影響評估。 | `docs/source/compliance/android/eu/gdpr_dpia_summary.md` |年度+材料遙測更改之前。 |參考 `sdk/android/telemetry_redaction.md` 中的編輯策略。 |
| SBOM認證| Gradle/Maven 工件的簽名 SBOM 和 SLSA 出處。 | `docs/source/compliance/android/eu/sbom_attestation.md` |每個 GA 版本。 |運行 `scripts/android_sbom_provenance.sh <version>` 以生成 CycloneDX 報告、共同簽名包和校驗和。 |
| FISC 安全控制清單 |已完成將 SDK 控制映射到 FISC 要求的清單。 | `docs/source/compliance/android/jp/fisc_controls_checklist.md` |年度 + JP 合作夥伴試點之前。 |提供雙語標題（EN/JP）。 |
| StrongBox 認證包（日本）|針對日本監管機構的每台設備的認證摘要 + 鏈。 | `docs/source/compliance/android/jp/strongbox_attestation.md` |當新硬件進入池時。 |指向 `artifacts/android/attestation/<device>/` 下的原始製品。 |
|法律簽署備忘錄|律師摘要涵蓋 ETSI/GDPR/FISC 範圍、隱私狀況以及所附物品的監管鏈。 | `docs/source/compliance/android/eu/legal_signoff_memo.md` |每次工件包發生變化或添加新的管轄區時。 |備忘錄引用了證據日誌中的哈希值以及設備實驗室應急包的鏈接。 |
|證據日誌|帶有哈希/時間戳元數據的已提交工件的索引。 | `docs/source/compliance/android/evidence_log.csv` |每當上述任何條目發生變化時都會更新。 |添加 Buildkite 鏈接 + 審閱者簽字。 |
|設備實驗室儀器包 |使用 `device_lab_instrumentation.md` 中定義的流程記錄的特定於插槽的遙測、隊列和證明證據。 | `artifacts/android/device_lab/<slot>/`（參見 `docs/source/compliance/android/device_lab_instrumentation.md`）|每個預留插槽+故障轉移演練。 |捕獲 SHA-256 清單並引用證據日誌 + 檢查表中的插槽 ID。 |
|設備實驗室預約日誌|用於在凍結期間保持 StrongBox 池 ≥80% 的預訂工作流程、審批、容量快照和升級階梯。 | `docs/source/compliance/android/device_lab_reservation.md` |每當創建/更改預訂時更新。 |請參考過程中註明的 `_android-device-lab` 票證 ID 和每週日曆導出。 |
|設備實驗室故障轉移操作手冊和練習包 |季度排練計劃和工件清單展示後備通道、Firebase 突發隊列和外部 StrongBox 保留器準備情況。 | `docs/source/compliance/android/device_lab_failover_runbook.md` + `artifacts/android/device_lab_contingency/<YYYYMMDD>-failover-drill/` |每季度一次（或在硬件名冊更改後）。 |在證據日誌中記錄演練 ID，並附加 Runbook 中記錄的清單哈希 + PagerDuty 導出。 |

> **提示：** 附加 PDF 或外部簽名的工件時，請存儲一個簡短的
> 表格路徑中的 Markdown 包裝器鏈接到中的不可變工件
> 治理份額。這使倉庫保持輕量級，同時保留
> 審計跟踪。

## 歐盟監管數據包 (ETSI/GDPR)歐盟數據包將上述三件文物以及法律備忘錄聯繫在一起：

- 使用版本標識符更新 `security_target.md`、Torii 清單哈希、
  和 SBOM 摘要，以便審計人員可以將二進製文件與聲明的範圍進行匹配。
- 使 DPIA 摘要與最新的遙測編輯政策保持一致，並且
  附上 `docs/source/sdk/android/telemetry_redaction.md` 中引用的 Norito 差異摘錄。
- SBOM 證明條目應包括：CycloneDX JSON 哈希、出處
  捆綁散列、聯合簽名語句以及生成它們的 Buildkite 作業 URL。
- `legal_signoff_memo.md` 必須捕獲建議/日期，列出所有文物 +
  SHA-256，概述任何補償控制，並鏈接到證據日誌行
  加上跟踪批准的 PagerDuty 票證 ID。

## 日本監管數據包 (FISC/StrongBox)

日本監管機構期望提供包含雙語文檔的並行捆綁包：

- `fisc_controls_checklist.md` 鏡像官方電子表格；填寫兩個
  EN 和 JA 列並參考 `sdk/android/security.md` 的特定部分
  或滿足每個控件的 StrongBox 證明包。
- `strongbox_attestation.md` 總結了最新的運行
  `docs/source/sdk/android/readiness/android_strongbox_attestation_bundle.md`
  （每個設備 JSON + Norito 信封）。嵌入指向不可變工件的鏈接
  在 `artifacts/android/attestation/<device>/` 下並記下旋轉節奏。
- 記錄內含提交內容的雙語求職信模板
  `docs/source/compliance/android/jp/README.md`，因此支持人員可以重複使用它。
- 使用引用清單的單行更新證據日誌，
  證明捆綁哈希，以及與交付相關的任何 JP 合作夥伴票證 ID。

## 提交工作流程

1. **草稿** - 所有者準備工件，記錄計劃的文件名
   上表，並打開一個 PR，其中包含更新的 Markdown 存根以及
   外部附件的校驗和。
2. **審查** - 發布工程確認來源哈希與暫存的匹配
   二進製文件；合規性驗證監管語言；支持確保 SLA 和
   正確引用了遙測策略。
3. **簽署** - 批准者將其姓名和日期添加到 `Sign-off` 表中
   下面。使用 PR URL 和 Buildkite 運行更新證據日誌。
4. **發布** - SRE 治理簽署後，將工件鏈接到
   `status.md` 並更新 Android 支持 Playbook 參考。

### 簽核日誌

|文物|評論者 |日期 |公關/證據|
|----------|-------------|------|---------------|
| *（待定）* | - | - | - |

## 設備實驗室預訂和應急計劃

為了減輕路線圖中指出的**設備實驗室可用性**風險：- 在 `docs/source/compliance/android/evidence_log.csv` 中跟踪每周容量
  （`device_lab_capacity_pct` 列）。如果可用，請通知發布工程
  連續兩週跌破70%。
- 預留保險箱/一般車道如下
  `docs/source/compliance/android/device_lab_reservation.md` 領先於每個
  凍結、排練或合規性掃描，以便請求、批准和工件
  在 `_android-device-lab` 隊列中捕獲。鏈接生成的工單 ID
  記錄容量快照時在證據日誌中。
- **後備池：**首先突發到共享像素池；如果仍然飽和，
  安排 Firebase 測試實驗室冒煙運行以進行 CI 驗證。
- **外部實驗室固定器：** 與 StrongBox 合作夥伴一起維護固定器
  實驗室，以便我們可以在凍結窗口期間保留硬件（至少提前 7 天）。
- **升級：**在 PagerDuty 中引發 `AND6-device-lab` 事件
  主池和後備池容量降至 50% 以下。硬件實驗室負責人
  與 SRE 協調以重新確定設備的優先級。
- **故障轉移證據包：** 將每次排練存儲在
  `artifacts/android/device_lab_contingency/<YYYYMMDD>/` 帶預訂
  請求、PagerDuty 導出、硬件清單和恢復記錄。參考
  來自 `device_lab_contingency.md` 的捆綁包並將 SHA-256 添加到證據日誌
  因此，法律部門可以證明應急工作流程已得到執行。
- **季度演習：** 練習操作手冊
  `docs/source/compliance/android/device_lab_failover_runbook.md`，附上
  生成的捆綁包路徑 + `_android-device-lab` 票證的清單哈希，以及
  在應急日誌和證據日誌中鏡像演練 ID。

記錄應急計劃的每次啟動
`docs/source/compliance/android/device_lab_contingency.md`（包括日期、
觸發器、操作和後續行動）。

## 靜態分析原型

- `make android-lint` 包裝 `ci/check_android_javac_lint.sh`，編譯
  `java/iroha_android` 和共享的 `java/norito_java` 源
  `javac --release 21 -Xlint:all -Werror`（標記類別在
- 編譯後，腳本強制執行 AND6 依賴策略
  `jdeps --summary`，如果任何模塊超出批准的允許列表，則失敗
  (`java.base`、`java.net.http`、`jdk.httpserver`) 出現。這保持了
  Android 表面符合 SDK 委員會的“無隱藏 JDK 依賴項”
  StrongBox 合規性審查之前的要求。
- CI 現在通過以下方式運行相同的門
  `.github/workflows/android-lint.yml`，它調用
  `ci/check_android_javac_lint.sh` 在每次涉及 Android 的推送/公關上
  共享 Norito Java 源代碼和上傳 `artifacts/android/lint/jdeps-summary.txt`
  因此合規性審查可以引用已簽名的模塊列表，而無需重新運行
  本地腳本。
- 當需要保留臨時數據時，設置`ANDROID_LINT_KEEP_WORKDIR=1`
  工作區。該腳本已經將生成的模塊摘要復製到
  `artifacts/android/lint/jdeps-summary.txt`；設置
  `ANDROID_LINT_SUMMARY_OUT=docs/source/compliance/android/evidence/android_lint_jdeps.txt`
  （或類似）當您需要額外的版本化工件進行審核時。
  工程師在提交 Android PR 之前仍應在本地運行該命令
  涉及 Java 源並將記錄的摘要/日誌附加到合規性
  評論。從發行說明中將其引用為“Android javac lint + dependency
  掃描”。

## CI 證據（Lint、測試、證明）- `.github/workflows/android-and6.yml` 現在運行所有 AND6 門（javac lint +
  依賴項掃描、Android 測試套件、StrongBox 證明驗證器以及
  device-lab 插槽驗證）在每次接觸 Android 表面的 PR/push 上。
- `ci/run_android_tests.sh` 包裝 `ci/run_android_tests.sh` 並發出
  `artifacts/android/tests/test-summary.json` 處的確定性摘要，同時
  將控制台日誌保存到 `artifacts/android/tests/test.log`。附上兩者
  引用 CI 運行時將文件添加到合規性數據包。
- `scripts/android_strongbox_attestation_ci.sh --summary-out` 產生
  `artifacts/android/attestation/ci-summary.json`，驗證捆綁的
  StrongBox 和 `artifacts/android/attestation/**` 下的證明鏈
  TEE 池。
- `scripts/check_android_device_lab_slot.py --root fixtures/android/device_lab`
  驗證 CI 中使用的示例槽 (`slot-sample/`)，並可指向
  真實運行在 `artifacts/android/device_lab/<slot-id>/` 下
  `--require-slot --json-out <dest>` 證明儀器包如下
  記錄的佈局。 CI 將驗證摘要寫入
  `artifacts/android/device_lab/summary.json`；示例槽包括
  佔位符遙測/證明/隊列/日誌提取加上記錄
  `sha256sum.txt` 用於可重現的哈希值。

## 設備實驗室儀器工作流程

每次預留或故障轉移演練必須遵循
`device_lab_instrumentation.md` 遙測、隊列和證明指南
文物與預訂日誌相符：

1. **種子槽文物。 ** 創建
   `artifacts/android/device_lab/<slot>/` 與標準子文件夾並運行
   插槽關閉後的 `shasum`（請參閱新的“Artifact Layout”部分）
   指南）。
2. **運行儀器命令。 ** 執行遙測/隊列捕獲，
   完全覆蓋摘要、StrongBox 工具和 lint/依賴項掃描
   記錄下來，以便輸出反映 CI。
3. **歸檔證據。 ** 更新
   `docs/source/compliance/android/evidence_log.csv` 和預訂票
   包含插槽 ID、SHA-256 清單路徑和相應的儀表板/Buildkite
   鏈接。

將 artefact 文件夾和哈希清單附加到 AND6 發布包中
受影響的凍結窗口。治理審核者將拒絕不符合要求的清單
不引用插槽標識符和儀器指南。

### 預留和故障轉移準備證據

路線圖項目“監管製品批准和實驗室應急”需要更多
比儀器儀表。每個 AND6 數據包還必須引用主動
預訂工作流程和季度故障轉移演練：- **預訂手冊 (`device_lab_reservation.md`).** 遵循預訂
  表（交貨時間、所有者、槽位長度），通過導出共享日曆
  `scripts/android_device_lab_export.py`，並記錄`_android-device-lab`
  `evidence_log.csv` 中的票證 ID 和容量快照。劇本
  詳細說明昇級階梯和應急觸發因素；複製這些詳細信息
  當預訂量移動或容量下降到低於預定值時，將進入清單條目
  80% 路線圖目標。
- **故障轉移演練操作手冊 (`device_lab_failover_runbook.md`)。 ** 執行
  每季度排練（模擬停電 → 推廣後備車道 → 參與
  Firebase 突發 + 外部 StrongBox 合作夥伴）並將文物存儲在
  `artifacts/android/device_lab_contingency/<drill-id>/`。每個捆綁必須
  包含清單、PagerDuty 導出、Buildkite 運行鏈接、Firebase 突發
  報告以及運行手冊中註明的保留確認書。參考
  證據日誌中的演習 ID、SHA-256 清單和後續票證
  這個清單。

這些文件共同證明了設備容量規劃、停電演練、
並且儀器包共享與所要求的相同的審計跟踪
路線圖和法律審查員。

## 回顧節奏

- **每季度** - 驗證 EU/JP 工件是否是最新的；刷新
  證據日誌哈希；排練出處捕獲。
- **預發布** - 在每次 GA/LTS 切換期間運行此清單並附加
  發布 RFC 的完整日誌。
- **事件後** - 如果 Sev 1/2 事件涉及遙測、簽名或
  證明，用修復說明更新相關的工件存根，以及
  捕獲證據日誌中的參考。