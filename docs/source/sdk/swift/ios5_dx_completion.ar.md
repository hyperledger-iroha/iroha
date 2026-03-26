---
lang: ar
direction: rtl
source: docs/source/sdk/swift/ios5_dx_completion.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 2cee724d4fdec9fc576d8c6e484ed57269c381ec49de03a68d952c62dd575525
source_last_modified: "2026-01-22T06:58:49.724263+00:00"
translation_last_reviewed: 2026-01-30
---

# IOS5 Developer Experience Completion

IOS5 closes the Swift developer experience track with higher-level client adapters
and repeatable smoke gates.

- **Combine/async publishers:** `ToriiClient+Combine` adds `assetsPublisher` for
  one-shot balance fetches and `verifyingKeyEventsPublisher` to surface the
  verifying-key SSE feed with cancellation-aware bridging. The helpers share the
  `makeValuePublisher`/`makeStreamPublisher` plumbing so callers can subscribe on
  their preferred queue while keeping Torii errors typed.
- **Coverage:** New unit tests cover both the value and SSE publishers, reusing
  the Torii stubs to assert headers, payloads, and stream completion paths.
- **Smoke gates:** The IOS5 sample app runner remains the CI guardrail; the JSON,
  JUnit, and Prometheus outputs documented in `swift_sample_smoke_tests.md` feed
  dashboards and alerts so drift in the quickstarts is caught automatically.
- **Usage example:**

  ```swift
  var cancellables: Set<AnyCancellable> = []
  let client = ToriiClient(baseURL: URL(string: "https://torii.dev")!)

  client.assetsPublisher(accountId: "<katakana-i105-account-id>")
      .sink(receiveCompletion: { completion in
          print("Finished: \(completion)")
      }, receiveValue: { balances in
          print("Balances:", balances)
      })
      .store(in: &cancellables)
  ```
