---
lang: zh-hant
direction: ltr
source: docs/source/compliance/android/eu/legal_signoff_memo_2026-02.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: eb92b77765ced36213a0bde55581f29d59c262f398c658f35a1fb43a182fe296
source_last_modified: "2025-12-29T18:16:35.926476+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# AND6 歐盟法律簽署備忘錄 — 2026.1 GA (Android SDK)

## 總結

- **發布/訓練：** 2026.1 GA (Android SDK)
- **審核日期：** 2026-04-15
- **顧問/審閱者：** Sofia Martins — 合規與法律
- **範圍：** ETSI EN 319 401 安全目標、GDPR DPIA 摘要、SBOM 證明、AND6 設備實驗室應急證據
- **相關票證：** `_android-device-lab` / AND6-DR-202602，AND6 治理跟踪器 (`GOV-AND6-2026Q1`)

## 工件清單

|文物| SHA-256 |位置/鏈接|筆記|
|----------|---------|-----------------|--------|
| `security_target.md` | `385d17a55579d2b0b365e21090ee081ded79e44655690b2abfbf54068c9b55b0` | `docs/source/compliance/android/eu/security_target.md` |匹配 2026.1 GA 版本標識符和威脅模型增量（Torii NRPC 添加）。 |
| `gdpr_dpia_summary.md` | `8ef338a20104dc5d15094e28a1332a604b68bdcfef1ff82fea784d43fdbd10b5` | `docs/source/compliance/android/eu/gdpr_dpia_summary.md` |參考 AND7 遙測政策 (`docs/source/sdk/android/telemetry_redaction.md`)。 |
| `sbom_attestation.md` | `c2e0de176d4bb8c8e09329e2b9ee5dd93228d3f0def78225c1d8b777a5613f2d` | `docs/source/compliance/android/eu/sbom_attestation.md` + Sigstore 捆綁包 (`android-sdk-release#4821`)。 | CycloneDX + 出處已審查；與 Buildkite 作業 `android-sdk-release#4821` 匹配。 |
|證據日誌| `0b2d2f9eddada06faa70620f608c3ad1ec38f378d2cbddc24b15d0a83fcc381d` | `docs/source/compliance/android/evidence_log.csv`（行 `android-device-lab-failover-20260220`）|確認日誌捕獲的捆綁散列+容量快照+備忘錄條目。 |
|設備實驗室應急包| `faf32356dfc0bbca1459b14d75f3306ea1c10cb40f3180fe1758ac5105016f85` | `artifacts/android/device_lab_contingency/20260220-failover-drill/` |取自 `bundle-manifest.json` 的哈希值； ticket AND6-DR-202602 recorded hand-off to Legal/Compliance. |

## 調查結果和例外情況

- 未發現阻塞問題。工件符合 ETSI/GDPR 要求； AND7 遙測奇偶校驗已在 DPIA 摘要中註明，無需額外緩解措施。
- 建議：監控計劃的 DR-2026-05-Q2 演練（票證 AND6-DR-202605），並在下一個治理檢查點之前將生成的包附加到證據日誌中。

## 批准

- **決定：** 批准
- **簽名/時間戳：** _Sofia Martins（通過治理門戶進行數字簽名，2026-04-15 14:32 UTC）_
- **後續所有者：** 設備實驗室運營（在 2026 年 5 月 31 日之前交付 DR-2026-05-Q2 證據包）