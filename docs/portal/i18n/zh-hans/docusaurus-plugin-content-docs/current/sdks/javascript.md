---
slug: /sdks/javascript
lang: zh-hans
direction: ltr
source: docs/portal/docs/sdks/javascript.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: JavaScript SDK quickstart
description: Build transactions, stream events, and drive Connect previews with `@iroha/iroha-js`.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

`@iroha/iroha-js` 是用于与 Torii 交互的规范 Node.js 包。它
捆绑 Norito 构建器、Ed25519 帮助器、分页实用程序和弹性
HTTP/WebSocket 客户端，以便您可以从 TypeScript 镜像 CLI 流。

## 安装

```bash
npm install @iroha/iroha-js
# Required once after install so the native bindings are compiled
npm run build:native
```

构建步骤包装 `cargo build -p iroha_js_host`。确保工具链来自
在运行 `npm run build:native` 之前，`rust-toolchain.toml` 在本地可用。

## 密钥管理

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

Norito 指令构建器标准化标识符、元数据和数量，以便
编码交易与 Rust/CLI 有效负载匹配。

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
  destinationAccountId: "i105...",
  quantity: "5",
});

const { signedTransaction } = buildMintAndTransferTransaction({
  chainId: "test-chain",
  authority: "i105...",
  mint: { assetId: "norito:4e52543000000001", quantity: "10" },
  transfers: [{ destinationAccountId: "i105...", quantity: "5" }],
  privateKey: Buffer.alloc(32, 0x42),
});
```

## Torii 客户端配置

`ToriiClient` 接受镜像 `iroha_config` 的重试/超时旋钮。使用
`resolveToriiClientConfig` 合并驼峰式配置对象（规范化
首先是 `iroha_config`）、环境覆盖和内联选项。

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

本地开发环境变量：

|变量|目的|
|----------|---------|
| `IROHA_TORII_TIMEOUT_MS` |请求超时（毫秒）。 |
| `IROHA_TORII_MAX_RETRIES` |最大重试次数。 |
| `IROHA_TORII_BACKOFF_INITIAL_MS` |初始重试退避。 |
| `IROHA_TORII_BACKOFF_MULTIPLIER` |指数退避乘数。 |
| `IROHA_TORII_MAX_BACKOFF_MS` |最大重试延迟。 |
| `IROHA_TORII_RETRY_STATUSES` |用于重试的以逗号分隔的 HTTP 状态代码。 |
| `IROHA_TORII_RETRY_METHODS` |用于重试的以逗号分隔的 HTTP 方法。 |
| `IROHA_TORII_API_TOKEN` |添加 `X-API-Token`。 |
| `IROHA_TORII_AUTH_TOKEN` |添加 `Authorization: Bearer …` 标头。 |

重试配置文件镜像 Android 默认值并导出以进行奇偶校验：
`DEFAULT_TORII_CLIENT_CONFIG`, `DEFAULT_RETRY_PROFILE_PIPELINE`,
`DEFAULT_RETRY_PROFILE_STREAMING`。参见 `docs/source/sdk/js/torii_retry_policy.md`
用于端点到配置文件映射和参数治理审计
JS4/JS7。

## 可迭代列表和分页

分页助手反映了 `/v1/accounts` 的 Python SDK 人体工程学，
`/v1/domains`、`/v1/assets/definitions`、NFT、余额、资产持有者以及
账户交易历史记录。

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
const balances = await torii.listAccountAssets("6cmzPVPX9mKibcHVns59R11W7wkcZTg7r71RLbydDr2HGf5MdMCQRm9", {
  limit: 10,
  assetId,
});
const txs = await torii.listAccountTransactions("6cmzPVPX9mKibcHVns59R11W7wkcZTg7r71RLbydDr2HGf5MdMCQRm9", {
  limit: 5,
  assetId,
});
const holders = await torii.listAssetHolders("rose#wonderland", {
  limit: 5,
  assetId,
});
console.log(balances.items, txs.items, holders.items);
```

## 离线津贴和判决元数据

