---
lang: fr
direction: ltr
source: docs/portal/docs/devportal/incident-runbooks.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Runbooks d'incidents et exercices de restauration

## Proposition

L'élément de la feuille de route **DOCS-9** exige des playbooks plus un plan d'enseignement pour cela
les opérateurs du portail consigam récupèrent les falhas de envio sem adivinhacao. Esta nota cobre tres
incidents de haut sinal - déploie des falhos, des dégradations de réplication et des problèmes d'analyse - e
documenta os drills trimestrais que provam que o rollback de alias e a validacao sintetica
continuam funcionando de bout en bout.

### Matériel lié

- [`devportal/deploy-guide`](./deploy-guide) - workflow de packaging, signature et promotion d'alias.
- [`devportal/observability`](./observability) - balises de version, analyses et sondes référencées abaixo.
-`docs/source/sorafs_node_client_protocol.md`
  et [`sorafs/pin-registry-ops`](../sorafs/pin-registry-ops)
  - télémétrie de l'enregistrement et des limites d'escalade.
- `docs/portal/scripts/sorafs-pin-release.sh` et aides `npm run probe:*`
  referenciados nos listes de contrôle.

### Comparaison de télémétrie et d'outillage| Sinal / Outil | Proposé |
| ------------- | ------- |
| `torii_sorafs_replication_sla_total` (réalisé/manqué/en attente) | Détecte les blocages de réplication et les violations de SLA. |
| `torii_sorafs_replication_backlog_total`, `torii_sorafs_replication_completion_latency_epochs` | Quantification profonde du retard et de la latence de réalisation pour le triage. |
| `torii_sorafs_gateway_refusals_total`, `torii_sorafs_manifest_submit_total{status="error"}` | Il y a des erreurs dans la passerelle qui suit fréquemment un déploiement en cours. |
| `npm run probe:portal` / `npm run probe:tryit-proxy` | Sondes synthétiques qui libèrent et valident les restaurations. |
| `npm run check:links` | Porte de liens quebrados; usado apos cada mitigacao. |
| `sorafs_cli manifest submit ... --alias-*` (utilisé par `scripts/sorafs-pin-release.sh`) | Mécanisme de promotion/inversion d’alias. |
| Carte `Docs Portal Publishing` Grafana (`dashboards/grafana/docs_portal.json`) | Ajouter des télémétries de refus/alias/TLS/replicacao. Les alertes de PagerDuty font référence à ces informations comme preuves. |

## Runbook - Déployer falho ou artefato ruim

### Conditions de disparité

- Sondes de prévisualisation/production Falham (`npm run probe:portal -- --expect-release=...`).
- Alertes Grafana et `torii_sorafs_gateway_refusals_total` ou
  `torii_sorafs_manifest_submit_total{status="error"}` après le déploiement.
- Manuel d'assurance qualité nota rotas quebradas ou falhas do proxy Essayez-le immédiatement après
  une promocao do alias.

### Contenu immédiat1. **Congelar déploie :** marquer le pipeline CI avec `DEPLOY_FREEZE=1` (entrée dans le flux de travail
   GitHub) ou suspendre le travail Jenkins pour que votre art soit envoyé.
2. **Capturer les artefatos :** baixar `build/checksums.sha256`,
   `portal.manifest*.{json,to,bundle,sig}`, et a déclaré que deux sondes sont construites avec une erreur pour que
   La référence de restauration du système d'exploitation digère les exatos.
3. **Notifier les parties prenantes :** stockage SRE, responsable Docs/DevRel, et l'agent de service de
   gouvernance pour la sensibilisation (en particulier lorsque `docs.sora` est impacté).

### Procédure de restauration

1. Identifier le dernier bien connu (LKG) manifeste. Le workflow de production des armes
   `artifacts/devportal/<release>/sorafs/portal.manifest.to`.
2. Re-vincule o alias a esse manifest com o helper de shipping:

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

3. Enregistrez le résumé du rollback sans ticket d'incident avec les résumés d'exploitation.
   manifeste LKG et manifeste avec falha.

### Validacao

1. `npm run probe:portal -- --expect-release=${LKG_TAG}`.
2. `npm run check:links`.
3. `sorafs_cli manifest verify-signature ...` et `sorafs_cli proof verify ...`
   (voir le guide de déploiement) pour confirmer que le manifeste est repromovido continua
   batendo com o CAR arquivado.
4. `npm run probe:tryit-proxy` pour garantir que le proxy Try-It staging soit disponible.

### Pos-incident

1. Réaliser le pipeline de déploiement après avoir entendu la cause.
2. Preencha comme entrées "Leçons apprises" dans [`devportal/deploy-guide`](./deploy-guide)
   avec de nouveaux ponts, se houver.
3. Rechercher des défauts pour une suite de testes falhada (sonde, vérificateur de liens, etc.).## Runbook - Dégradation de la réplication

### Conditions de disparité

- Alerte : `sum(torii_sorafs_replication_sla_total{outcome="met"}) /
  clamp_min(sum(torii_sorafs_replication_sla_total{outcome=~"met|missed"}), 1) <
  0,95` pour 10 minutes.
- `torii_sorafs_replication_backlog_total > 10` pendant 10 minutes (voir
  `pin-registry-ops.md`).
- Governanca reporta alias lento après une libération.

### Triage

1. Inspecione tableaux de bord de [`sorafs/pin-registry-ops`](../sorafs/pin-registry-ops) pour
   Confirmez que le backlog est localisé dans une classe de stockage ou dans une flotte de fournisseurs.
2. Les journaux Cruze font Torii pour les avertissements `sorafs_registry::submit_manifest` pour déterminer soi
   comme les soumissions estao falhando.
