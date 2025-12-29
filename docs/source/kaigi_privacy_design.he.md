<!-- Hebrew translation of docs/source/kaigi_privacy_design.md -->

---
lang: he
direction: rtl
source: docs/source/kaigi_privacy_design.md
status: complete
translator: manual
---

<div dir="rtl">

# עיצוב הפרטיות וה-Relay של Kaigi

המסמך מתעד את תכנית ההתרחבות ממוקדת הפרטיות, הכוללת הוכחות השתתפות בידע-אפס ו-relays בסגנון onion תוך שמירה על דטרמיניזם ועל יכולת ביקורת של הלדג'ר.

## סקירה כללית

העיצוב מחולק לשלוש שכבות:

- **פרטיות רשימה (Roster)** – הסתרת זהויות המשתתפים on-chain תוך שמירה על הרשאות Host וחיוב עקביים.
- **אטימות שימוש** – מתן אפשרות ל-Hosts לרשום שימוש מדוד בלי לחשוף פרטים לכל מקטע.
- **Relay Overlay** – ניתוב חבילות טרנספורט דרך צמתים מרובי-קפיצות כדי שמשקיפי רשת לא ידעו מי מתקשר עם מי.

כל ההרחבות מבוססות Norito, רצות תחת ABI גרסה 1 וחייבות להתבצע דטרמיניסטית בכל חומרה.

## יעדים

1. קבלה/הוצאה של משתתפים באמצעות הוכחות ZK כך שהלדג'ר לא יחשוף מזהי חשבון גולמיים.
2. שמירה על אחריות חזקה: כל הצטרפות, עזיבה ואירוע שימוש חייבים להתאזן דטרמיניסטית.
3. אפשרות למסמכי Relay אופציונליים המתארים מסלולי onion עבור ערוצי שליטה/מידע וניתנים לביקורת on-chain.
4. שמירת fallback שקוף לחלוטין עבור פריסות שאינן זקוקות לפרטיות.
5. הימנעות ממזלג קשיח של מודל הנתונים באמצעות התפתחות `KaigiRecord` באופן תואם לאחור עם קודקי Norito.

## תקציר מודל איומים

- **אויבים:** משקיפי רשת (ISP), ולידטורים סקרנים, מפעילי Relay זדוניים ו-Hosts חצי-כנים.
- **נכסים מוגנים:** זהות המשתתף, עיתוי השתתפות, פרטי שימוש/חיוב לכל מקטע ומטא-דאטה של ניתוב.
- **הנחות:** ה-Hosts עדיין מכירים את קבוצת המשתתפים offline; עמיתי הלדג'ר מאמתים הוכחות דטרמיניסטית; Relay-ים לא אמינים אך מוגבלי קצבים; HPKE ו-SNARK קיימים בקוד הבסיס.

## שינויי מודל נתונים

כל הטיפוסים ב-`iroha_data_model::kaigi`.

```rust
pub struct KaigiParticipantCommitment {
    pub commitment: FixedBinary<32>,
    pub alias_tag: Option<String>,
}

pub struct KaigiParticipantNullifier {
    pub digest: FixedBinary<32>,
    pub issued_at_ms: u64,
}

pub struct KaigiRelayManifest {
    pub hops: Vec<KaigiRelayHop>,
    pub expiry_ms: u64,
}

pub struct KaigiRelayHop {
    pub relay_id: AccountId,
    pub hpke_public_key: FixedBinary<32>,
    pub weight: u8,
}
```

התוספות ל-`KaigiRecord`:

- `roster_commitments: Vec<KaigiParticipantCommitment>` – מחליף את רשימת `participants` כשהפרטיות פעילה. ניתן להחזיק את שתי הגרסאות בזמן הגירה.
- `nullifier_log: Vec<KaigiParticipantNullifier>` – גדלה רק בהוספה, עם חלון מתגלגל למניעת תפיחה.
- `relay_manifest: Option<KaigiRelayManifest>` – מניפסט מובנה ב-Norito כך שה-Hop-ים, מפתחות HPKE והמשקולות יישארו קנוניים.
- `privacy_mode: KaigiPrivacyMode` עם וריאנטים:

