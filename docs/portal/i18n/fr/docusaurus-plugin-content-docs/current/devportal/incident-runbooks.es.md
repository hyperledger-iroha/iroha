---
lang: fr
direction: ltr
source: docs/portal/docs/devportal/incident-runbooks.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Runbooks d'incidents et exercices de rollback

## Proposition

L'élément de la feuille de route **DOCS-9** exige des playbooks accessibles mais un plan de pratique pour cela
les opérateurs du portail peuvent récupérer les erreurs d’entrée sans adivinar. Cette note
cubre tres incidents de alta senal: despliegues fallidos, degradacion de réplicacion y
caidas d'analyse, et documenta les exercices trimestriels qui vérifient le rollback de
l'alias et la validation synthétique fonctionnent de bout en bout.

### Matériel lié

- [`devportal/deploy-guide`](./deploy-guide) - flux d'emballage, signature et promotion d'alias.
- [`devportal/observability`](./observability) - balises de version, analyses et sondes référencées ci-dessous.
-`docs/source/sorafs_node_client_protocol.md`
  y [`sorafs/pin-registry-ops`](../sorafs/pin-registry-ops)
  - télémétrie du registre et des ombrelles d'escalade.
- `docs/portal/scripts/sorafs-pin-release.sh` et aides `npm run probe:*`
  référencées dans les listes de contrôle.

### Télémétrie et outillage comparés| Senal / Outil | Proposé |
| ------------- | ------- |
| `torii_sorafs_replication_sla_total` (réalisé/manqué/en attente) | Détecte les blocages de réplication et les violations de SLA. |
| `torii_sorafs_replication_backlog_total`, `torii_sorafs_replication_completion_latency_epochs` | Compte tenu de la profondeur du retard et de la latence de réalisation du triage. |
| `torii_sorafs_gateway_refusals_total`, `torii_sorafs_manifest_submit_total{status="error"}` | Il y a des erreurs de passerelle qui se produisent lorsque le déploiement est défectueux. |
| `npm run probe:portal` / `npm run probe:tryit-proxy` | Sondes synthétiques qui déclenchent les versions et valident les restaurations. |
| `npm run check:links` | Gate de enlaces rotos; se usa après chaque atténuation. |
| `sorafs_cli manifest submit ... --alias-*` (utilisé par `scripts/sorafs-pin-release.sh`) | Mécanisme de promotion/réversion d’alias. |
| Carte `Docs Portal Publishing` Grafana (`dashboards/grafana/docs_portal.json`) | Ajouter des télémétries de refus/alias/TLS/réplication. Les alertes de PagerDuty font référence à ces panneaux comme preuves. |

## Runbook - Supprimer une erreur ou un artefact incorrect

### Conditions de disparité

- Fallan los sondes de prévisualisation/production (`npm run probe:portal -- --expect-release=...`).
- Alertes Grafana et `torii_sorafs_gateway_refusals_total` ou
  `torii_sorafs_manifest_submit_total{status="error"}` après un déploiement.
- Manuel d'assurance qualité pour détecter les itinéraires ou les erreurs du proxy Essayez-le immédiatement après la
  promotion de l'alias.

### Conflit immédiat1. **Congelar despliegues:** marque le pipeline CI avec `DEPLOY_FREEZE=1` (entrée du flux de travail de
   GitHub) ou suspendre le travail de Jenkins pour ne pas récupérer plus d'artefacts.
2. **Capturer les artefacts :** télécharger `build/checksums.sha256`,
   `portal.manifest*.{json,to,bundle,sig}`, et la sortie des sondes de construction a échoué pour que
   la référence de restauration des résumés exacts.
3. **Notifier les parties prenantes :** Storage SRE, responsable de Docs/DevRel et le responsable de la garde de
   gouvernement pour la sensibilisation (en particulier lorsque `docs.sora` est impacté).

### Procédure de restauration

