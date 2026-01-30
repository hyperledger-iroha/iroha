---
lang: he
direction: rtl
source: docs/portal/i18n/he/docusaurus-plugin-content-docs/current/sorafs/gateway-dns-runbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 5fa84af0622cfd0d6bf4d2b16d1a93465a143d5abd840b4ad971391459d0baa7
source_last_modified: "2025-11-14T04:43:21.733607+00:00"
translation_last_reviewed: 2026-01-30
---

# ראנבוק השקת Gateway ו-DNS של SoraFS

עותק הפורטל הזה משקף את הראנבוק הקנוני שב-
[`docs/source/sorafs_gateway_dns_design_runbook.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs_gateway_dns_design_runbook.md).
הוא לוכד את הגדרות הבטיחות התפעוליות עבור ה-workstream של DNS מבוזר ו-Gateway,
כדי שמובילי רשת, Ops ותיעוד יוכלו לתרגל את סטאק האוטומציה לפני kickoff של 2025-03.

## היקף ותוצרים

- לקשור את אבני הדרך של DNS (SF-4) וה-gateway (SF-5) באמצעות חזרות על גזירה
  דטרמיניסטית של hosts, releases של ספריית resolvers, אוטומציית TLS/GAR ואיסוף
  ראיות.
- לשמור על קלטי kickoff (אג'נדה, הזמנה, מעקב נוכחות, snapshot של טלמטריית GAR)
  מסונכרנים עם ההקצאות העדכניות של owners.
- להפיק bundle ארטיפקטים בר-ביקורת עבור reviewers של governance: release notes
  של ספריית resolvers, לוגים של gateway probes, פלט harness התאמה וסיכום Docs/DevRel.

## תפקידים ואחריות

| Workstream | אחריות | ארטיפקטים נדרשים |
|------------|--------|------------------|
| Networking TL (מחסנית DNS) | לשמר תכנית hosts דטרמיניסטית, להריץ releases של ספריית RAD, לפרסם inputs של טלמטריית resolvers. | `artifacts/soradns_directory/<ts>/`, diffs עבור `docs/source/soradns/deterministic_hosts.md`, ו-metadata של RAD. |
| Ops Automation Lead (gateway) | לבצע drills לאוטומציה TLS/ECH/GAR, להריץ `sorafs-gateway-probe`, ולעדכן hooks של PagerDuty. | `artifacts/sorafs_gateway_probe/<ts>/`, probe JSON, ורשומות ב-`ops/drill-log.md`. |
| QA Guild & Tooling WG | להריץ `ci/check_sorafs_gateway_conformance.sh`, לאצור fixtures, ולארכב bundles של Norito self-cert. | `artifacts/sorafs_gateway_conformance/<ts>/`, `artifacts/sorafs_gateway_attest/<ts>/`. |
| Docs / DevRel | לתעד דקות, לעדכן design pre-read + נספחים, ולפרסם סיכום ראיות בפורטל. | קבצי `docs/source/sorafs_gateway_dns_design_*.md` מעודכנים ו-notes של rollout. |

## קלטים ודרישות מקדימות

- מפרט hosts דטרמיניסטיים (`docs/source/soradns/deterministic_hosts.md`) ותשתית
  attestation של resolvers (`docs/source/soradns/resolver_attestation_directory.md`).
- ארטיפקטים של gateway: חוברת מפעיל, helpers לאוטומציית TLS/ECH,
  הנחיות direct-mode ו-workflow של self-cert תחת `docs/source/sorafs_gateway_*`.
- Tooling: `cargo xtask soradns-directory-release`,
  `cargo xtask sorafs-gateway-probe`, `scripts/telemetry/run_soradns_transparency_tail.sh`,
  `scripts/sorafs_gateway_self_cert.sh`, ועזרי CI
  (`ci/check_sorafs_gateway_conformance.sh`, `ci/check_sorafs_gateway_probe.sh`).
- Secrets: מפתח release של GAR, credentials ACME עבור DNS/TLS, routing key של PagerDuty,
  token auth של Torii עבור fetches של resolvers.

## Checklist לפני השקה

1. אשרו משתתפים ואג'נדה על ידי עדכון
   `docs/source/sorafs_gateway_dns_design_attendance.md` והפצת האג'נדה
   הנוכחית (`docs/source/sorafs_gateway_dns_design_agenda.md`).
2. הכינו שורשי ארטיפקטים כגון
   `artifacts/sorafs_gateway_dns/<YYYYMMDD>/` ו-
   `artifacts/soradns_directory/<YYYYMMDD>/`.
3. רעננו fixtures (GAR manifests, RAD proofs, bundles של gateway conformance) וודאו
   שמצב `git submodule` תואם את תג ה-rehearsal האחרון.
4. בדקו secrets (מפתח release Ed25519, קובץ חשבון ACME, token של PagerDuty)
   וודאו התאמה ל-checksums ב-vault.
5. בצעו smoke-test ליעדי טלמטריה (endpoint של Pushgateway, לוח GAR ב-Grafana)
   לפני ה-drill.

## שלבי rehearsal לאוטומציה

### מפת hosts דטרמיניסטית & release של ספריית RAD

1. הריצו את helper הגזירה הדטרמיניסטית של hosts על סט ה-manifests המוצע ואשרו
   שאין drift מול
   `docs/source/soradns/deterministic_hosts.md`.
2. צרו bundle של ספריית resolvers:

```bash
cargo xtask soradns-directory-release \
  --rad-dir artifacts/soradns/rad_candidates \
  --output-root artifacts/soradns_directory \
  --release-key-path secrets/soradns/release.key \
  --car-cid bafybeigdyrdnsmanifest... \
  --note "dns-kickoff-20250303"
```

3. רשמו את ה-directory ID, ה-SHA-256 ונתיבי הפלט שהודפסו בתוך
   `docs/source/sorafs_gateway_dns_design_gar_telemetry.md` ובדקות ה-kickoff.

### לכידת טלמטריית DNS

- בצעו tail ללוגי שקיפות resolvers למשך ≥10 דקות עם
  `scripts/telemetry/run_soradns_transparency_tail.sh --mode staging`.
- ייצאו מטריקות Pushgateway וארכבו snapshots מסוג NDJSON ליד תיקיית run ID.

### Drills לאוטומציה של gateway

1. הריצו probe TLS/ECH:

```bash
cargo xtask sorafs-gateway-probe \
  --config configs/sorafs_gateway/probe.staging.toml \
  --output artifacts/sorafs_gateway_probe/<run-id>.json
```

2. הריצו את harness ההתאמה (`ci/check_sorafs_gateway_conformance.sh`) ואת helper
   ה-self-cert (`scripts/sorafs_gateway_self_cert.sh`) כדי לרענן bundle attestation של Norito.
3. לכדו אירועי PagerDuty/Webhook כדי להוכיח שהאוטומציה עובדת end-to-end.

### אריזת ראיות

- עדכנו את `ops/drill-log.md` עם timestamps, משתתפים ו-hashes של probes.
- שמרו ארטיפקטים בתיקיות run ID ופרסמו תקציר מנהלים בדקות Docs/DevRel.
- צרפו קישור ל-evidence bundle בכרטיס governance לפני ביקורת kickoff.

## הנחיית סשן והעברת ראיות

- **Timeline למודרטור:**
  - T-24 h — Program Management מפרסם תזכורת + snapshot אג'נדה/נוכחות ב-`#nexus-steering`.
  - T-2 h — Networking TL מרענן snapshot של טלמטריית GAR ומקליט deltas ב-`docs/source/sorafs_gateway_dns_design_gar_telemetry.md`.
  - T-15 m — Ops Automation מאמת מוכנות probes וכותב את run ID הפעיל אל `artifacts/sorafs_gateway_dns/current`.
  - במהלך השיחה — המודרטור משתף את הראנבוק וממנה סופר חי; Docs/DevRel מתעדים action items בזמן אמת.
- **תבנית דקות:** העתיקו את השלד מ-
  `docs/source/sorafs_gateway_dns_design_minutes.md` (גם משוכפל ב-portal bundle)
  והתחייבו על מופע מלא לכל סשן. כללו רשימת משתתפים, החלטות, action items, hashes
  של ראיות וסיכונים פתוחים.
- **העלאת ראיות:** בצעו zip לתיקיית `runbook_bundle/` מה-rehearsal, צרפו PDF דקות
  מרונדר, רשמו hashes SHA-256 בדקות + באג'נדה, ואז פינגו את alias ה-reviewers של governance
  לאחר שההעלאות הגיעו אל `s3://sora-governance/sorafs/gateway_dns/<date>/`.

## Snapshot ראיות (kickoff מרץ 2025)

הארטיפקטים העדכניים של rehearsal/live שמוזכרים ב-roadmap ובדקות נמצאים בבאקט
`s3://sora-governance/sorafs/gateway_dns/`. ה-hashes למטה משקפים את ה-manifest
הקנוני (`artifacts/sorafs_gateway_dns/<run-id>/runbook_bundle/evidence_manifest_*.json`).

- **Dry run — 2025-03-02 (`artifacts/sorafs_gateway_dns/20250302/`)**
  - Tarball של ה-bundle: `b13571d2822c51f771d0e471f4f66d088a78ed6c1a5adb0d4b020b04dd9a5ae0`
  - PDF דקות: `cac89ee3e6e4fa0adb9694941c7c42ffddb513f949cf1b0c9f375e14507f4f18`
- **Workshop חי — 2025-03-03 (`artifacts/sorafs_gateway_dns/20250303/runbook_bundle/`)**
  - `bc83e6a014c2d223433f04ddc3c588bfeff33ee5cdcb15aad6527efeba582a1c  minutes_20250303.md`
  - `030a98fb3e3a52dbb0fcf25a6ea4365b11d9487707bb6700cb632710f7c082e4  gar_snapshot_20250303.json`
  - `5ac17e684976d6862628672627f229f7719da74235aa0a5f0ce994dad34cb3c4  sorafs_gateway_dns_design_metrics_20250303.prom`
  - `5c6163d0ae9032c2d52ca2ecca4037dfaddcc503eb56239b53c5e9c4000997cf  probe_20250303.json`
  - `87f6341896bfb830966a4a5d0fc9158fabcc135ba16ef0d53882e558de77ba49  probe_20250303_webhook.jsonl`
  - `9b968b0bf4ca654d466ec2be5291936f1441908354e9d2da4d0a52f1568bbe03  probe.staging.toml`
  - _(Pending upload: `gateway_dns_minutes_20250303.pdf` — Docs/DevRel תוסיף את ה-SHA-256 כשה-PDF המרונדר יגיע ל-bundle.)_

## חומר קשור

- [Operations playbook ל-gateway](./operations-playbook.md)
- [תכנית observability של SoraFS](./observability-plan.md)
- [Tracker DNS מבוזר ו-gateway](https://github.com/hyperledger-iroha/iroha/blob/master/roadmap.md#core-workstreams)
