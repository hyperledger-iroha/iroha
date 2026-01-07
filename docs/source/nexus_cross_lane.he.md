---
lang: he
direction: rtl
source: docs/source/nexus_cross_lane.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 6e6f144bf3aef313ba55b539c9e92c827bd626973fe38b557f0b668cc909f589
source_last_modified: "2025-12-13T05:07:11.929584+00:00"
translation_last_reviewed: 2026-01-01
---

<div dir="rtl">

<!-- התרגום העברי ל- docs/source/nexus_cross_lane.md -->

# מחויבויות cross-lane ב-Nexus וצינור ההוכחות

> **סטטוס:** מסירת NX-4 — צינור מחויבויות cross-lane והוכחות (יעד רבעון 4 2025).  
> **בעלים:** Nexus Core WG / Cryptography WG / Networking TL.  
> **פריטי מפת דרכים קשורים:** NX-1 (lane geometry), NX-3 (settlement router), NX-4 (מסמך זה), NX-8 (global scheduler), NX-11 (SDK conformance).

מסמך זה מתאר כיצד נתוני ביצוע לכל lane הופכים להתחייבות גלובלית הניתנת לאימות. הוא מחבר את ה-settlement router הקיים (`crates/settlement_router`), את ה-lane block builder (`crates/iroha_core/src/block.rs`), את משטחי הטלמטריה/סטטוס, ואת חיבורי LaneRelay/DA המתוכננים שעדיין צריכים להגיע ל-roadmap **NX-4**.

## יעדים

- לייצר `LaneBlockCommitment` דטרמיניסטי לכל lane block שמכסה settlement, נזילות ונתוני שונות בלי לחשוף מצב פרטי.
- לשדר את המחויבויות האלה (ואת attestation של DA) לטבעת NPoS הגלובלית כדי שה-merge ledger יוכל לסדר, לאמת ולהתמיד עדכוני cross-lane.
- לחשוף את אותם payloads דרך Torii והטלמטריה כדי שמפעילים, SDKs ומבקרים יוכלו לשחזר את הצינור בלי כלים ייעודיים.
- להגדיר את האינווריאנטים וחבילות הראיות הנדרשות כדי להשלים NX-4: הוכחות lane, attestation של DA, אינטגרציית merge ledger וכיסוי רגרסיה.

## רכיבים ומשטחים

| רכיב | אחריות | הפניות מימוש |
|-----------|----------------|---------------------------|
| Lane executor ו-settlement router | לתמחר המרות XOR, לצבור receipts לכל טרנזקציה, ולאכוף מדיניות buffer | `crates/iroha_core/src/settlement/mod.rs`, `crates/settlement_router` |
| Lane block builder | לנקז `SettlementAccumulator`s, להנפיק `LaneBlockCommitment`s לצד ה-lane block | `crates/iroha_core/src/block.rs:3340-3415` |
| LaneRelay broadcaster | לאגד QCs של lane + הוכחות DA, להפיץ דרך `iroha_p2p`, ולהזין את ה-merge ring | `crates/iroha_core/src/nexus/lane_relay.rs`, `crates/iroha_core/src/sumeragi/main_loop.rs` |
| Global merge ledger | לאמת QCs של lane, לבצע הפחתת merge hints, ולהתמיד מחויבויות world-state | `docs/source/merge_ledger.md`, `crates/iroha_core/src/sumeragi/status.rs`, `crates/iroha_core/src/state.rs` |
| Torii status ולוחות | לחשוף `lane_commitments`, `lane_settlement_commitments`, `lane_relay_envelopes`, מדדי scheduler ולוחות Grafana | `crates/iroha_torii/src/routing.rs:16660-16880`, `dashboards/grafana/nexus_lanes.json` |
| אחסון ראיות | לארכב `LaneBlockCommitment`s, ארטיפקטים של RBC וצילומי Alertmanager לצורכי ביקורת | `docs/settlement-router.md`, `artifacts/nexus/*` (חבילה עתידית) |

## מבני נתונים ופריסת payload

ה-payloads הקנוניים נמצאים ב-`crates/iroha_data_model/src/block/consensus.rs`.

### `LaneSettlementReceipt`

- `source_id` — hash של טרנזקציה או id שסופק על ידי הקורא.
- `local_amount_micro` — חיוב טוקן הגז של ה-dataspace.
- `xor_due_micro` / `xor_after_haircut_micro` / `xor_variance_micro` — רשומות XOR דטרמיניסטיות ומרווח הבטיחות לכל receipt (`due - after haircut`).
- `timestamp_ms` — חותמת זמן UTC במילישניות שנלכדה בזמן settlement.

