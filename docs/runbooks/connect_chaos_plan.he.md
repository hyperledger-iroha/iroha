---
lang: he
direction: rtl
source: docs/runbooks/connect_chaos_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: b1d414173d2f43d403a6a1ba5cd59a645cb0b94f5765e69a00f7078b1e96b1cd
source_last_modified: "2025-11-18T04:13:57.609769+00:00"
translation_last_reviewed: 2026-01-01
---

<div dir="rtl">

<!-- תרגום עברי ל-docs/runbooks/connect_chaos_plan.md -->

# תוכנית חזרות כאוס ותקלות ל-Connect (IOS3 / IOS7)

פלייבוק זה מגדיר את תרגילי הכאוס החוזרים שממלאים את משימת ה-roadmap _"plan joint chaos rehearsal"_ (`roadmap.md:1527`). שלבו אותו עם רנבוק ה-preview של Connect (`docs/runbooks/connect_session_preview_runbook.md`) בעת הדגמות cross-SDK.

## מטרות וקריטריוני הצלחה
- להפעיל את מדיניות retry/back-off המשותפת של Connect, מגבלות תור אופליין ומייצאי טלמטריה תחת תקלות מבוקרות בלי לשנות קוד פרודקשן.
- ללכוד ארטיפקטים דטרמיניסטיים (פלט `iroha connect queue inspect`, snapshots של `connect.*`, לוגים של SDK Swift/Android/JS) כדי שה-governance יוכל לבקר כל drill.
- להוכיח ש-wallets ו-dApps מכבדים שינויי קונפיג (manifest drift, רוטציית salt, כשלי attestation) באמצעות קטגוריית `ConnectError` הקנונית ואירועי טלמטריה בטוחים לרדקציה.

## דרישות מקדימות
1. **Bootstrap לסביבה**
   - הפעילו את stack הדמו של Torii: `scripts/ios_demo/start.sh --telemetry-profile full`.
   - הפעילו לפחות sample אחד של SDK (`examples/ios/NoritoDemoXcode/NoritoDemoXcode`,
     `examples/ios/NoritoDemo`, Android `demo-connect`, JS `examples/connect`).
2. **אינסטרומנטציה**
   - הפעילו אבחוני SDK (`ConnectQueueDiagnostics`, `ConnectQueueStateTracker`,
     `ConnectSessionDiagnostics` ב-Swift; שקולים `ConnectQueueJournal` + `ConnectQueueJournalTests`
     ב-Android/JS).
   - ודאו שה-CLI `iroha connect queue inspect --sid <sid> --metrics` פותר
     את נתיב התור שמייצר ה-SDK (`~/.iroha/connect/<sid>/state.json` ו-
     `metrics.ndjson`).
   - חברו מייצאי טלמטריה כך שסדרות הזמן הבאות יהיו זמינות ב-Grafana
     ובאמצעות `scripts/swift_status_export.py telemetry`: `connect.queue_depth`,
     `connect.queue_dropped_total`, `connect.reconnects_total`,
     `connect.resume_latency_ms`, `swift.connect.frame_latency`,
     `android.telemetry.redaction.salt_version`.
3. **תיקיות ראיות** - צרו `artifacts/connect-chaos/<date>/` ושמרו:
   - לוגים גולמיים (`*.log`), snapshots של מטריקות (`*.json`), exports של דשבורד
     (`*.png`), פלטי CLI ו-PagerDuty IDs.

## מטריצת תרחישים

