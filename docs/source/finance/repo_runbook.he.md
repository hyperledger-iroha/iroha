---
lang: he
direction: rtl
source: docs/source/finance/repo_runbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: d5dd8e1b666be34bb9101898d355fe5e3c6efc32500c238c72a6ef9228c157f0
source_last_modified: "2026-01-22T15:38:30.662574+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Repo Settlement Runbook

מדריך זה מתעד את הזרימה הדטרמיניסטית עבור הסכמי ריפו והסכמי ריפו הפוכים ב-Iroha.
זה מכסה תזמור CLI, עוזרי SDK, וכפתורי הממשל הצפויים כדי שהמפעילים יוכלו
ליזום, להרחיק ולשחרר הסכמים מבלי לכתוב מטענים גולמיים של Norito. לממשל
רשימות ביקורת, לכידת ראיות והליכי הונאה/החזרה לאחור
[`repo_ops.md`](./repo_ops.md), שעומדת בפריט F1 במפת הדרכים.

## פקודות CLI

הפקודה `iroha app repo` מקבצת עוזרים ספציפיים לריפו:

```bash
# Stage an initiation instruction without submitting
iroha --config client.toml --output \
  repo initiate \
  --agreement-id daily_repo \
  --initiator <katakana-i105-account-id> \
  --counterparty <katakana-i105-account-id> \
  --custodian <katakana-i105-account-id> \
  --cash-asset 7EAD8EFYUx1aVKZPUU1fyKvr8dF1 \
  --cash-quantity 1000 \
  --collateral-asset 4fEiy2n5VMFVfi6BzDJge519zAzg \
  --collateral-quantity 1050 \
  --rate-bps 250 \
  --maturity-timestamp-ms 1704000000000 \
  --haircut-bps 1500 \
  --margin-frequency-secs 86400

# Generate the unwind leg
iroha --config client.toml --output \
  repo unwind \
  --agreement-id daily_repo \
  --initiator <katakana-i105-account-id> \
  --counterparty <katakana-i105-account-id> \
  --cash-asset 7EAD8EFYUx1aVKZPUU1fyKvr8dF1 \
  --cash-quantity 1005 \
  --collateral-asset 4fEiy2n5VMFVfi6BzDJge519zAzg \
  --collateral-quantity 1055 \
  --settlement-timestamp-ms 1704086400000

# Inspect the next margin checkpoint for an active agreement
iroha --config client.toml repo margin --agreement-id daily_repo

# Trigger a margin call when cadence elapses
iroha --config client.toml repo margin-call --agreement-id daily_repo
```

* `repo initiate` ו-`repo unwind` מכבדים את `--input/--output` כך שה-`InstructionBox` שנוצר
  ניתן להעביר מטענים לזרימות CLI אחרות או להגיש מיד.
* העבר `--custodian <account>` כדי לנתב בטחונות לאפוטרופוס תלת-צדדי. כאשר מושמט, ה
  הצד הנגדי מקבל את המשכון ישירות (ריפו דו-צדדי).
* `repo margin` מבצע שאילתות בספר החשבונות דרך `FindRepoAgreements` ומדווח על המרווח הצפוי הבא
  חותמת זמן (באלפיות שניות) לצד השאלה אם יש להמתין כעת להתקשרות חוזרת בשוליים.
* `repo margin-call` מוסיף הוראה `RepoMarginCallIsi`, מתעד את נקודת ביקורת השוליים
  פולט אירועים לכל המשתתפים. שיחות נדחות אם הקצב לא חלף או אם
  ההוראה מוגשת על ידי מי שאינו משתתף.

## עוזרי Python SDK