3. Amostre a saude das répliques via `sorafs_cli manifest status --manifest ...` (liste
   résultats par le fournisseur).

### Mitigacao

1. Réemita le manifeste avec le plus grand virus de répliques (`--pin-min-replicas 7`) en utilisant
   `scripts/sorafs-pin-release.sh` pour que le planificateur distribue la charge dans un ensemble plus important
   des fournisseurs. Enregistrez le nouveau résumé sans aucun incident.
2. Voir le backlog estiver preso a un fournisseur unique, désactivé temporairement via o
   planificateur de réplication (documenté dans `pin-registry-ops.md`) et envie une nouvelle
   manifest forcando os autres fournisseurs aactualizar o alias.
3. Quand la fraîcheur de l'alias est la plus critique pour la parité de réplication, re-vincule le
   alias un manifeste alors que celui-ci est en staging (`docs-preview`), après public un manifeste de
   accompagnement lorsque le SRE élimine le retard.### Récupération et récupération

1. Surveillez `torii_sorafs_replication_sla_total{outcome="missed"}` pour garantir que
   contador stabilise.
2. Capturez un dit `sorafs_cli manifest status` comme preuve que chaque réplique est en cours
   une conformité.
3. Commencez ou actualisez le post-mortem du backlog de réplication avec les étapes suivantes
   (mise à l'échelle des fournisseurs, réglage du chunker, etc.).

## Runbook - Queda d'analyse ou de télémétrie

### Conditions de disparité

- `npm run probe:portal` passe, mais les tableaux de bord configurent les événements à faire
  `AnalyticsTracker` pendant >15 minutes.
- Examen de la confidentialité concernant une augmentation inattendue des événements supprimés.
- `npm run probe:tryit-proxy` falha em chemins `/probe/analytics`.

### Réponse

1. Vérifier les entrées de build : `DOCS_ANALYTICS_ENDPOINT` e
   `DOCS_ANALYTICS_SAMPLE_RATE` aucun artefato ne publie (`build/release.json`).
2. Réexécutez `npm run probe:portal` avec `DOCS_ANALYTICS_ENDPOINT` en réponse à l'opération.
   collecteur de staging pour confirmer que le tracker a déjà émis des charges utiles.
3. Les collecteurs sont arrêtés, définis `DOCS_ANALYTICS_ENDPOINT=""` et reconstruits.
   pour que le tracker soit en court-circuit ; enregistrer une janela de panne na linha
   faire le tempo faire l'incident.
4. Validez que `scripts/check-links.mjs` a également une empreinte digitale de `checksums.sha256`.
   (quedas de Analytics *nao* doivent bloquer la validation du plan du site).
5. Quand le collecteur est allumé, j'ai utilisé `npm run test:widgets` pour effectuer les tests unitaires
   faire une aide d'analyse avant de republier.

### Pos-incident1. Actualisez [`devportal/observability`](./observability) avec les nouvelles limites du collecteur
   ou requisitos de amostragem.
2. Ouvrez un avis de gouvernance sur les données d'analyse du forum perdu ou redigidos
   la politique.

## Exercices trimestriels de résilience

Exécuter les deux exercices au cours d'une **première terca-feira de cada trimestre** (janvier/avril/juillet/sortie)
ou immédiatement après tout changement majeur d'infrastructure. Armazene artefatos em
`artifacts/devportal/drills/<YYYYMMDD>/`.

| Perceuse | Passos | Preuve |
| ----- | ----- | -------- |
| Ensaio de rollback de l'alias | 1. Répétez le rollback de "Deploy falho" en utilisant le manifeste de production le plus récent.2. Re-vincular a producao asim que os sondes passarem.3. Registraire `portal.manifest.submit.summary.json` e logs de sondes na pâtes do perceuse. | `rollback.submit.json`, dit des sondes, et l'étiquette de libération est enregistrée. |
| Auditorium de validation synthétique | 1. Rodar `npm run probe:portal` et `npm run probe:tryit-proxy` contre la production et la mise en scène.2. Rodar `npm run check:links` et archiveur `build/link-report.json`.3. Ajouter des captures d'écran/exportations de paineis Grafana confirmant le succès des sondes. | Journaux de sondes + `link-report.json` référençant l'empreinte digitale manifeste. |

Escalone exerce des pertes pour le gestionnaire de Docs/DevRel et la révision de la gouvernance de SRE,
Mais la feuille de route exige des preuves trimestrielles déterminantes pour le retour en arrière de l'alias et des os
les sondes font portal continuam saudaveis.

## Coordenacao PagerDuty et de garde- Le service PagerDuty **Docs Portal Publishing** et donne deux alertes générées à partir de
  `dashboards/grafana/docs_portal.json`. Comme regras `DocsPortal/GatewayRefusals`,
  `DocsPortal/AliasCache`, et `DocsPortal/TLSExpiry` page ou primaire de Docs/DevRel
  com Storage SRE comme secondaire.
- Quando a pagina tocar, inclua o `DOCS_RELEASE_TAG`, anexe captures d'écran dos paineis Grafana
  afetados e linke a dita de probe/link-check nas notas do incidente avant de démarrer
  un mitigacao.
- Après l'atténuation (rollback ou redéploiement), réexécutez `npm run probe:portal`,
  `npm run check:links`, et capturer des instantanés Grafana récents affichés en tant que mesures
  de volta aos seuils. Anexe toda a évidence ao incidente do PagerDuty
  avant le résolveur.
- Si deux alertes disparaissent à mon rythme (par exemple, l'expiration de TLS plus de backlog),
  trier les refus en premier (pour la publication), exécuter la procédure de restauration et
  Une fois résolu, TLS/backlog avec Storage SRE sur le pont.