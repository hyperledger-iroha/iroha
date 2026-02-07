---
lang: fr
direction: ltr
source: docs/portal/docs/devportal/incident-runbooks.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Runbooks d'incident et exercices de rollback

## Objectif

L'item de roadmap **DOCS-9** demande des playbooks actionnables plus un plan de répétition pour que
les opérateurs du portail peuvent récupérer les échecs de livraison sans deviner. Cette note
couvre trois incidents à fort signal - taux de déploiements, dégradation de réplication et
pannes d'analytics - et documente les exercices trimestriels qui prouvent que le rollback d'alias
et la validation synthétique fonctionnant toujours de bout en bout.

### Matériel connexe

- [`devportal/deploy-guide`](./deploy-guide) - workflow de packaging, signature et promotion d'alias.
- [`devportal/observability`](./observability) - références des balises de version, des analyses et des sondes ci-dessous.
-`docs/source/sorafs_node_client_protocol.md`
  et [`sorafs/pin-registry-ops`](../sorafs/pin-registry-ops)
  - télémétrie du registre et seuils d'escalade.
- `docs/portal/scripts/sorafs-pin-release.sh` et aides `npm run probe:*`
  références dans les listes de contrôle.

### Télémétrie et outillage partages| Signal / Outil | Objectif |
| ------------- | ------- |
| `torii_sorafs_replication_sla_total` (réalisé/manqué/en attente) | Détectez les blocages de réplication et les violations de SLA. |
| `torii_sorafs_replication_backlog_total`, `torii_sorafs_replication_completion_latency_epochs` | Quantifiez la profondeur du backlog et la latence de complétion pour le triage. |
| `torii_sorafs_gateway_refusals_total`, `torii_sorafs_manifest_submit_total{status="error"}` | Montrez les echecs cote gateway qui suivent souvent un taux de déploiement. |
| `npm run probe:portal` / `npm run probe:tryit-proxy` | Sondes synthétiques qui gatent les releases et valident les rollbacks. |
| `npm run check:links` | Gate de liens casses; utiliser après chaque atténuation. |
| `sorafs_cli manifest submit ... --alias-*` (encapsulé par `scripts/sorafs-pin-release.sh`) | Mécanisme de promotion/réversion d'alias. |
| Carte `Docs Portal Publishing` Grafana (`dashboards/grafana/docs_portal.json`) | Agréger la télémétrie refus/alias/TLS/réplication. Les alertes PagerDuty référencent ces panneaux comme preuve. |

## Runbook - Taux de déploiement ou artefact défectueux

### Conditions de déclenchement

- Les sondes aperçu/production échouent (`npm run probe:portal -- --expect-release=...`).
- Alertes Grafana sur `torii_sorafs_gateway_refusals_total` ou
  `torii_sorafs_manifest_submit_total{status="error"}` après un déploiement.
- QA manuel remarque des routes cassées ou des pannes du proxy Essayez-le immédiatement après
  la promotion de l'alias.

### Confinement immédiat1. **Geler les déploiements :** marquer le pipeline CI avec `DEPLOY_FREEZE=1` (entrée du workflow
   GitHub) ou mettre en pause le travail Jenkins pour qu'aucun artefact ne parte.
2. **Capturer les artefacts:** télécharger `build/checksums.sha256`,
   `portal.manifest*.{json,to,bundle,sig}`, et la sortie des sondes du build en echec afin que
   le rollback reference les digests exacts.
3. **Notifier les parties participent:** storage SRE, lead Docs/DevRel, et l'officier de garde
   gouvernance pour la sensibilisation (surtout si `docs.sora` est impacte).

### Procédure de restauration

1. Identifiant du manifeste du dernier bien connu (LKG). Le workflow de production les stocks sous
   `artifacts/devportal/<release>/sorafs/portal.manifest.to`.
2. Relier l'alias à ce manifeste avec le helper de shipping :

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

3. Enregistrer le CV du rollback dans le ticket d'incident avec les résumés du
   manifest LKG et du manifest en echec.

### Validation

1. `npm run probe:portal -- --expect-release=${LKG_TAG}`.
2. `npm run check:links`.
3. `sorafs_cli manifest verify-signature ...` et `sorafs_cli proof verify ...`
   (voir le guide de déploiement) pour confirmer que le manifeste repromu correspond
   toujours au CAR archive.
4. `npm run probe:tryit-proxy` pour s'assurer que le proxy Try-It staging génère des revenus.

### Post-incident1. Réactiver le pipeline de déploiement uniquement après avoir compris la cause racine.
2. Mettre à jour les entrées "Lessons learnt" dans [`devportal/deploy-guide`](./deploy-guide)
   avec de nouveaux points, si besoin.
3. Ouvrir des défauts pour la suite de tests en échec (sonde, vérificateur de liens, etc.).

## Runbook - Dégradation de réplication

### Conditions de déclenchement

- Alerte : `sum(torii_sorafs_replication_sla_total{outcome="met"}) /
  clamp_min(sum(torii_sorafs_replication_sla_total{outcome=~"met|missed"}), 1) <
  0,95` pendentif 10 minutes.
- Pendentif `torii_sorafs_replication_backlog_total > 10` 10 minutes (voir
  `pin-registry-ops.md`).
- Governance signale une disponibilité d'alias lente après une publication.

### Triage

1. Inspectez les tableaux de bord [`sorafs/pin-registry-ops`](../sorafs/pin-registry-ops) pour
   confirmer si le backlog est localisé sur une classe de stockage ou une flotte de fournisseurs.
