---
lang: he
direction: rtl
source: docs/source/nexus_public_lanes.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: f9bb3a13cec7d80bfd1729709eb0744a5a062954002ada5d48608f62f8907668
source_last_modified: "2025-12-08T18:48:53.874766+00:00"
translation_last_reviewed: 2026-01-01
---

<div dir="rtl">

<!-- תרגום עברי עבור docs/source/nexus_public_lanes.md -->

# הימור מסלולים ציבוריים ב‑Nexus (NX-9)

סטטוס: 🈺 בתהליך → **runtime + מסמכי מפעילים מיושרים** (אפריל 2026)
בעלים: Economics WG / Governance WG / Core Runtime
הפניה לרודמפ: NX-9 – Public lane staking & reward module

מסמך זה מתאר את מודל הנתונים הקנוני, משטח ההוראות, בקרות הממשל וההוקים התפעוליים
לתוכנית ה‑public‑lane staking של Nexus. היעד הוא לאפשר למאמתים ללא הרשאה להצטרף
למסלולים ציבוריים, לבצע bond ל‑stake, לשרת בלוקים ולקבל תגמולים, תוך שמירה על
מנופי slashing/runbook דטרמיניסטיים תחת ממשל.

תשתית הקוד נמצאת כעת ב:

- טיפוסי מודל נתונים: `crates/iroha_data_model/src/nexus/staking.rs`
- הגדרות ISI: `crates/iroha_data_model/src/isi/staking.rs`
- Stub של core executor (מחזיר שגיאת guard דטרמיניסטית עד שתגיע לוגיקת NX-9):
  `crates/iroha_core/src/smartcontracts/isi/staking.rs`

Torii/SDKs יכולים להתחיל לחווט את מטעני Norito לפני מימוש מלא של runtime; הוראות ההימור
נועלות כעת את asset ההימור המוגדר על ידי משיכה מ‑`stake_account`/`staker` אל חשבון escrow
קשור (`nexus.staking.stake_escrow_account_id`). Slashes מחייבים את ה‑escrow ומזכים את ה‑sink
המוגדר (`nexus.staking.slash_sink_account_id`), ו‑unbonds מחזירים כספים לחשבון המקורי לאחר תום הטיימר.

## 1. מצב ה‑ledger והטיפוסים

### 1.1 רשומות מאמתים

`PublicLaneValidatorRecord` עוקב אחר המצב הקנוני של כל מאמת:

| שדה | תיאור |
|-----|-------|
| `lane_id: LaneId` | המסלול שהמאמת משרת. |
| `validator: AccountId` | החשבון החותם על הודעות קונצנזוס. |
| `stake_account: AccountId` | החשבון שמספק self‑bond (יכול להיות שונה מזהות המאמת). |
| `total_stake: Numeric` | self stake + האצלות מאושרות. |
| `self_stake: Numeric` | ה‑stake שסופק על ידי המאמת. |
| `metadata: Metadata` | אחוז עמלה, מזהי טלמטריה, דגלי סמכות שיפוט, פרטי קשר. |
| `status: PublicLaneValidatorStatus` | מחזור חיים (pending/active/jailed/exiting/etc.). המטען של `PendingActivation` מכיל את ה‑epoch היעד. |
| `activation_epoch: Option<u64>` | ה‑epoch שבו המאמת הופעל (נקבע בהפעלה). |
| `activation_height: Option<u64>` | גובה הבלוק שנרשם בעת ההפעלה. |
| `last_reward_epoch: Option<u64>` | ה‑epoch האחרון ששילם תגמול. |

`PublicLaneValidatorStatus` מונה את שלבי מחזור החיים:

- `PendingActivation(epoch)` — ממתין ל‑epoch ההפעלה שקבעה הממשל; מטען הטופס שומר את
  ה‑epoch המוקדם ביותר שנגזר מ‑`epoch_length_blocks`.