离线津贴响应预先公开丰富的账本元数据 -
`expires_at_ms`、`policy_expires_at_ms`、`refresh_at_ms`、`verdict_id_hex`、
`attestation_nonce_hex` 和 `remaining_amount` 与原始数据一起返回
记录，以便仪表板不必解码嵌入式 Norito 有效负载。新的
倒计时助手（`deadline_kind`、`deadline_state`、`deadline_ms`、
`deadline_ms_remaining`) 突出显示下一个即将到期的截止日期（刷新→政策
→ 证书），以便 UI 徽章可以在津贴出现时警告操作员
剩余时间< 24 小时。软件开发工具包
镜像 `/v1/offline/allowances` 暴露的 REST 过滤器：
`certificateExpiresBeforeMs/AfterMs`, `policyExpiresBeforeMs/AfterMs`,
`verdictIdHex`、`attestationNonceHex`、`refreshBeforeMs/AfterMs` 和
`requireVerdict` / `onlyMissingVerdict` 布尔值。无效组合（对于
例如 `onlyMissingVerdict` + `verdictIdHex`) 在 Torii 之前被本地拒绝
被称为。

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

## 线下充值（发行+注册）

当您想要立即颁发证书时，请使用充值助手
将其登记在分类账上。 SDK验证颁发并注册的证书
返回前 ID 匹配，并且响应包含两个有效负载。有
无专用充值端点；助手链接问题+注册调用。如果
您已经拥有签名证书，请致电 `registerOfflineAllowance`（或
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

## Torii 查询和流式传输（WebSockets）

查询助手公开状态、Prometheus 指标、遥测快照和事件
使用 Norito 过滤语法的流。流媒体自动升级到
WebSockets 并在重试预算允许时恢复。

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

将 `streamBlocks`、`streamTransactions` 或 `streamTelemetry` 用于其他
WebSocket 端点。所有流式处理助手都表面重试尝试，因此挂钩
`onReconnect` 回调以提供仪表板和警报。

## 资源管理器快照和 QR 有效负载

Explorer 遥测为 `/v1/explorer/metrics` 和
`/v1/explorer/accounts/{account_id}/qr` 端点，以便仪表板可以重播
为门户提供支持的相同快照。 `getExplorerMetrics()` 标准化
当路由被禁用时，有效负载并返回 `null`。与它配对
`getExplorerAccountQr()` 每当您需要 I105（首选）/sora（第二好的）文字加上内联时
用于共享按钮的 SVG。

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

const qr = await torii.getExplorerAccountQr("i105...");
console.log("explorer literal", qr.literal);
await fs.writeFile("alice.svg", qr.svg, "utf8");
console.log(
  `qr metadata v${qr.qrVersion} ec=${qr.errorCorrection} prefix=${qr.networkPrefix}`,
);
```

传递 `I105` 镜像资源管理器的默认压缩
选择器；忽略首选 I105 输出的覆盖或请求 `i105_qr`
当您需要二维码安全版本时。压缩文字是第二好的
仅 Sora 的 UX 选项。助手总是返回规范标识符，
所选文字和元数据（网络前缀、QR 版本/模块、错误
校正层和内联 SVG），因此 CI/CD 可以发布与
Explorer 无需调用定制转换器即可浮出水面。

## 连接会话和排队

Connect 帮助程序镜像 `docs/source/connect_architecture_strawman.md`。的
预览就绪会话的最快路径是 `bootstrapConnectPreviewSession`，
它将确定性 SID/URI 生成和 Torii 缝合在一起
登记电话。

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

- 当您只需要 QR/deeplink 的确定性 URI 时，传递 `register: false`
  预览。
- 当您需要派生会话 ID 时，`generateConnectSid` 保持可用
  无需创建 URI。
- 方向键和密文信封来自本机桥；当
  不可用，SDK 会回退到 JSON 编解码器并抛出异常
  `ConnectQueueError.bridgeUnavailable`。
- 脱机缓冲区在 IndexedDB 中存储为 Norito `.to` blob。监控队列
  通过发出的 `ConnectQueueError.overflow(limit)` 状态/
  `.expired(ttlMs)` 错误并按概述馈送 `connect.queue_depth` 遥测
  在路线图中。

### 连接注册表和策略快照

平台运营商可以自省并更新 Connect 注册表，而无需
离开 Node.js。 `iterateConnectApps()` 通过注册表进行分页，同时
`getConnectStatus()` 和 `getConnectAppPolicy()` 公开运行时计数器和
当前的政策范围。 `updateConnectAppPolicy()` 接受驼峰命名法字段，
因此您可以暂存 Torii 期望的相同 JSON 有效负载。

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

在应用之前始终捕获最新的 `getConnectStatus()` 快照
突变——治理清单需要政策更新开始的证据
从舰队目前的限制来看。

### 连接WebSocket拨号

`ToriiClient.openConnectWebSocket()` 汇编规范
`/v1/connect/ws` URL（包括`sid`、`role`和令牌参数）、升级
`http→ws` / `https→wss`，并将最终 URL 交给任意一个 WebSocket
您提供的实施。浏览器自动重用全局
`WebSocket`。 Node.js 调用者应传递一个构造函数，例如 `ws`：

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

当您只需要 URL 时，请调用 `torii.buildConnectWebSocketUrl(params)` 或
顶级 `buildConnectWebSocketUrl(baseUrl, params)` 帮助程序并重用
自定义传输/队列中的结果字符串。

正在寻找完整的面向 CLI 的示例？的
[连接预览配方](./recipes/javascript-connect-preview.md) 包括一个
可运行的脚本加上遥测指导，反映了路线图的可交付成果
记录 Connect 队列 + WebSocket 流程。

### 队列遥测和警报

将队列指标直接连接到辅助界面，以便仪表板可以镜像
路线图 KPI。

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

`ConnectQueueError#toConnectError()` 将队列故障转换为通用故障
`ConnectError` 分类，因此共享 HTTP/WebSocket 拦截器可以发出
标准 `connect.queue_depth`、`connect.queue_overflow_total` 和
整个路线图中引用的 `connect.queue_expired_total` 指标。