```rust
pub enum KaigiPrivacyMode {
    Transparent,
    ZkRosterV1,
}
```

`NewKaigi` מקבל שדות אופציונליים תואמים כך ש-Hosts יוכלו לבחור בפרטיות בעת יצירה.

תאימות Norito:

- שימוש ב-`#[norito(with = "...")]` לשמירה על קידוד קנוני (למשל סדרת Hop לפי מיקום).
- `KaigiRecord::from_new` מאותחל עם וקטורים ריקים ומעתיק מניפסט אם קיים.
- מבחני סריאליזציה יוודאו round-trip עבור שני מצבי הפרטיות.

## שינויים בממשק ההוראות

### `CreateKaigi`

- מאמת את `privacy_mode` מול הרשאות ה-Host.
- אם סופק `relay_manifest`, נדרש ≥3 קפיצות, משקולות חיוביות, מפתח HPKE בכל Hop וייחודיות.
- שומר לוגים ריקים של Commit/Nullifier.

### `JoinKaigi`

פרמטרים:

- `proof: ZkProof` – הוכחת Groth16 לכך שהמתקשר יודע `(account_id, domain_salt)` שמפיקים Poseidon commitment.
- `commitment: FixedBinary<32>`
- `nullifier: FixedBinary<32>`
- `relay_hint: Option<KaigiRelayHop>` – Override אופציונלי למשתתף.

צעדים:

1. במצב Transparent – fallback להתנהגות הקיימת.
2. אימות Groth16 מול `KAIGI_ROSTER_V1`.
3. בדיקת היעדר ה-nullifier בלוג.
4. הוספת commit/nullifier. אם יש `relay_hint`, עדכון מצב in-memory עבור המשתתף (לא נשמר on-chain).

### `LeaveKaigi`

- במצב שקוף: התנהגות קיימת.
- במצב פרטי:
  1. הוכחה שהקורא מחזיק Commitment ברשימה.
  2. Nullifier חדש להבטחת חד-פעמיות.
  3. הסרת הערכים ועדכון Audit log.

### שימוש (`ReportKaigiUsage`)

מרכז חיוב. מצב פרטי מחייב:

- קומיטמנט של `(segment_id, gas, bytes, duration)` למניעת גילוי פרטים.
- הוכחה שהקומיטמנט נוצר ע"י אחד המשתתפים דרך nullifier מתאים.
- אחסון רק סכומים/קונטרים ב-`KaigiUsageSummary`.

## מוטציות WSV / Norito

- `KaigiRecord` שומר את הרוסטר כקומיטמנטים; המנגנון השיקוף המסורתי נשאר כ-Fallback.
- nullifier log מוגבל גודל (למשל 4096 רשומות) כדי לאפשר סריקה דטרמיניסטית.
- Relay manifest מאוחסן במלואו ב-Norito ומקבל בדיקות קנוניות (הקפדה על מיון Hop-ים).

## ZK ויובלי אירועים

