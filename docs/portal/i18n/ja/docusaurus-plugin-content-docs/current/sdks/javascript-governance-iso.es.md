---
lang: ja
direction: ltr
source: docs/portal/docs/sdks/javascript-governance-iso.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
タイトル: ガバナンスと ISO ブリッジの例
説明: `@iroha/iroha-js` を使用して、高度な Torii ワークフローを推進します。
スラグ: /sdks/javascript/governance-iso-examples
---

このフィールド ガイドでは、ガバナンスと
ISO 20022 ブリッジ フローは `@iroha/iroha-js` で動作します。スニペットは同じものを再利用します
`ToriiClient` に同梱されているランタイム ヘルパーなので、これらを直接コピーできます。
CLI ツール、CI ハーネス、または長時間実行されるサービス。

追加のリソース:

- `javascript/iroha_js/recipes/governance.mjs` — 実行可能なエンドツーエンド スクリプト
  提案、投票、評議会のローテーション。
- `javascript/iroha_js/recipes/iso_bridge.mjs` — 送信用の CLI ヘルパー
  pacs.008/pacs.009 ペイロードとポーリングの決定的なステータス。
- `docs/source/finance/settlement_iso_mapping.md` — 正規の ISO フィールド マッピング。

## バンドルされたレシピの実行

これらの例は、`javascript/iroha_js/recipes/` のスクリプトに依存します。走る
事前に `npm install && npm run build:native` なので、生成されたバインディングは次のようになります。
利用可能です。

### ガバナンスヘルパーのウォークスルー

を呼び出す前に、次の環境変数を構成します。
`recipes/governance.mjs`:

- `TORII_URL` — Torii エンドポイント。
- `AUTHORITY` / `PRIVATE_KEY_HEX` — 署名者のアカウントとキー (16 進数)。鍵を保管しておいてください
  安全な秘密のストア。
- `CHAIN_ID` — オプションのネットワーク識別子。
- `GOV_SUBMIT=1` — 生成されたトランザクションを Torii にプッシュします。
- `GOV_FETCH=1` — 送信後に提案/ロックをフェッチします。
- `GOV_PROPOSAL_ID`、`GOV_REFERENDUM_ID`、`GOV_LOCKS_ID` — オプションのルックアップが使用されます
  `GOV_FETCH=1`のとき。

```bash
npm run build:native

# Hashes only (safe for CI smoke runs).
TORII_URL=https://torii.testnet.sora \
node javascript/iroha_js/recipes/governance.mjs

# Submit + fetch using live credentials.
TORII_URL=https://torii.testnet.sora \
AUTHORITY=i105... \
PRIVATE_KEY_HEX="$(cat ~/.iroha/keys/alice.key)" \
CHAIN_ID=7f2c...-prod \
GOV_SUBMIT=1 GOV_FETCH=1 \
GOV_PROPOSAL_ID=calc.v1 \
node javascript/iroha_js/recipes/governance.mjs
```

ハッシュはステップごとにログに記録され、次の場合に Torii 応答が表示されます。
`GOV_SUBMIT=1` のため、CI ジョブは送信エラーですぐに失敗する可能性があります。

### ISO ブリッジ ヘルパー

`recipes/iso_bridge.mjs` は pacs.008 または pacs.009 メッセージを送信し、ポーリングします
ステータスが落ち着くまで ISO ブリッジを続けてください。次のように設定します。

- `TORII_URL` — ISO ブリッジ API を公開する Torii エンドポイント。
- `ISO_MESSAGE_KIND` — `pacs.008` (デフォルト) または `pacs.009`。ヘルパーが使用するのは、
  一致するサンプル ビルダー (`buildSamplePacs008Message` / `buildSamplePacs009Message`)
  独自の XML を提供しない場合。
- `ISO_MESSAGE_SUFFIX` — サンプル ペイロード ID に追加されるオプションのサフィックス
  繰り返されるリハーサルを一意に保ちます (デフォルトは現在のエポック秒 (16 進数))。
- `ISO_CONTENT_TYPE` — 送信用の `Content-Type` ヘッダーをオーバーライドします。
  (例: `application/pacs009+xml`);ポーリングのみの場合は無視されます
  既存のメッセージ ID。
- `ISO_MESSAGE_ID` — 送信を完全にスキップし、提供されたもののみをポーリングします。
  `waitForIsoMessageStatus` 経由の識別子。
- `ISO_POLL_ATTEMPTS` / `ISO_POLL_INTERVAL_MS` — 待機戦略を調整します。
  ブリッジの展開にノイズが多い、または遅い。
- `ISO_RESOLVE_ON_ACCEPTED=1` — Torii が `Accepted` を返したらすぐに終了します。
  トランザクション ハッシュがまだ保留中であっても (ブリッジのメンテナンス中に便利です)
  台帳のコミットが遅延した場合)。

