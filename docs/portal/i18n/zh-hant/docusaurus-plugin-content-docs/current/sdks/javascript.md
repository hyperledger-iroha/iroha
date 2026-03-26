---
slug: /sdks/javascript
lang: zh-hant
direction: ltr
source: docs/portal/docs/sdks/javascript.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: JavaScript SDK quickstart
description: Build transactions, stream events, and drive Connect previews with `@iroha/iroha-js`.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

`@iroha/iroha-js` 是用於與 Torii 交互的規範 Node.js 包。它
捆綁 Norito 構建器、Ed25519 幫助器、分頁實用程序和彈性
HTTP/WebSocket 客戶端，以便您可以從 TypeScript 鏡像 CLI 流。

## 安裝

```bash
npm install @iroha/iroha-js
# Required once after install so the native bindings are compiled
npm run build:native
```

構建步驟包裝 `cargo build -p iroha_js_host`。確保工具鏈來自
在運行 `npm run build:native` 之前，`rust-toolchain.toml` 在本地可用。

## 密鑰管理

```ts
import {
  generateKeyPair,
  publicKeyFromPrivate,
  signEd25519,
  verifyEd25519,
} from "@iroha/iroha-js";

const { publicKey, privateKey } = generateKeyPair();

const message = Buffer.from("hello iroha");
const signature = signEd25519(message, privateKey);

console.assert(verifyEd25519(message, signature, publicKey));

const derived = publicKeyFromPrivate(privateKey);
console.assert(Buffer.compare(derived, publicKey) === 0);
```

## 建立交易

Norito 指令構建器標準化標識符、元數據和數量，以便
編碼交易與 Rust/CLI 有效負載匹配。

```ts
import {
  buildMintAssetInstruction,
  buildTransferAssetInstruction,
  buildMintAndTransferTransaction,
} from "@iroha/iroha-js";

const mint = buildMintAssetInstruction({
  assetId: "norito:4e52543000000001",
  quantity: "10",
});

const transfer = buildTransferAssetInstruction({
  sourceAssetId: "norito:4e52543000000001",
  destinationAccountId: "<katakana-i105-account-id>",
  quantity: "5",
});

const { signedTransaction } = buildMintAndTransferTransaction({
  chainId: "test-chain",
  authority: "<katakana-i105-account-id>",
  mint: { assetId: "norito:4e52543000000001", quantity: "10" },
  transfers: [{ destinationAccountId: "<katakana-i105-account-id>", quantity: "5" }],
  privateKey: Buffer.alloc(32, 0x42),
});
```

## Torii 客戶端配置

`ToriiClient` 接受鏡像 `iroha_config` 的重試/超時旋鈕。使用
`resolveToriiClientConfig` 合併駝峰式配置對象（規範化
首先是 `iroha_config`）、環境覆蓋和內聯選項。

```ts
import { ToriiClient, resolveToriiClientConfig } from "@iroha/iroha-js";
import fs from "node:fs";

const rawConfig = JSON.parse(fs.readFileSync("./iroha_config.json", "utf8"));
const config = rawConfig?.torii
  ? {
      ...rawConfig,
      torii: {
        ...rawConfig.torii,
        apiTokens: rawConfig.torii.api_tokens ?? rawConfig.torii.apiTokens,
      },
    }
  : rawConfig;
const clientConfig = resolveToriiClientConfig({
  config,
  overrides: { timeoutMs: 2_000, maxRetries: 5 },
});

const torii = new ToriiClient(
  config?.torii?.address ?? "http://localhost:8080",
  {
    config,
    timeoutMs: clientConfig.timeoutMs,
    maxRetries: clientConfig.maxRetries,
  },
);
```

本地開發環境變量：