- `iroha_core::smartcontracts::isi::kaigi::privacy` מבצע כברירת מחדל ולידציה מלאה של הרוסטר. מאתר את `zk.kaigi_roster_join_vk`, ‏`zk.kaigi_roster_leave_vk` מקובץ התצורה, טוען את `VerifyingKeyRef` מה-WSV (כולל בדיקת מצב Active וזהות backend/circuit), מחייב חיוב byte ומעביר לבקנד ה-ZK.
- הפיצ'ר `kaigi_privacy_mocks` שומר מאמת מדמה דטרמיניסטי למבחנים/CI; אסור להפעילו בבילדי Release.
- בעת קומפילציה מתקבלת שגיאה אם `kaigi_privacy_mocks` מופעל ב-build שאינו בדיקה.
- המפעילים חייבים (1) לרשום את מפתחות המאמת דרך ממשל, (2) להגדיר `zk.kaigi_roster_join_vk`, ‏`zk.kaigi_roster_leave_vk`, ‏`zk.kaigi_usage_vk` בקובץ התצורה. עד אז, פעולות פרטיות נכשלות דטרמיניסטית.
- `crates/kaigi_zk` מספק כעת מעגלי Halo2 עבור הצטרפות/עזיבה ושימוש. מעגלי הרוסטר חושפים את שורש המרקל (ארבעה איברים של 64 סיביות בליטל אנדיאן) בקלט ציבורי כדי שה-Host יוכל לאמת מול שורש הרוסטר.
- קלטי מעגל Join: `(commitment, nullifier, domain_salt)` וקלט פרטי `account_id`. פלטים ציבוריים כוללים קומיטמנט, nullifier ושורש המרקל.
- דטרמיניזם: פרמטרי Poseidon, גרסאות מעגלים ואינדקסים נעולים. שינוי עתידי יוביל ל-`KaigiPrivacyMode::ZkRosterV2`.

## שכבת Relay בסגנון Onion

### רישום Relay

- Relay-ים נרשמים כמטא-דאטה על הדומיין `kaigi_relay::<relay_id>` עם מפתחות HPKE ומעמד רוחב פס.
- ההוראה `RegisterKaigiRelay` שומרת את התיאור, מפיקה אירוע `KaigiRelayRegistered` (עם טביעת HPKE ומעמד רוחב פס) ומאפשרת רוטציה דטרמיניסטית של מפתחות.
- הממשל יכול לנהל allowlists באמצעות כלי המטא-דאטה הקיימים.

### יצירת מניפסט

- ה-Hosts בונים נתיב מרובה קפיצות (מינימום 3) מתוך רשימת ה-Relays. המניפסט כולל את רצף ה-AccountId ואת מפתחות HPKE.
- `relay_manifest` on-chain מכיל את ההופס ותוקף; מפתחות אפמרליים וחלוקות session מתבצעים off-ledger עם HPKE.

### אותות ומדיה

- החלפת SDP/ICE ממשיכה דרך מטא-דאטה של Kaigi אך מוצפנת לכל Hop. הולידטורים רואים רק ciphertext.
- חבילות מדיה נוסעות דרך ה-Relays עם QUIC ומעטפות חתומות. כל Hop מפענח שכבה אחת כדי לזהות את הכתובת הבאה.

### Failover

- לקוחות עוקבים אחר בריאות Relay דרך משוב חתום (`kaigi_relay_feedback::<relay_id>`). כש Relay נופל, ה-Host מעדכן מניפסט ומפרסם אירוע `KaigiRelayManifestUpdated`.
- שינוי מניפסט מתבצע באמצעות `SetKaigiRelayManifest`, שמחליף או מנקה את הנתיב. ניקוי מפיק אירוע עם `hop_count = 0`.
- מדדי Prometheus (`kaigi_relay_registered_total`, ‏`kaigi_relay_manifest_updates_total`, ‏`kaigi_relay_manifest_hop_count`) מציגים churn וחלוקת hop.

## אירועים

הרחבת `DomainEvent`:

- `KaigiRosterSummary` – עם ספירות אנונימיות ושורש הרוסטר (או `None` במצב שקוף).
+- `KaigiRelayRegistered` – בעת רישום/רוטציה של Relay.
- `KaigiRelayManifestUpdated` – בעת שינוי מניפסט.
- `KaigiUsageSummary` – לאחר כל מקטע שימוש, מציג סכומים מצטברים.

האירועים מסורסלים ב-Norito וחושפים רק האשים וספירות.

כלי ה-CLI (`iroha kaigi …`) עוטפים כל ISI כך שמפעילים יוכלו לרשום סשנים, לעדכן רוסטרים ולתעד שימוש ללא צורך בבנייה ידנית של טרנזקציות. מניפסטים והוכחות נטענים מקבצי JSON/Hex ומתמסרים בנתיב ההגשה הרגיל, מה שמקל על סקריפטינג בסביבות Staging.

</div>