## 流式观察者和事件光标

`ToriiClient.streamEvents()` 将 `/v1/events/sse` 公开为具有自动功能的异步迭代器
重试，因此 Node/Bun CLI 可以像 Rust CLI 一样跟踪管道活动。
将 `Last-Event-ID` 光标保留在操作手册工件旁边，以便操作员可以
当进程重新启动时，恢复流而不跳过事件。

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

- 切换 `PIPELINE_STATUS`（例如 `Pending`、`Applied` 或 `Approved`）或设置
  `STREAM_FILTER_JSON` 重播 CLI 接受的相同过滤器。
- `STREAM_MAX_EVENTS=0 node ./recipes/streaming.mjs` 使迭代器保持活动状态，直到
  收到信号；当您只需要前几个事件时，通过 `STREAM_MAX_EVENTS=25`
  进行冒烟测试。
- `ToriiClient.streamSumeragiStatus()` 镜像相同的接口
  `/v1/sumeragi/status/sse` 因此共识遥测可以单独进行尾部，并且
  迭代器以同样的方式尊重 `Last-Event-ID`。
- 请参阅 `javascript/iroha_js/recipes/streaming.mjs` 以了解交钥匙 CLI（光标持久性、
  JS4 中使用的 env-var 过滤器覆盖和 `extractPipelineStatusKind` 日志记录）
  流/WebSocket 路线图可交付成果。

## UAID 组合和空间目录

空间目录 API 呈现通用帐户 ID (UAID) 生命周期。的
帮助程序接受 `uaid:<hex>` 文字或原始 64 十六进制摘要 (LSB=1) 和
在提交请求之前将它们规范化：

- `getUaidPortfolio(uaid, { assetId })` 聚合每个数据空间的余额，
  按规范账户 ID 对资产持有量进行分组；通过 `assetId` 来过滤
  投资组合缩减为单个资产实例。
- `getUaidBindings(uaid)` 枚举每个数据空间↔帐户
  绑定（`I105` 返回 `i105` 文字）。
- `getUaidManifests(uaid, { dataspaceId })` 返回每个功能清单，
  生命周期状态，以及绑定账户进行审计。对于操作员证据包、清单发布/撤销流程和 SDK 迁移
指导，遵循通用账户指南 (`docs/source/universal_accounts_guide.md`)
与这些客户端助手一起，使门户和源文档保持同步。

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

操作员还可以轮换清单或执行紧急拒绝获胜流程，而无需
下降到 CLI。两个助手都接受可选的 `{ signal }` 对象，因此
可以使用 `AbortController` 取消长时间运行的提交；非对象
选项或非 `AbortSignal` 输入在
请求命中 Torii：