| ID | תקלה | שלבי הזרקה | סימנים צפויים | ראיות |
|----|------|------------|---------------|-------|
| C1 | ניתוק WebSocket וחיבור מחדש | עטפו את `/v2/connect/ws` מאחורי proxy (למשל `kubectl -n demo port-forward svc/torii 18080:8080` + `toxiproxy-cli toxic add ... timeout`) או חסמו זמנית את השירות (`kubectl scale deploy/torii --replicas=0` למשך <=60 s). הכריחו את הארנק להמשיך לשלוח frames כדי למלא תורי אופליין. | `connect.reconnects_total` גדל, `connect.resume_latency_ms` קופץ אך נשאר <1 s p95, והתורים נכנסים ל-`state=Draining` דרך `ConnectQueueStateTracker`. ה-SDKs פולטים `ConnectError.Transport.reconnecting` פעם אחת ואז ממשיכים. | - פלט `iroha connect queue inspect --sid <sid>` שמראה `resume_attempts_total` לא אפס.<br>- אנוטציית דשבורד לחלון ה-outage.<br>- קטע לוג עם reconnect + drain. |
| C2 | Overflow בתור אופליין / פקיעת TTL | תקנו את ה-sample כדי לצמצם גבולות תור (Swift: יצירת `ConnectQueueJournal.Configuration(maxRecordsPerQueue: 4, maxBytesPerQueue: 4096, retentionInterval: 30)` בתוך `ConnectSessionDiagnostics`; Android/JS משתמשים בבנאים שקולים). השעו את הארנק למשך >=2x `retentionInterval` בזמן שה-dApp ממשיך להכניס בקשות לתור. | `connect.queue_dropped_total{reason="overflow"}` ו-`{reason="ttl"}` עולים, `connect.queue_depth` נתקע בגבול החדש, ה-SDKs מציגים `ConnectError.QueueOverflow(limit: 4)` (או `.QueueExpired`). `iroha connect queue inspect` מציג `state=Overflow` עם סימוני `warn/drop` ב-100%. | - צילום מונים של מטריקות.<br>- JSON מה-CLI עם overflow.<br>- לוג Swift/Android עם שורת `ConnectError`. |
| C3 | Manifest drift / דחיית admission | שנו את manifest של Connect שנשלח לארנקים (למשל שינוי sample ב-`docs/connect_swift_ios.md`, או הפעלת Torii עם `--connect-manifest-path` שמצביע לעותק שבו `chain_id` או `permissions` שונים). בקשו אישור מה-dApp וודאו שהארנק דוחה לפי המדיניות. | Torii מחזיר `HTTP 409` עבור `/v2/connect/session` עם `manifest_mismatch`, ה-SDKs פולטים `ConnectError.Authorization.manifestMismatch(manifestVersion)`, הטלמטריה מעלה `connect.manifest_mismatch_total`, והתורים נשארים ריקים (`state=Idle`). | - לוג Torii שמראה זיהוי mismatch.<br>- צילום SDK של השגיאה.<br>- Snapshot מטריקות שמוכיח שאין frames בתור בזמן הבדיקה. |
| C4 | רוטציית מפתח / קפיצת גרסת salt | בצעו רוטציה ל-salt או למפתח AEAD של Connect באמצע סשן. בסטאקים dev, אתחלו את Torii עם `CONNECT_SALT_VERSION=$((old+1))` (משקף את מבחן ה-salt של Android ב-`docs/source/sdk/android/telemetry_schema_diff.md`). השאירו את הארנק אופליין עד שהרוטציה מסתיימת ואז חזרו. | ניסיון החידוש הראשון נכשל עם `ConnectError.Authorization.invalidSalt`, התורים מתרוקנים (ה-dApp משליך frames מה-cache עם סיבה `salt_version_mismatch`), הטלמטריה פולטת `android.telemetry.redaction.salt_version` (Android) ו-`swift.connect.session_event{event="salt_rotation"}`. הסשן השני אחרי רענון SID מצליח. | - אנוטציה בדשבורד עם תקופת salt לפני/אחרי.<br>- לוגים עם invalid-salt והצלחה לאחר מכן.<br>- פלט `iroha connect queue inspect` שמראה `state=Stalled` ואז `state=Active`. |
| C5 | כשל attestation / StrongBox | בארנקים של Android הגדירו `ConnectApproval` כך שיכלול `attachments[]` + StrongBox attestation. השתמשו ב-harness של attestation (`scripts/android_keystore_attestation.sh` עם `--inject-failure strongbox-simulated`) או שנו את JSON של attestation לפני העברה ל-dApp. | ה-dApp דוחה את האישור עם `ConnectError.Authorization.invalidAttestation`, Torii מתעד את סיבת הכשל, ה-exporters מעלים `connect.attestation_failed_total`, והתור מסיר את הערך הפגום. dApps Swift/JS מתעדים את השגיאה תוך שמירה על הסשן פעיל. | - לוג harness עם מזהה כשל מוזרק.<br>- לוג שגיאה של SDK + צילום מונה טלמטריה.<br>- הוכחה שהתור הסיר frame רע (`recordsRemoved > 0`). |

## פרטי תרחישים

### C1 - ניתוק WebSocket וחיבור מחדש
1. עטפו את Torii מאחורי proxy (toxiproxy, Envoy או `kubectl port-forward`) כדי
   שתוכלו להחליף זמינות בלי להפיל את כל הצומת.
2. הפעילו ניתוק של 45 s:
   ```bash
   toxiproxy-cli toxic add connect-ws --type timeout --toxicity 1.0 --attribute timeout=45000
   sleep 45 && toxiproxy-cli toxic remove connect-ws --toxic timeout
   ```
3. עקבו אחרי דשבורדי הטלמטריה ו-`scripts/swift_status_export.py telemetry
   --json-out artifacts/connect-chaos/<date>/c1_metrics.json`.
4. צלמו את מצב התור מיד לאחר הניתוק:
   ```bash
   iroha connect queue inspect --sid "$SID" --metrics > artifacts/connect-chaos/<date>/c1_queue.txt
   ```
