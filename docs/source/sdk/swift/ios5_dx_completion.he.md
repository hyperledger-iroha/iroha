---
lang: he
direction: rtl
source: docs/source/sdk/swift/ios5_dx_completion.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 2cee724d4fdec9fc576d8c6e484ed57269c381ec49de03a68d952c62dd575525
source_last_modified: "2026-01-22T06:58:49.724263+00:00"
translation_last_reviewed: 2026-01-22
---

<div dir="rtl">

<!-- תרגום עברי ל-docs/source/sdk/swift/ios5_dx_completion.md -->

# השלמת חוויית מפתחים IOS5

IOS5 מסיים את מסלול חוויית המפתחים של Swift עם מתאמי לקוח ברמה גבוהה ושערי smoke חוזרים.

- **מפרסמי Combine/async:** `ToriiClient+Combine` מוסיף `assetsPublisher` לשליפת יתרות חד־פעמית ו‑`verifyingKeyEventsPublisher` להצגת זרם ה‑SSE של מפתח האימות עם גישור מודע לביטול. העזרים חולקים את התשתית `makeValuePublisher`/`makeStreamPublisher` כך שהקוראים יכולים להירשם בתור המועדף עליהם תוך שמירה על שגיאות Torii מטוּפָסות.
- **כיסוי:** בדיקות יחידה חדשות מכסות גם את מפרסמי הערך וגם את מפרסמי ה‑SSE, תוך שימוש חוזר ב‑stubs של Torii כדי לאשר כותרות, מטענים ונתיבי סיום זרם.
- **שערי smoke:** מריץ אפליקציית הדוגמה IOS5 נשאר חומת המגן של ה‑CI; פלטי JSON, JUnit ו‑Prometheus המתועדים ב‑`swift_sample_smoke_tests.md` מזינים דשבורדים והתראות כך שסטייה ב‑quickstarts נתפסת אוטומטית.
- **דוגמת שימוש:**

  ```swift
  var cancellables: Set<AnyCancellable> = []
  let client = ToriiClient(baseURL: URL(string: "https://torii.dev")!)

  client.assetsPublisher(accountId: "soraカタカナ...")
      .sink(receiveCompletion: { completion in
          print("Finished: \(completion)")
      }, receiveValue: { balances in
          print("Balances:", balances)
      })
      .store(in: &cancellables)
  ```

</div>
