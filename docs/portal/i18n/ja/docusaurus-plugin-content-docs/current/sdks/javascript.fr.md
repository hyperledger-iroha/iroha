---
lang: ja
direction: ltr
source: docs/portal/docs/sdks/javascript.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
タイトル: JavaScript SDK クイックスタート
説明: トランザクションを構築し、イベントをストリーミングし、`@iroha/iroha-js` でプレビューを接続します。
スラグ: /sdks/javascript
---

`@iroha/iroha-js` は、Torii と対話するための正規の Node.js パッケージです。それ
Norito ビルダー、Ed25519 ヘルパー、ページネーション ユーティリティ、および回復力のある
HTTP/WebSocket クライアントを使用すると、TypeScript から CLI フローをミラーリングできます。

## インストール

```bash
npm install @iroha/iroha-js
# Required once after install so the native bindings are compiled
npm run build:native
```

ビルド ステップは `cargo build -p iroha_js_host` をラップします。ツールチェーンが次からのものであることを確認します。
`rust-toolchain.toml` は、`npm run build:native` を実行する前にローカルで使用可能です。

## キー管理

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

## トランザクションを構築する

Norito 命令ビルダーは、識別子、メタデータ、数量を正規化します。
エンコードされたトランザクションは Rust/CLI ペイロードと一致します。

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
  destinationAccountId: "ih58...",
  quantity: "5",
});

const { signedTransaction } = buildMintAndTransferTransaction({
  chainId: "test-chain",
  authority: "ih58...",
  mint: { assetId: "norito:4e52543000000001", quantity: "10" },
  transfers: [{ destinationAccountId: "ih58...", quantity: "5" }],
  privateKey: Buffer.alloc(32, 0x42),
});
```

## Torii クライアント構成

`ToriiClient` は、`iroha_config` を反映する再試行/タイムアウト ノブを受け入れます。使用する
`resolveToriiClientConfig` は、キャメルケース構成オブジェクトをマージします (正規化します)
`iroha_config` 最初)、env オーバーライド、およびインライン オプション。

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

ローカル開発用の環境変数:

|変数 |目的 |
|----------|----------|
| `IROHA_TORII_TIMEOUT_MS` |リクエストのタイムアウト (ミリ秒)。 |
| `IROHA_TORII_MAX_RETRIES` |最大再試行回数。 |
| `IROHA_TORII_BACKOFF_INITIAL_MS` |初期再試行バックオフ。 |
| `IROHA_TORII_BACKOFF_MULTIPLIER` |指数バックオフ乗数。 |
| `IROHA_TORII_MAX_BACKOFF_MS` |最大再試行遅延。 |
| `IROHA_TORII_RETRY_STATUSES` |再試行するためのカンマ区切りの HTTP ステータス コード。 |
| `IROHA_TORII_RETRY_METHODS` |再試行するカンマ区切りの HTTP メソッド。 |
| `IROHA_TORII_API_TOKEN` | `X-API-Token` を追加します。 |
| `IROHA_TORII_AUTH_TOKEN` | `Authorization: Bearer …` ヘッダーを追加します。 |

再試行プロファイルは Android のデフォルトを反映し、パリティ チェックのためにエクスポートされます。
`DEFAULT_TORII_CLIENT_CONFIG`、`DEFAULT_RETRY_PROFILE_PIPELINE`、
`DEFAULT_RETRY_PROFILE_STREAMING`。 `docs/source/sdk/js/torii_retry_policy.md`を参照
エンドポイントからプロファイルへのマッピングとパラメーター ガバナンス監査については、
JS4/JS7。

## 反復可能なリストとページネーション

ページネーション ヘルパーは、`/v1/accounts` の Python SDK 人間工学を反映しています。
`/v1/domains`、`/v1/assets/definitions`、NFT、残高、資産保有者、および
アカウントの取引履歴。

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

## オフライン許可と判定メタデータ

オフライン手当の応答により、強化された台帳メタデータが事前に公開されます。
`expires_at_ms`、`policy_expires_at_ms`、`refresh_at_ms`、`verdict_id_hex`、
`attestation_nonce_hex` および `remaining_amount` が生のファイルと一緒に返されます。
記録することで、ダッシュボードが埋め込み Norito ペイロードをデコードする必要がなくなります。新しい
カウントダウン ヘルパー (`deadline_kind`、`deadline_state`、`deadline_ms`、
`deadline_ms_remaining`) 次に期限切れになる期限を強調表示します (更新 → ポリシー)
→ 証明書) を使用して、許容量が不足するたびに UI バッジがオペレーターに警告できるようにします。
残り 24 時間未満。 SDK
`/v1/offline/allowances` によって公開される REST フィルターをミラーリングします。
`certificateExpiresBeforeMs/AfterMs`、`policyExpiresBeforeMs/AfterMs`、
`verdictIdHex`、`attestationNonceHex`、`refreshBeforeMs/AfterMs`、および
`requireVerdict` / `onlyMissingVerdict` ブール値。無効な組み合わせ (
例 `onlyMissingVerdict` + `verdictIdHex`) は、Torii より前にローカルで拒否されます。
と呼ばれます。

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

## オフラインでのトップアップ (発行 + 登録)

証明書をすぐに発行したい場合は、トップアップ ヘルパーを使用します。
それを台帳に登録します。 SDKは発行および登録された証明書を検証します
ID は返される前に一致し、応答には両方のペイロードが含まれます。あります
専用のトップアップエンドポイントはありません。ヘルパーは問題と登録呼び出しを連鎖させます。もし
すでに署名付き証明書をお持ちの場合は、`registerOfflineAllowance` (または
`renewOfflineAllowance`) を直接使用します。

```ts
const topUp = await torii.topUpOfflineAllowance({
  authority: "<account_ih58>",
  privateKeyHex: alicePrivateKey,
  certificate: draftCertificate,
});
console.log(topUp.certificate.certificate_id_hex);
console.log(topUp.registration.certificate_id_hex);