- `Active` — משתתף בקונצנזוס ויכול לקבל תגמולים.
- `Jailed { reason }` — מושעה זמנית (downtime, הפרת טלמטריה וכו').
- `Exiting { releases_at_ms }` — unbonding; התגמולים מפסיקים להצטבר.
- `Exited` — הוסר מהסט.
- `Slashed { slash_id }` — אירוע slashing נרשם לצרכי ביקורת.

מטא‑דאטה של הפעלה הוא מונוטוני: `activation_epoch`/`activation_height` נקבעים בפעם הראשונה
שמואמת pending הופך active, וכל ניסיון להפעיל מחדש ב‑epoch/height מוקדם יותר נדחה.
מאמתים pending מקודמים אוטומטית בתחילת הבלוק הראשון שעומד בגבול המתוזמן, ומונה
המדדים (`nexus_public_lane_validator_activation_total`) מתעד את הקידום יחד עם שינוי הסטטוס.

פריסות permissioned משאירות את roster של genesis peers פעיל גם לפני שקיים stake של מאמת
public‑lane: כל עוד יש מפתחות קונצנזוס פעילים, runtime חוזר ל‑genesis peers לסט המאמתים.
כך נמנע bootstrap deadlock בזמן שה‑staking admission מושבת או בתהליך rollout.

### 1.2 חלקי stake ו‑unbonding

Delegators (וכן מאמתים שמגדילים את ה‑bond שלהם) מיוצגים באמצעות `PublicLaneStakeShare`:

- `bonded: Numeric` — סכום bonded פעיל.
- `pending_unbonds: BTreeMap<Hash, PublicLaneUnbonding>` — משיכות ממתינות, ממופות לפי `request_id` שסיפק הלקוח.
- `metadata` מאחסן רמזי UX/back‑office (למשל מספרי התייחסות לדסק קסטודיה).

`PublicLaneUnbonding` מחזיק את לוח המשיכות הדטרמיניסטי (`amount`, `release_at_ms`). Torii חושף
כעת את ה‑shares החיים והמשיכות הממתינות דרך `GET /v1/nexus/public_lanes/{lane}/stake` כדי
שארנקים יציגו טיימרים בלי RPCs ייעודיים.

Hooks של מחזור חיים (נאכפים ב‑runtime):

- רשומות `PendingActivation(epoch)` עוברות אוטומטית ל‑`Active` כאשר ה‑epoch הנוכחי מגיע ל‑`epoch`.
  ההפעלה רושמת `activation_epoch` ו‑`activation_height`, ו‑regressions נדחות גם באוטו‑הפעלה וגם
  בקריאות `ActivatePublicLaneValidator` מפורשות.
- רשומות `Exiting(releases_at_ms)` עוברות ל‑`Exited` כאשר חותמת הזמן של הבלוק חוצה את `releases_at_ms`,
  ומנקות שורות stake-share כדי להחזיר קיבולת בלי ניקוי ידני.
- רישום תגמולים דוחה shares של מאמתים אם המאמת אינו `Active`, כדי למנוע צבירת payouts עבור
  מאמתים pending/exiting/jailed.

### 1.3 רשומות תגמול

הפצות תגמול משתמשות ב‑`PublicLaneRewardRecord` וב‑`PublicLaneRewardShare`:

```norito
{
  "lane_id": 1,
  "epoch": 4242,
  "asset": "xor#wonderland",
  "total_reward": "250.0000",
  "shares": [
    { "account": "validator@lane", "role": "Validator", "amount": "150" },
    { "account": "delegator@lane", "role": "Nominator", "amount": "100" }
  ],
  "metadata": {
    "telemetry_epoch_root": "0x4afe…",
    "distribution_tx": "0xaabbccdd"
  }
}
```

הרשומות מספקות ראיות דטרמיניסטיות לכל payout עבור מבקרים ולוחות בקרה. מבנה התגמול
זורם ל‑ISI `RecordPublicLaneRewards`.

Guardrails של runtime:

- Nexus builds חייבים להיות פעילים; offline/stub builds דוחים רישום תגמולים.
- Reward epochs מתקדמים באופן מונוטוני לכל lane; epochs ישנים או כפולים נדחים.
- Reward assets חייבים להתאים ל‑fee sink המוגדר (`nexus.fees.fee_sink_account_id` /
  `nexus.fees.fee_asset_id`) ויתרת ה‑sink חייבת לכסות במלואה את `total_reward`.
- כל share חייב להיות חיובי ולכבד את הספסיפיקציה המספרית של ה‑asset; סכומי ה‑shares חייבים
  להשתוות ל‑`total_reward`.

## 2. קטלוג הוראות

כל ההוראות נמצאות תחת `iroha_data_model::isi::staking`. הן נגזרות כ‑Norito encoders/decoders כדי
ש‑SDKs יוכלו לשלוח payloads בלי codecs מותאמים.

### 2.1 `RegisterPublicLaneValidator`

רישום מאמת ובונד ראשוני:

```norito
{
  "lane_id": 1,
  "validator": "validator@lane",
  "stake_account": "validator@lane",
  "initial_stake": "150000",
  "metadata": {
    "commission_bps": 750,
    "jurisdiction": "JP",
    "telemetry_id": "val-01"
  }
}
```

כללי אימות:

- `initial_stake` >= `min_self_stake` (פרמטר ממשל).
- Metadata חייבת לכלול hooks של קשר/טלמטריה לפני ההפעלה.
- הממשל מאשר/דוחה את הרשומה; עד אז הסטטוס הוא `PendingActivation` וה‑runtime מקדם את
  המאמת ל‑`Active` בגבול ה‑epoch הבא לאחר שה‑epoch היעד הושג.

### 2.2 `BondPublicLaneStake`

בונד של stake נוסף (self‑bond של מאמת או תרומת delegator).

שדות מפתח: `staker`, `amount`, ו‑metadata אופציונלית. ה‑runtime חייב לאכוף מגבלות
ספציפיות ל‑lane (`max_delegators`, `min_bond`, `commission caps`).

### 2.3 `SchedulePublicLaneUnbond`

מתחיל טיימר unbonding. שולחים מספקים `request_id` דטרמיניסטי (המלצה: `blake2b(invoice)`),
`amount` ו‑`release_at_ms`. ה‑runtime חייב לוודא `amount` <= bonded stake ולתחום את
`release_at_ms` לפי תקופת ה‑unbonding המוגדרת.

### 2.4 `FinalizePublicLaneUnbond`

לאחר שהטיימר מסתיים, ISI זה משחרר את ה‑stake הממתין ומחזיר אותו ל‑`staker`. ה‑executor
מאמת את request id, בודק ש‑unlock timestamp נמצא בעבר, מפיק עדכון `PublicLaneStakeShare`
ומקליט טלמטריה.

### 2.5 `SlashPublicLaneValidator`

המשל משתמש בהוראה זו כדי לחייב stake ולכלוא/להוציא מאמתים.

- `slash_id` קושר את האירוע לטלמטריה + תיעוד אירוע.
- `reason_code` הוא מחרוזת enum יציבה (למשל `double_sign`, `downtime`, `safety_violation`).
- `metadata` שומר hashes של evidence bundles, הפניות runbook, או מזהי רגולטור.

Slashes מחלחלים ל‑delegators בהתאם למדיניות הממשל (איבוד פרופורציונלי או validator-first).
לוגיקת runtime תפיק הערות `PublicLaneRewardRecord` כאשר NX-9 יוטמע.

### 2.6 `RecordPublicLaneRewards`

רישום payout עבור epoch. שדות:

- `reward_asset`: ה‑asset שמחולק (ברירת מחדל `xor#nexus`).
- `total_reward`: סכום minted/transferred.
- `shares`: וקטור של `PublicLaneRewardShare`.
- `metadata`: הפניות ל‑payout transactions, root hashes או dashboards.

ISI זה idempotent לכל `(lane_id, epoch)` ומהווה בסיס לחשבונאות לילה.

## 3. תפעול, מחזור חיים וכלים

- **Lifecycle + modes:** stake-elected lanes מופעלות דרך
  `nexus.staking.public_validator_mode = stake_elected`, בעוד lanes מוגבלות נשארות
  admin-managed (`nexus.staking.restricted_validator_mode = admin_managed`). פריסות permissioned
  משאירות genesis peers פעילים עד שיש stake; עבור stake-elected lanes אנו עדיין דורשים peer
  רשום עם מפתח קונצנזוס פעיל ב‑commit topology לפני הצלחת `RegisterPublicLaneValidator`.
  genesis fingerprints ו‑`use_stake_snapshot_roster` קובעים אם runtime מפיק roster מ‑stake snapshots
  או נופל חזרה ל‑genesis peers.
- **Activation/exit operations:** רישומים נכנסים ל‑`PendingActivation` ומקודמים אוטומטית בבלוק הראשון
  שה‑epoch שלו מגיע לגבול (`epoch_length_blocks`). מפעילים יכולים גם לקרוא ל‑`ActivatePublicLaneValidator`
  אחרי הגבול כדי לכפות קידום. יציאות מעבירות מאמתים ל‑`Exiting(release_at_ms)` ומשחררות קיבולת רק כאשר
  חותמת זמן הבלוק מגיעה ל‑`release_at_ms`; רישום מחדש אחרי slash עדיין דורש יציאה כדי שהרשומה תסומן
  `Exited` והקיבולת תוחזר. בדיקות קיבולת משתמשות ב‑`nexus.staking.max_validators` ופועלות לאחר
  finalizer היציאה, כך שיציאות עתידיות חוסמות רישומים חדשים עד לסיום הטיימר.
- **Config knobs:** `nexus.staking.min_validator_stake`, `nexus.staking.stake_asset_id`,
  `nexus.staking.stake_escrow_account_id`, `nexus.staking.slash_sink_account_id`,
  `nexus.staking.unbonding_delay`, `nexus.staking.withdraw_grace`, `nexus.staking.max_validators`,
  `nexus.staking.max_slash_bps`, `nexus.staking.reward_dust_threshold`, וה‑mode switches לעיל. העבירו
  אותם דרך `iroha_config::parameters::actual::Nexus` וחשפו ב‑`status.md` לאחר אישור ערכי GA.
- **Torii/CLI quickstart:**
  - `iroha nexus lane-report --summary` מציג רשומות קטלוג lanes, readiness של manifests, ומצבי מאמתים
    (stake-elected לעומת admin-managed) כדי לאשר אם staking admission פעיל עבור lane.
  - `iroha_cli nexus public-lane validators --lane <id> [--summary] [--address-format {ih58,compressed}]`
    מציג סמני lifecycle/activation (pending target epoch, `activation_epoch` / `activation_height`,
    exit release, slash id) לצד stake bonded/self.
    `iroha_cli nexus public-lane stake --lane <id> [--validator account@domain] [--summary]` משקף את
    endpoint `/stake` עם רמזי pending-unbond לכל זוג `(validator, staker)`.
  - Torii snapshots ל‑dashboards ו‑SDKs:
    - `GET /v1/nexus/public_lanes/{lane}/validators` – metadata, status
      (`PendingActivation`/`Active`/`Exiting`/`Exited`/`Slashed`), activation epoch/height,
      release timers, bonded stake, last reward epoch.
      `address_format=ih58|compressed` שולט בהצגת literals (IH58 מועדף; compressed (`snx1`) הוא אפשרות שנייה ל-Sora בלבד).
    - `GET /v1/nexus/public_lanes/{lane}/stake` – stake shares (`validator`,
      `staker`, bonded amount) בתוספת pending unbond timers. `?validator=account@domain`
      מסנן את התגובה ל‑dashboards שממוקדים במאמת יחיד; `address_format` חל על כל literals.
  - Lifecycle ISIs משתמשים בנתיב טרנזקציה סטנדרטי (Torii `/v1/transactions`
    או CLI instruction pipeline). דוגמאות payloads של Norito JSON:

    ```jsonc
    [
      { "ActivatePublicLaneValidator": { "lane_id": 1, "validator": "validator@nexus" } },
      {
        "ExitPublicLaneValidator": {
          "lane_id": 1,
          "validator": "validator@nexus",
          "release_at_ms": 1730000000000
        }
      }
    ]
    ```
- **Telemetry + runbooks:** המטריקות חושפות מספרי מאמתים, stake bonded/pending, reward totals, ו‑slash counters
  תחת משפחת `nexus_public_lane_*`. חברו dashboards לאותה קבוצת נתונים שמשמשת את NX-9 acceptance tests כדי
  להשאיר validator deltas ו‑reward/slash evidence ברי ביקורת. הוראות slashing נשארות governance-only;
  רישום תגמולים חייב להוכיח payout totals (hash של batch payout).

## 4. יישור לרודמפ

- ✅ Runtime ו‑WSV storages מממשים את מחזור חיי המאמתים של NX-9; בדיקות רגרסיה מכסות timing של activation,
  prerequisites של peers, exits מאוחרים ו‑re-registration לאחר slashes.
- ✅ Torii חושף `/v1/nexus/public_lanes/{lane}/{validators,stake,rewards/pending}` ב‑Norito JSON כדי ש‑SDKs
  ו‑dashboards יעקבו אחרי מצב lane ללא custom RPCs.
- ✅ ה‑config knobs וה‑telemetry מתועדים; deployments מעורבים משאירים lanes stake-elected ו‑admin-managed
  מבודדות כך שה‑validator rosters נשארים דטרמיניסטיים.

</div>

### 2.7 `CancelConsensusEvidencePenalty`

Cancels consensus slashing before the delayed penalty applies.

- `evidence`: the Norito-encoded `Evidence` payload that was recorded in `consensus_evidence`.
- The record is marked `penalty_cancelled` and `penalty_cancelled_at_height`, preventing slashing when `slashing_delay_blocks` elapses.