|變量|目的|
|----------|---------|
| `IROHA_TORII_TIMEOUT_MS` |請求超時（毫秒）。 |
| `IROHA_TORII_MAX_RETRIES` |最大重試次數。 |
| `IROHA_TORII_BACKOFF_INITIAL_MS` |初始重試退避。 |
| `IROHA_TORII_BACKOFF_MULTIPLIER` |指數退避乘數。 |
| `IROHA_TORII_MAX_BACKOFF_MS` |最大重試延遲。 |
| `IROHA_TORII_RETRY_STATUSES` |用於重試的以逗號分隔的 HTTP 狀態代碼。 |
| `IROHA_TORII_RETRY_METHODS` |用於重試的以逗號分隔的 HTTP 方法。 |
| `IROHA_TORII_API_TOKEN` |添加 `X-API-Token`。 |
| `IROHA_TORII_AUTH_TOKEN` |添加 `Authorization: Bearer …` 標頭。 |

重試配置文件鏡像 Android 默認值並導出以進行奇偶校驗：
`DEFAULT_TORII_CLIENT_CONFIG`, `DEFAULT_RETRY_PROFILE_PIPELINE`,
`DEFAULT_RETRY_PROFILE_STREAMING`。參見 `docs/source/sdk/js/torii_retry_policy.md`
用於端點到配置文件映射和參數治理審計
JS4/JS7。

## 可迭代列表和分頁

分頁助手反映了 `/v1/accounts` 的 Python SDK 人體工程學，
`/v1/domains`、`/v1/assets/definitions`、NFT、餘額、資產持有者以及
賬戶交易歷史記錄。

```ts
const { items, total } = await torii.listDomains({
  limit: 25,
  sort: [{ key: "id", order: "asc" }],
});
console.log(`first page out of ${total}`, items);

for await (const account of torii.iterateAccounts({
  pageSize: 50,
  maxItems: 200,
})) {
  console.log(account.id);
}

const defs = await torii.queryAssetDefinitions({
  filter: { Eq: ["metadata.display_name", "Ticket"] },
  sort: [{ key: "metadata.display_name", order: "desc" }],
  fetchSize: 64,
});
console.log("filtered definitions", defs.items);

const assetId = "norito:4e52543000000001";
const balances = await torii.listAccountAssets("sorauロ1PaQスGh1エ6pAワnqクfJuソMムVqマvQミレシセヒaネウハc1コハ1GGM2D", {
  limit: 10,
  assetId,
});
const txs = await torii.listAccountTransactions("sorauロ1PaQスGh1エ6pAワnqクfJuソMムVqマvQミレシセヒaネウハc1コハ1GGM2D", {
  limit: 5,
  assetId,
});
const holders = await torii.listAssetHolders("62Fk4FPcMuLvW5QjDGNF2a4jAmjM", {
  limit: 5,
  assetId,
});
console.log(balances.items, txs.items, holders.items);
```

## 離線津貼和判決元數據

離線津貼響應預先公開豐富的賬本元數據 -
`expires_at_ms`、`policy_expires_at_ms`、`refresh_at_ms`、`verdict_id_hex`、
`attestation_nonce_hex` 和 `remaining_amount` 與原始數據一起返回
記錄，以便儀表板不必解碼嵌入式 Norito 有效負載。新的
倒計時助手（`deadline_kind`、`deadline_state`、`deadline_ms`、
`deadline_ms_remaining`) 突出顯示下一個即將到期的截止日期（刷新→政策
→ 證書），以便 UI 徽章可以在津貼出現時警告操作員
剩餘時間< 24 小時。軟件開發工具包
鏡像 `/v1/offline/reserve/topup` 暴露的 REST 過濾器：
`certificateExpiresBeforeMs/AfterMs`, `policyExpiresBeforeMs/AfterMs`,
`verdictIdHex`、`attestationNonceHex`、`refreshBeforeMs/AfterMs` 和
`requireVerdict` / `onlyMissingVerdict` 布爾值。無效組合（對於
例如 `onlyMissingVerdict` + `verdictIdHex`) 在 Torii 之前被本地拒絕
被稱為。

```ts
const { items: allowances } = await torii.listOfflineAllowances({
  limit: 25,
  policyExpiresBeforeMs: Date.now() + 86_400_000,
  requireVerdict: true,
});

for (const entry of allowances) {
  console.log(
    entry.controller_display,
    entry.remaining_amount,
    entry.verdict_id_hex,
    entry.refresh_at_ms,
  );
}
```