const renewed = await torii.topUpOfflineAllowanceRenewal(
  topUp.registration.certificate_id_hex,
  {
    authority: "<account_ih58>",
    privateKeyHex: alicePrivateKey,
    certificate: draftCertificate,
  },
);
console.log(renewed.registration.certificate_id_hex);
```

## Torii クエリとストリーミング (WebSocket)

クエリ ヘルパーはステータス、Prometheus メトリクス、テレメトリ スナップショット、およびイベントを公開します
Norito フィルター文法を使用したストリーム。ストリーミングは自動的に次のようにアップグレードされます
WebSocket を実行し、再試行予算が許す限り再開します。

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

その他には `streamBlocks`、`streamTransactions`、または `streamTelemetry` を使用します。
WebSocket エンドポイント。すべてのストリーミング ヘルパーは再試行を表示するため、
ダッシュボードとアラートをフィードするための `onReconnect` コールバック。

## Explorer のスナップショットと QR ペイロード

Explorer テレメトリは、`/v1/explorer/metrics` および
`/v1/explorer/accounts/{account_id}/qr` エンドポイント。ダッシュボードで
ポータルを強化するのと同じスナップショット。 `getExplorerMetrics()` は、
ペイロードをロードし、ルートが無効な場合は `null` を返します。と組み合わせてください
IH58 (推奨)/sora (2 番目に優れた) リテラルとインラインが必要な場合は常に `getExplorerAccountQr()`
共有ボタンの SVG。

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

const qr = await torii.getExplorerAccountQr("ih58...", {
  addressFormat: "compressed",
});
console.log("explorer literal", qr.literal);
await fs.writeFile("alice.svg", qr.svg, "utf8");
console.log(
  `qr metadata v${qr.qrVersion} ec=${qr.errorCorrection} prefix=${qr.networkPrefix}`,
);
````addressFormat: "compressed"` を渡すと、エクスプローラーのデフォルトの圧縮ファイルが反映されます。
セレクター;優先 IH58 出力のオーバーライドを省略するか、`ih58_qr` を要求します
QRセーフバリアントが必要な場合。圧縮リテラルは 2 番目に優れたものです
Sora 専用の UX オプション。ヘルパーは常に正規の識別子を返します。
選択したリテラル、およびメタデータ (ネットワーク プレフィックス、QR バージョン/モジュール、エラー)
修正層、およびインライン SVG) を使用できるため、CI/CD は、
Explorer は、特注のコンバーターを呼び出すことなく表示されます。