```ts
import { promises as fs } from "node:fs";
import { Buffer } from "node:buffer";

const manifest = JSON.parse(
  await fs.readFile("fixtures/space_directory/capability/cbdc.manifest.json", "utf8"),
);

const controller = new AbortController();

await torii.publishSpaceDirectoryManifest(
  {
    authority: "i105...",
    manifest,
    privateKeyHex: process.env.SPACE_DIRECTORY_KEY_HEX,
    reason: "Attester v2 rollout",
  },
  { signal: controller.signal },
);

await torii.revokeSpaceDirectoryManifest(
  {
    authority: "i105...",
    privateKey: Buffer.from(process.env.SPACE_DIRECTORY_KEY_SEED, "hex"),
    uaid,
    dataspaceId: 11,
    revokedEpoch: 9216,
    reason: "Emergency deny-wins",
  },
  { signal: controller.signal },
);
```

`publishSpaceDirectoryManifest()` 接受原始清单 JSON（与
`fixtures/space_directory/` 下的固定装置）或序列化到的任何对象
相同的结构。 `privateKey`、`privateKeyHex` 或 `privateKeyMultihash` 映射到
`ExposedPrivateKey` 字段 Torii 期望并默认为 `ed25519`
未提供前缀时的算法。一旦 Torii 入队，两个请求都会返回
指令（`202 Accepted`），此时账本将发出
匹配 `SpaceDirectoryEvent`。

## 治理和 ISO 桥梁

`ToriiClient` 公开用于检查合约、暂存的治理 API
提案、提交选票（普通或 ZK）、轮换理事会并呼吁
`governanceFinalizeReferendumTyped` /
`governanceEnactProposalTyped`，无手写 DTO。 ISO 20022 助手
通过 `buildPacs008Message`/`buildPacs009Message` 遵循相同的模式和
`submitIso*`/`waitForIsoMessageStatus` 三重奏。

请参阅[治理和 ISO 桥接秘诀](./recipes/javascript-governance-iso.md)
获取 CLI 就绪示例以及返回完整现场指南的指针
`docs/source/sdk/js/governance_iso_examples.md`。

## RBC 采样和交付证据

JS 路线图还需要 Roadrunner 区块承诺 (RBC) 抽样，以便运营商可以
证明他们通过 Sumeragi 获取的块与他们验证的块证明相匹配。
使用内置的帮助程序而不是手动构建有效负载：

1. `getSumeragiRbcSessions()` 镜像 `/v1/sumeragi/rbc/sessions`，以及
   `findRbcSamplingCandidate()` 使用块哈希自动选择第一个交付的会话
   （集成套件每当
   `IROHA_TORII_INTEGRATION_RBC_SAMPLE` 未设置）。
2. `ToriiClient.buildRbcSampleRequest(session, overrides)` 标准化 `{blockHash,height,view}`
   加上可选的 `{count,seed,apiToken}` 覆盖，因此格式错误的十六进制或负整数永远不会
   达到 Torii。
3. `sampleRbcChunks()` 将请求 POST 到 `/v1/sumeragi/rbc/sample`，返回块证明
   和 Merkle 路径（`samples[].chunkHex`、`chunkRoot`、`payloadHash`），您应该使用
   其余的收养证据。
4. `getSumeragiRbcDelivered(height, view)` 捕获队列的交付元数据，以便审核员
   可以端到端地重放证明。

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

将这两个响应保留在您提交给治理的工件根下。覆盖
通过 `RBC_SAMPLE_JSON='{"height":123,"view":4,"blockHash":"0x…"}'` 自动选择会话
每当您需要探测特定块时，并将获取 RBC 快照失败视为
飞行前选通错误，而不是默默降级到直接模式。

## 测试和持续集成

1. 缓存货物和 npm 工件。
2. 运行 `npm run build:native`。
3. 执行 `npm test`（或 `node --test` 用于烟雾作业）。

参考 GitHub Actions 工作流程位于
`docs/source/examples/iroha_js_ci.md`。

## 后续步骤

- 查看 `javascript/iroha_js/index.d.ts` 中生成的类型。
- 探索 `javascript/iroha_js/recipes/` 下的食谱。
- 将 `ToriiClient` 与 Norito 快速入门配对，以同时检查有效负载
  SDK调用。