## 線下充值（發行+註冊）

當您想要立即頒發證書時，請使用充值助手
將其登記在分類賬上。 SDK驗證頒發並註冊的證書
返回前 ID 匹配，並且響應包含兩個有效負載。有
無專用充值端點；助手鍊接問題+註冊調用。如果
您已經擁有簽名證書，請致電 `registerOfflineAllowance`（或
`renewOfflineAllowance`) 直接。

```ts
const topUp = await torii.topUpOfflineAllowance({
  authority: "<account_i105>",
  privateKeyHex: alicePrivateKey,
  certificate: draftCertificate,
});
console.log(topUp.certificate.certificate_id_hex);
console.log(topUp.registration.certificate_id_hex);

const renewed = await torii.topUpOfflineAllowanceRenewal(
  topUp.registration.certificate_id_hex,
  {
    authority: "<account_i105>",
    privateKeyHex: alicePrivateKey,
    certificate: draftCertificate,
  },
);
console.log(renewed.registration.certificate_id_hex);
```

## Torii 查詢和流式傳輸（WebSockets）

查詢助手公開狀態、Prometheus 指標、遙測快照和事件
使用 Norito 過濾語法的流。流媒體自動升級到
WebSockets 並在重試預算允許時恢復。

```ts
const status = await torii.getSumeragiStatus();
console.log(status?.leader_index);

const metrics = await torii.getMetrics({ asText: true });
console.log(metrics.split("\n").slice(0, 5));

const abort = new AbortController();
for await (const event of torii.streamEvents({
  filter: { Pipeline: { Block: {} } },
  signal: abort.signal,
})) {
  console.log(event.id, event.data);
  break;
}
abort.abort(); // closes the underlying WebSocket cleanly
```

將 `streamBlocks`、`streamTransactions` 或 `streamTelemetry` 用於其他
WebSocket 端點。所有流式處理助手都表面重試嘗試，因此掛鉤
`onReconnect` 回調以提供儀表板和警報。

## 資源管理器快照和 QR 有效負載

Explorer 遙測為 `/v1/explorer/metrics` 和
`/v1/explorer/accounts/{account_id}/qr` 端點，以便儀表板可以重播
為門戶提供支持的相同快照。 `getExplorerMetrics()` 標準化
當路由被禁用時，有效負載並返回 `null`。與它配對
`getExplorerAccountQr()` 每當您需要 i105（首選）/sora（第二好的）文字加上內聯時
用於共享按鈕的 SVG。

```ts
import { promises as fs } from "node:fs";

const snapshot = await torii.getExplorerMetrics();
if (!snapshot) {
  console.warn("explorer metrics unavailable");
} else {
  console.log("peers:", snapshot.peers);
  console.log("last block:", snapshot.blockHeight, snapshot.blockCreatedAt);
  console.log("avg commit ms:", snapshot.averageCommitTimeMs ?? "n/a");
}

const qr = await torii.getExplorerAccountQr("<katakana-i105-account-id>");
console.log("explorer literal", qr.literal);
await fs.writeFile("alice.svg", qr.svg, "utf8");
console.log(
  `qr metadata v${qr.qrVersion} ec=${qr.errorCorrection} prefix=${qr.networkPrefix}`,
);
```

傳遞 `i105` 鏡像資源管理器的默認壓縮
選擇器；忽略首選 i105 輸出的覆蓋或請求 `i105_qr`
當您需要二維碼安全版本時。壓縮文字是第二好的
僅 Sora 的 UX 選項。助手總是返回規範標識符，
所選文字和元數據（網絡前綴、QR 版本/模塊、錯誤
校正層和內聯 SVG），因此 CI/CD 可以發布與
Explorer 無需調用定制轉換器即可浮出水面。

## 連接會話和排隊

Connect 幫助程序鏡像 `docs/source/connect_architecture_strawman.md`。的
預覽就緒會話的最快路徑是 `bootstrapConnectPreviewSession`，
它將確定性 SID/URI 生成和 Torii 縫合在一起
登記電話。

