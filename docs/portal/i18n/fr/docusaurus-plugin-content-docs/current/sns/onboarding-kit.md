---
lang: fr
direction: ltr
source: docs/portal/docs/sns/onboarding-kit.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

# Metriques SNS et kit d'onboarding

L'item de roadmap **SN-8** regroupe deux engagements:

1. Publier des dashboards qui exposent registrations, renewals, ARPU, disputes et fenetres de freeze pour `.sora`, `.nexus`, et `.dao`.
2. Livrer un kit d'onboarding afin que registrars et stewards cablent DNS, pricing et APIs de facon coherente avant la mise en ligne de tout suffixe.

Cette page reflete la version source
[`docs/source/sns/onboarding_kit.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sns/onboarding_kit.md)
afin que les reviewers externes suivent la meme procedure.

## 1. Pack de metriques

### Dashboard Grafana et embed du portail

- Importez `dashboards/grafana/sns_suffix_analytics.json` dans Grafana (ou un autre host d'analytique)
  via l'API standard:

```bash
curl -H "Content-Type: application/json"          -H "Authorization: Bearer ${GRAFANA_TOKEN}"          -X POST https://grafana.sora.net/api/dashboards/db          --data-binary @dashboards/grafana/sns_suffix_analytics.json
```

- Le meme JSON alimente l'iframe de cette page (voir **SNS KPI Dashboard**).
  A chaque mise a jour du dashboard, lancez
  `npm run build && npm run serve-verified-preview` dans `docs/portal` pour
  confirmer que Grafana et l'embed restent synchronises.

### Panels et evidence

| Panel | Metriques | Evidence de gouvernance |
|-------|---------|-------------------------|
| Registrations et renewals | `sns_registrar_status_total` (success + renewal resolver labels) | Throughput par suffixe + suivi SLA. |
| ARPU / unites nettes | `sns_bulk_release_payment_net_units`, `sns_bulk_release_payment_gross_units` | Finance peut rapprocher les manifests du registrar des revenus. |
| Disputes et freezes | `guardian_freeze_active`, `sns_dispute_outcome_total`, `sns_governance_activation_total` | Montre les freezes actifs, la cadence d'arbitrage et la charge guardian. |
| SLA / taux d'erreur | `torii_request_duration_seconds`, `sns_registrar_status_total{status="error"}` | Met en evidence les regressions d'API avant impact client. |
| Suivi de manifest bulk | `sns_bulk_release_manifest_total`, metriques de paiement avec labels `manifest_id` | Connecte les drops CSV aux tickets de settlement. |

Exportez un PDF/CSV depuis Grafana (ou l'iframe embarquee) lors de la revue KPI mensuelle
et attachez-le a l'entree d'annexe pertinente sous
`docs/source/sns/regulatory/<suffix>/YYYY-MM.md`. Les stewards capturent aussi le SHA-256
du bundle exporte sous `docs/source/sns/reports/` (par exemple,
`steward_scorecard_2026q1.md`) afin que les audits puissent rejouer la piste d'evidence.

### Automatisation des annexes

Generez les fichiers d'annexe directement depuis l'export du dashboard pour que les
reviewers obtiennent un resume coherent:

```bash
cargo xtask sns-annex       --suffix .sora       --cycle 2026-03       --dashboard dashboards/grafana/sns_suffix_analytics.json       --dashboard-artifact artifacts/sns/regulatory/.sora/2026-03/sns_suffix_analytics.json       --output docs/source/sns/reports/.sora/2026-03.md       --regulatory-entry docs/source/sns/regulatory/eu-dsa/2026-03.md       --portal-entry docs/portal/docs/sns/regulatory/eu-dsa-2026-03.md
```

- Le helper hash l'export, capture UID/tags/nombre de panels, et ecrit une annexe
  Markdown sous `docs/source/sns/reports/.<suffix>/<cycle>.md` (voir l'exemple
  `.sora/2026-03` aux cotes de ce doc).
- `--dashboard-artifact` copie l'export sous
  `artifacts/sns/regulatory/<suffix>/<cycle>/` pour que l'annexe reference le chemin
  d'evidence canonique; utilisez `--dashboard-label` seulement si vous devez pointer
  vers une archive hors bande.
- `--regulatory-entry` pointe vers le memo reglementaire. Le helper insere (ou remplace)
  un bloc `KPI Dashboard Annex` qui enregistre le chemin d'annexe, l'artefact du dashboard,
  le digest et le timestamp pour garder l'evidence synchronisee.
- `--portal-entry` aligne la copie Docusaurus
  (`docs/portal/docs/sns/regulatory/*.md`) afin que les reviewers n'aient pas a
  comparer des resumes d'annexes separes manuellement.
- Si vous sautez `--regulatory-entry`/`--portal-entry`, attachez le fichier genere aux
  memos manuellement et uploadez tout de meme les snapshots PDF/CSV depuis Grafana.
- Pour les exports recurrents, listez les paires suffixe/cycle dans
  `docs/source/sns/regulatory/annex_jobs.json` et executez
  `python3 scripts/run_sns_annex_jobs.py --verbose`. Le helper parcourt chaque entree,
  copie l'export du dashboard (par defaut `dashboards/grafana/sns_suffix_analytics.json`
  si non specifie), et rafraichit le bloc d'annexe dans chaque memo reglementaire
  (et, quand present, memo portail) en un seul passage.
- Lancez `python3 scripts/check_sns_annex_schedule.py --jobs docs/source/sns/regulatory/annex_jobs.json --regulatory-root docs/source/sns/regulatory --report-root docs/source/sns/reports` (ou `make check-sns-annex`) pour prouver que la liste de jobs reste triee/sans doublons, que chaque memo contient le marqueur `sns-annex`, et que l'annexe stub existe. Le helper ecrit `artifacts/sns/annex_schedule_summary.json` a cote des resumes locale/hash utilises dans les paquets de gouvernance.
Ceci supprime les copier/coller manuels et maintient l'evidence d'annexe SN-8 coherente tout en protegant contre le drift de planning, de marqueur et de localisation dans CI.

## 2. Composants du kit d'onboarding

### Cablage de suffixe

- Schema registry + regles de selector:
  [`docs/source/sns/registry_schema.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sns/registry_schema.md)
  et [`docs/source/sns/local_to_global_toolkit.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sns/local_to_global_toolkit.md).
- Helper pour squelette DNS:
  [`scripts/sns_zonefile_skeleton.py`](https://github.com/hyperledger-iroha/iroha/blob/master/scripts/sns_zonefile_skeleton.py)
  avec le flux de repetion documente dans le
  [runbook gateway/DNS](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs_gateway_dns_owner_runbook.md).
- Pour chaque lancement de registrar, archivez une note courte sous
  `docs/source/sns/reports/` resumant les echantillons de selector, preuves GAR et hashes DNS.

### Cheatsheet de prix

| Longueur du label | Tarif de base (USD equiv) |
|--------------|---------------------|
| 3 | $240 |
| 4 | $90 |
| 5 | $30 |
| 6-9 | $12 |
| 10+ | $8 |

Coefficients de suffixe: `.sora` = 1.0x, `.nexus` = 0.8x, `.dao` = 1.3x.  
Multiplicateurs de terme: 2-year -5%, 5-year -12%; grace window = 30 days, redemption
= 60 days (20% fee, min $5, max $200). Consignez les ecarts negocies dans le
ticket du registrar.

### Auctions premium vs renewals

1. **Pool premium** -- commit/reveal en sealed-bid (SN-3). Suivez les bids avec
   `sns_premium_commit_total`, et publiez le manifest sous
   `docs/source/sns/reports/`.
2. **Dutch reopen** -- apres expiration de grace + redemption, demarrez une vente Dutch de 7-day
   a 10x qui decroit de 15% par jour. Etiquetez les manifests avec `manifest_id` pour que le
   dashboard puisse afficher le progres.
3. **Renewals** -- surveillez `sns_registrar_status_total{resolver="renewal"}` et
   capturez le checklist d'autorenew (notifications, SLA, rails de paiement de fallback)
   dans le ticket du registrar.

### APIs developpeur et automatisation

- Contrats d'API: [`docs/source/sns/registrar_api.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sns/registrar_api.md).
- Helper bulk et schema CSV:
  [`docs/source/sns/bulk_onboarding_toolkit.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sns/bulk_onboarding_toolkit.md).
- Commande exemple:

```bash
python3 scripts/sns_bulk_onboard.py registrations.csv       --ndjson artifacts/sns/releases/2026q2/requests.ndjson       --submission-log artifacts/sns/releases/2026q2/submissions.log       --submit-torii-url https://torii.sora.net       --submit-token-file ~/.config/sora/tokens/registrar.token
```

Incluez l'ID de manifest (sortie de `--submission-log`) dans le filtre du dashboard KPI
pour que finance puisse reconciler les panels de revenu par release.

### Bundle d'evidence

1. Ticket registrar avec contacts, scope de suffixe et rails de paiement.
2. Evidence DNS/resolver (zonefile skeletons + preuves GAR).
3. Fiche de prix + overrides approuves par gouvernance.
4. Artefacts de smoke-test API/CLI (exemples `curl`, transcripts CLI).
5. Capture dashboard KPI + export CSV, attachee a l'annexe mensuelle.

## 3. Checklist de lancement

| Etape | Owner | Artefact |
|------|-------|----------|
| Dashboard importe | Product Analytics | Reponse API Grafana + UID dashboard |
| Embed portail valide | Docs/DevRel | logs `npm run build` + screenshot preview |
| Repetition DNS terminee | Networking/Ops | sorties `sns_zonefile_skeleton.py` + log runbook |
| Dry run d'automatisation registrar | Registrar Eng | log submissions `sns_bulk_onboard.py` |
| Evidence gouvernance archivee | Governance Council | lien annexe + SHA-256 du dashboard exporte |

Completez la checklist avant d'activer un registrar ou un suffixe. Le bundle signe
leve le gate SN-8 et donne aux auditeurs une reference unique lors des
lancements marketplace.
