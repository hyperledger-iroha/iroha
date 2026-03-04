---
lang: zh-hant
direction: ltr
source: docs/portal/docs/sns/onboarding-kit.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: SNS metrics & onboarding kit
description: Dashboard, pricing, and automation artifacts referenced by roadmap item SN-8.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# SNS 指標和入門套件

路線圖項目 **SN-8** 捆綁了兩個承諾：

1. 發布儀表板，顯示註冊、續訂、ARPU、爭議和
   凍結 `.sora`、`.nexus` 和 `.dao` 的窗口。
2. 提供入門套件，以便註冊商和管理員可以連接 DNS、定價和
   在任何後綴上線之前，API 保持一致。

此頁面鏡像源版本
[`docs/source/sns/onboarding_kit.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sns/onboarding_kit.md)
因此外部評審員可以遵循相同的程序。

## 1. 度量捆綁

### Grafana 儀表板和門戶嵌入

- 將 `dashboards/grafana/sns_suffix_analytics.json` 導入 Grafana（或另一個
  分析主機）通過標準 API：

```bash
curl -H "Content-Type: application/json" \
     -H "Authorization: Bearer ${GRAFANA_TOKEN}" \
     -X POST https://grafana.sora.net/api/dashboards/db \
     --data-binary @dashboards/grafana/sns_suffix_analytics.json
```

- 相同的 JSON 為該門戶頁面的 iframe 提供支持（請參閱 **SNS KPI 儀表板**）。
  每當你撞到儀表板時，就跑
  `npm run build && npm run serve-verified-preview`裡面的`docs/portal`到
  確認 Grafana 和嵌入保持同步。

### 面板和證據

|面板|指標|治理證據|
|--------|---------|---------------------|
|註冊和續訂 | `sns_registrar_status_total`（成功+續訂解析器標籤）|每個後綴的吞吐量 + SLA 跟踪。 |
| ARPU / 淨單位 | `sns_bulk_release_payment_net_units`，`sns_bulk_release_payment_gross_units` |財務部門可以將註冊商清單與收入進行匹配。 |
|爭議和凍結 | `guardian_freeze_active`、`sns_dispute_outcome_total`、`sns_governance_activation_total` |顯示活動凍結、仲裁節奏和監護人工作負載。 |
| SLA/錯誤率 | `torii_request_duration_seconds`，`sns_registrar_status_total{status="error"}` |在 API 回歸影響客戶之前突出顯示它們。 |
|批量清單跟踪器 | `sns_bulk_release_manifest_total`，帶有 `manifest_id` 標籤的付款指標 |將 CSV drop 連接到結算票據。 |

在每月 KPI 期間從 Grafana（或嵌入式 iframe）導出 PDF/CSV
審查並將其附加到相關附件條目中
`docs/source/sns/regulatory/<suffix>/YYYY-MM.md`。管理員還捕獲了 SHA-256
`docs/source/sns/reports/` 下導出的包的名稱（例如，
`steward_scorecard_2026q1.md`），因此審計可以重播證據路徑。

### 附件自動化

直接從儀表板導出生成附件文件，以便審閱者獲得
一致摘要：

```bash
cargo xtask sns-annex \
  --suffix .sora \
  --cycle 2026-03 \
  --dashboard dashboards/grafana/sns_suffix_analytics.json \
  --dashboard-artifact artifacts/sns/regulatory/.sora/2026-03/sns_suffix_analytics.json \
  --output docs/source/sns/reports/.sora/2026-03.md \
  --regulatory-entry docs/source/sns/regulatory/eu-dsa/2026-03.md \
  --portal-entry docs/portal/docs/sns/regulatory/eu-dsa-2026-03.md
```

- 助手對導出進行哈希處理，捕獲 UID/標籤/面板計數，並寫入
  `docs/source/sns/reports/.<suffix>/<cycle>.md` 下的 Markdown 附件（參見
  `.sora/2026-03` 示例與本文檔一起提交）。
- `--dashboard-artifact` 將導出複製到
  `artifacts/sns/regulatory/<suffix>/<cycle>/` 因此附件引用了
  規範證據路徑；僅當需要指向時才使用 `--dashboard-label`
  在帶外存檔中。
- `--regulatory-entry` 指向管理備忘錄。助手插入（或
  替換）記錄附件路徑、儀表板的 `KPI Dashboard Annex` 塊
  人工製品、摘要和時間戳，以便證據在重新運行後保持同步。
- `--portal-entry` 保留 Docusaurus 副本 (`docs/portal/docs/sns/regulatory/*.md`)
  對齊，因此審閱者不必手動區分單獨的附件摘要。
- 如果您跳過 `--regulatory-entry`/`--portal-entry`，請將生成的文件附加到
  手動保存備忘錄，並上傳從 Grafana 捕獲的 PDF/CSV 快照。
- 對於經常性導出，請列出後綴/週期對
  `docs/source/sns/regulatory/annex_jobs.json` 並運行
  `python3 scripts/run_sns_annex_jobs.py --verbose`。助手走過每個入口，
  複製儀表板導出（默認為 `dashboards/grafana/sns_suffix_analytics.json`
  未指定時），並刷新每個監管內的附件塊（並且，
  如果可用，門戶）一次性備忘錄。
- 運行 `python3 scripts/check_sns_annex_schedule.py --jobs docs/source/sns/regulatory/annex_jobs.json --regulatory-root docs/source/sns/regulatory --report-root docs/source/sns/reports`（或 `make check-sns-annex`）以證明作業列表保持排序/重複數據刪除狀態，每個備忘錄都帶有匹配的 `sns-annex` 標記，並且附件存根存在。幫助程序在治理數據包中使用的區域設置/哈希摘要旁邊寫入 `artifacts/sns/annex_schedule_summary.json`。
這消除了手動複製/粘貼步驟，並保持 SN-8 附件證據的一致性，同時
保護 CI 中的時間表、標記和定位漂移。

## 2. 入門套件組件

### 後綴接線

- 註冊表架構+選擇器規則：
  [`docs/source/sns/registry_schema.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sns/registry_schema.md)
  和 [`docs/source/sns/local_to_global_toolkit.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sns/local_to_global_toolkit.md)。
- DNS 骨架助手：
  [`scripts/sns_zonefile_skeleton.py`](https://github.com/hyperledger-iroha/iroha/blob/master/scripts/sns_zonefile_skeleton.py)
  與捕捉到的排練流程
  [網關/DNS 操作手冊](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs_gateway_dns_owner_runbook.md)。
- 對於每次註冊商啟動，請在下面提交一份簡短說明
  `docs/source/sns/reports/` 總結選擇器示例、GAR 證明和 DNS 哈希。

### 定價備忘單

|標籤長度|基本費用（美元等值）|
|--------------|---------------------|
| 3 | 240 美元 |
| 4 | 90 美元 |
| 5 | 30 美元 |
| 6-9 | 6-9 12 美元 |
| 10+ | 8 美元 |

後綴係數：`.sora` = 1.0×、`.nexus` = 0.8×、`.dao` = 1.3×。  
期限乘數：2年-5%，5年-12%；寬限期 = 30 天，贖回
= 60 天（20% 費用，最低 5 美元，最高 200 美元）。記錄協商的偏差
登記員票。

### 優質拍賣與續訂

1. **溢價池** — 密封投標提交/揭示 (SN-3)。跟踪出價
   `sns_premium_commit_total`，並在下面發布清單
   `docs/source/sns/reports/`。
2. **荷蘭重新開放** - 寬限+贖回到期後，開始 7 天的荷蘭銷售
   在 10× 時，每天衰減 15%。標籤顯示為 `manifest_id`，因此
   儀表板可以顯示進度。
3. **續訂** — 監控 `sns_registrar_status_total{resolver="renewal"}` 和
   獲取自動續訂清單（通知、SLA、後備付款方式）
   內登記員票證。

### 開發者 API 和自動化

- API 合約：[`docs/source/sns/registrar_api.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sns/registrar_api.md)。
- 批量助手和 CSV 架構：
  [`docs/source/sns/bulk_onboarding_toolkit.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sns/bulk_onboarding_toolkit.md)。
- 命令示例：

```bash
python3 scripts/sns_bulk_onboard.py registrations.csv \
  --ndjson artifacts/sns/releases/2026q2/requests.ndjson \
  --submission-log artifacts/sns/releases/2026q2/submissions.log \
  --submit-torii-url https://torii.sora.net \
  --submit-token-file ~/.config/sora/tokens/registrar.token
```

在 KPI 儀表板篩選器中包含清單 ID（`--submission-log` 輸出）
因此財務部門可以協調每個版本的收入面板。

### 證據包

1. 包含聯繫方式、後綴範圍和付款方式的註冊商票據。
2. DNS/解析器證據（區域文件框架 + GAR 證明）。
3. 定價工作表+治理批准的任何覆蓋。
4. API/CLI 冒煙測試工件（`curl` 樣本、CLI 記錄）。
5. KPI儀表板截圖+CSV導出，附在每月附件中。

## 3.啟動清單

|步驟|業主|文物|
|------|--------|----------|
|儀表板進口|產品分析 | Grafana API 響應 + 儀表板 UID |
|門戶嵌入已驗證 |文檔/開發版本 | `npm run build` 日誌 + 預覽截圖 |
| DNS 演練完成 |網絡/運營 | `sns_zonefile_skeleton.py` 輸出 + 運行手冊日誌 |
|註冊商自動化試運行 |註冊商工程師 | `sns_bulk_onboard.py` 提交日誌 |
|提交治理證據 |治理委員會|附件鏈接 + 導出儀表板的 SHA-256 |

在激活註冊商或後綴之前，請完成清單。所簽署的
捆綁包清除了 SN-8 路線圖大門，並為審核員提供了單一參考
審查市場發布。