```ts
import {
  ToriiClient,
  bootstrapConnectPreviewSession,
  ConnectQueueError,
} from "@iroha/iroha-js";

const torii = new ToriiClient("https://torii.nexus.example");
const { preview, session, tokens } = await bootstrapConnectPreviewSession(
  torii,
  {
    chainId: "sora-mainnet",
    node: "https://torii.nexus.example",
    sessionOptions: { node: "https://torii.backup.example" },
  },
);

console.log("wallet QR", preview.walletUri);
console.log("Connect tokens", tokens?.wallet, tokens?.app);
```

- 當您只需要 QR/deeplink 的確定性 URI 時，傳遞 `register: false`
  預覽。
- 當您需要派生會話 ID 時，`generateConnectSid` 保持可用
  無需創建 URI。
- 方向鍵和密文信封來自本機橋；當
  不可用，SDK 會回退到 JSON 編解碼器並拋出異常
  `ConnectQueueError.bridgeUnavailable`。
- 脫機緩衝區在 IndexedDB 中存儲為 Norito `.to` blob。監控隊列
  通過發出的 `ConnectQueueError.overflow(limit)` 狀態/
  `.expired(ttlMs)` 錯誤並按概述饋送 `connect.queue_depth` 遙測
  在路線圖中。

### 連接註冊表和策略快照

平台運營商可以自省並更新 Connect 註冊表，而無需
離開 Node.js。 `iterateConnectApps()` 通過註冊表進行分頁，同時
`getConnectStatus()` 和 `getConnectAppPolicy()` 公開運行時計數器和
當前的政策範圍。 `updateConnectAppPolicy()` 接受駝峰命名法字段，
因此您可以暫存 Torii 期望的相同 JSON 有效負載。

```ts
const status = await torii.getConnectStatus();
console.log("connect enabled:", status?.enabled ?? false);
console.log("active sessions:", status?.sessionsActive ?? 0);
console.log("buffered bytes:", status?.totalBufferBytes ?? 0);

for await (const app of torii.iterateConnectApps({ limit: 100 })) {
  console.log(app.appId, app.namespaces, app.policy?.relayEnabled ? "relay" : "wallet-only");
}

const policy = await torii.getConnectAppPolicy();
if ((policy.wsPerIpMaxSessions ?? 0) < 5) {
  await torii.updateConnectAppPolicy({
    wsPerIpMaxSessions: 5,
    pingIntervalMs: policy.pingIntervalMs ?? 30_000,
    pingMissTolerance: policy.pingMissTolerance ?? 3,
  });
}
```

在應用之前始終捕獲最新的 `getConnectStatus()` 快照
突變——治理清單需要政策更新開始的證據
從艦隊目前的限制來看。

### 連接WebSocket撥號

`ToriiClient.openConnectWebSocket()` 彙編規範
`/v1/connect/ws` URL（包括`sid`、`role`和令牌參數）、升級
`http→ws` / `https→wss`，並將最終 URL 交給任意一個 WebSocket
您提供的實施。瀏覽器自動重用全局
`WebSocket`。 Node.js 調用者應傳遞一個構造函數，例如 `ws`：

```ts
import WebSocket from "ws";
import { ToriiClient } from "@iroha/iroha-js";

const torii = new ToriiClient(process.env.IROHA_TORII_URL ?? "https://torii.nexus.example");
const preview = await torii.createConnectSessionPreview({ chainId: "sora-mainnet" });
const session = await torii.createConnectSession({ sid: preview.sidBase64Url });

const socket = torii.openConnectWebSocket({
  sid: session.sid,
  role: "wallet",
  token: session.token_wallet,
  WebSocketImpl: WebSocket,
  protocols: ["iroha-connect"],
});

socket.addEventListener("message", (event) => {
  console.log("Connect payload", event.data);
});
socket.addEventListener("close", () => {
  console.log("Connect socket closed");
});

socket.binaryType = "arraybuffer";
socket.addEventListener("message", (event) => {
  if (typeof event.data === "string") {
    const control = JSON.parse(event.data);
    console.log("[ws] control", control.kind);
    return;
  }
  pendingFrames.enqueue(new Uint8Array(event.data));
});
```

