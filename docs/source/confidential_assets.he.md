<!-- Hebrew translation of docs/source/confidential_assets.md -->

---
lang: he
direction: rtl
source: docs/source/confidential_assets.md
status: complete
translator: manual
---

<div dir="rtl">

# נכסים חסויים ותכנון העברות ZK

## מניע
- לספק זרימות נכסים ממוסכות לפי opt-in כדי שדומיינים ישמרו פרטיות טרנזקציונית מבלי לשנות את המחזור השקוף.
- להבטיח ביצוע דטרמיניסטי על פני חומרי Validator הטרוגניים ולהשאיר תאימות ל-ABI v1 של Norito/Kotodama.
- להעניק למבקרים/מפעילים שליטה על מחזור החיים (הפעלה, סיבוב, ביטול) של מעגלים ומערכי פרמטרים קריפטוגרפיים.

## מודל איומים
- Validators מוגדרים כחסרי-זדון אך סקרנים: מבצעים קונצנזוס נאמנה אך מנסים לצפות במצב/לוג.
- משקיפי רשת רואים נתוני בלוקים וטרנזקציות מגוססות; אין הנחת ערוץ פרטי.
- מחוץ לטווח: ניתוח תעבורה חיצוני, יריב קוונטי (מנוהל במפת PQ), התקפות זמינות.

## סקירה
- נכסים רשאים להצהיר על *מאגר ממוסך* בנוסף ליתרות שקופות; מחזור ממוסך מיוצג באמצעות קומיטמנטים קריפטוגרפיים.
- Notes מכילים `(asset_id, amount, recipient_view_key, blinding, rho)`:
  - קומיטמנט: `Comm = Pedersen(params_id || asset_id || amount || recipient_view_key || blinding)`.
  - Nullifier: `Null = Poseidon(domain_sep || nk || rho || asset_id || chain_id)`.
  - Payload מוצפן: `enc_payload = AEAD_XChaCha20Poly1305(ephemeral_shared_key, note_plaintext)`.
- טרנזקציות מעבירות מטען Norito `ConfidentialTransfer` עם:
  - קלטים ציבוריים: עוגן מרקל, nullifiers, קומיטמנטים חדשים, asset id, גרסת מעגל.
  - מטענים מוצפנים למקבלים ומבקרים אופציונליים.
  - הוכחת אפס-ידע ששומרת ערך, בעלות והרשאה.
- מפתחות בקרת־אימות ופרמטרים מנוהלים ברשומות על השרשרת עם חלונות הפעלה; צמתים מסרבים להוכחות שמפנות לזיהויים לא מוכרים/מבוטלים.
- כותרות קונצנזוס מתחייבות לדיג'סט תכונות חסוי; בלוקים מתקבלים רק אם הדיג'סט תואם את מצב הרישום.
- בניית הוכחות מבוססת Halo2 (Plonkish) ללא trusted setup; Groth16 וכדומה אינם נתמכים ב-v1.

