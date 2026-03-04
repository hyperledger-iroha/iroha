---
lang: he
direction: rtl
source: docs/portal/docs/sorafs/gateway-dns-runbook.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Runbook de lancement Gateway & DNS SoraFS

Cette copie du portail reflète le runbook canonique dans
[`docs/source/sorafs_gateway_dns_design_runbook.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs_gateway_dns_design_runbook.md).
ללכוד את פעולות ה-DNS וה-Gateway של זרם העבודה
afin que les responsables réseau, ops et documentation puissent répéter la pile
d'automatisation avant le lancement 2025-03.

## Portée et livrables

- Lier les jalons DNS (SF-4) et gateway (SF-5) en répétant la dérivation déterministe
  des hôtes, les releases du répertoire de resolvers, l'automatisation TLS/GAR et la
  לכידת דה פריובס.
- Garder les entrées du kickoff (סדר יום, הזמנה, suivi de presence, תמונת מצב
  télémétrie GAR) Syncées avec les dernières affectations d'owners.
- Produire un bundle d'artefacts הניתנים לביקורת עבור סוקרי השלטון: הערות
  de release du répertoire de resolvers, logs de sondes gateway, sortie du harness de
  conformité et synthèse Docs/DevRel.

## תפקידים ואחריות

| זרם עבודה | אחריות | חפצי אמנות דרושים |
|------------|----------------|----------------|
| רשת TL (ערימת DNS) | Maintenir le plan déterministe des hôtes, מוציא לאור של רפרטואר RAD, מוציא לאור תשומות של טלמטריה של פתרונות. | `artifacts/soradns_directory/<ts>/`, הבדלים של `docs/source/soradns/deterministic_hosts.md`, מדונים RAD. |
| Ops Automation Lead (שער) | Exécuter les drills d'automatisation TLS/ECH/GAR, lancer `sorafs-gateway-probe`, mettre à jour les hooks PagerDuty. | `artifacts/sorafs_gateway_probe/<ts>/`, JSON de probe, כניסות `ops/drill-log.md`. |
| QA Guild & Tooling WG | Lancer `ci/check_sorafs_gateway_conformance.sh`, גופי מרפא, חבילות ארכיון עם אישור עצמי Norito. | `artifacts/sorafs_gateway_conformance/<ts>/`, `artifacts/sorafs_gateway_attest/<ts>/`. |
| Docs / DevRel | לוכד דקות, mettre à jour le pre-read de design + נספחים, Puber la synthèse d'évidence dans ce portail. | קבצים `docs/source/sorafs_gateway_dns_design_*.md` à jour et notes de rollout. |

## מנות ראשונות ודרישות

- מפרט d'hôtes déterministes (`docs/source/soradns/deterministic_hosts.md`) et
  פיגום אישור פתרון (`docs/source/soradns/resolver_attestation_directory.md`).
- שער חפצים: manuel opérateur, helpers d'automatisation TLS/ECH,
  הדרכה במצב ישיר, זרימת עבודה באישור עצמי sous `docs/source/sorafs_gateway_*`.
- כלי עבודה: `cargo xtask soradns-directory-release`,
  `cargo xtask sorafs-gateway-probe`, `scripts/telemetry/run_soradns_transparency_tail.sh`,
  `scripts/sorafs_gateway_self_cert.sh`, et helpers CI
  (`ci/check_sorafs_gateway_conformance.sh`, `ci/check_sorafs_gateway_probe.sh`).
- סודות: clé de release GAR, אישורים ACME DNS/TLS, מפתח ניתוב PagerDuty,
  token auth Torii pour les feches de resolvers.

## רשימת רשימת קדם כרך1. Confirmer les deelnemers et l’agenda en mettant à jour
   `docs/source/sorafs_gateway_dns_design_attendance.md` et en diffusant l'agenda
   courant (`docs/source/sorafs_gateway_dns_design_agenda.md`).
2. Préparer les racines d'artefacts telles que
   `artifacts/sorafs_gateway_dns/<YYYYMMDD>/` et
   `artifacts/soradns_directory/<YYYYMMDD>/`.
3. Rafraîchir les fixtures (מציג GAR, preuves RAD, bundles de conformité gateway) et
   s'assurer que l'état `git submodule` correspond au dernier tag de rehearsal.
4. Vérifier les secrets (clé de release Ed25519, fichier de compte ACME, token PagerDuty)
   et leur correspondance avec les checksums du vault.
5. Faire un smoke-test des cibles de télémétrie (נקודת קצה Pushgateway, לוח GAR Grafana)
   avant le drill.

## חזרות לאוטומציה

### Carte d'hôtes déterministe & release du répertoire RAD

1. Exécuter l'helper de dérivation déterministe des hôtes sur le set de manifests
   proposé et confirmer l’absence de drift par rapport à
   `docs/source/soradns/deterministic_hosts.md`.
2. Générer un bundle de répertoire de resolvers:

```bash
cargo xtask soradns-directory-release \
  --rad-dir artifacts/soradns/rad_candidates \
  --output-root artifacts/soradns_directory \
  --release-key-path secrets/soradns/release.key \
  --car-cid bafybeigdyrdnsmanifest... \
  --note "dns-kickoff-20250303"
```

3. Enregistrer l'ID du répertoire, le SHA-256 et les chemins de sortie imprimés dans
   `docs/source/sorafs_gateway_dns_design_gar_telemetry.md` ושלוש דקות הפתיחה.

### לכידת DNS של טלמטריה

- Tailer les logs de transparence des resolvers תליון ≥10 דקות עם
  `scripts/telemetry/run_soradns_transparency_tail.sh --mode staging`.
- יצואן למטרות Pushgateway ומאחסן תמונות NDJSON à côté du
  répertoire du run ID.

### תרגילי אוטומציה של שער

1. Exécuter la sonde TLS/ECH :

```bash
cargo xtask sorafs-gateway-probe \
  --config configs/sorafs_gateway/probe.staging.toml \
  --output artifacts/sorafs_gateway_probe/<run-id>.json
```

2. Lancer le harness de conformité (`ci/check_sorafs_gateway_conformance.sh`) et
   l'helper self-cert (`scripts/sorafs_gateway_self_cert.sh`) pour rafraîchir le bundle
   אישורים Norito.
3. Capturer les événements PagerDuty/Webhook pour prouver que la chaîne d'automatisation
   fonctionne de bout en bout.

### Packaging des preuves

- Mettre à jour `ops/drill-log.md` עם חותמות זמן, משתתפים ו-hashes de probe.
- Stocker les artefacts dans les répertoires de run ID et publier une synthèse exécutive
  בדקות של Docs/DevRel.
- Lier le bundle de preuves dans le ticket de gouvernance avant la revue de kickoff.

## אנימציה של סשן וחופשה- **ציר הזמן של המודרר:**
  - T-24 שעות — ניהול תוכנית poste le rappel + תמונת מצב סדר יום/נוכחות ב-`#nexus-steering`.
  - T-2 h — Networking TL rafraîchit le snapshot télémétrie GAR et consigne les deltas dans `docs/source/sorafs_gateway_dns_design_gar_telemetry.md`.
  - T-15 m — Ops Automation vérifie la preparation des probes et écrit le run ID actif dans `artifacts/sorafs_gateway_dns/current`.
  - Pendant l'appel — Le modérateur partage ce runbook et assigne un scribe en direct; Docs/DevRel לוכד את הפעולות בשגרה.
- **תבנית דקות :** Copiez le squelette de
  `docs/source/sorafs_gateway_dns_design_minutes.md` (également miroir dans le bundle
  du portail) et commitez une instance remplie par session. כלול לה רשימה דה
  משתתפים, החלטות, פעולות, hashes de preuves ו-risques ouverts.
- **העלה את des preuves :** Zipper le répertoire `runbook_bundle/` du rehearsal,
  joindre le PDF de minutes rendu, enregistrer les hashes SHA-256 in les דקות +
  סדר היום, puis pinguer l'alias des reviewers de governance une fois les uploads
  disponibles dans `s3://sora-governance/sorafs/gateway_dns/<date>/`.

## תמונת Snapshot des preuves (בעיטה במרץ 2025)

Les derniers חפצי אמנות חזרות/חיים référencés dans la roadmap et les דקות
sont stockés dans le bucket `s3://sora-governance/sorafs/gateway_dns/`. Les hashes
ci-dessous reflètent le manifest canonique (`artifacts/sorafs_gateway_dns/<run-id>/runbook_bundle/evidence_manifest_*.json`).

- **ריצה יבשה — 2025-03-02 (`artifacts/sorafs_gateway_dns/20250302/`)**
  - חבילת טרבול: `b13571d2822c51f771d0e471f4f66d088a78ed6c1a5adb0d4b020b04dd9a5ae0`
  - דקות PDF: `cac89ee3e6e4fa0adb9694941c7c42ffddb513f949cf1b0c9f375e14507f4f18`
- **אטלייה בשידור חי — 2025-03-03 (`artifacts/sorafs_gateway_dns/20250303/runbook_bundle/`)**
  - `bc83e6a014c2d223433f04ddc3c588bfeff33ee5cdcb15aad6527efeba582a1c  minutes_20250303.md`
  - `030a98fb3e3a52dbb0fcf25a6ea4365b11d9487707bb6700cb632710f7c082e4  gar_snapshot_20250303.json`
  - `5ac17e684976d6862628672627f229f7719da74235aa0a5f0ce994dad34cb3c4  sorafs_gateway_dns_design_metrics_20250303.prom`
  - `5c6163d0ae9032c2d52ca2ecca4037dfaddcc503eb56239b53c5e9c4000997cf  probe_20250303.json`
  - `87f6341896bfb830966a4a5d0fc9158fabcc135ba16ef0d53882e558de77ba49  probe_20250303_webhook.jsonl`
  - `9b968b0bf4ca654d466ec2be5291936f1441908354e9d2da4d0a52f1568bbe03  probe.staging.toml`
  - _(העלה בתשומת לב: `gateway_dns_minutes_20250303.pdf` — Docs/DevRel ajoutera le SHA-256 dès que le PDF rendu sera dans le bundle.)_

## חיבור Matériel

- [שער Playbook d'operations](./operations-playbook.md)
- [Plan d'observabilité SoraFS](./observability-plan.md)
- [מעקב DNS décentralisé ושער](https://github.com/hyperledger-iroha/iroha/blob/master/roadmap.md#core-workstreams)