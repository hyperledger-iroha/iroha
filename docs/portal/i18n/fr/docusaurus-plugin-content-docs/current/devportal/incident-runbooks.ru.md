---
lang: fr
direction: ltr
source: docs/portal/docs/devportal/incident-runbooks.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Rollback des incidents et des exercices

## Назначение

Les nouvelles cartes **DOCS-9** proposent des playbooks et des plans de répétition, des choses
Les opérateurs du portail peuvent s'occuper de l'expédition après l'expédition du chien. C'est ça
охватывает три высокосигнальных инцидента - неудачные деплои, деградацию репликации и
сбои аналитики - и документирует квартальные тренировки, доказывающие, что rollback alias
et la validation synthétique produit un travail de bout en bout.

### Matériaux suisses

- [`devportal/deploy-guide`](./deploy-guide) — workflow упаковки, подписи и promotion alias.
- [`devportal/observability`](./observability) — balises de version, analyses et sondes, etc.
-`docs/source/sorafs_node_client_protocol.md`
  et [`sorafs/pin-registry-ops`](../sorafs/pin-registry-ops)
  — chaînes télémétriques et programmes de télémétrie.
- `docs/portal/scripts/sorafs-pin-release.sh` et aides `npm run probe:*`
  упомянуты в чеклистах.

### Surveillance de la télévision et des instruments| Signal / Instrument | Назначение |
| ------------- | ------- |
| `torii_sorafs_replication_sla_total` (réalisé/manqué/en attente) | Vous pouvez effectuer des réplications et effectuer des SLA. |
| `torii_sorafs_replication_backlog_total`, `torii_sorafs_replication_completion_latency_epochs` | Réduire l'arriéré et assurer le triage. |
| `torii_sorafs_gateway_refusals_total`, `torii_sorafs_manifest_submit_total{status="error"}` | Lorsque vous utilisez la passerelle de stockage, vous devez procéder au déploiement du projet. |
| `npm run probe:portal` / `npm run probe:tryit-proxy` | Les sondes synthétiques, les déclenchements de portes et les rollbacks vérifiés. |
| `npm run check:links` | Porte битых ссылок; используется после каждой atténuation. |
| `sorafs_cli manifest submit ... --alias-*` (voir `scripts/sorafs-pin-release.sh`) | Alias ​​de promotion/reversion du mécanisme. |
| Carte `Docs Portal Publishing` Grafana (`dashboards/grafana/docs_portal.json`) | Агрегирует refus/alias/TLS/réplication télémétrique. Les alertes PagerDuty sont envoyées à ces panneaux pour le document. |

## Ранбук - Неудачный деплой или плохой артефакт

### Planification de l'emploi

- Le module de prévisualisation/production des tests (`npm run probe:portal -- --expect-release=...`).
- Alertes Grafana pour `torii_sorafs_gateway_refusals_total` ou
  `torii_sorafs_manifest_submit_total{status="error"}` après le déploiement.
- QA вручную замечает сломанные маршруты или сбои proxy Essayez-le plus tard
  pseudonyme de promotion.

### Немедленное сдерживание1. **Déployer :** Supprimer le pipeline CI `DEPLOY_FREEZE=1` (workflow GitHub d'entrée)
   Si vous prenez le travail de Jenkins, de nouveaux œuvres d'art ne sont pas disponibles.
2. **Зафиксировать артефакты:** скачать `build/checksums.sha256`,
   `portal.manifest*.{json,to,bundle,sig}`, et les sondes en cas d'échec de construction,
   чтобы rollback ссылался на точные digests.
3. **Уведомить стейкхолдеров :** stockage SRE, responsable Docs/DevRel et responsable de la gouvernance
   (особенно если затронут `docs.sora`).

### Procédure de restauration

1. Ouvrir le manifeste du dernier bien connu (LKG). Flux de travail de production
   `artifacts/devportal/<release>/sorafs/portal.manifest.to`.
2. Prenez l'alias de ce manifeste avec votre assistant d'expédition :

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

3. Affichez le résumé de l'annulation dans le ticket d'incident en même temps que le résumé LKG et le manifeste immédiat.

### Validation

1. `npm run probe:portal -- --expect-release=${LKG_TAG}`.
2. `npm run check:links`.
3. `sorafs_cli manifest verify-signature ...` et `sorafs_cli proof verify ...`
   (avec guide de déploiement), чтобы убедиться, что repromoted manifest совпадает с архивным CAR.
4. `npm run probe:tryit-proxy` est à utiliser pour la version du proxy de transfert Try-It.

### Après un incident

1. Verrouiller le déploiement du pipeline uniquement après avoir identifié la cause première.
2. Ajouter la section "Leçons apprises" dans [`devportal/deploy-guide`](./deploy-guide)
   новыми выводами, если есть.
3. Vérifiez les défauts pour les tests de vérification (sonde, vérificateur de liens, etc.).

## Ранбук - Réplications de dégradation

