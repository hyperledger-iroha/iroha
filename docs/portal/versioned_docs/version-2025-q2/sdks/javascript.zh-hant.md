---
lang: zh-hant
direction: ltr
source: docs/portal/versioned_docs/version-2025-q2/sdks/javascript.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 7c19e80d060b1ecd57524e7398420990bd9159e7c4ac431ee5b85cfbf3b3df07
source_last_modified: "2026-01-22T16:26:46.562559+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# JavaScript SDK 快速入門

`@iroha2/torii-client` 為 Torii 提供了一個瀏覽器和 Node.js 友好的包裝器。
本快速入門反映了 SDK 配方中的核心流程，因此您可以獲得
客戶端在幾分鐘內運行。有關更完整的示例，請參閱
存儲庫中的 `javascript/iroha_js/recipes/`。

## 1.安裝

```sh
npm install @iroha2/torii-client
# or
yarn add @iroha2/torii-client
```

如果您計劃在本地簽署交易，還需安裝加密助手：

```sh
npm install @iroha2/crypto-target-node  # Node18+/Bun/Deno
```

## 2.創建Torii客戶端

```ts title="client.ts"
import {ToriiClient} from '@iroha2/torii-client';

const client = ToriiClient.create({
  apiUrl: 'https://localhost:8080',
  telemetryUrl: 'https://localhost:8080', // optional
});
```

該配置反映了配方中使用的構造函數。如果你的節點
使用基本身份驗證，通過 `basicAuth` 選項傳遞 `{username, password}`。

## 3. 獲取節點狀態

```ts
const status = await client.getStatus();
console.log(status.irohaVersion, status.latestBlock.height);
```

所有讀取操作都會返回 Norito 支持的 JSON 對象。查看生成的類型
`index.d.ts` 了解字段詳細信息。

## 4.提交交易

簽名者可以使用輔助 API 構建交易：

```ts
import {createKeyPairFromHex} from '@iroha2/crypto-target-node';
import {ToriiClient, buildTransaction} from '@iroha2/torii-client';

const {publicKey, privateKey} = createKeyPairFromHex(process.env.IROHA_PRIVATE_KEY!);

const tx = buildTransaction({
  chain: '00000000-0000-0000-0000-000000000000', // ChainId
  authority: 'i105...',
  instructions: [
    {Register: {domain: {name: 'research', logo: null}}},
  ],
});

tx.sign({publicKey, privateKey});
const hash = await client.submitTransaction(tx);
console.log('Submitted tx', hash);
```

幫助程序自動將交易包裝在預期的 Norito 信封中
由 Torii 提供。有關更豐富的示例（包括等待最終結果），請參閱
`javascript/iroha_js/recipes/registration.mjs`。

## 5. 使用高級助手

SDK 捆綁了反映 CLI 的專用流程：

- **治理助手** – `recipes/governance.mjs` 演示分期
  與 `governance` 指令構建者的提案和投票。
- **ISO 橋** – `recipes/iso_bridge.mjs` 顯示如何提交 `pacs.008` 和
  使用 `/v1/iso20022` 端點輪詢傳輸狀態。
- **SoraFS 和触發器** – `src/toriiClient.js` 下的分頁助手公開
  合約、資產、觸發器和 SoraFS 提供程序的類型化迭代器。

從 `@iroha2/torii-client` 導入相關構建器函數以重用這些流。

## 6. 錯誤處理

所有 SDK 調用都會拋出帶有傳輸元數據的豐富 `ToriiClientError` 實例
和 Norito 錯誤負載。將調用包裝在 `try/catch` 中或使用 `.catch()`
用戶的表面上下文：

```ts
try {
  await client.submitTransaction(tx);
} catch (error) {
  console.error('Torii submission failed', error);
}
```

## 後續步驟

- 探索 `javascript/iroha_js/recipes/` 中端到端流程的秘訣。
- 詳細閱讀 `javascript/iroha_js/index.d.ts` 中生成的類型
  方法簽名。
- 將此 SDK 與 Norito 快速入門配對以檢查和調試有效負載
  您發送至 Torii。