5. הצלחה = ניסיון reconnect יחיד, צמיחה מוגבלת של התור, ו-drain אוטומטי אחרי שה-proxy חוזר.

### C2 - Overflow בתור אופליין / פקיעת TTL
1. הקטינו ספי תור בבילדים מקומיים:
   - Swift: עדכנו את אתחול `ConnectQueueJournal` בתוך ה-sample
     (לדוגמה `examples/ios/NoritoDemoXcode/NoritoDemoXcode/ContentView.swift`)
     כדי להעביר `ConnectQueueJournal.Configuration(maxRecordsPerQueue: 4, maxBytesPerQueue: 4096, retentionInterval: 30)`.
   - Android/JS: העבירו config שקול בעת יצירת `ConnectQueueJournal`.
2. השעו את הארנק (רקע סימולטור או מצב טיסה) למשך >=60 s
   בזמן שה-dApp שולח קריאות `ConnectClient.requestSignature(...)`.
3. השתמשו ב-`ConnectQueueDiagnostics`/`ConnectQueueStateTracker` (Swift) או helper JS
   כדי לייצא את חבילת הראיות (`state.json`, `journal/*.to`, `metrics.ndjson`).
4. הצלחה = מוני overflow עולים, ה-SDK מציג `ConnectError.QueueOverflow`
   פעם אחת, והתור מתאושש לאחר חזרת הארנק.

### C3 - Manifest drift / דחיית admission
1. צרו עותק של manifest admission, למשל:
   ```bash
   cp configs/connect_manifest.json /tmp/manifest_drift.json
   sed -i '' 's/"chain_id": ".*"/"chain_id": "bogus-chain"/' /tmp/manifest_drift.json
   ```
2. הפעילו Torii עם `--connect-manifest-path /tmp/manifest_drift.json` (או
   עדכנו את קונפיג docker compose/k8s לתרגיל).
3. נסו להתחיל סשן מהארנק; צפו ל-HTTP 409.
4. אספו לוגים של Torii + SDK ואת `connect.manifest_mismatch_total` מהדשבורד.
5. הצלחה = דחייה ללא צמיחת תור, והארנק מציג את שגיאת הטקסונומיה המשותפת
   (`ConnectError.Authorization.manifestMismatch`).

### C4 - רוטציית מפתח / שינוי salt
1. תעדו את גרסת ה-salt הנוכחית מהטלמטריה:
   ```bash
   scripts/swift_status_export.py telemetry --json-out artifacts/connect-chaos/<date>/c4_before.json
   ```
2. אתחלו את Torii עם salt חדש (`CONNECT_SALT_VERSION=$((OLD+1))` או עדכון config map).
   השאירו את הארנק אופליין עד סיום האתחול.
3. החזירו את הארנק; הניסיון הראשון צריך להיכשל עם invalid-salt
   ו-`connect.queue_dropped_total{reason="salt_version_mismatch"}` יעלה.
4. כפו על האפליקציה להשליך frames מה-cache ע"י מחיקת ספריית הסשן
   (`rm -rf ~/.iroha/connect/<sid>` או ניקוי cache לפי פלטפורמה), ואז
   התחילו סשן מחדש עם tokens טריים.
5. הצלחה = הטלמטריה מציגה שינוי salt, אירוע invalid-resume נרשם פעם אחת,
   והסשן הבא מצליח ללא התערבות ידנית.

### C5 - כשל attestation / StrongBox
1. צרו bundle של attestation באמצעות `scripts/android_keystore_attestation.sh`
   (השתמשו ב-`--inject-failure strongbox-simulated` כדי לשבש את החתימה).
2. בקשו מהארנק לצרף את ה-bundle דרך API `ConnectApproval`; ה-dApp
   צריך לאמת ולדחות את ה-payload.
3. בדקו טלמטריה (`connect.attestation_failed_total`, מדדי incident של Swift/Android)
   וודאו שהתור הסיר את הערך המורעל.
4. הצלחה = הדחייה מבודדת לאישור הבעייתי, התורים נשארים בריאים,
   ולוג attestation נשמר יחד עם ראיות ה-drill.

## צ'קליסט ראיות
- exports `artifacts/connect-chaos/<date>/c*_metrics.json` מתוך
  `scripts/swift_status_export.py telemetry`.
- פלטי CLI (`c*_queue.txt`) מ-`iroha connect queue inspect`.
- לוגים של SDK + Torii עם חותמות זמן והאש של SID.
- צילומי דשבורד עם אנוטציות לכל תרחיש.
- PagerDuty / incident IDs אם התרעות Sev 1/2 הופעלו.

השלמת המטריצה פעם ברבעון מספקת את שער ה-roadmap ומדגימה שהטמעות
Connect ב-Swift/Android/JS מגיבות בצורה דטרמיניסטית למצבי הכשל
המסוכנים ביותר.

</div>