```python
from iroha_python import (
    create_torii_client,
    RepoAgreementRecord,
    RepoCashLeg,
    RepoCollateralLeg,
    RepoGovernance,
    TransactionConfig,
    TransactionDraft,
)

client = create_torii_client("client.toml")

cash = RepoCashLeg(asset_definition_id="7EAD8EFYUx1aVKZPUU1fyKvr8dF1", quantity="1000")
collateral = RepoCollateralLeg(
    asset_definition_id="4fEiy2n5VMFVfi6BzDJge519zAzg",
    quantity="1050",
    metadata={"isin": "ABC123"},
)
governance = RepoGovernance(haircut_bps=1500, margin_frequency_secs=86_400)

draft = TransactionDraft(TransactionConfig(chain_id="dev-chain", authority="<katakana-i105-account-id>"))
draft.repo_initiate(
    agreement_id="daily_repo",
    initiator="<katakana-i105-account-id>",
    counterparty="<katakana-i105-account-id>",
    cash_leg=cash,
    collateral_leg=collateral,
    rate_bps=250,
    maturity_timestamp_ms=1_704_000_000_000,
    governance=governance,
)
# ... additional instructions ...
envelope = draft.sign_with_keypair(my_keypair)
client.submit_transaction_envelope(envelope)

# Margin schedule
agreements = client.list_repo_agreements()
record = RepoAgreementRecord.from_payload(agreements[0])
next_margin = record.next_margin_check_after(at_timestamp_ms=now_ms)
```

* שני העוזרים מנרמלים כמויות מספריות ושדות מטא נתונים לפני הפעלת ה- PyO3 bindings.
* `RepoAgreementRecord` משקף את חישוב לוח הזמנים של זמן הריצה כך שאוטומציה מחוץ לפנקס יכולה
  לקבוע מתי יש לבצע התקשרות חוזרת מבלי לחשב מחדש את הקצב באופן ידני.

## התנחלויות DvP / PvP

הפקודה `iroha app settlement` משלבת הוראות מסירה מול תשלום ותשלום מול תשלום:

```bash
# Delivery leg first, then payment
iroha --config client.toml --output \
  settlement dvp \
  --settlement-id trade_dvp \
  --delivery-asset 4fEiy2n5VMFVfi6BzDJge519zAzg \
  --delivery-quantity 10 \
  --delivery-from <katakana-i105-account-id> \
  --delivery-to <katakana-i105-account-id> \
  --delivery-instrument-id US0378331005 \
  --payment-asset 7EAD8EFYUx1aVKZPUU1fyKvr8dF1 \
  --payment-quantity 1000 \
  --payment-from <katakana-i105-account-id> \
  --payment-to <katakana-i105-account-id> \
  --order payment-then-delivery \
  --atomicity all-or-nothing \
  --iso-reference-crosswalk /opt/iso/isin_crosswalk.json \
  --iso-xml-out trade_dvp.xml

# Cross-currency swap (payment-versus-payment)
iroha --config client.toml --output \
  settlement pvp \
  --settlement-id trade_pvp \
  --primary-asset 7EAD8EFYUx1aVKZPUU1fyKvr8dF1 \
  --primary-quantity 500 \
  --primary-from <katakana-i105-account-id> \
  --primary-to <katakana-i105-account-id> \
  --counter-asset 5tPkFK6s2zUcd1qUHyTmY7fDVa2n \
  --counter-quantity 460 \
  --counter-from <katakana-i105-account-id> \
  --counter-to <katakana-i105-account-id> \
  --iso-xml-out trade_pvp.xml
```

* כמויות רגל מקבלות ערכים אינטגרלים או עשרוניים ומאומתות מול דיוק הנכס.
* `--atomicity` מקבל `all-or-nothing`, `commit-first-leg`, או `commit-second-leg`. השתמש במצבים אלה
  עם `--order` כדי לבטא איזו רגל תישאר מחויבת אם העיבוד הבא נכשל (`commit-first-leg`
  שומר על הרגל הראשונה מונחת; `commit-second-leg` שומר על השני).
* קריאות CLI פולטות מטא נתונים ריקים של הוראות היום; השתמש בעוזרים של Python ברמת ההתנחלות
  יש לצרף מטא נתונים.
* ראה [`settlement_iso_mapping.md`](./settlement_iso_mapping.md) למיפוי שדות ISO 20022 ש
  תומך בהוראות אלה (`sese.023`, `sese.025`, `colr.007`, `pacs.009`, `camt.054`).
