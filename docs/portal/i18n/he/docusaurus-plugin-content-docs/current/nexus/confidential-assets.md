---
lang: he
direction: rtl
source: docs/portal/docs/nexus/confidential-assets.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: נכסים חסויים והעברות ZK
description: תכנית Phase C למחזור shielded, רגיסטרים ובקרות אופרטור.
slug: /nexus/confidential-assets
---

<!--
SPDX-License-Identifier: Apache-2.0
-->
# תכנון נכסים חסויים והעברות ZK

## מוטיבציה
- לספק זרימות נכסים shielded באופציה, כך שדומיינים ישמרו על פרטיות טרנזקציות בלי לשנות את המחזור השקוף.
- לשמור על ביצוע דטרמיניסטי על גבי חומרת ולידטורים הטרוגנית ולהותיר תאימות Norito/Kotodama ABI v1.
- לספק לאודיטורים ולאופרטורים בקרות מחזור חיים (הפעלה, רוטציה, ביטול) עבור circuits ופרמטרים קריפטוגרפיים.

## מודל איומים
- הוולידטורים הם honest-but-curious: מבצעים קונצנזוס נאמנה אך מנסים לבדוק ledger/state.
- צופי רשת רואים נתוני בלוקים וטרנזקציות בגוסיפ; אין הנחה לערוצי gossip פרטיים.
- מחוץ לטווח: ניתוח תעבורה off-ledger, יריבים קוונטיים (מנוהל בנפרד ב-PQ roadmap), מתקפות זמינות על ledger.

## סקירת תכנון
- נכסים יכולים להכריז על *shielded pool* בנוסף ליתרות שקופות קיימות; המחזור shielded מיוצג באמצעות commitments קריפטוגרפיים.
- Notes עוטפות `(asset_id, amount, recipient_view_key, blinding, rho)` עם:
  - Commitment: `Comm = Pedersen(params_id || asset_id || amount || recipient_view_key || blinding)`.
  - Nullifier: `Null = Poseidon(domain_sep || nk || rho || asset_id || chain_id)` ללא תלות בסדר ה-notes.
  - Encrypted payload: `enc_payload = AEAD_XChaCha20Poly1305(ephemeral_shared_key, note_plaintext)`.
- טרנזקציות נושאות payloads `ConfidentialTransfer` המקודדים ב-Norito וכוללים:
  - Public inputs: Merkle anchor, nullifiers, commitments חדשים, asset id, גרסת circuit.
  - Payloads מוצפנים עבור נמענים ואודיטורים אופציונליים.
  - Zero-knowledge proof המעידה על שמירת ערך, בעלות והרשאה.
- Verifying keys ומערכי פרמטרים נשלטים דרך רגיסטרים on-ledger עם חלונות הפעלה; צמתים מסרבים לאמת proofs שמפנים לרשומות לא ידועות או מבוטלות.
- כותרות קונצנזוס מתחייבות ל-digest של יכולות חסויות פעילות כך שבלוקים מתקבלים רק כאשר מצב הרגיסטרי והפרמטרים תואם.
- בניית proofs משתמשת בסטאק Halo2 (Plonkish) ללא trusted setup; Groth16 או וריאנטים אחרים של SNARK אינם נתמכים בכוונה ב-v1.

### Fixtures דטרמיניסטיים

מעטפות memo חסויות מסופקות כעת עם fixture קנוני ב-`fixtures/confidential/encrypted_payload_v1.json`. מערך הנתונים כולל מעטפת v1 חיובית ודוגמאות שליליות פגומות כדי ש-SDKs יאמתו שוויון פרסינג. בדיקות data-model של Rust (`crates/iroha_data_model/tests/confidential_encrypted_payload_vectors.rs`) וסוויטת Swift (`IrohaSwift/Tests/IrohaSwiftTests/ConfidentialEncryptedPayloadTests.swift`) טוענות את ה-fixture ישירות, ומבטיחות ש-Norito encoding, משטחי שגיאה וכיסוי רגרסיה נשארים מיושרים כשהקודק מתפתח.

Swift SDKs יכולים כעת להנפיק הוראות shield בלי glue JSON ייעודי: בנו `ShieldRequest` עם note commitment של 32 בתים, payload מוצפן ו-debit metadata, ואז קראו ל-`IrohaSDK.submit(shield:keypair:)` (או `submitAndWait`) כדי לחתום ולהעביר את הטרנזקציה דרך `/v1/pipeline/transactions`. העוזר מאמת אורכי commitments, מזרים `ConfidentialEncryptedPayload` לתוך Norito encoder, ומשקף את ה-`zk::Shield` layout המתואר להלן כדי שה-wallets יישארו מסונכרנים עם Rust.