2. Croiser les logs Torii pour les avertissements `sorafs_registry::submit_manifest` afin de
   déterminer si les observations font écho.
3. Echantillonner la santé des répliques via `sorafs_cli manifest status --manifest ...`
   (liste les résultats par fournisseur).

### Atténuation1. Remettre le manifeste avec un nombre de répliques plus onze (`--pin-min-replicas 7`) via
   `scripts/sorafs-pin-release.sh` afin que le planificateur étale la charge sur plus de fournisseurs.
   Enregistrer le nouveau digest dans le journal d'incident.
2. Si le backlog est un fournisseur unique, le désactiver temporairement via le planificateur
   de réplication (document dans `pin-registry-ops.md`) et soumettre un nouveau manifeste
   forcant les autres fournisseurs à rafraichir l'alias.
3. Quand la fraicheur de l'alias est plus critique que la parité de réplication, re-lier
   l'alias a un manifest chaud deja stage (`docs-preview`), puis publier un manifest de
   suivi une fois que SRE a nettoye le backlog.

### Récupération et clôture

1. Surveiller `torii_sorafs_replication_sla_total{outcome="missed"}` pour s'assurer que le
   le compteur se stabilise.
2. Capturer la sortie `sorafs_cli manifest status` comme preuve que chaque réplique est de
   retour en conformité.
3. Ouvrir ou mettre à jour le post-mortem du backlog de réplication avec les prochains
   étapes (fournisseurs de mise à l'échelle, réglage du chunker, etc.).

## Runbook - Panne d'analytics ou de télémétrie

### Conditions de déclenchement

- `npm run probe:portal` réussit mais les tableaux de bord cessent d'ingérer des événements
  `AnalyticsTracker` pendentif >15 minutes.
- L'examen de la confidentialité détecte une hausse inattendue d'événements abandonnés.
- `npm run probe:tryit-proxy` échoue sur les chemins `/probe/analytics`.

### Réponse1. Vérifier les entrées de build : `DOCS_ANALYTICS_ENDPOINT` et
   `DOCS_ANALYTICS_SAMPLE_RATE` dans l'artefact de libération (`build/release.json`).
2. Re-executer `npm run probe:portal` avec `DOCS_ANALYTICS_ENDPOINT` pointant vers le
   collector de staging pour confirmer que le tracker emet encore des payloads.
3. Si les collecteurs sont en panne, définissez `DOCS_ANALYTICS_ENDPOINT=""` et reconstruisez pour
   que le tracker court-circuite; consigner la fenêtre de panne dans la timeline.
4. Valider que `scripts/check-links.mjs` continue de l'empreinte digitale `checksums.sha256`
   (les pannes d'analytics ne doivent *pas* bloquer la validation du plan du site).
5. Une fois le collector retabli, lancer `npm run test:widgets` pour exécuter les
   tests unitaires du helper Analytics avant de republier.

### Post-incident

1. Mettre à jour [`devportal/observability`](./observability) avec les nouvelles limites
   du collecteur ou les exigences de sampling.
2. Emettre une notice gouvernance si des données analytiques ont été perdues ou expurgées
   hors politique.

## Drills trimestriels de résilience

Lancer les deux exercices durant le **premier mardi de chaque trimestre** (Jan/Avr/Jul/Oct)
ou immédiatement après tout changement majeur d'infrastructure. Stocker les artefacts sous
`artifacts/devportal/drills/<YYYYMMDD>/`.| Perceuse | Étapes | Préuve |
| ----- | ----- | -------- |
| Répétition du rollback d'alias | 1. Rejouer le rollback "Deploiement rate" avec le manifeste production le plus récent.2. Relier a production une fois que les sondes passent.3. Enregistrer `portal.manifest.submit.summary.json` et les logs de sondes dans le dossier du forage. | `rollback.submit.json`, sortie des sondes, et release tag de la répétition. |
| Audit de validation synthétique | 1. Lancer `npm run probe:portal` et `npm run probe:tryit-proxy` contre production et staging.2. Lancer `npm run check:links` et archiveur `build/link-report.json`.3. Joindre captures d'écran/exports de panneaux Grafana confirmant le succès des sondes. | Logs de sondes + `link-report.json` référençant l'empreinte digitale du manifeste. |

Escalader les exercices manques au manager Docs/DevRel et à la revue gouvernance SRE, car le
roadmap exige une preuve trimestrielle déterministe que le rollback d'alias et les sondes
le portail reste sain.

## Coordination PagerDuty et astreinte- Le service PagerDuty **Docs Portal Publishing** possède les alertes générées depuis
  `dashboards/grafana/docs_portal.json`. Les règles `DocsPortal/GatewayRefusals`,
  `DocsPortal/AliasCache`, et `DocsPortal/TLSExpiry` page la primaire Docs/DevRel
  avec Stockage SRE en secondaire.
- Quand sur est page, incluez le `DOCS_RELEASE_TAG`, joindre des captures d'écran des panneaux
  Grafana impacte et lier la sortie sonde/link-check dans les notes d'incident avant
  de commencer l’atténuation.
- Après atténuation (rollback ou redéploiement), ré-exécuteur `npm run probe:portal`,
  `npm run check:links`, et capturer des instantanés Grafana affichant les métriques
  revenus dans les seuils. Joignez toute la preuve à l'incident PagerDuty
  résolution avant.
- Si deux alertes se déclenchent en même temps (par ex. TLS expiry plus backlog), trier
  refus en premier (arrêter la publication), exécuter la procédure de rollback, puis
  traiter TLS/backlog avec Storage SRE sur le bridge.