```bash
# Submit a pacs.009 message and wait for completion.
TORII_URL=https://torii.testnet.sora \
ISO_MESSAGE_KIND=pacs.009 \
ISO_POLL_ATTEMPTS=20 \
ISO_POLL_INTERVAL_MS=1500 \
node javascript/iroha_js/recipes/iso_bridge.mjs

# Poll an existing message id without re-submitting XML.
TORII_URL=https://torii.testnet.sora \
ISO_MESSAGE_ID=iso-demo-1 \
node javascript/iroha_js/recipes/iso_bridge.mjs
```

Torii が端末を報告しない場合、両方のスクリプトはステータス コード `1` で終了します。
移行するため、CI ゲート ジョブに適しています。

### ISO エイリアス ヘルパー

`recipes/iso_alias.mjs` は ISO エイリアス エンドポイントをターゲットにするため、リハーサルでカバーできるようになります。
カスタムのツールを作成せずに、ブラインド要素のハッシュとエイリアス検索を実行できます。それ
`ToriiClient.evaluateAliasVoprf` と `resolveAlias` / `resolveAliasByIndex` を呼び出します
バックエンド、ダイジェスト、アカウント バインディング、ソース、および決定論的インデックスを出力します。
Torii によって返されます。

環境変数:

- `TORII_URL` — エイリアス ヘルパーを公開する Torii エンドポイント。
- `ISO_VOPRF_INPUT` — 16 進数でエンコードされたブラインド要素 (デフォルトは `deadbeef`)。
- `ISO_SKIP_VOPRF=1` — ルックアップのみをテストする場合は、VOPRF 呼び出しをスキップします。
- `ISO_ALIAS_LABEL` — 解決するリテラル エイリアス (IBAN スタイルの文字列など)。
- `ISO_ALIAS_INDEX` — `resolveAliasByIndex` に渡される 10 進数または `0x` という接頭辞が付いたインデックス。
- `TORII_AUTH_TOKEN` / `TORII_API_TOKEN` — 安全な Torii 展開用のオプションのヘッダー。

```bash
# Evaluate a blinded element and resolve an alias literal + deterministic index.
TORII_URL=https://torii.testnet.sora \
ISO_VOPRF_INPUT=deadbeefcafebabe \
ISO_ALIAS_LABEL="GB82 WEST 1234 5698 7654 32" \
ISO_ALIAS_INDEX=0 \
node javascript/iroha_js/recipes/iso_alias.mjs

# Only perform literal resolution.
TORII_URL=https://torii.testnet.sora \
ISO_SKIP_VOPRF=1 \
ISO_ALIAS_LABEL="iso:demo:alpha" \
node javascript/iroha_js/recipes/iso_alias.mjs
```

ヘルパーは Torii の動作を反映しており、エイリアスが欠落している場合に 404 を表示します。
ランタイム無効エラーをソフト スキップとして扱うため、CI フローはブリッジを許容できます。
メンテナンスウィンドウ。

## ガバナンスのワークフロー

### 契約事例と提案書を検査する

```ts
import { ToriiClient } from "@iroha/iroha-js";

const torii = new ToriiClient(process.env.TORII_URL ?? "https://torii.nexus.example");

const instances = await torii.listGovernanceInstances("apps", {
  contains: "ledger",
  hashPrefix: "deadbeef",
  order: "hash_desc",
  limit: 5,
});
for (const entry of instances.instances) {
  console.log(`${entry.contract_id} :: ${entry.code_hash_hex}`);
}

const controller = new AbortController();
const proposal = await torii.getGovernanceProposal("proposal-001", {
  signal: controller.signal,
});
console.log(proposal?.kind, proposal?.status);
```

### 提案書と投票用紙を提出する

ガバナンス提出をキャンセルする必要がある場合、または期限付きのガバナンス提出を行う必要がある場合は、`AbortController` を使用します (SDK)
以下に示すすべての POST ヘルパーに対して、オプションの `{ signal }` オブジェクトを受け入れます。

```ts
const authority = "i105...";
const privateKey = Buffer.alloc(32, 0xaa);

// All governance writes accept optional `{ signal }` options for cancellation.
const writeController = new AbortController();
const deployDraft = await torii.governanceProposeDeployContract({
  namespace: "apps",
  contractId: "calc.v1",
  codeHash: "hash:7B38...#ABCD",
  abiHash: Buffer.alloc(32, 0xbb),
  abiVersion: "1",
  window: { lower: 12_345, upper: 12_500 },
  mode: "Plain",
}, { signal: writeController.signal });
console.log("draft instructions", deployDraft.tx_instructions.length);

const ballot = await torii.governanceSubmitPlainBallot({
  authority,
  chainId: "00000000-0000-0000-0000-000000000000",
  referendumId: "ref-plain",
  owner: authority,
  amount: "5000",
  durationBlocks: 7_200,
  direction: "Aye",
}, { signal: writeController.signal });
if (!ballot.accepted) {
  console.warn("ballot rejected", ballot.reason);
}

const zkOwner = "i105..."; // canonical i105 account id for ZK public inputs
await torii.governanceSubmitZkBallot({
  authority,
  chainId: "00000000-0000-0000-0000-000000000000",
  electionId: "ref-zk",
  proof: Buffer.alloc(96, 0xcd),
  public: {
    owner: zkOwner,
    amount: "5000",
    duration_blocks: 7_200,
    direction: "Aye",
  },
}, { signal: writeController.signal });
```