* עוברים את `--iso-xml-out <path>` כדי שה-CLI ישדר תצוגה מקדימה של XML קנונית לצד Norito
  הוראה; הקובץ עוקב אחר המיפוי שלמעלה (`sese.023` עבור DvP, `sese.025` עבור PvP`). זוג את
  דגל עם `--iso-reference-crosswalk <path>` כך שה-CLI מאמת את `--delivery-instrument-id` מול
  אותה תמונת מצב שבה Torii משתמשת במהלך הקבלה בזמן ריצה.

עוזרי פייתון משקפים את משטח ה-CLI:

```python
from iroha_python import (
    SettlementLeg,
    SettlementPlan,
    SettlementExecutionOrder,
    TransactionConfig,
    TransactionDraft,
)

draft = TransactionDraft(TransactionConfig(chain_id="dev-chain", authority="<katakana-i105-account-id>"))
delivery = SettlementLeg(
    asset_definition_id="4fEiy2n5VMFVfi6BzDJge519zAzg",
    quantity="10",
    from_account="<katakana-i105-account-id>",
    to_account="<katakana-i105-account-id>",
    metadata={"isin": "ABC123"},
)
payment = SettlementLeg(
    asset_definition_id="7EAD8EFYUx1aVKZPUU1fyKvr8dF1",
    quantity="1000",
    from_account="<katakana-i105-account-id>",
    to_account="<katakana-i105-account-id>",
)
plan = SettlementPlan(order=SettlementExecutionOrder.PAYMENT_THEN_DELIVERY)

draft.settlement_dvp("trade_dvp", delivery, payment, plan=plan, metadata={"desk": "rates"})
draft.settlement_pvp(
    "trade_pvp",
    SettlementLeg(
        asset_definition_id="7EAD8EFYUx1aVKZPUU1fyKvr8dF1",
        quantity="500",
        from_account="<katakana-i105-account-id>",
        to_account="<katakana-i105-account-id>",
    ),
    SettlementLeg(
        asset_definition_id="5tPkFK6s2zUcd1qUHyTmY7fDVa2n",
        quantity="460",
        from_account="<katakana-i105-account-id>",
        to_account="<katakana-i105-account-id>",
    ),
)
```

## דטרמיניזם וציפיות ממשל

הוראות ריפו מסתמכות אך ורק על סוגים מספריים מקודדים ב-Norito והסוגים המשותפים
לוגיקה `RepoGovernance::with_defaults`. זכור את האינווריאנטים הבאים:* כמויות מסודרות עם ערכי `NumericSpec` דטרמיניסטיים: שימוש במזומן
  `fractional(2)` (שני מקומות עשרוניים), רגלי בטחונות משתמשות ב-`integer()`. אל תגיש
  ערכים בדיוק רב יותר - שומרי זמן ריצה ידחו אותם ועמיתים יתבדו.
* החזרות משולשות מחזיקות את מזהה חשבון האפוטרופוס ב-`RepoAgreement`. אירועי מחזור חיים ושוליים
  פולט מטען `RepoAccountRole::Custodian` כדי שהאפוטרופוסים יוכלו להירשם ולהתאים את המלאי.
* התספורות מוצמדות ל-10000bps (100%) ותדרי השוליים הם שניות שלמות. לספק
  פרמטרי ממשל באותן יחידות קנוניות כדי להישאר מיושרים עם ציפיות זמן הריצה.
*חותמות זמן הן תמיד מילי-שניות יוניקס. כל העוזרים מעבירים אותם ללא שינוי ל-Norito
  מטען כך שעמיתים מוצאים לוחות זמנים זהים.
* הוראות ייזום ושחרור השתמשו מחדש באותו מזהה הסכם. זמן הריצה דוחה
  שכפול תעודות זהות והתנתקות עבור הסכמים לא ידועים; עוזרי CLI/SDK מציגים את השגיאות הללו מוקדם.
* `repo margin`/`RepoAgreementRecord::next_margin_check_after` מחזירים את הקצב הקנוני. תמיד
  עיין בתמונת מצב זו לפני הפעלת התקשרויות חוזרות כדי להימנע מהפעלה חוזרת של לוחות זמנים מיושנים.