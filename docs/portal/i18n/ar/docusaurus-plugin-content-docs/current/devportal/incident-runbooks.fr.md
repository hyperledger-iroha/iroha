---
lang: ar
direction: rtl
source: docs/portal/docs/devportal/incident-runbooks.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

# Runbooks d'incident et drills de rollback

## Objectif

L'item de roadmap **DOCS-9** demande des playbooks actionnables plus un plan de repetition pour que
les operateurs du portail puissent recuperer des echecs de livraison sans deviner. Cette note
couvre trois incidents a fort signal - deploiements rates, degradation de replication et
pannes d'analytics - et documente les drills trimestriels qui prouvent que le rollback d'alias
et la validation synthetique fonctionnent toujours de bout en bout.

### Materiel connexe

- [`devportal/deploy-guide`](./deploy-guide) - workflow de packaging, signing et promotion d'alias.
- [`devportal/observability`](./observability) - release tags, analytics et probes references ci-dessous.
- `docs/source/sorafs_node_client_protocol.md`
  et [`sorafs/pin-registry-ops`](../sorafs/pin-registry-ops)
  - telemetrie du registre et seuils d'escalade.
- `docs/portal/scripts/sorafs-pin-release.sh` et helpers `npm run probe:*`
  references dans les checklists.

### Telemetrie et tooling partages

| Signal / Outil | Objectif |
| ------------- | ------- |
| `torii_sorafs_replication_sla_total` (met/missed/pending) | Detecte les blocages de replication et les breaches de SLA. |
| `torii_sorafs_replication_backlog_total`, `torii_sorafs_replication_completion_latency_epochs` | Quantifie la profondeur du backlog et la latence de completion pour le triage. |
| `torii_sorafs_gateway_refusals_total`, `torii_sorafs_manifest_submit_total{status="error"}` | Montre les echecs cote gateway qui suivent souvent un deploy rate. |
| `npm run probe:portal` / `npm run probe:tryit-proxy` | Probes synthetiques qui gate les releases et valident les rollbacks. |
| `npm run check:links` | Gate de liens casses; utilise apres chaque mitigation. |
| `sorafs_cli manifest submit ... --alias-*` (wrapped par `scripts/sorafs-pin-release.sh`) | Mecanisme de promotion/reversion d'alias. |
| `Docs Portal Publishing` Grafana board (`dashboards/grafana/docs_portal.json`) | Agrege la telemetrie refusals/alias/TLS/replication. Les alertes PagerDuty referencent ces panneaux comme preuve. |

## Runbook - Deploiement rate ou artefact defectueux

### Conditions de declenchement

- Les probes preview/production echouent (`npm run probe:portal -- --expect-release=...`).
- Alertes Grafana sur `torii_sorafs_gateway_refusals_total` ou
  `torii_sorafs_manifest_submit_total{status="error"}` apres un rollout.
- QA manuel remarque des routes cassees ou des pannes du proxy Try it immediatement apres
  la promotion de l'alias.

### Confinement immediat

1. **Geler les deploiements:** marquer le pipeline CI avec `DEPLOY_FREEZE=1` (input du workflow
   GitHub) ou mettre en pause le job Jenkins pour qu'aucun artefact ne parte.
2. **Capturer les artefacts:** telecharger `build/checksums.sha256`,
   `portal.manifest*.{json,to,bundle,sig}`, et la sortie des probes du build en echec afin que
   le rollback reference les digests exacts.
3. **Notifier les parties prenantes:** storage SRE, lead Docs/DevRel, et l'officier de garde
   governance pour awareness (surtout si `docs.sora` est impacte).

### Procedure de rollback

1. Identifier le manifest last-known-good (LKG). Le workflow de production les stocke sous
   `artifacts/devportal/<release>/sorafs/portal.manifest.to`.
2. Re-lier l'alias a ce manifest avec le helper de shipping:

```bash
cd docs/portal
./scripts/sorafs-pin-release.sh \
  --build-dir build \
  --artifact-dir artifacts/revert-$(date +%Y%m%d%H%M) \
  --sorafs-dir artifacts/revert-$(date +%Y%m%d%H%M)/sorafs \
  --pin-min-replicas 5 \
  --alias "docs-prod-revert" \
  --alias-namespace "${PIN_ALIAS_NAMESPACE}" \
  --alias-name "${PIN_ALIAS_NAME}" \
  --alias-proof "${PIN_ALIAS_PROOF_PATH}" \
  --torii-url "${TORII_URL}" \
  --submitted-epoch "$(date +%Y%m%d)" \
  --authority "${AUTHORITY}" \
  --private-key "${PRIVATE_KEY}" \
  --skip-submit

# swap in the LKG artefacts before submission
cp /secure/archive/lkg/portal.manifest.to artifacts/.../sorafs/portal.manifest.to
cp /secure/archive/lkg/portal.manifest.bundle.json artifacts/.../sorafs/

cargo run -p sorafs_orchestrator --bin sorafs_cli -- \
  manifest submit \
  --manifest artifacts/.../sorafs/portal.manifest.to \
  --chunk-plan artifacts/.../sorafs/portal.plan.json \
  --torii-url "${TORII_URL}" \
  --authority "${AUTHORITY}" \
  --private-key "${PRIVATE_KEY}" \
  --alias-namespace "${PIN_ALIAS_NAMESPACE}" \
  --alias-name "${PIN_ALIAS_NAME}" \
  --alias-proof "${PIN_ALIAS_PROOF_PATH}" \
  --metadata rollback_from="${FAILED_RELEASE}" \
  --summary-out artifacts/.../sorafs/rollback.submit.json
```

3. Enregistrer le resume du rollback dans le ticket d'incident avec les digests du
   manifest LKG et du manifest en echec.

### Validation

1. `npm run probe:portal -- --expect-release=${LKG_TAG}`.
2. `npm run check:links`.
3. `sorafs_cli manifest verify-signature ...` et `sorafs_cli proof verify ...`
   (voir le guide de deploiement) pour confirmer que le manifest repromu correspond
   toujours au CAR archive.
4. `npm run probe:tryit-proxy` pour s'assurer que le proxy Try-It staging est revenu.

### Post-incident

1. Reactiver le pipeline de deploiement uniquement apres avoir compris la cause racine.
2. Mettre a jour les entrees "Lessons learned" dans [`devportal/deploy-guide`](./deploy-guide)
   avec de nouveaux points, si besoin.
3. Ouvrir des defects pour la suite de tests en echec (probe, link checker, etc.).

## Runbook - Degradation de replication

### Conditions de declenchement

- Alerte: `sum(torii_sorafs_replication_sla_total{outcome="met"}) /
  clamp_min(sum(torii_sorafs_replication_sla_total{outcome=~"met|missed"}), 1) <
  0.95` pendant 10 minutes.
- `torii_sorafs_replication_backlog_total > 10` pendant 10 minutes (voir
  `pin-registry-ops.md`).
- Governance signale une disponibilite d'alias lente apres un release.

### Triage

1. Inspecter les dashboards [`sorafs/pin-registry-ops`](../sorafs/pin-registry-ops) pour
   confirmer si le backlog est localise sur une classe de stockage ou une flotte de providers.
2. Croiser les logs Torii pour les warnings `sorafs_registry::submit_manifest` afin de
   determiner si les submissions echouent.
3. Echantillonner la sante des replicas via `sorafs_cli manifest status --manifest ...`
   (liste les resultats par provider).

### Mitigation

1. Reemettre le manifest avec un nombre de replicas plus eleve (`--pin-min-replicas 7`) via
   `scripts/sorafs-pin-release.sh` afin que le scheduler etale la charge sur plus de providers.
   Enregistrer le nouveau digest dans le log d'incident.
2. Si le backlog est lie a un provider unique, le desactiver temporairement via le scheduler
   de replication (documente dans `pin-registry-ops.md`) et soumettre un nouveau manifest
   forcant les autres providers a rafraichir l'alias.
3. Quand la fraicheur de l'alias est plus critique que la parite de replication, re-lier
   l'alias a un manifest chaud deja stage (`docs-preview`), puis publier un manifest de
   suivi une fois que SRE a nettoye le backlog.

### Recuperation et cloture

1. Surveiller `torii_sorafs_replication_sla_total{outcome="missed"}` pour s'assurer que le
   compteur se stabilise.