當您只需要 URL 時，請調用 `torii.buildConnectWebSocketUrl(params)` 或
頂級 `buildConnectWebSocketUrl(baseUrl, params)` 幫助程序並重用
自定義傳輸/隊列中的結果字符串。

正在尋找完整的面向 CLI 的示例？的
[連接預覽配方](./recipes/javascript-connect-preview.md) 包括一個
可運行的腳本加上遙測指導，反映了路線圖的可交付成果
記錄 Connect 隊列 + WebSocket 流程。

### 隊列遙測和警報

將隊列指標直接連接到輔助界面，以便儀表板可以鏡像
路線圖 KPI。

```ts
import { bootstrapConnectPreviewSession, ConnectQueueError } from "@iroha/iroha-js";

async function dialWithTelemetry(client: ToriiClient) {
  try {
    const { session } = await bootstrapConnectPreviewSession(client, { chainId: "sora-mainnet" });
    queueDepthGauge.record(session.queue_depth ?? 0);
    // …open the WebSocket here…
  } catch (error) {
    if (error instanceof ConnectQueueError) {
      if (error.kind === ConnectQueueError.KIND.OVERFLOW) {
        queueOverflowCounter.add(1, { limit: error.limit ?? 0 });
      } else if (error.kind === ConnectQueueError.KIND.EXPIRED) {
        queueExpiryCounter.add(1, { ttlMs: error.ttlMs ?? 0 });
      }
      return;
    }
    throw error;
  }
}
```

`ConnectQueueError#toConnectError()` 將隊列故障轉換為通用故障
`ConnectError` 分類，因此共享 HTTP/WebSocket 攔截器可以發出
標準 `connect.queue_depth`、`connect.queue_overflow_total` 和
整個路線圖中引用的 `connect.queue_expired_total` 指標。

## 流式觀察者和事件光標

`ToriiClient.streamEvents()` 將 `/v1/events/sse` 公開為具有自動功能的異步迭代器
重試，因此 Node/Bun CLI 可以像 Rust CLI 一樣跟踪管道活動。
將 `Last-Event-ID` 光標保留在操作手冊工件旁邊，以便操作員可以
當進程重新啟動時，恢復流而不跳過事件。

```ts
import fs from "node:fs/promises";
import { ToriiClient, extractPipelineStatusKind } from "@iroha/iroha-js";

const torii = new ToriiClient(process.env.TORII_URL ?? "http://127.0.0.1:8080");
const cursorFile = process.env.STREAM_CURSOR_FILE ?? ".cache/torii.cursor";
const resumeId = await fs
  .readFile(cursorFile, "utf8")
  .then((value) => value.trim())
  .catch(() => null);
const controller = new AbortController();

process.once("SIGINT", () => controller.abort());
process.once("SIGTERM", () => controller.abort());

for await (const event of torii.streamEvents({
  filter: { Pipeline: { Transaction: { status: "Committed" } } },
  lastEventId: resumeId || undefined,
  signal: controller.signal,
})) {
  if (event.id) {
    await fs.writeFile(cursorFile, `${event.id}\n`, "utf8");
  }
  const status = event.data ? extractPipelineStatusKind(event.data) : null;
  console.log(`[${event.event}] id=${event.id ?? "∅"} status=${status ?? "n/a"}`);
}
```

- 切換 `PIPELINE_STATUS`（例如 `Pending`、`Applied` 或 `Approved`）或設置
  `STREAM_FILTER_JSON` 重播 CLI 接受的相同過濾器。
- `STREAM_MAX_EVENTS=0 node ./recipes/streaming.mjs` 使迭代器保持活動狀態，直到
  收到信號；當您只需要前幾個事件時，通過 `STREAM_MAX_EVENTS=25`
  進行冒煙測試。
- `ToriiClient.streamSumeragiStatus()` 鏡像相同的接口
  `/v1/sumeragi/status/sse` 因此共識遙測可以單獨進行尾部，並且
  迭代器以同樣的方式尊重 `Last-Event-ID`。