1. Identifia el manifest ultimo conocido bono (LKG). Le flux de production de la garde
   bas `artifacts/devportal/<release>/sorafs/portal.manifest.to`.
2. Renvoyez l'alias à ce manifeste avec l'assistant d'envoi :

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

3. Enregistrez le résumé du rollback sur le ticket de l'incident avec les résumés du
   manifester LKG et du manifeste failli.

### Validation

1. `npm run probe:portal -- --expect-release=${LKG_TAG}`.
2. `npm run check:links`.
3. `sorafs_cli manifest verify-signature ...` et `sorafs_cli proof verify ...`
   (voir la guia de despliegue) pour confirmer que le manifeste est re-promocionado sigue
   coïncidant avec les archives de la CAR.
4. `npm run probe:tryit-proxy` pour garantir que le proxy Try-It staging est enregistré.

### Post-incident1. Réhabiliter le pipeline de despliegue solo après avoir entendu la cause principale.
2. Rellena entradas de "Lessons Learned" en [`devportal/deploy-guide`](./deploy-guide)
   avec de nouvelles notes, si applicable.
3. Abre défauts para el suite de pruebas fallidas (sonde, vérificateur de lien, etc.).

## Runbook - Dégradation de la réplication

### Conditions de disparité

- Alerte : `sum(torii_sorafs_replication_sla_total{outcome="met"}) /
  clamp_min(sum(torii_sorafs_replication_sla_total{outcome=~"met|missed"}), 1) <
  0,95` pour 10 minutes.
- `torii_sorafs_replication_backlog_total > 10` pendant 10 minutes (version
  `pin-registry-ops.md`).
- Gobernanza rapporte la disponibilité du pseudonyme après une libération.

### Triage

1. Inspection des tableaux de bord de [`sorafs/pin-registry-ops`](../sorafs/pin-registry-ops) pour
   confirmer si le backlog est localisé dans une classe de stockage ou dans une flotte de fournisseurs.
2. Cruza enregistre le Torii pour les avertissements `sorafs_registry::submit_manifest` pour déterminer si
   las soumissions sontan fallando.
3. Muestrea salud de répliques via `sorafs_cli manifest status --manifest ...` (liste des résultats
   de réplication par le fournisseur).

### Atténuation1. Reemite el manifest con mayor conteo de répliques (`--pin-min-replicas 7`) en utilisant
   `scripts/sorafs-pin-release.sh` pour que le planificateur distribue des charges à un ensemble principal
   des fournisseurs. Enregistrez le nouveau résumé dans le journal de l'incident.
2. Si le backlog est établi par un fournisseur unique, déshabilité temporairement via le
   planificateur de réplication (documenté en `pin-registry-ops.md`) et envoyer un nouveau
   manifeste forzando a los autres fournisseurs pour refrescar el alias.
3. Lorsque la fresque de l'alias est la plus critique pour la parité de réplication, re-vincula el
   alias un manifeste caliente ya mis en scène (`docs-preview`), luego publica un manifeste de
   Ensuite, une fois que SRE a supprimé le backlog.

### Récupération et cierre

1. Monitorea `torii_sorafs_replication_sla_total{outcome="missed"}` pour garantir que le
   le conteo se stabilise.
2. Capturez la sortie de `sorafs_cli manifest status` comme preuve que chaque réplique est cette
   de nuevo en cumplimiento.
3. Ouvrez ou actualisez le post-mortem du backlog de réplication avec les étapes suivantes
   (escalade des fournisseurs, réglage du chunker, etc.).

## Runbook - Tableau d'analyse ou de télémétrie

### Conditions de disparité

- `npm run probe:portal` vous quittez mais les tableaux de bord doivent gérer les événements de
  `AnalyticsTracker` pendant >15 minutes.
- L'examen de la confidentialité détecte une augmentation inattendue dans les événements supprimés.
- `npm run probe:tryit-proxy` tombe sur les chemins `/probe/analytics`.