## 接続セッションとキューイング

Connect ヘルパーは `docs/source/connect_architecture_strawman.md` をミラーリングします。の
プレビュー対応セッションへの最速パスは `bootstrapConnectPreviewSession` です。
確定的な SID/URI 生成と Torii を結合します。
登録電話。

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

- QR/ディープリンクの決定論的な URI のみが必要な場合は、`register: false` を渡します。
  プレビュー。
- セッション ID を取得する必要がある場合は、`generateConnectSid` を引き続き使用できます
  URI を鋳造せずに。
- 方向キーと暗号文エンベロープはネイティブ ブリッジから取得されます。いつ
  利用できない場合、SDK は JSON コーデックにフォールバックしてスローします
  `ConnectQueueError.bridgeUnavailable`。
- オフライン バッファーは、Norito `.to` BLOB として IndexedDB に保存されます。監視キュー
  発行された `ConnectQueueError.overflow(limit)` / による状態
  `.expired(ttlMs)` エラーとフィード `connect.queue_depth` テレメトリ (概要を参照)
  ロードマップでは。

### レジストリとポリシーのスナップショットを接続する

プラットフォーム オペレーターは、何もせずに Connect レジストリをイントロスペクトし、更新できます。
Node.jsを離れる。 `iterateConnectApps()` はレジストリをページングしますが、
`getConnectStatus()` および `getConnectAppPolicy()` はランタイム カウンターを公開し、
現在の保険契約の封筒。 `updateConnectAppPolicy()` はキャメルケースのフィールドを受け入れます。
したがって、Torii が期待するのと同じ JSON ペイロードをステージングできます。

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

適用する前に、必ず最新の `getConnectStatus()` スナップショットをキャプチャしてください。
突然変異 - ガバナンス チェックリストには、ポリシーの更新が開始されたという証拠が必要です
フリートの現在の制限から。

### WebSocket ダイヤルに接続する

`ToriiClient.openConnectWebSocket()` は正規をアセンブルします
`/v1/connect/ws` URL (`sid`、`role`、およびトークンパラメータを含む)、アップグレード
`http→ws` / `https→wss`、最終 URL をいずれかの WebSocket に渡します
あなたが提供する実装。ブラウザは自動的にグローバルを再利用します。
`WebSocket`。 Node.js の呼び出し元は、`ws` のようなコンストラクターを渡す必要があります。

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

URL のみが必要な場合は、`torii.buildConnectWebSocketUrl(params)` または
トップレベルの `buildConnectWebSocketUrl(baseUrl, params)` ヘルパーを再利用します。
カスタムトランスポート/キュー内の結果の文字列。

完全な CLI 指向のサンプルをお探しですか?の
[接続プレビュー レシピ](./recipes/javascript-connect-preview.md) には、
実行可能なスクリプトと、ロードマップの成果物を反映するテレメトリ ガイダンス
Connect キュー + WebSocket フローを文書化します。

### キューのテレメトリとアラート

キュー メトリクスをヘルパー サーフェスに直接配線して、ダッシュボードにミラーリングできるようにします。
ロードマップの KPI。

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

`ConnectQueueError#toConnectError()` はキューの失敗を汎用エラーに変換します。
`ConnectError` 分類法により、共有 HTTP/WebSocket インターセプターは
標準 `connect.queue_depth`、`connect.queue_overflow_total`、および
`connect.queue_expired_total` メトリクスはロードマップ全体で参照されます。

## ストリーミング ウォッチャーとイベント カーソル

`ToriiClient.streamEvents()` は、自動を備えた非同期イテレータとして `/v1/events/sse` を公開します。
再試行するため、Node/Bun CLI は Rust CLI と同じ方法でパイプライン アクティビティを追跡できます。
`Last-Event-ID` カーソルを Runbook アーティファクトの横に保持して、オペレーターが
プロセスの再起動時にイベントをスキップせずにストリームを再開します。

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