Receipts יורשים את כללי התמחור הדטרמיניסטיים מ-`SettlementEngine` ומאוגדים בתוך כל `LaneBlockCommitment`.

### `LaneSwapMetadata`

מטא-דאטה אופציונלי שמתעד את הפרמטרים ששימשו בתמחור:

- `epsilon_bps`, `twap_window_seconds`, `volatility_class`.
- bucket של `liquidity_profile` (Tier1-Tier3).
- מחרוזת `twap_local_per_xor` כדי שמבקרים יוכלו לחשב המרות מחדש בדיוק.

### `LaneBlockCommitment`

סיכום per-lane הנשמר עם כל בלוק:

- כותרת: `block_height`, `lane_id`, `dataspace_id`, `tx_count`.
- סיכומים: `total_local_micro`, `total_xor_due_micro`, `total_xor_after_haircut_micro`, `total_xor_variance_micro`.
- `swap_metadata` אופציונלי.
- וקטור `receipts` מסודר.

ה-structs האלו כבר מפיקים `NoritoSerialize`/`NoritoDeserialize`, כך שניתן להזרים אותם on-chain, דרך Torii או באמצעות fixtures ללא סטיית סכימה.

### `LaneRelayEnvelope`

`LaneRelayEnvelope` (ראו `crates/iroha_data_model/src/nexus/relay.rs`) אורז את ה-`BlockHeader` של
ה-lane, `commit QC (`Qc`)` אופציונלי, hash אופציונלי של `DaCommitmentBundle`, את
`LaneBlockCommitment` המלא, ואת מונה הבייטים של RBC לכל lane. המעטפה שומרת
`settlement_hash` שמופק מ-Norito (דרך `compute_settlement_hash`) כדי שמקבלים יוכלו לאמת את
payload ה-settlement לפני העברה ל-merge ledger. יש לדחות מעטפות כאשר `verify` נכשל
(אי התאמת QC subject, אי התאמת DA hash או אי התאמת settlement hash), כאשר `verify_with_quorum`
נכשל (שגיאות אורך bitmap של חותמים/קווארום), או כאשר לא ניתן לאמת את חתימת QC המצטברת מול
הרוסטר של ועדת ה-dataspace. ה-preimage של ה-QC מכסה את hash של ה-lane block יחד עם
`parent_state_root` ו-`post_state_root`, כך שהחברות ונכונות state-root נבדקות יחד.

### בחירת ועדת lane

QCs של lane relay מאומתים מול ועדה לכל dataspace. גודל הוועדה הוא `3f+1`, כאשר `f` מוגדר בקטלוג
ה-dataspace (`fault_tolerance`). מאגר המאמתים הוא מאמתים של dataspace: manifests של
Governance עבור lanes admin-managed ורשומות staking של public lanes עבור lanes stake-elected.
חברות בוועדה נדגמת באופן דטרמיניסטי לכל אפוק בעזרת זרע אפוק של VRF הקשור ל-`dataspace_id`
ו-`lane_id` (יציב לאורך האפוק). אם המאגר קטן מ-`3f+1`, סופיות ה-lane relay נעצרת עד שהקווארום
משוקם. מפעילים יכולים להרחיב את המאגר באמצעות הוראת multisig admin
`SetLaneRelayEmergencyValidators` (נדרש `CanManagePeers` ו-`nexus.lane_relay_emergency.enabled = true`,
שכבוי כברירת מחדל). בעת הפעלה, הסמכות חייבת להיות חשבון multisig שעומד במינימום המוגדר
(`nexus.lane_relay_emergency.multisig_threshold`/`multisig_members`, ברירת מחדל 3-of-5). ה-overrides
נשמרים לכל dataspace, חלים רק כשהמאגר מתחת לקווארום, ומנוקים על ידי שליחת רשימת מאמתים ריקה.
כאשר `expires_at_height` מוגדר, האימות מתעלם מה-override לאחר ש-`block_height` של מעטפת ה-lane relay
עובר את גובה התפוגה. מונה הטלמטריה
`lane_relay_emergency_override_total{lane,dataspace,outcome}` רושם האם ה-override הוחל (`applied`)
או היה חסר/פג תוקף/לא מספיק/מנוטרל בזמן האימות.

## מחזור חיי המחויבות

1. **תמחור והכנת receipts.**  
   חזית ה-settlement (`SettlementEngine`, `SettlementAccumulator`) רושמת `PendingSettlement` לכל
   טרנזקציה. כל רשומה שומרת קלטי TWAP, פרופיל נזילות, חותמות זמן וסכומי XOR כדי להפוך מאוחר יותר
   ל-`LaneSettlementReceipt`.

