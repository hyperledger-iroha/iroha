---
lang: he
direction: rtl
source: docs/examples/finance/repo_governance_packet_template.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 7fe5fb47af37f86d33bfa884dc920efbf66714bbe3535842a786755dd5649f65
source_last_modified: "2025-12-07T08:26:38.035018+00:00"
translation_last_reviewed: 2026-01-01
---

<div dir="rtl">

<!-- תרגום עברי ל-docs/examples/finance/repo_governance_packet_template.md -->

# תבנית חבילת ממשל Repo (Roadmap F1)

השתמשו בתבנית זו בעת הכנת חבילת הארטיפקטים הנדרשת לפי פריט ה-roadmap F1 (תיעוד וכלי מחזור חיי repo). המטרה היא לתת למבקרים קובץ Markdown אחד שמונה כל input, hash וחבילת ראיות כדי שמועצת הממשל תוכל לשחזר את הבייטים שהוזכרו בהצעה.

> העתיקו את התבנית לתיקיית הראיות שלכם (לדוגמה
> `artifacts/finance/repo/2026-03-15/packet.md`), החליפו את ה-placeholders, ובצעו commit/upload ליד הארטיפקטים המחושבים המוזכרים להלן.

## 1. מטאדאטה

| שדה | ערך |
|-------|-------|
| מזהה הסכם/שינוי | `<repo-yyMMdd-XX>` |
| הוכן על ידי / תאריך | `<desk lead> - 2026-03-15T10:00Z` |
| נבדק על ידי | `<dual-control reviewer(s)>` |
| סוג שינוי | `Initiation / Haircut update / Substitution matrix change / Margin policy` |
| Custodian(s) | `<custodian id(s)>` |
| הצעה/משאל מקושר | `<governance ticket id or GAR link>` |
| תיקיית ראיות | ``artifacts/finance/repo/<slug>/`` |

## 2. Payloads להוראות

תעדו את הוראות Norito ה-staged שעליהן ה-desk אישר via
`iroha app repo ... --output`. כל רשומה צריכה לכלול את hash הקובץ שהופק
ותיאור קצר של הפעולה שתוגש לאחר שההצבעה תאושר.

| פעולה | קובץ | SHA-256 | הערות |
|--------|------|---------|-------|
| Initiate | `instructions/initiate.json` | `<sha256>` | כולל את רגלי cash/collateral שאושרו ע"י desk + counterparty. |
| Margin call | `instructions/margin_call.json` | `<sha256>` | לוכד cadence + participant id שהפעיל את הקריאה. |
| Unwind | `instructions/unwind.json` | `<sha256>` | הוכחת reverse-leg לאחר קיום התנאים. |

```bash
# Example hash helper (repeat per instruction file)
sha256sum artifacts/finance/repo/<slug>/instructions/initiate.json       | tee artifacts/finance/repo/<slug>/hashes/initiate.sha256
```

## 2.1 אישורי Custodian (tri-party בלבד)

השלימו סעיף זה בכל פעם ש-repo משתמש ב-`--custodian`. חבילת הממשל חייבת לכלול אישור חתום מכל custodian וכן את hash הקובץ שמוזכר בסעיף 2.8 של `docs/source/finance/repo_ops.md`.

| Custodian | קובץ | SHA-256 | הערות |
|-----------|------|---------|-------|
| `<i105...>` | `custodian_ack_<custodian>.md` | `<sha256>` | SLA חתום המכסה חלון משמורת, חשבון ניתוב ואיש קשר ל-drill. |

> שמרו את האישור לצד הראיות האחרות (`artifacts/finance/repo/<slug>/`) כדי ש-
> `scripts/repo_evidence_manifest.py` ירשום את הקובץ באותו עץ עם ההוראות ה-staged
> וקבצי config. ראו
> `docs/examples/finance/repo_custodian_ack_template.md` לתבנית מוכנה התואמת לחוזה הראיות של הממשל.

## 3. קטע תצורה

הדביקו את בלוק TOML `[settlement.repo]` שיוחל על הקלאסטר (כולל
`collateral_substitution_matrix`). שמרו את ה-hash לצד הקטע כך שמבקרים יוכלו לאמת את מדיניות הריצה שהייתה פעילה כאשר אושרה עסקת repo.

```toml
[settlement.repo]
eligible_collateral = ["bond#wonderland", "note#wonderland"]
default_margin_percent = "0.025"

[settlement.repo.collateral_substitution_matrix]
"bond#wonderland" = ["bill#wonderland"]
```

`SHA-256 (config snippet): <sha256>`

### 3.1 Snapshots לתצורה לאחר אישור

לאחר השלמת משאל העם או הצבעת הממשל והפצת השינוי `[settlement.repo]`, לכדו snapshots של `/v2/configuration` מכל peer כדי שמבקרים יוכלו להוכיח שהמדיניות המאושרת חיה בכל הקלאסטר (ראו `docs/source/finance/repo_ops.md` סעיף 2.9 לתהליך הראיות).

```bash
mkdir -p artifacts/finance/repo/<slug>/config/peers
curl -fsSL https://peer01.example/v2/configuration       | jq '.'       > artifacts/finance/repo/<slug>/config/peers/peer01.json
```

| Peer / מקור | קובץ | SHA-256 | גובה בלוק | הערות |
|---------------|------|---------|--------------|-------|
| `peer01` | `config/peers/peer01.json` | `<sha256>` | `<block-height>` | Snapshot שנלכד מיד לאחר rollout התצורה. |
| `peer02` | `config/peers/peer02.json` | `<sha256>` | `<block-height>` | מאשר ש-`[settlement.repo]` תואם ל-TOML ה-staged. |