2. Capturer la sortie `sorafs_cli manifest status` comme preuve que chaque replica est de
   retour en conformite.
3. Ouvrir ou mettre a jour le post-mortem du backlog de replication avec les prochaines
   etapes (scaling providers, tuning du chunker, etc.).

## Runbook - Panne d'analytics ou de telemetrie

### Conditions de declenchement

- `npm run probe:portal` reussit mais les dashboards cessent d'ingester des evenements
  `AnalyticsTracker` pendant >15 minutes.
- Privacy review detecte une hausse inattendue d'evenements abandonnes.
- `npm run probe:tryit-proxy` echoue sur les paths `/probe/analytics`.

### Reponse

1. Verifier les inputs de build: `DOCS_ANALYTICS_ENDPOINT` et
   `DOCS_ANALYTICS_SAMPLE_RATE` dans l'artefact de release (`build/release.json`).
2. Re-executer `npm run probe:portal` avec `DOCS_ANALYTICS_ENDPOINT` pointant vers le
   collector de staging pour confirmer que le tracker emet encore des payloads.
3. Si les collectors sont down, definir `DOCS_ANALYTICS_ENDPOINT=""` et rebuild pour
   que le tracker court-circuite; consigner la fenetre d'outage dans la timeline.
4. Valider que `scripts/check-links.mjs` continue de fingerprint `checksums.sha256`
   (les pannes d'analytics ne doivent *pas* bloquer la validation du sitemap).
5. Une fois le collector retabli, lancer `npm run test:widgets` pour executer les
   unit tests du helper analytics avant de republish.

### Post-incident

1. Mettre a jour [`devportal/observability`](./observability) avec les nouvelles limites
   du collector ou les exigences de sampling.
2. Emettre une notice governance si des donnees analytics ont ete perdues ou redactees
   hors politique.

## Drills trimestriels de resilience

Lancer les deux drills durant le **premier mardi de chaque trimestre** (Jan/Avr/Jul/Oct)
ou immediatement apres tout changement majeur d'infrastructure. Stocker les artefacts sous
`artifacts/devportal/drills/<YYYYMMDD>/`.

| Drill | Etapes | Preuve |
| ----- | ----- | -------- |
| Repetition du rollback d'alias | 1. Rejouer le rollback "Deploiement rate" avec le manifest production le plus recent.<br/>2. Re-lier a production une fois que les probes passent.<br/>3. Enregistrer `portal.manifest.submit.summary.json` et les logs de probes dans le dossier du drill. | `rollback.submit.json`, sortie des probes, et release tag de la repetition. |
| Audit de validation synthetique | 1. Lancer `npm run probe:portal` et `npm run probe:tryit-proxy` contre production et staging.<br/>2. Lancer `npm run check:links` et archiver `build/link-report.json`.<br/>3. Joindre screenshots/exports de panneaux Grafana confirmant le succes des probes. | Logs de probes + `link-report.json` referencant le fingerprint du manifest. |

Escalader les drills manques au manager Docs/DevRel et a la revue governance SRE, car le
roadmap exige une preuve trimestrielle deterministe que le rollback d'alias et les probes
portal restent sains.

## Coordination PagerDuty et on-call

- Le service PagerDuty **Docs Portal Publishing** possede les alertes generees depuis
  `dashboards/grafana/docs_portal.json`. Les regles `DocsPortal/GatewayRefusals`,
  `DocsPortal/AliasCache`, et `DocsPortal/TLSExpiry` page la primaire Docs/DevRel
  avec Storage SRE en secondaire.
- Quand on est page, inclure le `DOCS_RELEASE_TAG`, joindre des screenshots des panneaux
  Grafana impactes et lier la sortie probe/link-check dans les notes d'incident avant
  de commencer la mitigation.
- Apres mitigation (rollback ou redeploy), re-executer `npm run probe:portal`,
  `npm run check:links`, et capturer des snapshots Grafana montrant les metriques
  revenues dans les seuils. Joindre toute l'evidence a l'incident PagerDuty
  avant resolution.
- Si deux alertes se declenchent en meme temps (par ex. TLS expiry plus backlog), trier
  refusals en premier (arreter la publication), executer la procedure de rollback, puis
  traiter TLS/backlog avec Storage SRE sur le bridge.