- `PIPELINE_STATUS` (たとえば、`Pending`、`Applied`、または `Approved`) を切り替えるか、設定します
  `STREAM_FILTER_JSON` は、CLI が受け入れるのと同じフィルターを再生します。
- `STREAM_MAX_EVENTS=0 node ./recipes/streaming.mjs` は、終了するまで反復子を存続させます。
  信号が受信される。最初のいくつかのイベントだけが必要な場合は、`STREAM_MAX_EVENTS=25` を渡します
  煙のテストのため。
- `ToriiClient.streamSumeragiStatus()` は、同じインターフェイスをミラーリングします。
  `/v1/sumeragi/status/sse` なので、コンセンサス テレメトリは個別に追跡できます。
  イテレータは同じように `Last-Event-ID` を尊重します。
- ターンキー CLI (カーソル永続性、
  env-var フィルターのオーバーライド、および `extractPipelineStatusKind` ロギング) が JS4 で使用されます。
  ストリーミング/WebSocket ロードマップの成果物。

## UAID ポートフォリオと宇宙ディレクトリ

Space Directory API は、ユニバーサル アカウント ID (UAID) のライフサイクルを明らかにします。の
ヘルパーは `uaid:<hex>` リテラルまたは生の 64 16 進ダイジェスト (LSB=1) を受け入れます。
リクエストを送信する前にそれらを正規化します。- `getUaidPortfolio(uaid, { assetId })` はデータスペースごとの残高を集計します。
  正規のアカウント ID ごとに資産保有をグループ化します。 `assetId` を渡してフィルタリングします。
  ポートフォリオを単一の資産インスタンスにまで落とし込みます。
- `getUaidBindings(uaid, { addressFormat })` はすべてのデータスペース ↔ アカウントを列挙します
  バインディング (`addressFormat: "compressed"` は `sora…` リテラルを返します)。
- `getUaidManifests(uaid, { dataspaceId })` は各機能マニフェストを返します。
  ライフサイクル ステータス、および監査用にバインドされたアカウント。

オペレーター証拠パック、マニフェスト発行/取り消しフロー、および SDK 移行の場合
ガイダンスに従って、ユニバーサル アカウント ガイド (`docs/source/universal_accounts_guide.md`) に従ってください。
これらのクライアント ヘルパーと並行して、ポータルとソース ドキュメントの同期を維持します。

```ts
import { promises as fs } from "node:fs";

const uaid = "uaid:0f4d86b20839a8ddbe8a1a3d21cf1c502d49f3f79f0fa1cd88d5f24c56c0ab11";

const portfolio = await torii.getUaidPortfolio(uaid, {
  assetId: "norito:4e52543000000002",
});
portfolio.dataspaces.forEach((entry) => {
  console.log(entry.dataspace_alias ?? entry.dataspace_id, entry.accounts.length);
});

const bindings = await torii.getUaidBindings(uaid, { addressFormat: "compressed" });
console.log("bindings", bindings.dataspaces);

const manifests = await torii.getUaidManifests(uaid, { dataspaceId: 11 });
console.log("manifests", manifests.manifests[0].manifest.entries.length);
```

オペレーターは、マニフェストをローテーションしたり、緊急拒否フローを実行したりすることもできます。
CLI にドロップします。どちらのヘルパーもオプションの `{ signal }` オブジェクトを受け入れるため、
長時間実行される送信は `AbortController` でキャンセルできます。非オブジェクト
オプションまたは非 `AbortSignal` 入力は、
リクエストは Torii にヒットします:

```ts
import { promises as fs } from "node:fs";
import { Buffer } from "node:buffer";

const manifest = JSON.parse(
  await fs.readFile("fixtures/space_directory/capability/cbdc.manifest.json", "utf8"),
);

const controller = new AbortController();

await torii.publishSpaceDirectoryManifest(
  {
    authority: "ih58...",
    manifest,
    privateKeyHex: process.env.SPACE_DIRECTORY_KEY_HEX,
    reason: "Attester v2 rollout",
  },
  { signal: controller.signal },
);

await torii.revokeSpaceDirectoryManifest(
  {
    authority: "ih58...",
    privateKey: Buffer.from(process.env.SPACE_DIRECTORY_KEY_SEED, "hex"),
    uaid,
    dataspaceId: 11,
    revokedEpoch: 9216,
    reason: "Emergency deny-wins",
  },
  { signal: controller.signal },
);
```