2. **אטימת receipts בבלוק.**  
   במהלך `BlockBuilder::finalize`, כל זוג `(lane_id, dataspace_id)` מנקז את ה-accumulator שלו. ה-builder
   יוצר `LaneBlockCommitment`, מעתיק את רשימת ה-receipts, מצטבר לסכומים, ושומר מטא-דאטה של swap
   אופציונלי (דרך `SwapEvidence`). הווקטור שנוצר נדחף ל-slot הסטטוס של Sumeragi
   (`crates/iroha_core/src/sumeragi/status.rs`) כדי שטוריי והטלמטריה יחשפו אותו מיד.

3. **אריזה של relay ו-attestation של DA.**  
   `LaneRelayBroadcaster` צורך כעת את `LaneRelayEnvelope`s שנפלטו בזמן אטימת הבלוק ומפיץ אותם כ-frames
   בעלי עדיפות גבוהה `NetworkMessage::LaneRelay`. המעטפות מאומתות, מדוללות לפי
   `(lane_id,dataspace_id,height,settlement_hash)`, ונשמרות בצילום הסטטוס של Sumeragi
   (`/v1/sumeragi/status`) עבור מפעילים ומבקרים. ה-broadcaster ימשיך להתפתח כדי לצרף ארטיפקטים של DA
   (הוכחות RBC chunk, Norito headers, manifests של SoraFS/Object) ולהזין את ה-merge ring בלי חסימת
   head-of-line.

4. **סידור גלובלי ו-merge ledger.**  
   טבעת ה-NPoS מאמתת כל relay envelope: בודקת `lane_qc` מול ועדת ה-dataspace, מחשבת מחדש סיכומי
   settlement, מאמתת הוכחות DA, ואז מזינה את tip של ה-lane ל-merge ledger המתואר ב-
   `docs/source/merge_ledger.md`. כאשר רשומת ה-merge נאטמת, ה-hash של ה-world-state
   (`global_state_root`) מתחייב לכל `LaneBlockCommitment`.

5. **התמדה וחשיפה.**  
   Kura כותבת את ה-lane block, את רשומת ה-merge ואת `LaneBlockCommitment` בצורה אטומית כדי שה-replay
   יוכל לשחזר את אותה הפחתה. `/v1/sumeragi/status` חושף:
   - `lane_commitments` (מטא-דאטה של ביצוע).
   - `lane_settlement_commitments` (ה-payload המתואר כאן).
   - `lane_relay_envelopes` (relay headers, QCs, DA digests, settlement hash ומוני bytes של RBC).
  Dashboards (`dashboards/grafana/nexus_lanes.json`) קוראים את אותם משטחי טלמטריה וסטטוס כדי להציג
  throughput של lane, אזהרות זמינות DA, נפח RBC, דלתות settlement וראיות relay.

## כללי אימות והוכחות

ה-merge ring חייב לאכוף את הבא לפני קבלת מחויבות lane:

1. **תקפות QC של lane.** לאמת את חתימת ה-BLS המצטברת על preimage של הצבעת הביצוע (hash של הבלוק,
   `parent_state_root`, `post_state_root`, גובה/תצוגה/אפוק, `chain_id` ותג מצב) מול רוסטר הוועדה של
   dataspace; לוודא שאורך bitmap החותמים תואם לוועדה, שהחותמים ממופים לאינדקסים תקינים, ושגובה הכותרת
   תואם `LaneBlockCommitment.block_height`.
2. **שלמות receipts.** לחשב מחדש את אגרגטי `total_*` מתוך וקטור ה-receipts; לדחות את המחויבות אם
   הסכומים חורגים או אם ה-receipts מכילים `source_id` כפולים.
3. **תקינות swap metadata.** לוודא ש-`swap_metadata` (אם קיים) תואם את תצורת ה-settlement ואת מדיניות
   ה-buffer של ה-lane.
4. **Attestation של DA.** לאמת שההוכחות RBC/SoraFS שסופקו על ידי relay נחתמות ל-digest המוטמע ושסט
   ה-chunks מכסה את כל payload הבלוק (`rbc_bytes_total` בטלמטריה חייב לשקף זאת).
5. **הפחתת merge.** לאחר שההוכחות לכל lane עברו, לכלול את tip ה-lane ברשומת merge ledger ולחשב מחדש
   את הפחתת Poseidon2 (`reduce_merge_hint_roots`). כל אי התאמה מבטלת את רשומת ה-merge.
