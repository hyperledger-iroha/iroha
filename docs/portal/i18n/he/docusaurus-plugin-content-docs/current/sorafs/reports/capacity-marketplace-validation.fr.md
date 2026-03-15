---
lang: he
direction: rtl
source: docs/portal/docs/sorafs/reports/capacity-marketplace-validation.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
כותרת: Validation du marche de capacite SoraFS
תגיות: [SF-2c, קבלה, רשימת תיוג]
תקציר: רשימת רשימת קבלה לכניסה לספקים, זרימת משפט והתאמה בין תנאי הסף לבלתי פעילים כלליים למארשה דה קיבולת SoraFS.
---

# רשימת אימות למארשה דה קיבולת SoraFS

**Fenetre de revue:** 2026-03-18 -> 2026-03-24  
**אחראי התוכנית:** צוות אחסון (`@storage-wg`), מועצת ממשל (`@council`), גילדת האוצר (`@treasury`)  
**Portee:** Pipelines d'onboarding des providers, flux d'judication des litiges and processus de reconciliation du tresor requis pour la GA SF-2c.

La checklist ci-dessous doit etre revue avant d'activer le marche pour des operateurs externes. Chaque ligne renvoie vers une evidence deterministe (בדיקות, מתקנים או תיעוד) que les auditeurs peuvent rejouer.

## רשימת רשימת קבלה

### כניסה לספקים

| בדוק | אימות | עדות |
|-------|----------------|--------|
| Le registry accepte les declarations canoniques de capacite | בדיקת האינטגרציה `/v1/sorafs/capacity/declare` דרך ה-API של האפליקציה, מאומתת החתימות, לכידת מטא-נתונים ו-Hand-off לעומת הרישום. | `crates/iroha_torii/src/routing.rs:7654` |
| Le smart contract rejette les payloads incoherents | Le test unitaire garantit que les IDs de provider et les champs GiB עוסקת בכתב א-לה הצהרת חתם אוונט התמדה. | `crates/iroha_core/src/smartcontracts/isi/sorafs.rs:3445` |
| Le CLI emet des artefacts d'onboarding canoniques | ה-CLI לרתום את המיונים Norito/JSON/Base64 קובעים ותקפים לנסיעות הלוך ושוב כדי שהמתפעלים מכינים הצהרות במצב לא מקוון. | `crates/sorafs_car/tests/capacity_cli.rs:17` |
| Le guide operationur couvre le workflow d'admission et les garde-fous de governance | התיעוד מונה את סכמת ההצהרה, ברירת המחדל של מדיניות ו-les etapes de revue pour le Council. | `../storage-capacity-marketplace.md` |

### Resolution des litiges

| בדוק | אימות | עדות |
|-------|----------------|--------|
| Les enregistrements de litige persistent avec un digest canonique du payload | הבדיקה היחידה נרשמה למשפטים, פענוחה של מטען המטען ותאשר את החוק ממתין להבטחת קביעת החשבונות. | `crates/iroha_core/src/smartcontracts/isi/sorafs.rs:1835` |
| Le genereur de litiges du CLI correspond au schema canonique | בדיקת CLI couvre les sorties Base64/Norito ו-les resumes JSON pour `CapacityDisputeV1`, en assurant que les ראיות חבילות hashent de maniere deterministe. | `crates/sorafs_car/tests/capacity_cli.rs:455` |
| Le test de replay prouve le determinisme litige/penalite | La telemetry de proof-failure rejouee deux fois produit des snapshots identiques de book, credit and litige afin que les slashes soient deterministes entre peers. | `crates/iroha_core/src/smartcontracts/isi/sorafs.rs:3430` |
| Le runbook documente le flux d'escalade et de revocation | Le guide d'operations ללכוד את זרימת העבודה של המועצה, דרישות קדם ונהלי החזרה. | `../dispute-revocation-runbook.md` |

### Reconciliation du Tresor| בדוק | אימות | עדות |
|-------|----------------|--------|
| L'accumulation du ledger correspond a la projection de soak sur 30 jours | Le test soak couvre cinq providers sur 30 fenetres de settlement, and comparant les מנות ראשונות דו פנקס a la reference de payout attendue. | `crates/iroha_core/src/smartcontracts/isi/sorafs.rs:3000` |
| La Reconciliation des Exports de Ledger est רשום chaque nuit | `capacity_reconcile.py` להשוות את תשומת הלב של עמלת פנקס הוצאות יצוא XOR מבצע, מדדי Prometheus ו-gate l'approbation du tresor דרך Alertmanager. | `scripts/telemetry/capacity_reconcile.py:1`,`docs/source/sorafs/runbooks/capacity_reconciliation.md:1`,`dashboards/alerts/sorafs_capacity_rules.yml:100` |
| לוחות המחוונים של חיוב חשיפת עונשים וצבירת טלמטריה | היבוא Grafana עקבות אחרי צבירה של שעת GiB, les compteurs de strikes et le collateral מעורבים pour la visibilite on-call. | `dashboards/grafana/sorafs_capacity_penalties.json:1` |
| Le rapport publie archive la methodologie de soak et les commandes de replay | Le rapport detaille la portee du soak, les commandes d'execution et les hooks d'observabilite pour les auditeurs. | `./sf2c-capacity-soak.md` |

## הערות לביצוע

Relancez la suite de validation avant le sign-off:

```bash
cargo test -p iroha_torii --features app_api -- capacity_declaration_handler_accepts_request
cargo test -p iroha_core -- register_capacity_declaration_rejects_provider_mismatch
cargo test -p iroha_core -- register_capacity_dispute_inserts_record
cargo test -p iroha_core -- capacity_dispute_replay_is_deterministic
cargo test -p iroha_core -- capacity_fee_ledger_30_day_soak_deterministic
cargo test -p sorafs_car --features cli --test capacity_cli
python3 scripts/telemetry/capacity_reconcile.py --snapshot <state.json> --ledger <ledger.ndjson> --warn-only
```

המפעילים מבצעים מחדש את מטענים הדרושים ל-onboarding/litige avec `sorafs_manifest_stub capacity {declaration,dispute}` ו-archiver les bytes JSON/Norito תוצאות aux cotes du ticket de governance.

## חפצי אמנות בסימן

| חפץ | נתיב | blake2b-256 |
|--------|------|--------|
| Paquet d'approvation d'onboarding des providers | `docs/examples/sorafs_capacity_marketplace_validation/2026-03-24_onboarding_signoff.md` | `8f41a745d8d94710fe81c07839651520429d4abea5729bc00f8f45bbb11daa4c` |
| Paquet d'approbation de resolution des litiges | `docs/examples/sorafs_capacity_marketplace_validation/2026-03-24_dispute_signoff.md` | `c3ac3999ef52857170fedb83cddbff7733ef5699f8b38aea2e65ae507a6229f7` |
| Paquet d'approbation de reconciliation du Tresor | `docs/examples/sorafs_capacity_marketplace_validation/2026-03-24_treasury_signoff.md` | `0511aeed1f5607c329428cd49c94d1af51292c85134c10c3330c172b0140e8c6` |

Conservez les copies signees de ces artefacts avec le bundle de release et reliez-les dans le registre de changement de governance.

## הסכמות

- ראש צוות אחסון - @storage-tl (2026-03-24)  
- מזכיר מועצת הממשל - @council-sec (2026-03-24)  
- מוביל תפעול משרד האוצר - @treasury-ops (2026-03-24)