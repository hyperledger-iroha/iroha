---
lang: ja
direction: ltr
source: docs/portal/versioned_docs/version-2025-q2/sdks/javascript.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 7c19e80d060b1ecd57524e7398420990bd9159e7c4ac431ee5b85cfbf3b3df07
source_last_modified: "2026-01-22T06:58:48.951155+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# JavaScript SDK クイックスタート

`@iroha2/torii-client` は、Torii のブラウザーおよび Node.js フレンドリーなラッパーを提供します。
このクイックスタートは SDK レシピのコア フローを反映しているため、
クライアントは数分以内に実行されます。より完全な例については、を参照してください。
リポジトリ内の `javascript/iroha_js/recipes/`。

## 1. インストール

```sh
npm install @iroha2/torii-client
# or
yarn add @iroha2/torii-client
```

ローカルでトランザクションに署名する予定がある場合は、暗号ヘルパーもインストールします。

```sh
npm install @iroha2/crypto-target-node  # Node18+/Bun/Deno
```

## 2. Torii クライアントを作成する

```ts title="client.ts"
import {ToriiClient} from '@iroha2/torii-client';

const client = ToriiClient.create({
  apiUrl: 'https://localhost:8080',
  telemetryUrl: 'https://localhost:8080', // optional
});
```

この構成は、レシピで使用されるコンストラクターを反映しています。あなたのノードの場合
基本認証を使用する場合は、`basicAuth` オプションを介して `{username, password}` を渡します。

## 3. ノードのステータスを取得する

```ts
const status = await client.getStatus();
console.log(status.irohaVersion, status.latestBlock.height);
```

すべての読み取り操作は、Norito ベースの JSON オブジェクトを返します。生成された型を参照してください。
フィールドの詳細については `index.d.ts`。

## 4. トランザクションを送信する

署名者はヘルパー API を使用してトランザクションを構築できます。

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

ヘルパーは、予期される Norito エンベロープでトランザクションを自動的にラップします。
Torii による。より豊富な例 (ファイナリティの待機を含む) については、を参照してください。
`javascript/iroha_js/recipes/registration.mjs`。

## 5. 高レベルのヘルパーを使用する

SDK には、CLI をミラーリングする特殊なフローがバンドルされています。

- **ガバナンス ヘルパー** – `recipes/governance.mjs` がステージングをデモンストレーションします
  `governance` 命令ビルダーによる提案と投票。
- **ISO ブリッジ** – `recipes/iso_bridge.mjs` は、`pacs.008` を送信する方法を示し、
  `/v1/iso20022` エンドポイントを使用して転送ステータスをポーリングします。
- **SoraFS とトリガー** – `src/toriiClient.js` の下のページネーション ヘルパーを公開します
  コントラクト、アセット、トリガー、および SoraFS プロバイダーの型付きイテレーター。

関連するビルダー関数を `@iroha2/torii-client` からインポートして、これらのフローを再利用します。

## 6. エラー処理

すべての SDK 呼び出しは、トランスポート メタデータを含む豊富な `ToriiClientError` インスタンスをスローします
および Norito エラー ペイロード。呼び出しを `try/catch` でラップするか、`.catch()` を使用して
ユーザーにコンテキストを表面化します。

```ts
try {
  await client.submitTransaction(tx);
} catch (error) {
  console.error('Torii submission failed', error);
}
```

## 次のステップ

- `javascript/iroha_js/recipes/` のエンドツーエンド フローのレシピを調べます。
- 詳細については、`javascript/iroha_js/index.d.ts` で生成された型を参照してください。
  メソッドのシグネチャ。
- この SDK を Norito クイックスタートと組み合わせて、ペイロードを検査およびデバッグします
  Torii に送信します。