רשמו את ה-digests לצד מזהי ה-peer ב-`hashes.txt` (או סיכום מקביל) כדי שמבקרים יוכלו לעקוב אילו nodes קלטו את השינוי. ה-snapshots נמצאים תחת `config/peers/` לצד קטע ה-TOML וייאספו אוטומטית ע"י `scripts/repo_evidence_manifest.py`.

## 4. ארטיפקטים של בדיקות דטרמיניסטיות

צרפו את הפלטים האחרונים מ:

- `cargo test -p iroha_core -- repo_deterministic_lifecycle_proof_matches_fixture`
- `cargo test --package integration_tests --test repo`

רשמו נתיבי קבצים + hashes עבור חבילות הלוגים או JUnit XML שנוצרים ב-CI.

| ארטיפקט | קובץ | SHA-256 | הערות |
|----------|------|---------|-------|
| לוג proof של lifecycle | `tests/repo_lifecycle.log` | `<sha256>` | נלכד עם `--nocapture`. |
| לוג integration test | `tests/repo_integration.log` | `<sha256>` | כולל כיסוי substitution + cadence של margin. |

## 5. Snapshot של proof למחזור חיים

כל חבילה חייבת לכלול snapshot דטרמיניסטי שמיוצא מ-`repo_deterministic_lifecycle_proof_matches_fixture`. הריצו את ה-harness עם כפתורי export פעילים כדי שמבקרים יוכלו להשוות את מסגרת ה-JSON וה-digest מול ה-fixture שב-`crates/iroha_core/tests/fixtures/` (ראו `docs/source/finance/repo_ops.md` סעיף 2.7).

```bash
REPO_PROOF_SNAPSHOT_OUT=artifacts/finance/repo/<slug>/repo_proof_snapshot.json     REPO_PROOF_DIGEST_OUT=artifacts/finance/repo/<slug>/repo_proof_digest.txt     cargo test -p iroha_core       -- --exact smartcontracts::isi::repo::tests::repo_deterministic_lifecycle_proof_matches_fixture
```

או השתמשו ב-helper המוצמד כדי לייצר מחדש fixtures ולהעתיק אותם לחבילת הראיות בצעד אחד:

```bash
scripts/regen_repo_proof_fixture.sh --toolchain <toolchain>       --bundle-dir artifacts/finance/repo/<slug>
```

| ארטיפקט | קובץ | SHA-256 | הערות |
|----------|------|---------|-------|
| Snapshot JSON | `repo_proof_snapshot.json` | `<sha256>` | מסגרת lifecycle קנונית שהופקה ע"י proof harness. |
| Digest file | `repo_proof_digest.txt` | `<sha256>` | hex digest באותיות גדולות שמשתקף מ-`crates/iroha_core/tests/fixtures/repo_lifecycle_proof.digest`; לצרף גם אם אין שינוי. |

## 6. Evidence Manifest

צרו manifest לכל תיקיית הראיות כדי שמבקרים יוכלו לאמת hashes בלי לפרק את הארכיון. ה-helper משקף את ה-workflow המתואר ב-`docs/source/finance/repo_ops.md` סעיף 3.2.

```bash
python3 scripts/repo_evidence_manifest.py       --root artifacts/finance/repo/<slug>       --agreement-id <repo-identifier>       --output artifacts/finance/repo/<slug>/manifest.json
```

| ארטיפקט | קובץ | SHA-256 | הערות |
|----------|------|---------|-------|
| Evidence manifest | `manifest.json` | `<sha256>` | כללו את ה-checksum בכרטיס הממשל / הערות משאל העם. |

## 7. Snapshot טלמטריה ואירועים

ייצאו את רשומות `AccountEvent::Repo(*)` הרלוונטיות וכל דשבורד או CSV שהוזכרו ב-`docs/source/finance/repo_ops.md`. רשמו קבצים + hashes כאן כדי שמבקרים יוכלו להגיע ישירות לראיות.

| Export | קובץ | SHA-256 | הערות |
|--------|------|---------|-------|
| Repo events JSON | `evidence/repo_events.ndjson` | `<sha256>` | זרם Torii גולמי שמסונן לחשבונות desk. |
| Telemetry CSV | `evidence/repo_margin_dashboard.csv` | `<sha256>` | יצוא מ-Grafana באמצעות פאנל Repo Margin. |

## 8. אישורים וחתימות

- **חותמי dual-control:** `<names + timestamps>`
- **GAR / minutes digest:** `<sha256>` של PDF GAR חתום או העלאת minutes.
- **מיקום אחסון:** `governance://finance/repo/<slug>/packet/`

## 9. Checklist

סמנו כל פריט לאחר השלמה.

- [ ] Payloads של הוראות staged, hashed ומצורפים.
- [ ] Hash של snippet תצורה נרשם.
- [ ] לוגים של בדיקות דטרמיניסטיות נאספו + hashed.
- [ ] Snapshot lifecycle + digest יוצאו.
- [ ] Evidence manifest נוצר ו-hash נרשם.
- [ ] Exports של אירועים/טלמטריה נאספו + hashed.
- [ ] אישורי dual-control נשמרו.
- [ ] GAR/minutes הועלו; digest נרשם לעיל.

שמירה על תבנית זו לצד כל חבילה שומרת על DAG ממשל דטרמיניסטי ומספקת למבקרים manifest נייד להחלטות מחזור חיי repo.

</div>
