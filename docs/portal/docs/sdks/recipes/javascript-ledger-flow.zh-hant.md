---
lang: zh-hant
direction: ltr
source: docs/portal/docs/sdks/recipes/javascript-ledger-flow.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: c39b3066b18659388a549cd23d5ba6bfed27844b5ad3ff2e3e6779301e4b53a3
source_last_modified: "2026-01-22T16:26:46.513222+00:00"
translation_last_reviewed: 2026-02-07
title: JavaScript ledger flow recipe
description: Register an asset, mint, transfer, and query balances with `@iroha2/torii-client`.
slug: /sdks/recipes/javascript-ledger-flow
translator: machine-google-reviewed
---

從“@site/src/components/SampleDownload”導入 SampleDownload；

本配方使用 Node.js `@iroha2/torii-client` 和
`@iroha2/crypto-target-node` 軟件包可重現 CLI 賬本演練。

<樣本下載
  href="/sdk-recipes/javascript/ledger-flow.mjs"
  文件名=“ledger-flow.mjs”
  description="下載本分類帳演練中使用的確切 JavaScript 腳本。"
/>

## 先決條件

```bash
npm install @iroha2/torii-client @iroha2/crypto-target-node
export ADMIN_ACCOUNT="<katakana-i105-account-id>"
export RECEIVER_ACCOUNT="<katakana-i105-account-id>"
export ADMIN_PRIVATE_KEY="802620CCF31D85E3B32A4BEA59987CE0C78E3B8E2DB93881468AB2435FE45D5C9DCD53"
```

## 示例腳本

```ts title="ledger-flow.mjs"
import {ToriiClient, buildTransaction} from '@iroha2/torii-client';
import {createKeyPairFromHex} from '@iroha2/crypto-target-node';

const adminAccount = process.env.ADMIN_ACCOUNT!;
const receiverAccount = process.env.RECEIVER_ACCOUNT!;
const adminPrivateKey = process.env.ADMIN_PRIVATE_KEY!;

const client = ToriiClient.create({
  apiUrl: 'http://127.0.0.1:8080',
});

const {publicKey, privateKey} = createKeyPairFromHex(adminPrivateKey);

const tx = buildTransaction({
  chain: '00000000-0000-0000-0000-000000000000',
  authority: adminAccount,
  instructions: [
    {Register: {assetDefinition: {numeric: {id: '7Sp2j6zDvJFnMoscAiMaWbWHRDBZ'}}}},
    {Mint: {asset: {id: `norito:4e52543000000002`}, value: {quantity: '250'}}},
    {Transfer: {
      asset: {id: `norito:4e52543000000002`},
      destination: receiverAccount,
      value: {quantity: '50'},
    }},
  ],
});

tx.sign({publicKey, privateKey});
const receipt = await client.submitTransaction(tx);
const txHash = receipt?.payload?.tx_hash ?? "<pending>";
console.log('Submitted tx', txHash);

const balances = await client.listAccountAssets(receiverAccount, {limit: 10});
for (const asset of balances.items) {
  if (asset.id.definition === '7Sp2j6zDvJFnMoscAiMaWbWHRDBZ') {
    console.log('Receiver holds', asset.value, 'units of', asset.id.definition);
  }
}
```

使用 `node --env-file=.env ledger-flow.mjs` 運行（或導出環境
手動變量）。日誌應顯示交易哈希（來自收據
有效負載）和更新的接收方餘額。

## 驗證奇偶校驗

- 通過 `iroha --config defaults/client.toml transaction get --hash <hash>` 獲取交易詳細信息。
- 與 `iroha --config defaults/client.toml asset list filter '{"id":"norito:4e52543000000002"}'` 交叉檢查餘額。
- 將發出的哈希值與 Rust 和 Python 配方進行比較，以確保 SDK 奇偶校驗。