### Réponse1. Vérifiez les entrées de build : `DOCS_ANALYTICS_ENDPOINT` et
   `DOCS_ANALYTICS_SAMPLE_RATE` sur l'artefact de la version (`build/release.json`).
2. Relancez `npm run probe:portal` avec `DOCS_ANALYTICS_ENDPOINT` en pointant vers
   collecteur de mise en scène pour confirmer que le tracker émet des charges utiles.
3. Si les collectionneurs sont en panne, setea `DOCS_ANALYTICS_ENDPOINT=""` et reconstruction
   pour que le tracker ait un court-circuit ; enregistrer la ventana de panne en la
   ligne de temps de l'incident.
4. Valida que `scripts/check-links.mjs` siga empreinte digitale `checksums.sha256`
   (les caidas de analitica *no* doivent bloquer la validation du plan du site).
5. Lorsque le collecteur est récupéré, corre `npm run test:widgets` pour l'exécuter.
   tests unitaires de l'assistant d'analyse avant la réédition.

### Post-incident

1. Actualiser [`devportal/observability`](./observability) avec de nouvelles limitations du
   collector o requisitos de muestreo.
2. Émettre un avis de gouvernement si vous perdez ou rédigez des données d'analyse plus loin
   de politique.

## Exercices trimestriels de résilience

Ejecuta ambos drills durante el **primer martes de cada trimestre** (Ene/Abr/Jul/Oct)
o immédiatement après tout changement de maire d'infrastructure. Garder les objets en bas
`artifacts/devportal/drills/<YYYYMMDD>/`.| Perceuse | Pas à pas | Preuve |
| ----- | ----- | -------- |
| Ensayo de rollback de alias | 1. Répétez le rollback de "Despliegue fallido" en utilisant le manifeste de production le plus récent.2. Re-vinculer une production une fois que les sondes pasen.3. Registrar `portal.manifest.submit.summary.json` et journaux de sondes sur le tapis du foret. | `rollback.submit.json`, sortie de sondes et étiquette de libération de l'essai. |
| Auditorium de validation synthétique | 1. Exécuter `npm run probe:portal` et `npm run probe:tryit-proxy` contre la production et la mise en scène.2. Exécuter `npm run check:links` et archiver `build/link-report.json`.3. Captures d'écran/exportations supplémentaires des panneaux Grafana confirmant la sortie de la sonde. | Logs de sonde + `link-report.json` référençant l'empreinte digitale du manifeste. |

Escala los drills perdidos al manager de Docs/DevRel et à la révision de gouvernance de SRE,
parce que la feuille de route exige des preuves trimestrielles déterminantes pour le retour en arrière des alias et des
sondes del portal siguen saludables.

## Coordinacion de PagerDuty et de garde- Le service PagerDuty **Docs Portal Publishing** est dû aux alertes générées depuis
  `dashboards/grafana/docs_portal.json`. Le verre `DocsPortal/GatewayRefusals`,
  `DocsPortal/AliasCache`, et `DocsPortal/TLSExpiry` page principale de Docs/DevRel
  avec Storage SRE comme secondaire.
- Lorsque cette page comprend le `DOCS_RELEASE_TAG`, des captures d'écran supplémentaires des panneaux Grafana
  concernés et enlaza la sortie de sonde/lien-vérification dans les notes de l'incident avant de
  lancer une atténuation.
- Après l'atténuation (rollback ou redéploiement), relancez l'exécution `npm run probe:portal`,
  `npm run check:links`, et capture d'instantanés Grafana fresques montrant les mesures
  de nuevo dentro de umbrales. Complément à la preuve de l'incident de PagerDuty
  avant de résoudre.
- Si deux alertes sont disparates au même moment (par exemple, expiration de TLS plus backlog), triage
  refus primero (detener Publishing), ejecuta el procedimiento de rollback, ensuite
  Nettoyer les éléments de TLS/backlog avec Storage SRE sur le pont.