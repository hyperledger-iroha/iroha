---
lang: ja
direction: ltr
source: docs/source/sdk/swift/ios5_dx_completion.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 2cee724d4fdec9fc576d8c6e484ed57269c381ec49de03a68d952c62dd575525
source_last_modified: "2026-01-22T06:58:49.724263+00:00"
translation_last_reviewed: 2026-01-22
---

<!-- 日本語訳: docs/source/sdk/swift/ios5_dx_completion.md -->

# IOS5 開発者体験の完了

IOS5 は、より高レベルなクライアントアダプターと再現可能な smoke ゲートにより、Swift 開発者体験のトラックを完了させる。

- **Combine/async publishers:** `ToriiClient+Combine` は、一度きりの残高取得向け `assetsPublisher` と、キャンセル対応のブリッジで検証鍵 SSE フィードを表出する `verifyingKeyEventsPublisher` を追加する。ヘルパーは `makeValuePublisher`/`makeStreamPublisher` の基盤を共有し、呼び出し元が好みのキューで購読しても Torii エラーの型付けを維持できる。
- **Coverage:** 新しいユニットテストは value と SSE の両方の publisher をカバーし、Torii のスタブを再利用してヘッダー、ペイロード、ストリーム完了パスを検証する。
- **Smoke gates:** IOS5 サンプルアプリのランナーは CI のガードレールとして残り、`swift_sample_smoke_tests.md` に記載された JSON/JUnit/Prometheus の出力がダッシュボードとアラートに流れることで、quickstart のドリフトを自動検出する。
- **使用例:**

  ```swift
  var cancellables: Set<AnyCancellable> = []
  let client = ToriiClient(baseURL: URL(string: "https://torii.dev")!)

  client.assetsPublisher(accountId: "ih58...")
      .sink(receiveCompletion: { completion in
          print("Finished: \(completion)")
      }, receiveValue: { balances in
          print("Balances:", balances)
      })
      .store(in: &cancellables)
  ```
