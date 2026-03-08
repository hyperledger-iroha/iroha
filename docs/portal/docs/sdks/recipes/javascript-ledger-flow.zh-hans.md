---
lang: zh-hans
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

从“@site/src/components/SampleDownload”导入 SampleDownload；

本配方使用 Node.js `@iroha2/torii-client` 和
`@iroha2/crypto-target-node` 软件包可重现 CLI 账本演练。

<样本下载
  href="/sdk-recipes/javascript/ledger-flow.mjs"
  文件名=“ledger-flow.mjs”
  description="下载本分类帐演练中使用的确切 JavaScript 脚本。"
/>

## 先决条件

```bash
npm install @iroha2/torii-client @iroha2/crypto-target-node
export ADMIN_ACCOUNT="ih58..."
export RECEIVER_ACCOUNT="ih58..."
export ADMIN_PRIVATE_KEY="802620CCF31D85E3B32A4BEA59987CE0C78E3B8E2DB93881468AB2435FE45D5C9DCD53"
```

## 示例脚本

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
    {Register: {assetDefinition: {numeric: {id: 'coffee#wonderland'}}}},
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
  if (asset.id.definition === 'coffee#wonderland') {
    console.log('Receiver holds', asset.value, 'units of', asset.id.definition);
  }
}
```

使用 `node --env-file=.env ledger-flow.mjs` 运行（或导出环境
手动变量）。日志应显示交易哈希（来自收据
有效负载）和更新的接收方余额。

## 验证奇偶校验

- 通过 `iroha --config defaults/client.toml transaction get --hash <hash>` 获取交易详细信息。
- 与 `iroha --config defaults/client.toml asset list filter '{"id":"norito:4e52543000000002"}'` 交叉检查余额。
- 将发出的哈希值与 Rust 和 Python 配方进行比较，以确保 SDK 奇偶校验。