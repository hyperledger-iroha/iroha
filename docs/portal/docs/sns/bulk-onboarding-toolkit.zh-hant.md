---
lang: zh-hant
direction: ltr
source: docs/portal/docs/sns/bulk-onboarding-toolkit.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: a583af55cf8b4cf5070828bfb52146be88f92937c8d7887ab37a2056bf55ec9e
source_last_modified: "2026-01-22T16:26:46.515965+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->
---
id：批量入門工具包
標題：SNS 批量入職工具包
sidebar_label：批量入門工具包
描述：用於 SN-3b 註冊器運行的 CSV 到 RegisterNameRequestV1 自動化。
---

:::注意規範來源
鏡像 `docs/source/sns/bulk_onboarding_toolkit.md`，以便外部操作員看到
相同的 SN-3b 指南，無需克隆存儲庫。
:::

# SNS 批量入門工具包 (SN-3b)

**路線圖參考：** SN-3b“批量入職工具”  
**文物：** `scripts/sns_bulk_onboard.py`、`scripts/tests/test_sns_bulk_onboard.py`、
`docs/portal/scripts/sns_bulk_release.sh`

大型註冊商通常會預先準備數百個 `.sora` 或 `.nexus` 註冊
具有相同的治理批准和結算規則。手動製作 JSON
負載或重新運行 CLI 無法擴展，因此 SN-3b 提供了確定性
CSV 到 Norito 構建器，用於準備 `RegisterNameRequestV1` 結構
Torii 或 CLI。助手驗證前面的每一行，發出兩個
聚合清單和可選的換行符分隔的 JSON，並且可以提交
自動有效負載，同時記錄結構化收據以供審計。

## 1. CSV 架構

解析器需要以下標題行（順序靈活）：

|專欄 |必填 |描述 |
|--------|----------|-------------|
| `label` |是的 |請求的標籤（接受混合大小寫；工具根據 Norm v1 和 UTS-46 進行標準化）。 |
| `suffix_id` |是的 |數字後綴標識符（十進製或 `0x` 十六進制）。 |
| `owner` |是的 | AccountId string (domainless encoded literal; canonical Katakana i105 only; no `@<domain>` suffix). |
| `term_years` |是的 |整數 `1..=255`。 |
| `payment_asset_id` |是的 |結算資產（例如 `61CtjvNd9T3THAR65GsMVHr82Bjc`）。 |
| `payment_gross` / `payment_net` |是的 |表示資產本機單位的無符號整數。 |
| `settlement_tx` |是的 |描述支付交易或哈希的 JSON 值或文字字符串。 |
| `payment_payer` |是的 |授權付款的AccountId。 |
| `payment_signature` |是的 |包含管理員或財務簽名證明的 JSON 或文字字符串。 |
| `controllers` |可選|以分號或逗號分隔的控制者帳戶地址列表。省略時默認為 `[owner]`。 |
| `metadata` |可選|內聯 JSON 或 `@path/to/file.json` 提供解析器提示、TXT 記錄等。默認為 `{}`。 |
| `governance` |可選|內聯 JSON 或 `@path` 指向 `GovernanceHookV1`。 `--require-governance` 強制執行此列。 |

任何列都可以通過在單元格值前添加 `@` 來引用外部文件。
路徑是相對於 CSV 文件解析的。

## 2. 運行助手

```bash
python3 scripts/sns_bulk_onboard.py registrations.csv \
  --output artifacts/sns_bulk_manifest.json \
  --ndjson artifacts/sns_bulk_requests.ndjson
```

關鍵選項：

- `--require-governance` 拒絕沒有治理掛鉤的行（對於
  優質拍賣或保留轉讓）。
- `--default-controllers {owner,none}` 決定控制器單元是否為空
  回退到所有者帳戶。
- `--controllers-column`、`--metadata-column` 和 `--governance-column` 重命名
  使用上游導出時的可選列。

成功後，腳本會寫入聚合清單：

