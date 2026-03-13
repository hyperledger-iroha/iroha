---
lang: he
direction: rtl
source: docs/portal/docs/sorafs/reports/capacity-marketplace-validation.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

# צ'קליסט אימות שוק קיבולת SoraFS

**חלון סקירה:** 2026-03-18 -> 2026-03-24  
**בעלי התכנית:** Storage Team (`@storage-wg`), Governance Council (`@council`), Treasury Guild (`@treasury`)  
**היקף:** pipelines של onboarding ל-providers, זרימות adjudication של disputes ותהליכי reconciliation של treasury הנדרשים ל-GA SF-2c.

הצ'קליסט להלן חייב להיבדק לפני הפעלת השוק למפעילים חיצוניים. כל שורה מקשרת לראיות דטרמיניסטיות (tests, fixtures או documentation) שאודיטורים יכולים לשחזר.

## צ'קליסט קבלה

### Onboarding ל-providers

| בדיקה | אימות | ראיה |
|-------|------------|----------|
| ה-registry מקבל הצהרות קיבולת קנוניות | בדיקת אינטגרציה מפעילה את `/v2/sorafs/capacity/declare` דרך app API, ומאמתת טיפול בחתימות, קליטת metadata והעברה ל-node registry. | `crates/iroha_torii/src/routing.rs:7654` |
| ה-smart contract דוחה payloads לא תואמים | בדיקת יחידה מבטיחה ש-IDs של provider ושדות GiB מחויבים תואמים להצהרה החתומה לפני persisting. | `crates/iroha_core/src/smartcontracts/isi/sorafs.rs:3445` |
| ה-CLI מפיק artefacts onboarding קנוניים | ה-CLI harness כותב פלטי Norito/JSON/Base64 דטרמיניסטיים ומאמת round-trips כדי שהמפעילים יוכלו להכין הצהרות offline. | `crates/sorafs_car/tests/capacity_cli.rs:17` |
| מדריך המפעיל מתאר את זרימת הקבלה ו-guardrails של governance | התיעוד מפרט סכמת הצהרה, policy defaults ושלבי ביקורת עבור ה-council. | `../storage-capacity-marketplace.md` |

### פתרון disputes

| בדיקה | אימות | ראיה |
|-------|------------|----------|
| רשומות dispute נשמרות עם digest קנוני של payload | בדיקת יחידה רושמת dispute, מפענחת את ה-payload המאוחסן ומאמתת סטטוס pending כדי להבטיח דטרמיניזם של ledger. | `crates/iroha_core/src/smartcontracts/isi/sorafs.rs:1835` |
| מחולל disputes ב-CLI תואם לסכמה הקנונית | בדיקת CLI מכסה פלטי Base64/Norito וסיכומי JSON עבור `CapacityDisputeV1`, ומבטיחה שה-evidence bundles עוברים hash דטרמיניסטי. | `crates/sorafs_car/tests/capacity_cli.rs:455` |
| בדיקת replay מוכיחה דטרמיניזם של dispute/penalty | telemetry של proof-failure שמורצת פעמיים מפיקה snapshots זהים של ledger, credit ו-dispute כדי ש-slashes יהיו דטרמיניסטיים בין peers. | `crates/iroha_core/src/smartcontracts/isi/sorafs.rs:3430` |
| ה-runbook מתעד זרימת escalation ו-revocation | מדריך התפעול לוכד את workflow של ה-council, דרישות ה-evidence ונהלי rollback. | `../dispute-revocation-runbook.md` |

### Reconciliation של treasury

| בדיקה | אימות | ראיה |
|-------|------------|----------|
| ה-ledger accrual תואם להקרנת soak של 30 ימים | בדיקת soak חוצה חמישה providers לאורך 30 חלונות settlement, ומשווה את רשומות ה-ledger מול refererence payout הצפוי. | `crates/iroha_core/src/smartcontracts/isi/sorafs.rs:3000` |
| reconciliation של ledger export נרשמת מדי לילה | `capacity_reconcile.py` משווה ציפיות fee ledger מול XOR transfer exports שבוצעו, מפיקה מדדי Prometheus ומבצעת gating לאישור treasury דרך Alertmanager. | `scripts/telemetry/capacity_reconcile.py:1`,`docs/source/sorafs/runbooks/capacity_reconciliation.md:1`,`dashboards/alerts/sorafs_capacity_rules.yml:100` |
| dashboards של billing מציגים penalties ו-telemetry של accrual | יבוא Grafana מצייר accrual GiB-hour, מוני strikes ו-bonded collateral כדי לתת נראות ל-on-call. | `dashboards/grafana/sorafs_capacity_penalties.json:1` |
| הדוח המתפרסם שומר את מתודולוגיית soak ופקודות replay | הדוח מפרט את היקף ה-soak, פקודות ביצוע ו-hooks של observability עבור אודיטורים. | `./sf2c-capacity-soak.md` |

## הערות ביצוע

הפעילו מחדש את חבילת ה-validation לפני sign-off:

```bash
cargo test -p iroha_torii --features app_api -- capacity_declaration_handler_accepts_request
cargo test -p iroha_core -- register_capacity_declaration_rejects_provider_mismatch
cargo test -p iroha_core -- register_capacity_dispute_inserts_record
cargo test -p iroha_core -- capacity_dispute_replay_is_deterministic
cargo test -p iroha_core -- capacity_fee_ledger_30_day_soak_deterministic
cargo test -p sorafs_car --features cli --test capacity_cli
python3 scripts/telemetry/capacity_reconcile.py --snapshot <state.json> --ledger <ledger.ndjson> --warn-only
```

המפעילים צריכים לייצר מחדש payloads של בקשות onboarding/dispute עם `sorafs_manifest_stub capacity {declaration,dispute}` ולארכב את ה-bytes JSON/Norito שנוצרו לצד כרטיס ה-governance.

## artefacts לאישור

| Artefact | Path | blake2b-256 |
|----------|------|-------------|
| חבילת אישור onboarding של providers | `docs/examples/sorafs_capacity_marketplace_validation/2026-03-24_onboarding_signoff.md` | `8f41a745d8d94710fe81c07839651520429d4abea5729bc00f8f45bbb11daa4c` |
| חבילת אישור פתרון disputes | `docs/examples/sorafs_capacity_marketplace_validation/2026-03-24_dispute_signoff.md` | `c3ac3999ef52857170fedb83cddbff7733ef5699f8b38aea2e65ae507a6229f7` |
| חבילת אישור reconciliation של treasury | `docs/examples/sorafs_capacity_marketplace_validation/2026-03-24_treasury_signoff.md` | `0511aeed1f5607c329428cd49c94d1af51292c85134c10c3330c172b0140e8c6` |

שמרו את העותקים החתומים של artefacts אלה עם חבילת השחרור וקשרו אותם ברשומת השינוי של governance.

## אישורים

- Storage Team Lead — @storage-tl (2026-03-24)  
- Governance Council Secretary — @council-sec (2026-03-24)  
- Treasury Operations Lead — @treasury-ops (2026-03-24)
