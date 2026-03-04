---
lang: zh-hans
direction: ltr
source: docs/portal/versioned_docs/version-2025-q2/sdks/javascript.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 7c19e80d060b1ecd57524e7398420990bd9159e7c4ac431ee5b85cfbf3b3df07
source_last_modified: "2026-01-22T16:26:46.562559+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# JavaScript SDK 快速入门

`@iroha2/torii-client` 为 Torii 提供了一个浏览器和 Node.js 友好的包装器。
本快速入门反映了 SDK 配方中的核心流程，因此您可以获得
客户端在几分钟内运行。有关更完整的示例，请参阅
存储库中的 `javascript/iroha_js/recipes/`。

## 1.安装

```sh
npm install @iroha2/torii-client
# or
yarn add @iroha2/torii-client
```

如果您计划在本地签署交易，还需安装加密助手：

```sh
npm install @iroha2/crypto-target-node  # Node18+/Bun/Deno
```

## 2.创建Torii客户端

```ts title="client.ts"
import {ToriiClient} from '@iroha2/torii-client';

const client = ToriiClient.create({
  apiUrl: 'https://localhost:8080',
  telemetryUrl: 'https://localhost:8080', // optional
});
```

该配置反映了配方中使用的构造函数。如果你的节点
使用基本身份验证，通过 `basicAuth` 选项传递 `{username, password}`。

## 3. 获取节点状态

```ts
const status = await client.getStatus();
console.log(status.irohaVersion, status.latestBlock.height);
```

所有读取操作都会返回 Norito 支持的 JSON 对象。查看生成的类型
`index.d.ts` 了解字段详细信息。

## 4.提交交易

签名者可以使用辅助 API 构建交易：

```ts
import {createKeyPairFromHex} from '@iroha2/crypto-target-node';
import {ToriiClient, buildTransaction} from '@iroha2/torii-client';

const {publicKey, privateKey} = createKeyPairFromHex(process.env.IROHA_PRIVATE_KEY!);

const tx = buildTransaction({
  chain: '00000000-0000-0000-0000-000000000000', // ChainId
  authority: 'ih58...',
  instructions: [
    {Register: {domain: {name: 'research', logo: null}}},
  ],
});

tx.sign({publicKey, privateKey});
const hash = await client.submitTransaction(tx);
console.log('Submitted tx', hash);
```

帮助程序自动将交易包装在预期的 Norito 信封中
由 Torii 提供。有关更丰富的示例（包括等待最终结果），请参阅
`javascript/iroha_js/recipes/registration.mjs`。

## 5. 使用高级助手

SDK 捆绑了反映 CLI 的专用流程：

- **治理助手** – `recipes/governance.mjs` 演示分期
  与 `governance` 指令构建者的提案和投票。
- **ISO 桥** – `recipes/iso_bridge.mjs` 显示如何提交 `pacs.008` 和
  使用 `/v1/iso20022` 端点轮询传输状态。
- **SoraFS 和触发器** – `src/toriiClient.js` 下的分页助手公开
  合约、资产、触发器和 SoraFS 提供程序的类型化迭代器。

从 `@iroha2/torii-client` 导入相关构建器函数以重用这些流。

## 6. 错误处理

所有 SDK 调用都会抛出带有传输元数据的丰富 `ToriiClientError` 实例
和 Norito 错误负载。将调用包装在 `try/catch` 中或使用 `.catch()`
用户的表面上下文：

```ts
try {
  await client.submitTransaction(tx);
} catch (error) {
  console.error('Torii submission failed', error);
}
```

## 后续步骤

- 探索 `javascript/iroha_js/recipes/` 中端到端流程的秘诀。
- 详细阅读 `javascript/iroha_js/index.d.ts` 中生成的类型
  方法签名。
- 将此 SDK 与 Norito 快速入门配对以检查和调试有效负载
  您发送至 Torii。