## Commitments של קונצנזוס ו-gating יכולות
- כותרות בלוקים חושפות `conf_features = { vk_set_hash, poseidon_params_id, pedersen_params_id, conf_rules_version }`; ה-digest משתתף ב-hash הקונצנזוס וחייב להתאים לתצוגת הרגיסטרי המקומית לקבלת הבלוק.
- Governance יכולה להכין שדרוגים על ידי תכנות `next_conf_features` עם `activation_height` עתידי; עד לגובה זה, מפיקי בלוקים חייבים להמשיך לפלוט את ה-digest הקודם.
- צמתי ולידטור חייבים לפעול עם `confidential.enabled = true` ו-`assume_valid = false`. בדיקות אתחול מסרבות להצטרף לקבוצת הוולידטורים אם תנאי כלשהו נכשל או אם `conf_features` מקומי שונה.
- מטא-דאטה של P2P handshake כוללת כעת `{ enabled, assume_valid, conf_features }`. Peers שמפרסמים יכולות לא תואמות נדחים עם `HandshakeConfidentialMismatch` ולעולם לא נכנסים לרוטציית קונצנזוס.
- תוצאות התאימות בין ולידטורים, observers ו-peers מיושנים נלכדות במטריצת ה-handshake תחת [Node Capability Negotiation](#node-capability-negotiation). כשלי handshake מציגים `HandshakeConfidentialMismatch` ומשאירים את ה-peer מחוץ לרוטציית הקונצנזוס עד שה-digest תואם.
- Observers שאינם ולידטורים יכולים להגדיר `assume_valid = true`; הם מיישמים דלתאות חסויות באופן עיוור אך אינם משפיעים על בטיחות הקונצנזוס.

## מדיניות נכסים
- כל הגדרת נכס נושאת `AssetConfidentialPolicy` שנקבעה על ידי היוצר או באמצעות governance:
  - `TransparentOnly`: מצב ברירת מחדל; רק הוראות שקופות (`MintAsset`, `TransferAsset` וכו') מותרות והפעולות shielded נדחות.
  - `ShieldedOnly`: כל ההנפקה וההעברות חייבות להשתמש בהוראות חסויות; `RevealConfidential` אסור כך שהיתרות אינן נחשפות פומבית.
  - `Convertible`: מחזיקים יכולים להעביר ערך בין ייצוגים שקופים וחסויים באמצעות הוראות on/off-ramp להלן.
- המדיניות עוקבת אחר FSM מוגבל כדי למנוע תקיעת כספים:
  - `TransparentOnly → Convertible` (הפעלת shielded pool מיידית).
  - `TransparentOnly → ShieldedOnly` (דורש מעבר ממתין וחלון המרה).
  - `Convertible → ShieldedOnly` (עיכוב מינימלי מחייב).
  - `ShieldedOnly → Convertible` (נדרשת תכנית הגירה כדי ש-shielded notes יישארו ברות הוצאה).
  - `ShieldedOnly → TransparentOnly` אסור אלא אם ה-shielded pool ריק או אם governance מקודדת הגירה שמסירה חסיון מ-notes פתוחות.
- הוראות governance קובעות `pending_transition { new_mode, effective_height, previous_mode, transition_id, conversion_window }` דרך ISI `ScheduleConfidentialPolicyTransition` ויכולות לבטל שינויים מתוזמנים עם `CancelConfidentialPolicyTransition`. אימות mempool מבטיח שאף טרנזקציה לא חוצה את גובה המעבר ושכללה של מדיניות שמשתנה באמצע בלוק נכשלת דטרמיניסטית.
- מעבר ממתין מיושם אוטומטית עם פתיחת בלוק חדש: כאשר הגובה נכנס לחלון ההמרה (לשדרוגי `ShieldedOnly`) או מגיע ל-`effective_height`, ה-runtime מעדכן את `AssetConfidentialPolicy`, מרענן את metadata `zk.policy` ומנקה את ה-entry הממתין. אם נשאר supply שקוף כאשר מעבר `ShieldedOnly` מבשיל, ה-runtime מבטל את השינוי ומרשום אזהרה תוך שמירה על המצב הקודם.
- knobs בקונפיג `policy_transition_delay_blocks` ו-`policy_transition_window_blocks` אוכפים התראה מינימלית ותקופות חסד כדי לאפשר המרות ב-wallets סביב המעבר.
- `pending_transition.transition_id` משמש גם כ-audit handle; על governance לצטט אותו בעת סיום או ביטול מעברים כדי שהאופרטורים יקשרו דוחות on/off-ramp.
- `policy_transition_window_blocks` ברירת מחדל 720 (כ-12 שעות בזמן בלוק של 60 שניות). צמתים מצמצמים בקשות governance שמנסות התראה קצרה יותר.
- Genesis manifests ותהליכי CLI מציגים מדיניות נוכחית וממתינה. לוגיקת admission קוראת את המדיניות בזמן הביצוע כדי לאשר שכל הוראה חסויה מורשית.
- Checklist הגירה — ראו “Migration sequencing” להלן לתכנית שדרוג מדורגת לפי Milestone M0.

#### ניטור מעברים דרך Torii

Wallets ואודיטורים בודקים `GET /v1/confidential/assets/{definition_id}/transitions` כדי לבחון את `AssetConfidentialPolicy` הפעילה. ה-payload של JSON כולל תמיד asset id קנוני, גובה בלוק אחרון שנצפה, `current_mode` של המדיניות, המצב האפקטיבי בגובה זה (חלונות המרה מדווחים זמנית `Convertible`), ואת מזהי הפרמטרים הצפויים `vk_set_hash`/Poseidon/Pedersen. כאשר מעבר governance ממתין, התגובה כוללת גם:

- `transition_id` - audit handle שמוחזר מ-`ScheduleConfidentialPolicyTransition`.
- `previous_mode`/`new_mode`.
- `effective_height`.
- `conversion_window` וה-`window_open_height` הנגזר (הבלוק שבו wallets חייבים להתחיל המרה לקראת cut-over של ShieldedOnly).

דוגמת תגובה:

```json
{
  "asset_id": "62Fk4FPcMuLvW5QjDGNF2a4jAmjM",
  "block_height": 4217,
  "current_mode": "Convertible",
  "effective_mode": "Convertible",
  "vk_set_hash": "8D7A4B0A95AB1C33F04944F5D332F9A829CEB10FB0D0797E2D25AEFBAAF1155D",
  "poseidon_params_id": 7,
  "pedersen_params_id": 11,
  "pending_transition": {
    "transition_id": "BF2C6F9A4E9DF389B6F7E5E6B5487B39AE00D2A4B7C0FBF2C9FEF6D0A961C8ED",
    "previous_mode": "Convertible",
    "new_mode": "ShieldedOnly",
    "effective_height": 5000,
    "conversion_window": 720,
    "window_open_height": 4280
  }
}
```

תגובה `404` מציינת שאין הגדרת asset תואמת. כאשר אין מעבר מתוזמן, השדה `pending_transition` הוא `null`.

### מכונת מצבי מדיניות

| מצב נוכחי           | מצב הבא           | תנאים מוקדמים                                                                 | טיפול ב-effective_height                                                                                         | הערות                                                                                     |
|--------------------|------------------|-------------------------------------------------------------------------------|-------------------------------------------------------------------------------------------------------------------|-------------------------------------------------------------------------------------------|
| TransparentOnly    | Convertible      | Governance הפעילה רשומות registry עבור verifier/parameter. הגישו `ScheduleConfidentialPolicyTransition` עם `effective_height ≥ current_height + policy_transition_delay_blocks`. | המעבר מבוצע בדיוק ב-`effective_height`; shielded pool זמין מיד.                   | נתיב ברירת מחדל להפעלת חסיון תוך שמירת זרימות שקופות.               |
| TransparentOnly    | ShieldedOnly     | כמו לעיל, בתוספת `policy_transition_window_blocks ≥ 1`.                                                         | ה-runtime נכנס אוטומטית ל-`Convertible` ב-`effective_height - policy_transition_window_blocks`; מתהפך ל-`ShieldedOnly` ב-`effective_height`. | מספק חלון המרה דטרמיניסטי לפני כיבוי הוראות שקופות.   |
| Convertible        | ShieldedOnly     | מעבר מתוזמן עם `effective_height ≥ current_height + policy_transition_delay_blocks`. Governance SHOULD לאשר (`transparent_supply == 0`) באמצעות audit metadata; runtime אוכף זאת בעת cut-over. | אותן סמנטיקות חלון כמו לעיל. אם ה-supply השקוף אינו אפס ב-`effective_height`, המעבר מתבטל עם `PolicyTransitionPrerequisiteFailed`. | נועל את הנכס למחזור חסוי מלא.                                     |
| ShieldedOnly       | Convertible      | מעבר מתוזמן; אין emergency withdrawal פעיל (`withdraw_height` לא מוגדר).                                    | המצב מתהפך ב-`effective_height`; reveal ramps נפתחים מחדש בעוד shielded notes נשארים תקפים.                           | משמש לחלונות תחזוקה או ביקורות אודיטור.                                          |
| ShieldedOnly       | TransparentOnly  | Governance חייבת להוכיח `shielded_supply == 0` או להכין תכנית `EmergencyUnshield` חתומה (נדרשות חתימות אודיטור). | ה-runtime פותח חלון `Convertible` לפני `effective_height`; בגובה זה הוראות חסויות נכשלות קשיח והנכס חוזר למצב שקוף בלבד. | יציאה של מוצא אחרון. המעבר מתבטל אוטומטית אם note חסוי נצרך במהלך החלון. |
| Any                | Same as current  | `CancelConfidentialPolicyTransition` מנקה שינוי ממתין.                                                        | `pending_transition` מוסר מיד.                                                                          | שומר על status quo; מוצג להשלמה.                                             |

מעברים שלא מופיעים לעיל נדחים בעת הגשתם ל-governance. ה-runtime בודק את הדרישות ממש לפני יישום מעבר מתוזמן; כשל בתנאים מחזיר את הנכס למצב הקודם ומוציא `PolicyTransitionPrerequisiteFailed` בטלמטריה ובאירועי בלוק.

### Migration sequencing

1. **Prepare registries:** הפעילו את כל רשומות ה-verifier והפרמטרים המוזכרות במדיניות היעד. צמתים מפרסמים את `conf_features` שנוצרו כדי ש-peers יאמתו תאימות.
2. **Stage the transition:** הגישו `ScheduleConfidentialPolicyTransition` עם `effective_height` שמכבד `policy_transition_delay_blocks`. במעבר ל-`ShieldedOnly` ציינו חלון המרה (`window ≥ policy_transition_window_blocks`).
3. **Publish operator guidance:** רשמו את `transition_id` שהוחזר והפיצו runbook של on/off-ramp. Wallets ואודיטורים נרשמים ל-`/v1/confidential/assets/{id}/transitions` כדי לדעת את גובה פתיחת החלון.
4. **Window enforcement:** בעת פתיחת החלון ה-runtime משנה את המדיניות ל-`Convertible`, מפיץ `PolicyTransitionWindowOpened { transition_id }`, ומתחיל לדחות בקשות governance סותרות.
5. **Finalize or abort:** ב-`effective_height` ה-runtime בודק תנאים מוקדמים (supply שקוף אפס, ללא משיכות חירום וכו'). הצלחה מחליפה את המדיניות למצב המבוקש; כשל מוציא `PolicyTransitionPrerequisiteFailed`, מנקה את המעבר הממתין ומשאיר את המדיניות ללא שינוי.
6. **Schema upgrades:** לאחר מעבר מוצלח, governance מעלה גרסת סכימת נכס (למשל `asset_definition.v2`) וכלי CLI דורשים `confidential_policy` בעת סדרת manifests. מסמכי שדרוג genesis מנחים את האופרטורים להוסיף הגדרות מדיניות וטביעות registry לפני אתחול מחדש של הוולידטורים.

רשתות חדשות שמתחילות עם confidentiality מופעלת מקודדות את המדיניות הרצויה ישירות ב-genesis. הן עדיין עוקבות אחר הצ'קליסט לעיל בעת שינוי מצבים לאחר ההשקה כדי לשמור חלונות המרה דטרמיניסטיים ולתת ל-wallets זמן להסתגל.

### Norito manifest versioning & activation

- Genesis manifests חייבים לכלול `SetParameter` עבור המפתח המותאם `confidential_registry_root`. ה-payload הוא Norito JSON התואם `ConfidentialRegistryMeta { vk_set_hash: Option<String> }`: השמיטו את השדה (`null`) כאשר אין entries פעילים, אחרת ספקו hex באורך 32 בתים (`0x…`) השווה ל-hash שמופק על ידי `compute_vk_set_hash` מעל הוראות ה-verifier שב-manifest. צמתים מסרבים להתחיל אם הפרמטר חסר או אם ה-hash לא תואם לכתיבות registry המקודדות.
- ה-`ConfidentialFeatureDigest::conf_rules_version` on-wire מטמיע את גרסת layout של ה-manifest. ברשתות v1 הוא חייב להישאר `Some(1)` ושווה ל-`iroha_config::parameters::defaults::confidential::RULES_VERSION`. כאשר ה-ruleset מתפתח, העלו את הקבוע, הפיקו manifests מחדש והפיצו בינאריים במקביל; ערבוב גרסאות גורם לוולידטורים לדחות בלוקים עם `ConfidentialFeatureDigestMismatch`.
- Activation manifests צריכים לשלב עדכוני registry, שינויי מחזור חיים של פרמטרים ומעברי מדיניות כך שה-digest יישאר עקבי:
  1. החילו את מוטציות registry המתוכננות (`Publish*`, `Set*Lifecycle`) בתצוגת מצב offline וחישבו את ה-digest לאחר ההפעלה באמצעות `compute_confidential_feature_digest`.
  2. הפיקו `SetParameter::custom(confidential_registry_root, {"vk_set_hash": "0x…"})` עם ה-hash המחושב כדי ש-peers מאחרים יוכלו לשחזר את ה-digest הנכון גם אם פספסו הוראות registry ביניים.
  3. הוסיפו הוראות `ScheduleConfidentialPolicyTransition`. כל הוראה חייבת לצטט את `transition_id` שהונפק ב-governance; manifests ששוכחים אותו יידחו על ידי ה-runtime.
  4. שמרו את bytes של ה-manifest, טביעת SHA-256 וה-digest ששימש בתכנית ההפעלה. האופרטורים מאמתים את שלושת הארטיפקטים לפני הצבעה כדי למנוע פיצול.
- כאשר rollout דורש cut-over נדחה, רשמו את גובה היעד בפרמטר מותאם מלווה (למשל `custom.confidential_upgrade_activation_height`). זה מספק לאודיטורים הוכחה מקודדת Norito שהוולידטורים כיבדו את חלון ההודעה לפני שה-digest נכנס לתוקף.

## מחזור חיים של verifier ופרמטרים
### ZK Registry
- ה-ledger מאחסן `ZkVerifierEntry { vk_id, circuit_id, version, proving_system, curve, public_inputs_schema_hash, vk_hash, vk_len, max_proof_bytes, gas_schedule_id, activation_height, deprecation_height, withdraw_height, status, metadata_uri_cid, vk_bytes_cid }` כאשר `proving_system` קבוע כעת ל-`Halo2`.
- זוגות `(circuit_id, version)` ייחודיים גלובלית; הרגיסטרי מחזיק אינדקס משני לחיפושים לפי metadata של circuit. ניסיונות לרשום זוג כפול נדחים ב-admission.
- `circuit_id` חייב להיות לא ריק ו-`public_inputs_schema_hash` חייב להיות מסופק (לרוב hash Blake2b-32 של הקידוד הקנוני של public-input). Admission דוחה רשומות שחסרות שדות אלו.
- הוראות governance כוללות:
  - `PUBLISH` להוספת entry `Proposed` עם metadata בלבד.
  - `ACTIVATE { vk_id, activation_height }` כדי לתזמן הפעלה בגבול epoch.
  - `DEPRECATE { vk_id, deprecation_height }` לציון הגובה האחרון שבו proofs יכולים להפנות ל-entry.
  - `WITHDRAW { vk_id, withdraw_height }` לכיבוי חירום; נכסים מושפעים מקפיאים הוצאות חסויות אחרי withdraw height עד להפעלת entries חדשים.
- Genesis manifests מנפיקים אוטומטית פרמטר `confidential_registry_root` עם `vk_set_hash` התואם entries פעילים; אימות מצליב את ה-digest מול מצב הרגיסטרי המקומי לפני שהצומת יכול להצטרף לקונצנזוס.
- רישום או עדכון verifier דורש `gas_schedule_id`; אימות אוכף שה-entry `Active`, קיים באינדקס `(circuit_id, version)`, וש-Halo2 proofs מספקים `OpenVerifyEnvelope` שבו `circuit_id`, `vk_hash` ו-`public_inputs_schema_hash` תואמים לרשומת הרגיסטרי.

### Proving Keys
- Proving keys נשארים off-ledger אך מיוחסים באמצעות מזהים content-addressed (`pk_cid`, `pk_hash`, `pk_len`) שמפורסמים לצד metadata של verifier.
- Wallet SDKs מושכים את נתוני PK, מאמתים hashes ומאחסנים מקומית.

### Pedersen & Poseidon Parameters
- רגיסטרים נפרדים (`PedersenParams`, `PoseidonParams`) משקפים את בקרות מחזור החיים של verifier, לכל אחד `params_id`, hashes של generators/constants, וגבהי activation/deprecation/withdraw.

## Deterministic Ordering & Nullifiers
- כל נכס שומר `CommitmentTree` עם `next_leaf_index`; בלוקים מצרפים commitments בסדר דטרמיניסטי: מעבר על טרנזקציות לפי סדר הבלוק; בתוך כל טרנזקציה מעבר על outputs shielded לפי `output_idx` מסודר.
- `note_position` נגזר מאופסטים של העץ אבל **לא** חלק מה-nullifier; הוא מזין רק נתיבי membership ב-proof witness.
- יציבות nullifier תחת reorgs מובטחת על ידי עיצוב PRF; קלט ה-PRF קושר `{ nk, note_preimage_hash, asset_id, chain_id, params_id }`, וה-anchors מפנים ל-Merkle roots היסטוריים המוגבלים על ידי `max_anchor_age_blocks`.

## זרימת ledger
1. **MintConfidential { asset_id, amount, recipient_hint }**
   - דורש מדיניות `Convertible` או `ShieldedOnly`; admission בודק סמכות asset, שולף `params_id` נוכחי, דוגם `rho`, מפיק commitment ומעדכן את Merkle tree.
   - מפיק `ConfidentialEvent::Shielded` עם commitment חדש, delta של Merkle root ו-hash של קריאת הטרנזקציה לצרכי audit.
2. **TransferConfidential { asset_id, proof, circuit_id, version, nullifiers, new_commitments, enc_payloads, anchor_root, memo }**
   - Syscall ה-VM מאמת proof מול entry ברגיסטרי; host מבטיח nullifiers לא בשימוש, commitments מצורפים דטרמיניסטית, וה-anchor עדכני.
   - ה-ledger רושם entries של `NullifierSet`, מאחסן payloads מוצפנים לנמענים/אודיטורים, ומפיק `ConfidentialEvent::Transferred` המסכם nullifiers, outputs מסודרים, proof hash ו-Merkle roots.
3. **RevealConfidential { asset_id, proof, circuit_id, version, nullifier, amount, recipient_account, anchor_root }**
   - זמין רק לנכסי `Convertible`; proof מאמת שערך ה-note שווה לסכום הגלוי, ה-ledger מזכה יתרה שקופה ושורף את ה-note החסוי ע"י סימון nullifier כ-spent.
   - מפיק `ConfidentialEvent::Unshielded` עם הסכום הציבורי, nullifiers שנצרכו, מזהי proof ו-hash של קריאת הטרנזקציה.

## תוספות ל-Data Model
- `ConfidentialConfig` (סעיף קונפיג חדש) עם דגל enablement, `assume_valid`, knobs לגז/מגבלות, חלון anchor ו-verifier backend.
- `ConfidentialNote`, `ConfidentialTransfer`, ו-`ConfidentialMint` סכמות Norito עם בייט גרסה מפורש (`CONFIDENTIAL_ASSET_V1 = 0x01`).
- `ConfidentialEncryptedPayload` עוטף AEAD memo bytes ב-`{ version, ephemeral_pubkey, nonce, ciphertext }`, ברירת מחדל `version = CONFIDENTIAL_ENCRYPTED_PAYLOAD_V1` עבור פריסת XChaCha20-Poly1305.
- וקטורי key-derivation קנוניים נמצאים ב-`docs/source/confidential_key_vectors.json`; גם CLI וגם Torii endpoint מבצעים רגרסיה מול fixtures אלה.
- `asset::AssetDefinition` מקבל `confidential_policy: AssetConfidentialPolicy { mode, vk_set_hash, poseidon_params_id, pedersen_params_id, pending_transition }`.
- `ZkAssetState` שומר את הקישור `(backend, name, commitment)` עבור transfer/unshield verifiers; ביצוע דוחה proofs שמפתחות האימות שלהן אינם תואמים ל-commitment הרשום.
- `CommitmentTree` (לכל נכס עם frontier checkpoints), `NullifierSet` לפי `(chain_id, asset_id, nullifier)`, `ZkVerifierEntry`, `PedersenParams`, `PoseidonParams` נשמרים ב-world state.
- Mempool מחזיק מבנים זמניים `NullifierIndex` ו-`AnchorIndex` לזיהוי כפילויות מוקדם ובדיקת גיל anchor.
- עדכוני סכמת Norito כוללים סדר קנוני ל-public inputs; round-trip tests מבטיחים דטרמיניזם של קידוד.
- round-trip של payloads מוצפנים ננעל בבדיקות יחידה (`crates/iroha_data_model/src/confidential.rs`). וקטורי wallet עתידיים יצרפו AEAD transcripts קנוניים לאודיטורים. `norito.md` מתעד את ה-on-wire header של ה-envelope.

## אינטגרציית IVM ו-syscall
- להציג syscall `VERIFY_CONFIDENTIAL_PROOF` שמקבל:
  - `circuit_id`, `version`, `scheme`, `public_inputs`, `proof` וה-`ConfidentialStateDelta { asset_id, nullifiers, commitments, enc_payloads }` שנוצר.
  - Syscall טוען metadata של verifier מהרגיסטרי, אוכף מגבלות גודל/זמן, גובה gas דטרמיניסטי, ומיישם delta רק אם proof מצליח.
- Host חושף trait לקריאה בלבד `ConfidentialLedger` לשליפת snapshots של Merkle root ומצב nullifier; ספריית Kotodama מספקת helpers להרכבת witness ואימות schema.
- מסמכי pointer-ABI עודכנו כדי להבהיר את layout של proof buffer ו-registry handles.

## Node Capability Negotiation
- Handshake מפרסם `feature_bits.confidential` יחד עם `ConfidentialFeatureDigest { vk_set_hash, poseidon_params_id, pedersen_params_id, conf_rules_version }`. השתתפות ולידטור דורשת `confidential.enabled=true`, `assume_valid=false`, מזהי verifier backend זהים ו-digest תואם; חוסר התאמה נכשל ב-`HandshakeConfidentialMismatch`.
- Config תומך ב-`assume_valid` לצמתי observer בלבד: כאשר כבוי, מפגש עם הוראות חסויות מפיק `UnsupportedInstruction` דטרמיניסטי ללא panic; כאשר מופעל, observers מיישמים state deltas מוצהרים ללא אימות proofs.
- Mempool דוחה טרנזקציות חסויות אם היכולת המקומית כבויה. Gossip filters נמנעים מלשלוח טרנזקציות shielded ל-peers לא תואמים תוך העברת מזהי verifier לא ידועים בצורה עיוורת בתוך מגבלות גודל.

### Reveal Pruning & Nullifier Retention Policy

Ledgers חסויים חייבים לשמור מספיק היסטוריה כדי להוכיח freshness של notes ולשחזר audits מונעי governance. המדיניות ברירת המחדל, שמיושמת על ידי `ConfidentialLedger`, היא:

- **Nullifier retention:** לשמור nullifiers שנצרכו לפחות `730` ימים (24 חודשים) אחרי גובה ההוצאה, או חלון ארוך יותר אם נדרש רגולטורית. אופרטורים יכולים להאריך דרך `confidential.retention.nullifier_days`. Nullifiers צעירים מחלון השימור חייבים להישאר ניתנים לשאילתה דרך Torii כדי שהאודיטורים יוכלו להוכיח היעדר double-spend.
- **Reveal pruning:** reveals שקופים (`RevealConfidential`) גוזמים את note commitments המשויכים מיד לאחר סיום הבלוק, אך ה-nullifier הנצרך נשאר כפוף לכלל השימור לעיל. אירועי reveal (`ConfidentialEvent::Unshielded`) מתעדים את הסכום הציבורי, הנמען ו-hash ה-proof כך ששחזור reveal היסטורי לא דורש ciphertext שנגזם.
- **Frontier checkpoints:** commitment frontiers שומרים checkpoints מתגלגלים שמכסים את הגדול בין `max_anchor_age_blocks` לחלון השימור. צמתים מדחסים checkpoints ישנים רק לאחר שפגו כל nullifiers בתוך הטווח.
- **Stale digest remediation:** אם `HandshakeConfidentialMismatch` מופעל עקב סטייה ב-digest, על האופרטורים (1) לוודא שחלונות שימור nullifier מיושרים בכל הקלאסטר, (2) להריץ `iroha_cli app confidential verify-ledger` לשחזור digest מול קבוצת nullifier שמורה, ו-(3) לפרוס מחדש manifest מעודכן. Nullifiers שנגזמו מוקדם צריכים להיות משוחזרים מ-cold storage לפני חיבור מחדש לרשת.

תעדו overrides מקומיים ב-operations runbook; מדיניות governance שמאריכות את חלון השימור חייבות לעדכן את קונפיגורציית הצמתים ואת תוכניות האחסון הארכיוני במקביל.

### Eviction & Recovery Flow

1. בעת dial, `IrohaNetwork` משווה את היכולות המפורסמות. אי התאמה מעלה `HandshakeConfidentialMismatch`; החיבור נסגר וה-peer נשאר ב-discovery queue ללא מעבר ל-`Ready`.
2. הכשל מוצג בלוג שירות הרשת (כולל digest ו-backend של הצד המרוחק), ו-Sumeragi לעולם לא מתזמן את ה-peer ל-proposal או voting.
3. אופרטורים מתקנים באמצעות יישור רגיסטרים וסטי פרמטרים (`vk_set_hash`, `pedersen_params_id`, `poseidon_params_id`) או על ידי staging `next_conf_features` עם `activation_height` מוסכם. כש-digest תואם, ה-handshake הבא מצליח אוטומטית.
4. אם peer מיושן מצליח לשדר בלוק (למשל דרך archival replay), ולידטורים דוחים אותו דטרמיניסטית עם `BlockRejectionReason::ConfidentialFeatureDigestMismatch`, ומשמרים עקביות ledger בכל הרשת.

### Replay-safe handshake flow

1. כל ניסיון יציאה מקצה חומר מפתחות Noise/X25519 חדש. ה-handshake payload החתום (`handshake_signature_payload`) מחבר את המפתחות הציבוריים האפמרליים המקומיים והמרוחקים, את כתובת הסוקט המפורסמת בקידוד Norito, ואת מזהה ה-chain כאשר מקומפל עם `handshake_chain_id`. ההודעה מוצפנת ב-AEAD לפני שהיא נשלחת.
2. המגיב מחשב מחדש את ה-payload בסדר מפתחות peer/local הפוך ומאמת את חתימת Ed25519 המוטמעת ב-`HandshakeHelloV1`. מכיוון ששני המפתחות האפמרליים והכתובת המפורסמת הם חלק מדומיין החתימה, שידור חוזר של הודעה שנתפסה נגד peer אחר או שחזור חיבור מיושן נכשל דטרמיניסטית.
3. דגלי capability חסויים וה-`ConfidentialFeatureDigest` מועברים בתוך `HandshakeConfidentialMeta`. המקבל משווה את tuple `{ enabled, assume_valid, verifier_backend, digest }` מול `ConfidentialHandshakeCaps` המקומי; כל אי התאמה מסיים את ה-handshake עם `HandshakeConfidentialMismatch` לפני המעבר ל-`Ready`.
4. אופרטורים חייבים לחשב מחדש את ה-digest (באמצעות `compute_confidential_feature_digest`) ולהפעיל מחדש צמתים עם registries/policies מעודכנים לפני חיבור מחדש. peers שמפרסמים digests ישנים ממשיכים להיכשל, ומונעים חזרת מצב מיושן לקבוצת הוולידטורים.
5. הצלחות וכשלי handshake מעדכנים את מוני `iroha_p2p::peer` הסטנדרטיים (`handshake_failure_count`, helpers של taxonomy) ומפיקים לוגים מובנים עם תג של remote peer ID וטביעת digest. ניטור אינדיקטורים אלה מסייע לזהות ניסיונות replay או קונפיגורציות שגויות במהלך rollout.

## ניהול מפתחות ו-payloads
- היררכיית derivation לכל חשבון:
  - `sk_spend` → `nk` (nullifier key), `ivk` (incoming viewing key), `ovk` (outgoing viewing key), `fvk`.
- payloads מוצפנים של notes משתמשים ב-AEAD עם מפתחות שיתוף נגזרים ב-ECDH; אפשר לצרף auditor view keys לאופציות outputs בהתאם למדיניות הנכס.
- תוספות CLI: `confidential create-keys`, `confidential send`, `confidential export-view-key`, tooling לאודיטורים לפענוח memos, ו-helper `iroha app zk envelope` ליצירה/בדיקה של מעטפות Norito offline.

## Gas, מגבלות ובקרות DoS
- לוח גז דטרמיניסטי:
  - Halo2 (Plonkish): בסיס `250_000` gas + `2_000` gas לכל public input.
  - `5` gas לכל בייט proof, ועוד תשלומים per-nullifier (`300`) ו-per-commitment (`500`).
  - אופרטורים יכולים לשנות קבועים אלה דרך קונפיגורציית הצומת (`confidential.gas.{proof_base, per_public_input, per_proof_byte, per_nullifier, per_commitment}`); שינויים מוחלים ב-startup או hot-reload ומיושמים דטרמיניסטית בכל הקלאסטר.
- מגבלות קשיחות (ברירות מחדל ניתנות לקונפיג):
- `max_proof_size_bytes = 262_144`.
- `max_nullifiers_per_tx = 8`, `max_commitments_per_tx = 8`, `max_confidential_ops_per_block = 256`.
- `verify_timeout_ms = 750`, `max_anchor_age_blocks = 10_000`. proofs שחורגים מ-`verify_timeout_ms` מפסיקים את ההוראה דטרמיניסטית (ballots של governance מפיקים `proof verification exceeded timeout`, `VerifyProof` מחזיר שגיאה).
- מכסות נוספות מבטיחות liveness: `max_proof_bytes_block`, `max_verify_calls_per_tx`, `max_verify_calls_per_block`, ו-`max_public_inputs` מגבילים block builders; `reorg_depth_bound` (≥ `max_anchor_age_blocks`) שולט בשימור frontier checkpoints.
- ה-runtime דוחה טרנזקציות שחורגות ממגבלות per-transaction או per-block, מוציא שגיאות `InvalidParameter` דטרמיניסטיות ומשאיר את מצב ה-ledger ללא שינוי.
- Mempool מסנן מראש טרנזקציות חסויות לפי `vk_id`, אורך proof וגיל anchor לפני הפעלת verifier כדי להגביל שימוש במשאבים.
- האימות נעצר דטרמיניסטית על timeout או חריגה מהגבולות; טרנזקציות נכשלות עם שגיאות מפורשות. backends של SIMD אופציונליים אך אינם משנים את חשבון הגז.

### Baselines של כיול וספי קבלה
- **Reference platforms.** ריצות הכיול חייבות לכסות את שלושת פרופילי החומרה להלן. ריצות שאינן מכסות את כולם נדחות בביקורת.

  | פרופיל | ארכיטקטורה | CPU / Instance | דגלי קומפילר | מטרה |
  | --- | --- | --- | --- | --- |
  | `baseline-simd-neutral` | `x86_64` | AMD EPYC 7B12 (32c) או Intel Xeon Gold 6430 (24c) | `RUSTFLAGS="-C target-feature=-avx,-avx2,-fma"` | קובע ערכי רצפה ללא אינסטרוקציות וקטוריות; משמש לכיוון טבלאות עלות fallback. |
  | `baseline-avx2` | `x86_64` | Intel Xeon Gold 6430 (24c) | default release | מאמת את מסלול AVX2; בודק שהאצות SIMD בתוך סבילות הגז הנייטרלי. |
  | `baseline-neon` | `aarch64` | AWS Graviton3 (c7g.4xlarge) | default release | מבטיח ש-backend NEON דטרמיניסטי ומתואם עם לוחות זמנים של x86. |

- **Benchmark harness.** דוחות כיול גז חייבים להיות מופקים עם:
  - `CRITERION_HOME=target/criterion cargo bench -p iroha_core isi_gas_calibration -- --sample-size 200 --warm-up-time 5 --save-baseline <profile-label>`
  - `cargo test -p iroha_core bench_repro -- --ignored` לאימות ה-fixture הדטרמיניסטית.
  - `CRITERION_HOME=target/criterion cargo bench -p ivm gas_calibration -- --sample-size 200 --warm-up-time 5 --save-baseline <profile-label>` כאשר עלויות VM opcode משתנות.

- **Fixed randomness.** יצאו `IROHA_CONF_GAS_SEED=conf-gas-seed-2026Q1` לפני הרצת benches כך ש-`iroha_test_samples::gen_account_in` יעבור למסלול הדטרמיניסטי `KeyPair::from_seed`. ה-harness מדפיס `IROHA_CONF_GAS_SEED_ACTIVE=…` פעם אחת; אם המשתנה חסר, הביקורת חייבת להיכשל. כלי כיול חדשים חייבים להמשיך לכבד את env var זה בעת הכנסת אקראיות עזר.

- **Result capture.**
  - העלו Criterion summaries (`target/criterion/**/raw.csv`) לכל פרופיל לארטיפקט הריליס.
  - שמרו מדדים נגזרים (`ns/op`, `gas/op`, `ns/gas`) ב-[Confidential Gas Calibration ledger](./confidential-gas-calibration) יחד עם git commit וגרסת קומפילר.
  - שמרו את שתי הבסיסיות האחרונות לכל פרופיל; מחקו snapshot ישן לאחר אימות הדוח החדש.

- **Acceptance tolerances.**
  - דלתאות גז בין `baseline-simd-neutral` ל-`baseline-avx2` חייבות להישאר ≤ ±1.5%.
  - דלתאות גז בין `baseline-simd-neutral` ל-`baseline-neon` חייבות להישאר ≤ ±2.0%.
  - הצעות כיול החורגות מספים אלה דורשות התאמות בלוח הזמנים או RFC שמסביר את הפער והמיתון.

- **Review checklist.** המגישים אחראים על:
  - הכללת `uname -a`, קטעי `/proc/cpuinfo` (model, stepping), ו-`rustc -Vv` בלוג הכיול.
  - אימות ש-`IROHA_CONF_GAS_SEED` מודפס ביציאת הבנצ'ים (הבנצ'ים מדפיסים את ה-seed הפעיל).
  - ווידוא ש-pacemaker ודגלי confidential verifier תואמים לפרודקשן (`--features confidential,telemetry` בעת הרצת benches עם Telemetry).

## Config & Operations
- `iroha_config` מקבל סעיף `[confidential]`:
  ```toml
  [confidential]
  enabled = true
  assume_valid = false
  verifier_backend = "ark_bls12_381"
  max_proof_size_bytes = 262144
  max_nullifiers_per_tx = 8
  max_commitments_per_tx = 8
  max_confidential_ops_per_block = 256
  verify_timeout_ms = 750
  max_anchor_age_blocks = 10000
  max_proof_bytes_block = 1048576
  max_verify_calls_per_tx = 4
  max_verify_calls_per_block = 128
  max_public_inputs = 32
  reorg_depth_bound = 10000
  policy_transition_delay_blocks = 100
  policy_transition_window_blocks = 200
  tree_roots_history_len = 10000
  tree_frontier_checkpoint_interval = 100
  registry_max_vk_entries = 64
  registry_max_params_entries = 32
  registry_max_delta_per_block = 4
  ```
- Telemetry מפיקה מדדים מצטברים: `confidential_proof_verified`, `confidential_verifier_latency_ms`, `confidential_proof_bytes_total`, `confidential_nullifier_spent`, `confidential_commitments_appended`, `confidential_mempool_rejected_total{reason}`, ו-`confidential_policy_transitions_total` בלי לחשוף plaintext.
- RPC surfaces:
  - `GET /confidential/capabilities`
  - `GET /confidential/zk_registry`
  - `GET /confidential/params`

## אסטרטגיית בדיקות
- דטרמיניזם: ערבוב אקראי של טרנזקציות בתוך בלוקים נותן אותם Merkle roots ו-nullifier sets.
- עמידות reorg: סימולציות reorg מרובות בלוקים עם anchors; nullifiers נשארים יציבים ו-anchors ישנים נדחים.
- אינוואריאנטים של gas: שימוש גז זהה על צמתים עם ובלי האצת SIMD.
- בדיקות קצה: proofs בתקרות גודל/gas, כמות in/out מקסימלית, ואכיפת timeout.
- מחזור חיים: פעולות governance להפעלה/השהיה של verifier ופרמטרים, בדיקות הוצאה בזמן רוטציה.
- Policy FSM: מעברים מותר/אסור, עיכובי pending transition ודחיית mempool סביב effective heights.
- חירום registry: emergency withdrawal מקפיא נכסים מושפעים בגובה `withdraw_height` ודוחה proofs לאחר מכן.
- Capability gating: ולידטורים עם `conf_features` לא תואמים דוחים בלוקים; observers עם `assume_valid=true` נשארים מסונכרנים בלי להשפיע על קונצנזוס.
- שוויון מצב: צמתי validator/full/observer מייצרים אותם state roots בשרשרת הקנונית.
- Negative fuzzing: proofs פגומים, payloads גדולים מדי והתנגשויות nullifier נדחים דטרמיניסטית.

## הגירה ותאימות
- Rollout מונחה feature: עד ש-Phase C3 מושלם, `enabled` ברירת מחדל `false`; צמתים מפרסמים capabilities לפני הצטרפות ל-validator set.
- נכסים שקופים אינם מושפעים; הוראות חסויות דורשות registry entries ומשא ומתן על capabilities.
- צמתים שנבנו ללא תמיכה ב-confidential דוחים בלוקים רלוונטיים דטרמיניסטית; אינם יכולים להצטרף ל-validator set אך עשויים לפעול כ-observers עם `assume_valid=true`.
- Genesis manifests כוללים registry entries ראשוניים, סטי פרמטרים, מדיניות חסויה לנכסים ומפתחות אודיטור אופציונליים.
- אופרטורים עוקבים אחרי runbooks שפורסמו לרוטציית registry, מעברי מדיניות ו-emergency withdrawal כדי לשמור על שדרוגים דטרמיניסטיים.

## עבודה שנותרה
- לבצע benchmark לסטי פרמטרים של Halo2 (גודל circuit, אסטרטגיית lookup) ולרשום את התוצאות ב-calibration playbook כדי לעדכן ברירות מחדל של gas/timeout יחד עם הרענון הבא של `confidential_assets_calibration.md`.
- לסיים מדיניות גילוי לאודיטורים ו-API של selective-viewing, ולחבר את הזרימה המאושרת ל-Torii לאחר אישור טיוטת governance.
- להרחיב את סכמת witness encryption לכיסוי outputs עם כמה נמענים ו-batched memos, ולתעד את פורמט ה-envelope עבור מפתחי SDK.
- להזמין ביקורת אבטחה חיצונית על circuits, registries ונהלי רוטציית פרמטרים ולשמור את הממצאים לצד דוחות הביקורת הפנימיים.
- להגדיר APIs לתיאום spentness עבור אודיטורים ולפרסם הנחיות scope של view-key כדי שספקי wallets יממשו את אותן סמנטיקות attestation.

## פאזות מימוש
1. **Phase M0 — Stop-Ship Hardening**
   - ✅ נגזרת nullifier כעת עוקבת אחרי עיצוב Poseidon PRF (`nk`, `rho`, `asset_id`, `chain_id`) עם enforcement של deterministic commitment ordering בעדכוני ledger.
   - ✅ הביצוע אוכף תקרות גודל proof ומכסי confidential per-transaction/per-block, ודוחה טרנזקציות חורגות עם שגיאות דטרמיניסטיות.
   - ✅ P2P handshake מפרסם `ConfidentialFeatureDigest` (backend digest + טביעות registry) ומפיל אי התאמות דטרמיניסטית דרך `HandshakeConfidentialMismatch`.
   - ✅ הוסרו panics במסלולי ביצוע confidential ונוסף role gating לצמתים לא תואמים.
   - ⚪ לאכוף budgets של verifier timeout ומגבלות עומק reorg ל-frontier checkpoints.
     - ✅ תקציבי timeout נאכפים; proofs שחורגים מ-`verify_timeout_ms` נכשלים דטרמיניסטית.
     - ✅ frontier checkpoints מכבדים `reorg_depth_bound`, גוזמים checkpoints ישנים יותר מהחלון המוגדר תוך שמירת snapshots דטרמיניסטיים.
   - להציג `AssetConfidentialPolicy`, policy FSM ו-enforcement gates עבור הוראות mint/transfer/reveal.
   - להתחייב ל-`conf_features` בכותרות בלוק ולסרב להשתתפות ולידטורים כאשר digestים של registry/parameter שונים.
2. **Phase M1 — Registries & Parameters**
   - להשיק registries של `ZkVerifierEntry`, `PedersenParams`, `PoseidonParams` עם governance ops, anchoring של genesis וניהול cache.
   - לחבר syscall לדרוש registry lookups, מזהי gas schedule, schema hashing ובדיקות גודל.
   - לספק פורמט payload מוצפן v1, וקטורי derivation למפתחות wallet ותמיכת CLI בניהול מפתחות confidential.
3. **Phase M2 — Gas & Performance**
   - ליישם gas schedule דטרמיניסטי, מונים per-block ו-harness של benchmark עם telemetry (verify latency, גדלי proof, mempool rejections).
   - לחזק CommitmentTree checkpoints, טעינת LRU ו-nullifier indices לעומסי ריבוי נכסים.
4. **Phase M3 — Rotation & Wallet Tooling**
   - לאפשר קבלת proofs multi-parameter ו-multi-version; לתמוך ב-activation/deprecation מונחה governance עם transition runbooks.
   - לספק זרימות הגירה ל-wallet SDK/CLI, תהליכי סריקה לאודיטורים וכלי reconciliation של spentness.
5. **Phase M4 — Audit & Ops**
   - לספק workflows למפתחות אודיטור, APIs של selective disclosure ו-runbooks תפעוליים.
   - לתזמן סקירה קריפטוגרפית/אבטחתית חיצונית ולפרסם ממצאים ב-`status.md`.

כל פאזה מעדכנת milestones ב-roadmap ובדיקות קשורות כדי לשמר ערבויות ביצוע דטרמיניסטי לרשת הבלוקצ'יין.