<a id="node-capability-negotiation"></a>
## התחייבויות קונצנזוס ושערי יכולת
- כותרת בלוק חושפת `conf_features = { vk_set_hash, poseidon_params_id, pedersen_params_id, conf_rules_version }`. הדיג'סט נכנס ל-hash וקבלת בלוק מותנית בהשוואה לרישום המקומי.
- ממשל יכול להגדיר `next_conf_features` עם `activation_height`; עד לגובה זה יש להמשיך לשדר דיג'סט קודם.
- צמתי Validator חייבים לפעול עם `confidential.enabled = true` ו-`assume_valid = false`; בדיקת אתחול מונעת הצטרפות אם התנאים לא מתקיימים או אם הדיג'סט שונה.
- Handshake של P2P נושא `{ enabled, assume_valid, conf_features }`. חוסר התאמה ⇒ `HandshakeConfidentialMismatch`.
- תוצאות תאימות בין Validators, צופים וצמתים ישנים מכוסות במטריצת היכולות (ראה [משא ומתן יכולות](#node-capability-negotiation)).
- צופות שאינן Validator רשאיות להפעיל `assume_valid = true` כדי לעבד דלתות but without affecting safety.

## מדיניות נכסים
- כל הגדרת נכס כוללת `AssetConfidentialPolicy`:
  - `TransparentOnly`: ברירת מחדל; רק הוראות שקופות, הוראות ממוסכות נדחות.
  - `ShieldedOnly`: כל ההנפקות/העברות חייבות להיות חסויות; `RevealConfidential` אסור.
  - `Convertible`: holder-ים רשאים להמיר בין שקוף לממוסך באמצעות הוראות on/off ramp.
- מדיניות כפופה ל-FSM מוגבל:
  - `TransparentOnly → Convertible`
  - `TransparentOnly → ShieldedOnly` (צריך חלון מעבר)
  - `Convertible → ShieldedOnly` (עיכוב מינימלי)
  - `ShieldedOnly → Convertible` (דורש תכנית הגירה)
  - `ShieldedOnly → TransparentOnly` אסור אם המאגר לא ריק או ללא תכנית שחרור).
- ממשל קובע `pending_transition` באמצעות `ScheduleConfidentialPolicyTransition` ויכול לבטל עם `CancelConfidentialPolicyTransition`.

## הוראות וחישוב גז
- הוראות on/off ramp: `ConfidentialMint`, ‏`ConfidentialTransfer`, ‏`ConfidentialBurn`, ‏`RevealConfidential`, ‏`ShieldConfidential`.
- תעריף גז: Halo2 בסיס 250,000 + 2,000 לכל קלט ציבורי; 5 גז לבייט הוכחה, 300 לכל nullifier, ‏500 לכל קומיטמנט.
- מגבלות: `max_proof_size_bytes = 262_144`, ‏`max_nullifiers_per_tx = 8`, ‏`max_commitments_per_tx = 8`, ‏`max_confidential_ops_per_block = 256`, ‏`verify_timeout_ms = 750`, ‏`max_anchor_age_blocks = 10_000`.
- חריגה ⇒ כישלון דטרמיניסטי ללא שינוי Ledger. סינון מוקדם בודק `vk_id`, אורך הוכחה וגיל עוגן.

## CLI וכלים
- הפקודות: `confidential create-keys`, ‏`confidential send`, ‏`confidential export-view-key`, כלי auditor, ו-`iroha app zk envelope`.
- Torii: ‏`POST /v2/confidential/derive-keyset` מחזיר היררכיית מפתחות ב-hex/base64.
- היררכיית מפתח: `sk_spend` → `nk`, ‏`ivk`, ‏`ovk`, ‏`fvk`.
- Payloadים מוצפנים עם AEAD XChaCha20-Poly1305; ניתן לצרף מפתחות צופים בהתאם למדיניות נכס.

## פרמטרים ו-Calibration
- Benchmarks ב-`docs/source/confidential_assets_calibration.md`; `verify_timeout_ms` בלתי חורג.
- התחזוקה כולל סיבוב מערכי פרמטרים (`vk_set_hash`) ותיעוד ב-`confidential/calibration`.

## ממשל ורישומים
- `ConfidentialVerifierRegistry` קולט מפתחות אימות עם חלונות הפעלה/ביטול.
- Validators מאמתים שההוכחות מפנות לאינדקס תקף, ואם לא — `UnknownVerifier`.
- אי התאמות דיג'סט מובילות לדחיית handshake ובלוקים (`ConfidentialFeatureDigestMismatch`).

## תאימות רשת
- Handshake משווה `{ enabled, assume_valid, conf_features }`.
- צמתים ללא verifier רשאים לצרוך בלוקים אך אינם מצטרפים לקונצנזוס.
- לאחר התאמת רגיסטר/פרמטרים או תזמון `next_conf_features`, handshake מצליח.

---

המסמך ממשיך לפרט הוראות, הגנות DoS, אינטגרציית נוד capabilities ועוד כפי שמופיע במקור. מפעילים ומפתחים צריכים להתייחס גם ל-`confidential_assets_calibration.md` ו-`handshake_matrix` בזמן יישום ותחזוקה של התכונות החסויות.

</div>