- 請參閱 `javascript/iroha_js/recipes/streaming.mjs` 以了解交鑰匙 CLI（光標持久性、
  JS4 中使用的 env-var 過濾器覆蓋和 `extractPipelineStatusKind` 日誌記錄）
  流/WebSocket 路線圖可交付成果。

## UAID 組合和空間目錄

空間目錄 API 呈現通用帳戶 ID (UAID) 生命週期。的
幫助程序接受 `uaid:<hex>` 文字或原始 64 十六進制摘要 (LSB=1) 和
在提交請求之前將它們規範化：

- `getUaidPortfolio(uaid, { assetId })` 聚合每個數據空間的餘額，
  按規範賬戶 ID 對資產持有量進行分組；通過 `assetId` 來過濾
  投資組合縮減為單個資產實例。
- `getUaidBindings(uaid)` 枚舉每個數據空間↔帳戶
  綁定（`i105` 返回 `i105` 文字）。
- `getUaidManifests(uaid, { dataspaceId })` 返回每個功能清單，
  生命週期狀態，以及綁定賬戶進行審計。對於操作員證據包、清單發布/撤銷流程和 SDK 遷移
指導，遵循通用賬戶指南 (`docs/source/universal_accounts_guide.md`)
與這些客戶端助手一起，使門戶和源文檔保持同步。

```ts
import { promises as fs } from "node:fs";

const uaid = "uaid:0f4d86b20839a8ddbe8a1a3d21cf1c502d49f3f79f0fa1cd88d5f24c56c0ab11";

const portfolio = await torii.getUaidPortfolio(uaid, {
  assetId: "norito:4e52543000000002",
});
portfolio.dataspaces.forEach((entry) => {
  console.log(entry.dataspace_alias ?? entry.dataspace_id, entry.accounts.length);
});

const bindings = await torii.getUaidBindings(uaid, {} );
console.log("bindings", bindings.dataspaces);

const manifests = await torii.getUaidManifests(uaid, { dataspaceId: 11 });
console.log("manifests", manifests.manifests[0].manifest.entries.length);
```

操作員還可以輪換清單或執行緊急拒絕獲勝流程，而無需
下降到 CLI。兩個助手都接受可選的 `{ signal }` 對象，因此
可以使用 `AbortController` 取消長時間運行的提交；非對象
選項或非 `AbortSignal` 輸入在
請求命中 Torii：

```ts
import { promises as fs } from "node:fs";
import { Buffer } from "node:buffer";

const manifest = JSON.parse(
  await fs.readFile("fixtures/space_directory/capability/cbdc.manifest.json", "utf8"),
);

const controller = new AbortController();

await torii.publishSpaceDirectoryManifest(
  {
    authority: "<katakana-i105-account-id>",
    manifest,
    privateKeyHex: process.env.SPACE_DIRECTORY_KEY_HEX,
    reason: "Attester v2 rollout",
  },
  { signal: controller.signal },
);

await torii.revokeSpaceDirectoryManifest(
  {
    authority: "<katakana-i105-account-id>",
    privateKey: Buffer.from(process.env.SPACE_DIRECTORY_KEY_SEED, "hex"),
    uaid,
    dataspaceId: 11,
    revokedEpoch: 9216,
    reason: "Emergency deny-wins",
  },
  { signal: controller.signal },
);
```

`publishSpaceDirectoryManifest()` 接受原始清單 JSON（與
`fixtures/space_directory/` 下的固定裝置）或序列化到的任何對象
相同的結構。 `privateKey`、`privateKeyHex` 或 `privateKeyMultihash` 映射到
`ExposedPrivateKey` 字段 Torii 期望並默認為 `ed25519`
未提供前綴時的算法。一旦 Torii 入隊，兩個請求都會返回
指令（`202 Accepted`），此時賬本將發出
匹配 `SpaceDirectoryEvent`。

## 治理和 ISO 橋樑