### Planification de l'emploi- Alerte : `sum(torii_sorafs_replication_sla_total{outcome="met"}) /
  clamp_min(sum(torii_sorafs_replication_sla_total{outcome=~"met|missed"}), 1) <
  0,95` dans la technologie 10 minutes.
- `torii_sorafs_replication_backlog_total > 10` en technologie 10 minutes (см.
  `pin-registry-ops.md`).
- La gouvernance concerne la mise à jour immédiate de l'alias après la sortie.

### Triage

1. Vérifier les tableaux de bord [`sorafs/pin-registry-ops`](../sorafs/pin-registry-ops), ici
   Vous pouvez localiser le retard dans la classe de stockage ou chez les fournisseurs de flotte.
2. Sauvegardez le journal Torii sur `sorafs_registry::submit_manifest` pour l'exécuter.
   не падают ли soumissions.
3. Vous devez vérifier la réponse à partir de `sorafs_cli manifest status --manifest ...`
   (показывает исходы репликации по провайдерам).

### Atténuation

1. Ouvrir le manifeste avec votre réponse (`--pin-min-replicas 7`) ici
   `scripts/sorafs-pin-release.sh`, ce planificateur a été déployé pour le plus grand nombre
   провайдеров. Ajoutez un nouveau résumé dans le journal des incidents.
2. Si le backlog est utilisé par un fournisseur particulier, vous ouvrez régulièrement votre planificateur de réplication.
   (opisé dans `pin-registry-ops.md`) et ouvrir un nouveau manifeste, en premier lieu
   провайдеров обновить alias.
3. Lorsque vous envisagez d'utiliser un alias pour des réplications de parité, reliez l'alias au manifeste chaud lors de la mise en scène.
   (`docs-preview`), vous devez ouvrir le manifeste de suivi après la suppression du retard SRE.

### Récupération et clôture1. Surveillez `torii_sorafs_replication_sla_total{outcome="missed"}` et détectez-le
   счетчик стабилизировался.
2. Examinez votre `sorafs_cli manifest status` en tant que preuve, pour que la réplique soit conforme à la norme.
3. Gérer ou éliminer le retard de réplication post-mortem avec vos clients
   (масштабирование провайдеров, tuning chunker, и т.д.).

## Ранбук - Ouvrir l'analyse ou la télémétrie

### Planification de l'emploi

- `npm run probe:portal` fonctionne, mais les tableaux de bord ne sont pas compatibles avec la sécurité
  `AnalyticsTracker` est disponible dans 15 minutes.
- L'examen de la confidentialité фиксирует неожиданный рост a supprimé les événements.
- `npm run probe:tryit-proxy` est compatible avec le `/probe/analytics`.

### Réponse

1. Vérifier les entrées au moment de la construction : `DOCS_ANALYTICS_ENDPOINT` et
   `DOCS_ANALYTICS_SAMPLE_RATE` dans l'artefact de libération (`build/release.json`).
2. Connectez `npm run probe:portal` à `DOCS_ANALYTICS_ENDPOINT`, en premier.
   Dans le collecteur de staging, vous devez mettre à jour le tracker qui produit les charges utiles.
3. Si les collecteurs ne sont pas installés, installez `DOCS_ANALYTICS_ENDPOINT=""` et reconstruisez,
   чтобы tracker court-circuit; indiquez la panne en cas de panne dans la chronologie de l'incident.
4. Vérifier que `scripts/check-links.mjs` produit l'empreinte digitale `checksums.sha256`
   (analytique сбои *не* должны блокировать проверку sitemap).
5. Après la mise en service du collecteur, installez `npm run test:widgets` pour effectuer le programme.
   Assistant d'analyse des tests unitaires avant la réédition.

### Après un incident1. Ouvrir [`devportal/observability`](./observability) avec de nouveaux logiciels
   collecteur ou échantillonnage.
2. Consultez l'avis de gouvernance, si vous avez recours à des analyses ou à des retraits.
   вне политики.

## Квартальные учения по устойчивости

Запускайте оба drill в **первый вторник каждого квартала** (Jan/Apr/Jul/Oct)
ou srazу после любого крупного изменения инфраструктуры. Храните артефакты в
`artifacts/devportal/drills/<YYYYMMDD>/`.

| Учение | Shagi | Доказательства |
| ----- | ----- | -------- |
| Répétition de la restauration de l'alias | 1. Effectuez la restauration "Échec du déploiement" avec le même manifeste de production.2. Re-lier на production после успешных sondes.3. Connectez les sondes `portal.manifest.submit.summary.json` et logiques à la perceuse à papier. | `rollback.submit.json`, utilisez les sondes et les répétitions d'étiquettes de libération. |
| Audits de validation synthétique | 1. Lancez `npm run probe:portal` et `npm run probe:tryit-proxy` pour la production et la mise en scène.2. Запустить `npm run check:links` и ARCHивировать `build/link-report.json`.3. Utilisez les captures d'écran/exportations du panneau Grafana, en modifiant vos sondes. | Les sondes log + `link-report.json` correspondent au manifeste d'empreintes digitales. |

Développer les exercices proposés par Docs/DevRel et examiner la gouvernance SRE,
En ce qui concerne la feuille de route, il faut déterminer les détails des documents, c'est-à-dire la restauration de l'alias
и portail sondes остаются здоровыми.

## PagerDuty et coordination de garde- Le service PagerDuty **Docs Portal Publishing** vous envoie des alertes
  `dashboards/grafana/docs_portal.json`. Правила `DocsPortal/GatewayRefusals`,
  `DocsPortal/AliasCache` et `DocsPortal/TLSExpiry` pour Docs/DevRel primaire
  с Stockage SRE как secondaire.
- Lorsque vous sélectionnez `DOCS_RELEASE_TAG`, affichez les captures d'écran des captures d'écran.
  Panneau Grafana et assurez-vous de vérifier la sonde/le lien dans les zones d'incendie
  начала atténuation.
- Après l'atténuation (restauration ou redéploiement), vous pouvez installer `npm run probe:portal`,
  `npm run check:links` et sélectionnez les instantanés Grafana pour déterminer la mesure du débit
  в пороги. Utilisez toutes les preuves de l'incident de PagerDuty pour la sécurité.
- Si une alerte survient régulièrement (par exemple expiration et retard de TLS), cela se produit
  refus de triage (publication), выполнить rollback, затем закрыть TLS/backlog
  с Stockage SRE sur pont.