```json
{
  "schema_version": 1,
  "generated_at": "2026-03-30T06:48:00.123456Z",
  "source_csv": "/abs/path/registrations.csv",
  "requests": [
    {
      "selector": {"version":1,"suffix_id":1,"label":"alpha"},
      "owner": "<katakana-i105-account-id>",
      "controllers": [
        {"controller_type":{"kind":"Account"},"account_address":"<katakana-i105-account-id>","resolver_template_id":null,"payload":{}}
      ],
      "term_years": 2,
      "pricing_class_hint": null,
      "payment": {
        "asset_id":"61CtjvNd9T3THAR65GsMVHr82Bjc",
        "gross_amount":240,
        "net_amount":240,
        "settlement_tx":"alpha-settlement",
        "payer":"<katakana-i105-account-id>",
        "signature":"alpha-signature"
      },
      "governance": null,
      "metadata":{"notes":"alpha cohort"}
    }
  ],
  "summary": {
    "total_requests": 120,
    "total_gross_amount": 28800,
    "total_net_amount": 28800,
    "suffix_breakdown": {"1":118,"42":2}
  }
}
```

如果提供了 `--ndjson`，則每個 `RegisterNameRequestV1` 也被寫為
單行 JSON 文檔，因此自動化可以將請求直接流式傳輸到
Torii：

```bash
jq -c '.requests[]' artifacts/sns_bulk_manifest.json |
  while read -r payload; do
    curl -H "Authorization: Bearer $TOKEN" \
         -H "Content-Type: application/json" \
         -d "$payload" \
         https://torii.sora.net/v1/sns/names
  done
```

## 3. 自動提交

### 3.1 Torii 休息模式

指定 `--submit-torii-url` 加 `--submit-token` 或
`--submit-token-file` 將每個清單條目直接推送到 Torii：

```bash
python3 scripts/sns_bulk_onboard.py --manifest artifacts/sns_bulk_manifest.json \
  --submit-torii-url https://torii.sora.net \
  --submit-token-file ~/.config/sora/tokens/registrar.token \
  --poll-status \
  --suffix-map configs/sns_suffix_map.json \
  --submission-log artifacts/sns_bulk_submit.log
```

- 幫助程序針對每個請求發出一個 `POST /v1/sns/names` 併中止
  第一個 HTTP 錯誤。響應以 NDJSON 形式附加到日誌路徑
  記錄。
- `--poll-status` 在每次之後重新查詢 `/v1/sns/names/{namespace}/{literal}`
  提交（最多`--poll-attempts`，默認5）以確認該記錄
  可見。提供 `--suffix-map` （`suffix_id` 到 `"suffix"` 值的 JSON）
  該工具可以派生 `{label}.{suffix}` 文字進行輪詢。
- 可調參數：`--submit-timeout`、`--poll-attempts` 和 `--poll-interval`。

### 3.2 iroha CLI 模式

要通過 CLI 路由每個清單條目，請提供二進制路徑：

```bash
python3 scripts/sns_bulk_onboard.py --manifest artifacts/sns_bulk_manifest.json \
  --submit-cli-path ./target/release/iroha \
  --submit-cli-config configs/registrar.toml \
  --submit-cli-extra-arg --chain-id=devnet \
  --submission-log artifacts/sns_bulk_submit.log
```

- 控制器必須是 `Account` 條目 (`controller_type.kind = "Account"`)
  因為 CLI 目前僅公開基於帳戶的控制器。
- 元數據和治理 blob 根據請求寫入臨時文件，
  轉發至 `iroha sns register --metadata-json ... --governance-json ...`。
- 記錄 CLI stdout 和 stderr 以及退出代碼；非零退出代碼中止
  奔跑。

兩種提交模式可以一起運行（Torii 和 CLI）以交叉檢查註冊商
部署或演練後備方案。

### 3.3 提交回執

當提供 `--submission-log <path>` 時，腳本會附加 NDJSON 條目
捕獲：

```json
{"timestamp":"2026-03-30T07:22:04.123Z","mode":"torii","index":12,"selector":"1:alpha","status":200,"success":true,"detail":"..."}
{"timestamp":"2026-03-30T07:22:05.456Z","mode":"torii-poll","index":12,"selector":"1:alpha","status":200,"success":true,"detail":"{...}","attempt":2}
{"timestamp":"2026-03-30T07:22:06.789Z","mode":"cli","index":12,"selector":"1:alpha","status":0,"success":true,"detail":"Registration accepted"}
```

成功的 Torii 響應包括從以下位置提取的結構化字段
`NameRecordV1` 或 `RegisterNameResponseV1`（例如 `record_status`，
`record_pricing_class`、`record_owner`、`record_expires_at_ms`、
`registry_event_version`、`suffix_id`、`label`）等儀表板和治理
報告可以解析日誌而無需檢查自由格式文本。將此日誌附加到
登記員票據與清單一起提供可複制的證據。