`publishSpaceDirectoryManifest()` は、生のマニフェスト JSON (
`fixtures/space_directory/` の下のフィクスチャ)、または
同じ構造。 `privateKey`、`privateKeyHex`、または `privateKeyMultihash` へのマップ
`ExposedPrivateKey` フィールド Torii が期待されており、デフォルトは `ed25519` です
プレフィックスが指定されていない場合のアルゴリズム。 Torii がキューに登録されると、両方のリクエストが返されます。
命令 (`202 Accepted`)。この時点で台帳は
`SpaceDirectoryEvent` と一致します。

## ガバナンスと ISO のブリッジ

`ToriiClient` は、契約の検査、ステージングのためのガバナンス API を公開します。
提案、投票用紙（普通投票または ZK）の提出、評議会の輪番化、および招集
`governanceFinalizeReferendumTyped` /
手書きの DTO なしの `governanceEnactProposalTyped`。 ISO 20022 ヘルパー
`buildPacs008Message`/`buildPacs009Message` 経由で同じパターンに従います。
`submitIso*`/`waitForIsoMessageStatus` トリオ。

[ガバナンスと ISO ブリッジのレシピ](./recipes/javascript-governance-iso.md) を参照してください。
CLI 対応のサンプルと、完全なフィールド ガイドへのポインタについては、
`docs/source/sdk/js/governance_iso_examples.md`。

## 赤血球のサンプリングと納品の証拠

JS ロードマップでは、オペレーターが次のことができるように、Roadrunner Block Commitment (RBC) サンプリングも必要です
Sumeragi を通じてフェッチしたブロックが、検証したチャンクプルーフと一致することを証明します。
ペイロードを手動で構築する代わりに、組み込みヘルパーを使用します。

1. `getSumeragiRbcSessions()` は `/v1/sumeragi/rbc/sessions` をミラーリングします。
   `findRbcSamplingCandidate()` は、ブロック ハッシュを使用して最初に配信されたセッションを自動選択します
   (統合スイートは常にそれにフォールバックします)
   `IROHA_TORII_INTEGRATION_RBC_SAMPLE` は設定されていません)。
2. `ToriiClient.buildRbcSampleRequest(session, overrides)` は `{blockHash,height,view}` を正規化します
   さらに、オプションの `{count,seed,apiToken}` がオーバーライドされるため、不正な 16 進数や負の整数が入力されることはありません
   Torii に達します。
3. `sampleRbcChunks()` はリクエストを `/v1/sumeragi/rbc/sample` に POST し、チャンクプルーフを返します。
   およびマークル パス (`samples[].chunkHex`、`chunkRoot`、`payloadHash`) を使用してアーカイブする必要があります。
   養子縁組の証拠の残りの部分。
4. `getSumeragiRbcDelivered(height, view)` はコホートの配信メタデータをキャプチャします。
   証明をエンドツーエンドで再生できます。

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

両方の応答をガバナンスに送信するアーティファクト ルートの下に保持します。オーバーライド
`RBC_SAMPLE_JSON='{"height":123,"view":4,"blockHash":"0x…"}'` 経由の自動選択セッション
特定のブロックをプローブし、RBC スナップショットのフェッチの失敗を問題として扱う必要がある場合は常に
サイレントにダイレクト モードにダウングレードするのではなく、プリフライト ゲーティング エラーを修正します。

## テストと CI

1. カーゴと npm アーティファクトをキャッシュします。
2. `npm run build:native` を実行します。
3. `npm test` (またはスモーク ジョブの場合は `node --test`) を実行します。

参照 GitHub Actions ワークフローは次の場所にあります。
`docs/source/examples/iroha_js_ci.md`。

## 次のステップ

- `javascript/iroha_js/index.d.ts` で生成された型を確認します。
- `javascript/iroha_js/recipes/` の下のレシピを調べます。
- `ToriiClient` を Norito クイックスタートと組み合わせて、ペイロードを同時に検査します
  SDK 呼び出し。