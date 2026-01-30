---
lang: he
direction: rtl
source: docs/portal/i18n/ja/docusaurus-plugin-content-docs/current/sdks/recipes/javascript-ledger-flow.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 6765ca388de1b4cb937f86c8fee738012b10d7f145c74030fba54d048a89b2e1
source_last_modified: "2026-01-30T14:57:12+00:00"
translation_last_reviewed: 2026-01-30
---

---
lang: ja
direction: ltr
source: docs/portal/docs/sdks/recipes/javascript-ledger-flow.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
slug: /sdks/recipes/javascript-ledger-flow
title: JavaScript 台帳フローレシピ
description: `@iroha2/torii-client` でアセットを登録し、ミント、転送、残高照会を行う。
---

import SampleDownload from '@site/src/components/SampleDownload';

このレシピは Node.js の `@iroha2/torii-client` と `@iroha2/crypto-target-node` パッケージを使って、CLI の台帳ウォークスルーを再現します。

<SampleDownload
  href="/sdk-recipes/javascript/ledger-flow.mjs"
  filename="ledger-flow.mjs"
  description="この台帳ウォークスルーで使用した JavaScript スクリプトをダウンロードします。"
/>

## 前提条件

```bash
npm install @iroha2/torii-client @iroha2/crypto-target-node
export ADMIN_ACCOUNT="ih58..."
export RECEIVER_ACCOUNT="ih58..."
export ADMIN_PRIVATE_KEY="802620CCF31D85E3B32A4BEA59987CE0C78E3B8E2DB93881468AB2435FE45D5C9DCD53"
```

## サンプルスクリプト

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
    {Mint: {asset: {id: `coffee#wonderland##${adminAccount}`}, value: {quantity: '250'}}},
    {Transfer: {
      asset: {id: `coffee#wonderland##${adminAccount}`},
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

`node --env-file=.env ledger-flow.mjs` で実行します（または環境変数を手動でエクスポートします）。ログにはトランザクションハッシュ（レシートペイロード由来）と受信者の更新後残高が表示されます。

## パリティの確認

- `iroha --config defaults/client.toml transaction get --hash <hash>` でトランザクション詳細を取得します。
- `iroha --config defaults/client.toml asset list filter '{"id":"coffee#wonderland##<account>"}'` で残高を突き合わせます。
- 発行されたハッシュを Rust と Python のレシピと比較して SDK のパリティを確認します。