## 4. 文檔門戶發布自動化

CI 和門戶作業調用 `docs/portal/scripts/sns_bulk_release.sh`，它包含
幫助程序並將工件存儲在 `artifacts/sns/releases/<timestamp>/` 下：

```bash
docs/portal/scripts/sns_bulk_release.sh \
  --csv assets/sns/registrations_2026q2.csv \
  --torii-url https://torii.sora.network \
  --token-env SNS_TORII_TOKEN \
  --suffix-map configs/sns_suffix_map.json \
  --poll-status \
  --cli-path ./target/release/iroha \
  --cli-config configs/registrar.toml
```

腳本：

1. 構建 `registrations.manifest.json`、`registrations.ndjson`，並複制
   將原始 CSV 複製到發布目錄中。
2. 使用 Torii 和/或 CLI（配置後）提交清單，寫入
   `submissions.log` 以及上述結構化收據。
3. 發出 `summary.json` 描述該版本（路徑、Torii URL、CLI 路徑、
   時間戳），以便門戶自動化可以將包上傳到工件存儲。
4. 生成 `metrics.prom`（通過 `--metrics` 覆蓋），其中包含
   Prometheus-格式計數器，用於總請求、後綴分佈、
   資產總額和提交結果。摘要 JSON 鏈接到此文件。

工作流程只是將發布目錄歸檔為單個工件，現在
包含審計治理所需的一切。

## 5. 遙測和儀表板

`sns_bulk_release.sh` 生成的指標文件公開了以下內容
系列：

```
# HELP sns_bulk_release_requests_total Number of registration requests per release and suffix.
# TYPE sns_bulk_release_requests_total gauge
sns_bulk_release_requests_total{release="2026q2-beta",suffix_id="all"} 120
sns_bulk_release_requests_total{release="2026q2-beta",suffix_id="1"} 118
sns_bulk_release_payment_gross_units{release="2026q2-beta",asset_id="61CtjvNd9T3THAR65GsMVHr82Bjc"} 28800
sns_bulk_release_submission_events_total{release="2026q2-beta",mode="torii",success="true"} 118
```

將 `metrics.prom` 餵入您的 Prometheus sidecar（例如通過 Promtail 或
批量導入程序）以使註冊商、管理員和治理同行保持一致
批量進展。 Grafana板
`dashboards/grafana/sns_bulk_release.json` 使用面板可視化相同的數據
每個後綴計數、付款量和提交成功/失敗比率。
該委員會按 `release` 進行篩選，因此審核員可以深入了解單個 CSV 運行。

## 6. 驗證和失敗模式

- **標籤規範化：** 輸入使用 Python IDNA plus 進行規範化
  小寫和 Norm v1 字符過濾器。無效標籤在任何標籤之前都會快速失敗
  網絡通話。
- **數字護欄：** 後綴 ID、學期年份和定價提示必須下降
  在 `u16` 和 `u8` 範圍內。付款字段接受十進製或十六進制整數
  高達 `i64::MAX`。
- **元數據或治理解析：**直接解析內聯JSON；文件
  引用是相對於 CSV 位置解析的。非對像元數據
  產生驗證錯誤。
- **控制器：** 空白單元符合 `--default-controllers`。提供明確的
  委派給非所有者時的控制器列表（例如 `<katakana-i105-account-id>;<katakana-i105-account-id>`）
  演員。

使用上下文行號報告失敗（例如
`error: row 12 term_years must be between 1 and 255`）。腳本退出時顯示
驗證錯誤時代碼為 `1`，CSV 路徑丟失時代碼為 `2`。

## 7. 測試和出處

- `python3 -m pytest scripts/tests/test_sns_bulk_onboard.py` 涵蓋 CSV 解析，
  NDJSON 發射、治理執行和 CLI 或 Torii 提交
  路徑。
- 助手是純Python（沒有額外的依賴）並且可以在任何地方運行
  `python3` 可用。提交歷史記錄與 CLI 一起跟踪
  可重複性的主存儲庫。

對於生產運行，請將生成的清單和 NDJSON 捆綁包附加到
註冊商票證，以便管理員可以重放已提交的確切有效負載
至 Torii。