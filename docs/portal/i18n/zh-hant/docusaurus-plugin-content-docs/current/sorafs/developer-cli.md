---
id: developer-cli
lang: zh-hant
direction: ltr
source: docs/portal/docs/sorafs/developer-cli.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: SoraFS CLI Cookbook
sidebar_label: CLI Cookbook
description: Task-focused walkthrough of the consolidated `sorafs_cli` surface.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::注意規範來源
:::

合併的 `sorafs_cli` 表面（由 `sorafs_car` 板條箱提供，
啟用 `cli` 功能）公開準備 SoraFS 所需的每個步驟
文物。使用這本食譜可以直接跳轉到常見的工作流程；將其與
用於操作上下文的清單管道和編排器運行手冊。

## 包有效負載

使用 `car pack` 生成確定性 CAR 檔案和塊計劃。的
除非提供句柄，否則命令會自動選擇 SF-1 分塊器。

```bash
sorafs_cli car pack \
  --input fixtures/video.mp4 \
  --car-out artifacts/video.car \
  --plan-out artifacts/video.plan.json \
  --summary-out artifacts/video.car.json
```

- 默認分塊器句柄：`sorafs.sf1@1.0.0`。
- 目錄輸入按字典順序排列，因此校驗和保持穩定
  跨平台。
- JSON 摘要包括有效負載摘要、每個塊元數據和根
  由註冊表和協調器識別的 CID。

## 構建清單

```bash
sorafs_cli manifest build \
  --summary artifacts/video.car.json \
  --pin-min-replicas 4 \
  --pin-storage-class hot \
  --pin-retention-epoch 96 \
  --manifest-out artifacts/video.manifest.to \
  --manifest-json-out artifacts/video.manifest.json
```

- `--pin-*` 選項直接映射到 `PinPolicy` 字段
  `sorafs_manifest::ManifestBuilder`。
- 當您希望 CLI 重新計算 SHA3 塊時提供 `--chunk-plan`
  提交前先進行消化；否則它會重用嵌入的摘要
  總結。
- JSON 輸出鏡像 Norito 有效負載，以便在
  評論。

## 在沒有長期密鑰的情況下簽署清單

```bash
sorafs_cli manifest sign \
  --manifest artifacts/video.manifest.to \
  --bundle-out artifacts/video.manifest.bundle.json \
  --signature-out artifacts/video.manifest.sig \
  --identity-token-env SIGSTORE_ID_TOKEN
```

- 接受內聯標記、環境變量或基於文件的源。
- 添加出處元數據（`token_source`、`token_hash_hex`、塊摘要）
  除非 `--include-token=true`，否則不會保留原始 JWT。
- 在 CI 中運行良好：通過設置與 GitHub Actions OIDC 結合
  `--identity-token-provider=github-actions`。

## 提交清單至 Torii

```bash
sorafs_cli manifest submit \
  --manifest artifacts/video.manifest.to \
  --chunk-plan artifacts/video.plan.json \
  --torii-url https://gateway.example/v1 \
  --authority soraカタカナ... \
  --private-key ed25519:0123...beef \
  --alias-namespace sora \
  --alias-name video::launch \
  --alias-proof fixtures/alias_proof.bin \
  --summary-out artifacts/video.submit.json
```

- 對別名證明執行 Norito 解碼並驗證它們是否匹配
  在發佈到 Torii 之前清單摘要。
- 根據計劃重新計算塊 SHA3 摘要以防止不匹配攻擊。
- 響應摘要捕獲 HTTP 狀態、標頭和註冊表有效負載
  後期審核。

## 驗證CAR內容和證明

```bash
sorafs_cli proof verify \
  --manifest artifacts/video.manifest.to \
  --car artifacts/video.car \
  --summary-out artifacts/video.verify.json
```

- 重建 PoR 樹並將有效負載摘要與清單摘要進行比較。
- 捕獲提交複製證明時所需的計數和標識符
  到治理。

## 流證明遙測

```bash
sorafs_cli proof stream \
  --manifest artifacts/video.manifest.to \
  --gateway-url https://gateway.example/v1/sorafs/proof/stream \
  --provider-id provider::alpha \
  --samples 32 \
  --stream-token "$(cat stream.token)" \
  --summary-out artifacts/video.proof_stream.json \
  --governance-evidence-dir artifacts/video.proof_stream_evidence
```

- 為每個流式證明發出 NDJSON 項目（禁用重播）
  `--emit-events=false`）。
- 聚合成功/失敗計數、延遲直方圖和採樣失敗
  摘要 JSON，以便儀表板可以繪製結果而無需抓取日誌。
- 當網關報告故障或本地 PoR 驗證時退出非零
  （通過 `--por-root-hex`）拒絕證明。調整閾值
  `--max-failures` 和 `--max-verification-failures` 用於排練。
- 今天支持PoR； PDP 和 PoTR 在 SF-13/SF-14 後重複使用相同的信封
  土地。
- `--governance-evidence-dir` 寫入渲染的摘要、元數據（時間戳、
  CLI 版本、網關 URL、清單摘要）以及清單的副本
  提供的目錄，以便治理數據包可以存檔證明流
  無需重播運行即可獲得證據。

## 其他參考資料

- `docs/source/sorafs_cli.md` — 詳盡的標誌文檔。
- `docs/source/sorafs_proof_streaming.md` — 證明遙測模式和 Grafana
  儀表板模板。
- `docs/source/sorafs/manifest_pipeline.md` — 深入探討分塊、清單
  組成和 CAR 處理。