### 評議会 VRF と制定

```ts
const validatorPk = Buffer.alloc(48, 0xdd);
const validatorProof = Buffer.alloc(96, 0xee);

const current = await torii.getGovernanceCouncilCurrent();
console.log(`epoch=${current.epoch} members=${current.members.length}`);

const derived = await torii.governanceDeriveCouncilVrf({
  committeeSize: 2,
  candidates: [
    {
      accountId: "i105...",
      variant: "Normal",
      pk: validatorPk,
      proof: validatorProof,
    },
  ],
}, { signal: writeController.signal });
await torii.governancePersistCouncil({
  committeeSize: derived.members.length,
  candidates: derived.members.map((member) => ({
    accountId: member.account_id,
    variant: "Normal",
    pk: validatorPk,
    proof: validatorProof,
  })),
  authority,
  privateKey,
}, { signal: writeController.signal });

const finalizeDraft = await torii.governanceFinalizeReferendumTyped({
  referendumId: "ref-mainnet-001",
  proposalId: "0123abcd...beef",
}, { signal: writeController.signal });
console.log("finalize tx count", finalizeDraft.tx_instructions.length);

const enactDraft = await torii.governanceEnactProposalTyped({
  proposalId: "abcd0123...cafe",
  window: { lower: 10, upper: 25 },
}, { signal: writeController.signal });
console.log("enact tx count", enactDraft.tx_instructions.length);
```

## ISO 20022 ブリッジ レシピ

### pacs.008 / pacs.009 ペイロードをビルドする

```ts
import { buildPacs008Message } from "@iroha/iroha-js";

const settlement = buildPacs008Message({
  messageId: "iso-demo-1",
  instructionId: "instr-1",
  settlementDate: "2026-02-10",
  amount: { currency: "EUR", value: "25.00" },
  instigatingAgent: { bic: "DEUTDEFF", lei: "529900ODI3047E2LIV03" },
  instructedAgent: { bic: "COBADEFF" },
  debtorAccount: { iban: "DE89370400440532013000" },
  creditorAccount: { otherId: "i105..." },
  purposeCode: "SECU",
  supplementaryData: { account_id: "i105...", leg: "delivery" },
});
```XML が作成される前に、すべての識別子 (BIC、LEI、IBAN、ISO 金額) が検証されます。
生成された。 `buildPacs008Message` を `buildPacs009Message` に交換して PvP を発行します
資金ペイロード。

### ISO メッセージの送信とポーリング

```ts
import { ToriiClient } from "@iroha/iroha-js";

const torii = new ToriiClient(process.env.TORII_URL ?? "https://torii.nexus.example");

const status = await torii.submitIsoPacs008AndWait(settlement, {
  wait: {
    maxAttempts: Number(process.env.ISO_POLL_ATTEMPTS ?? 20),
    pollIntervalMs: Number(process.env.ISO_POLL_INTERVAL_MS ?? 3_000),
    resolveOnAccepted: process.env.ISO_RESOLVE_ON_ACCEPTED === "1",
    onPoll: ({ attempt, status: snapshot }) => {
      console.log(`[attempt ${attempt}] status=${snapshot?.status ?? "pending"}`);
    },
  },
});
console.log(status.message_id, status.status, status.transaction_hash);

await torii.waitForIsoMessageStatus(process.env.ISO_MESSAGE_ID!, {
  maxAttempts: 10,
  pollIntervalMs: 2_000,
});

// Build XML on the fly from structured fields (skips the sample payloads).
await torii.submitIsoMessage(
  {
    instructionId: "pvpfund-1",
    amount: { currency: "USD", value: "1250.50" },
    instigatingAgent: { bic: "BOFAUS3N" },
    instructedAgent: { bic: "DEUTDEFF" },
  },
  {
    kind: "pacs.009",
    wait: { maxAttempts: 5, pollIntervalMs: 1_500 },
  },
);
```

`resolveOnAccepted` と `resolveOnAcceptedWithoutTransaction` は両方とも有効です。どちらかのフラグを使用してください
ポーリングを調整するときに、`Accepted` ステータス (トランザクション ハッシュなし) をターミナルとして扱います。

ブリッジが報告しない場合、ヘルパーは `IsoMessageTimeoutError` をスローします。
末期状態。下位レベルの `submitIsoPacs008` / `submitIsoPacs009` を使用します。
カスタム ポーリング ロジックを調整する必要がある場合に呼び出します。 `getIsoMessageStatus`
シングルショット ルックアップを公開します。

### 関連するサーフェス

- `torii.getSorafsPorWeeklyReport("2026-W05")` は ISO 週の PoR バンドルを取得します
  ロードマップで参照され、アラートの待機ヘルパーを再利用できます。
- `resolveAlias` / `resolveAliasByIndex` は ISO ブリッジ エイリアス バインディングを公開するため、
  調整ツールは、支払いを行う前にアカウントの所有権を証明できます。