`ToriiClient` 公開用於檢查合約、暫存的治理 API
提案、提交選票（普通或 ZK）、輪換理事會並呼籲
`governanceFinalizeReferendumTyped` /
`governanceEnactProposalTyped`，無手寫 DTO。 ISO 20022 助手
通過 `buildPacs008Message`/`buildPacs009Message` 遵循相同的模式和
`submitIso*`/`waitForIsoMessageStatus` 三重奏。

請參閱[治理和 ISO 橋接秘訣](./recipes/javascript-governance-iso.md)
獲取 CLI 就緒示例以及返回完整現場指南的指針
`docs/source/sdk/js/governance_iso_examples.md`。

## RBC 採樣和交付證據

JS 路線圖還需要 Roadrunner 區塊承諾 (RBC) 抽樣，以便運營商可以
證明他們通過 Sumeragi 獲取的塊與他們驗證的塊證明相匹配。
使用內置的幫助程序而不是手動構建有效負載：

1. `getSumeragiRbcSessions()` 鏡像 `/v1/sumeragi/rbc/sessions`，以及
   `findRbcSamplingCandidate()` 使用塊哈希自動選擇第一個交付的會話
   （集成套件每當
   `IROHA_TORII_INTEGRATION_RBC_SAMPLE` 未設置）。
2. `ToriiClient.buildRbcSampleRequest(session, overrides)` 標準化 `{blockHash,height,view}`
   加上可選的 `{count,seed,apiToken}` 覆蓋，因此格式錯誤的十六進製或負整數永遠不會
   達到 Torii。
3. `sampleRbcChunks()` 將請求 POST 到 `/v1/sumeragi/rbc/sample`，返回塊證明
   和 Merkle 路徑（`samples[].chunkHex`、`chunkRoot`、`payloadHash`），您應該使用
   其餘的收養證據。
4. `getSumeragiRbcDelivered(height, view)` 捕獲隊列的交付元數據，以便審核員
   可以端到端地重放證明。

```js
import assert from "node:assert";
import { ToriiClient } from "@iroha/iroha-js";

const torii = new ToriiClient(process.env.TORII_URL ?? "http://127.0.0.1:8080", {
  apiToken: process.env.TORII_API_TOKEN,
});

const candidate =
  (await torii.findRbcSamplingCandidate().catch(() => null)) ??
  (await torii.getSumeragiRbcSessions()).items.find((session) => session.delivered);
if (!candidate) {
  throw new Error("no delivered RBC session available; set IROHA_TORII_INTEGRATION_RBC_SAMPLE");
}

const request = ToriiClient.buildRbcSampleRequest(candidate, {
  count: Number(process.env.RBC_SAMPLE_COUNT ?? 2),
  seed: Number(process.env.RBC_SAMPLE_SEED ?? 0),
  apiToken: process.env.RBC_SAMPLE_API_TOKEN ?? process.env.TORII_API_TOKEN,
});

const sample = await torii.sampleRbcChunks(request);
sample.samples.forEach((chunk) => {
  assert.ok(Buffer.from(chunk.chunkHex, "hex").length > 0, "chunk must be hex");
});

const delivery = await torii.getSumeragiRbcDelivered(sample.height, sample.view);
console.log(
  `rbc height=${sample.height} view=${sample.view} chunks=${sample.samples.length} delivered=${delivery?.delivered}`,
);
```

將這兩個響應保留在您提交給治理的工件根下。覆蓋
通過 `RBC_SAMPLE_JSON='{"height":123,"view":4,"blockHash":"0x…"}'` 自動選擇會話
每當您需要探測特定塊時，並將獲取 RBC 快照失敗視為
飛行前選通錯誤，而不是默默降級到直接模式。

## 測試和持續集成

1. 緩存貨物和 npm 工件。
2. 運行 `npm run build:native`。
3. 執行 `npm test`（或 `node --test` 用於菸霧作業）。

參考 GitHub Actions 工作流程位於
`docs/source/examples/iroha_js_ci.md`。

## 後續步驟

- 查看 `javascript/iroha_js/index.d.ts` 中生成的類型。
- 探索 `javascript/iroha_js/recipes/` 下的食譜。
- 將 `ToriiClient` 與 Norito 快速入門配對，以同時檢查有效負載
  SDK調用。