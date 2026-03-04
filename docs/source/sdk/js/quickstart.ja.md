<!-- Japanese translation of docs/source/sdk/js/quickstart.md (Connect section) -->

---
lang: ja
direction: ltr
source: docs/source/sdk/js/quickstart.md
status: draft
translator: LLM (Codex)
translation_last_reviewed: 2026-05-02
---

# JS/TS クイックスタート（抜粋）

Connect セッションとキューイング節のみ翻訳済みです。他の章は `docs/source/sdk/js/quickstart.md` を参照してください。

## Connect セッションとキューイング

Connect ヘルパーは `docs/source/connect_architecture_strawman.md` に記載されているストローマンをそのまま反映しています。プレビュー向けのセッションを最短で用意するには `bootstrapConnectPreviewSession` を呼び出し、決定論的な SID/URI 生成と Torii の登録 API を一括で配線します。

```js
import {
  ToriiClient,
  bootstrapConnectPreviewSession,
  ConnectQueueError,
} from "@iroha/iroha-js";

const torii = new ToriiClient("https://torii.nexus.example");
const { preview, session, tokens } = await bootstrapConnectPreviewSession(torii, {
  chainId: "sora-mainnet",
  node: "https://torii.nexus.example",
  // 任意: 登録に使用する Torii ノードを上書きする
  sessionOptions: { node: "https://torii.backup.example" },
});

console.log("wallet QR", preview.walletUri);
console.log("Connect tokens", tokens?.wallet, tokens?.app);
```

- `bootstrapConnectPreviewSession` は既定で Torii へセッションを登録します。QR/ディープリンクのプレビュー用に SID/URI だけが欲しい場合は `register: false` を指定します。
- 内部的にこのヘルパーは `createConnectSessionPreview` と `ToriiClient.createConnectSession` を順番に呼び出すため、リトライやストレージ処理を独自に挟みたい場合でも多段フローを構成できます。
- 決定論的なセッション ID だけが必要で URI を発行したくないケース（CI ハーネスで SID を保存する等）では `generateConnectSid` をそのまま利用できます。
- 方向性付き鍵と暗号文エンベロープはネイティブブリッジが生成します。ブリッジが利用できない場合、SDK は JSON コーデックにフォールバックし `ConnectError.bridgeUnavailable` を送出します。
- オフラインバッファは IndexedDB 上の Norito `.to` ブロブとして保持されます。`ConnectQueueError.overflow(limit)` や `.expired(ttlMs)` 例外でキュー状態を監視し、テレメトリにはロードマップに沿って `connect.queue_depth`、`connect.replay_success_total`、`connect.resume_latency_ms` を流してください。