6. **טלמטריה ושרשרת ביקורת.** להגדיל את מוני הביקורת לכל lane
   (`nexus_audit_outcome_total{lane_id,...}`) ולשמור את המעטפה כדי שחבילת הראיות תכלול גם את ההוכחה
   וגם את מסלול התצפית.

## זמינות נתונים ותצפית

- **מדדים:**  
  `nexus_scheduler_lane_teu_*`, `nexus_scheduler_dataspace_*`, `sumeragi_rbc_da_reschedule_total`,
  `da_reschedule_total`, `sumeragi_da_gate_block_total{reason="missing_local_data"}`,
  `lane_relay_invalid_total{error}`, `lane_relay_emergency_override_total{outcome}` ו-
  `nexus_audit_outcome_total` קיימים כבר ב-`crates/iroha_telemetry/src/metrics.rs`. מפעילים צריכים
  `lane_relay_invalid_total` צריך להישאר אפס מחוץ לתרגילי יריב.
- **משטחי Torii:**  
  `/v1/sumeragi/status` כולל `lane_commitments`, `lane_settlement_commitments` וצילומי dataspace.
  `/v1/nexus/lane-config` (מתוכנן) יפרסם את הגאומטריה של `LaneConfig` כדי שלקוחות יוכלו למפות
  `lane_id` לתוויות dataspace.
- **Dashboards:**  
  `dashboards/grafana/nexus_lanes.json` מציג backlog של lane, אותות זמינות DA ואת סכומי settlement
  המוצגים לעיל. הגדרות ההתראה צריכות לעמוד כאשר:
  - `nexus_scheduler_dataspace_age_slots` מפר את המדיניות.
  - `sumeragi_da_gate_block_total{reason="missing_local_data"}` עולה באופן מתמשך.
  - `total_xor_variance_micro` סוטה מהנורמות ההיסטוריות.
- **חבילות ראיות:**  
  כל שחרור חייב לצרף ייצואי `LaneBlockCommitment`, צילומי Grafana/Alertmanager ו-manifests של relay DA
  תחת `artifacts/nexus/cross-lane/<date>/`. החבילה הופכת לסט הראיות הקנוני בעת הגשת דוחות readiness של NX-4.

## רשימת בדיקה ליישום (NX-4)

1. **שירות LaneRelay**
   - הסכמה מוגדרת ב-`LaneRelayEnvelope`; broadcaster מיושם ב-
     `crates/iroha_core/src/nexus/lane_relay.rs` ומחובר לאטימת בלוקים
     (`crates/iroha_core/src/sumeragi/main_loop.rs`), ושולח `NetworkMessage::LaneRelay` עם דה-דופליקציה
     לפי node והתמדה של סטטוס.
   - לשמור ארטיפקטים של relay לצרכי ביקורת (`artifacts/nexus/relay/...`).
2. **Hooks של attestation DA**
   - לשלב RBC / SoraFS chunk proofs עם relay envelopes ולאחסן מדדים מסכמים ב-`SumeragiStatus`.
   - לחשוף סטטוס DA דרך Torii ו-Grafana למפעילים.
3. **אימות merge ledger**
   - להרחיב את מאמת ה-merge entry כך שידרוש relay envelopes ולא raw lane headers.
   - להוסיף בדיקות replay (`integration_tests/tests/nexus/*.rs`) שמזינות commitments סינתטיים לתוך
     merge ledger ומאשרות הפחתה דטרמיניסטית.
4. **עדכוני SDK וכלים**
   - לתעד את ה-Norito layout של `LaneBlockCommitment` עבור צרכני SDK
     (`docs/portal/docs/nexus/lane-model.md` כבר מקשר לכאן; להרחיב עם API snippets).
   - fixtures דטרמיניסטיים נמצאים תחת `fixtures/nexus/lane_commitments/*.{json,to}`; להריץ
     `cargo xtask nexus-fixtures` כדי לחדש (או `--verify` כדי לבדוק) את הדגימות
     `default_public_lane_commitment` ו-`cbdc_private_lane_commitment` כאשר יש שינויי סכימה.
5. **תצפית ו-runbooks**
   - לחבר חבילת Alertmanager למדדים החדשים ולתעד את זרימת הראיות ב-
     `docs/source/runbooks/nexus_cross_lane_incident.md` (מעקב).

השלמת הרשימה לעיל יחד עם המפרט הזה עומדת בדרישת התיעוד של **NX-4** ומשחררת את יתר העבודה המעשית.

</div>
