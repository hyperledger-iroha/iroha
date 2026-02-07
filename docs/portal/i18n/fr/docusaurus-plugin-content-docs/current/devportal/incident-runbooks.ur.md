---
lang: fr
direction: ltr
source: docs/portal/docs/devportal/incident-runbooks.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# انسیڈنٹ رن بکس اور رول بیک ڈرلز

## مقصد

روڈمیپ آئٹم **DOCS-9** قابل عمل playbooks اور répétition پلان کا تقاضا کرتا ہے تاکہ
پورٹل آپریٹرز ڈلیوری فیلئرز سے بغیر اندازے کے ریکور کر سکیں۔ یہ نوٹ تین ہائی سگنل
Problèmes — Déploiements récents, dégradation de la réplication et pannes d'analyse — En savoir plus
Pour les exercices trimestriels et pour la récupération des alias et pour la validation synthétique
اب بھی bout à bout کام کرتے ہیں۔

### متعلقہ مواد

- [`devportal/deploy-guide`](./deploy-guide) — packaging, signature et workflow de promotion d'alias
- [`devportal/observability`](./observability) — balises de version, analyses et sondes pour les utilisateurs
-`docs/source/sorafs_node_client_protocol.md`
  et [`sorafs/pin-registry-ops`](../sorafs/pin-registry-ops)
  — télémétrie du registre et seuils d'escalade
- Aides `docs/portal/scripts/sorafs-pin-release.sh` et `npm run probe:*`
  جو چیک لسٹس میں ریفرنس ہیں۔

### مشترکہ télémétrie et outillage| Signal / Outil | مقصد |
| ------------- | ------- |
| `torii_sorafs_replication_sla_total` (réalisé/manqué/en attente) | la réplication se bloque et les violations SLA sont détectées |
| `torii_sorafs_replication_backlog_total`, `torii_sorafs_replication_completion_latency_epochs` | profondeur de l'arriéré et latence d'achèvement et triage pour quantifier les tâches |
| `torii_sorafs_gateway_refusals_total`, `torii_sorafs_manifest_submit_total{status="error"}` | échecs côté passerelle pendant le déploiement et le déploiement |
| `npm run probe:portal` / `npm run probe:tryit-proxy` | les sondes synthétiques et libèrent la porte et les rollbacks valident les choses |
| `npm run check:links` | portail à maillons brisés ; ہر atténuation کے بعد استعمال ہوتا ہے۔ |
| `sorafs_cli manifest submit ... --alias-*` (`scripts/sorafs-pin-release.sh` en anglais) | mécanisme de promotion/réversion d'alias۔ |
| Carte `Docs Portal Publishing` Grafana (`dashboards/grafana/docs_portal.json`) | refus/alias/TLS/télémétrie de réplication et agrégation PagerDuty alerte les panneaux et les preuves et se réfère à la question |

## Runbook - Déploiement de nouveaux artefacts

### شروع ہونے کی شرائط

- Les sondes de prévisualisation/production échouent (`npm run probe:portal -- --expect-release=...`).
- Alertes Grafana sur `torii_sorafs_gateway_refusals_total` یا
  Déploiement `torii_sorafs_manifest_submit_total{status="error"}` en cours
- Promotion manuelle des alias QA pour les routes cassées et Essayez-le, échecs de proxy

### فوری روک تھام1. **Les déploiements sont gelés :** Pipeline CI sous `DEPLOY_FREEZE=1` et marque (entrée de workflow GitHub)
   La pause de travail de Jenkins s'occupe des artefacts.
2. **Capture des artefacts کریں :** échec de la construction کے `build/checksums.sha256`,
   `portal.manifest*.{json,to,bundle,sig}`, la sortie de la sonde est en cours de restauration
   عین digère et référence کرے۔
3. **Parties prenantes et responsables :** stockage SRE, responsable Docs/DevRel, responsable de la gouvernance
   (خصوصاً جب `docs.sora` متاثر ہو)۔

### رول بیک طریقہ کار

1. manifeste du dernier bien connu (LKG) workflow de production انہیں
   `artifacts/devportal/<release>/sorafs/portal.manifest.to` میں اسٹور کرتا ہے۔
2. assistant d'expédition ou alias ou manifeste pour lier la personne :

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

3. Résumé de l'annulation pour un ticket d'incident ou un LKG et un résumé du manifeste ayant échoué.

### توثیق

1. `npm run probe:portal -- --expect-release=${LKG_TAG}`.
2. `npm run check:links`.
3. `sorafs_cli manifest verify-signature ...` et `sorafs_cli proof verify ...`
   (guide de déploiement ici) Il s'agit d'un manifeste repromoté archivé CAR et correspond à une correspondance
4. `npm run probe:tryit-proxy` pour le proxy de mise en scène Try-It pour le client

### واقعے کے بعد

1. cause première du pipeline de déploiement du pipeline de déploiement
2. [`devportal/deploy-guide`](./deploy-guide) Entrées « Leçons apprises » avec des points et des points
3. suite de tests défaillants (sonde, vérificateur de liens, etc.) pour les défauts et les défauts

## Runbook - ریپلیکیشن میں گراوٹ

### شروع ہونے کی شرائط- Titre : `sum(torii_sorafs_replication_sla_total{outcome="met"}) /
  clamp_min(sum(torii_sorafs_replication_sla_total{outcome=~"met|missed"}), 1) <
  0.95` 10 minutes de jeu
- `torii_sorafs_replication_backlog_total > 10` 10 minutes (`pin-registry-ops.md` دیکھیں)۔
- Gouvernance ریلیز کے بعد alias کی دستیابی سست ہونے کی رپورٹ کرے۔

### ابتدائی جانچ

1. [`sorafs/pin-registry-ops`](../sorafs/pin-registry-ops) tableaux de bord qui traitent de l'arriéré
   Classe de stockage et flotte du fournisseur
2. Torii enregistre les avertissements `sorafs_registry::submit_manifest` et les soumissions échouent ou les soumissions échouent. نہیں۔
3. `sorafs_cli manifest status --manifest ...` pour un échantillon de santé de réplique (résultats par fournisseur)

### تخفیفی اقدامات

1. `scripts/sorafs-pin-release.sh` compte le nombre de répliques (`--pin-min-replicas 7`) et le manifeste est affiché.
   Chargement du planificateur et fournisseurs de services pour les fournisseurs نیا digest incident log میں ریکارڈ کریں۔
2. Ajouter un fournisseur de backlog et un planificateur de réplication pour désactiver l'option
   (`pin-registry-ops.md` documenté) Le manifeste est soumis pour les fournisseurs de services et l'actualisation de l'alias pour le client
3. La fraîcheur de l'alias, la parité de réplication est critique et la réplication de l'alias est un manifeste par étapes (`docs-preview`) pour la reliure.
   Pour SRE, le retard est éliminé et le manifeste de suivi est publié.

### بحالی اور اختتام1. `torii_sorafs_replication_sla_total{outcome="missed"}` moniteur et plateau de comptage
2. Sortie `sorafs_cli manifest status` et preuves pour capturer des objets et des répliques conformes aux normes
3. post-mortem du backlog de réplication (mise à l'échelle du fournisseur, réglage du chunker, etc.)

## Runbook - اینالیٹکس یا ٹیلیمیٹری آؤٹیج

### شروع ہونے کی شرائط

- `npm run probe:portal` prend en charge les tableaux de bord `AnalyticsTracker` événements > 15 minutes pour ingérer plus de détails
- Événements supprimés de l'examen de la confidentialité
- Les chemins `npm run probe:tryit-proxy` `/probe/analytics` échouent ہو۔

### جوابی اقدامات

1. Les entrées au moment de la construction sont vérifiées : `DOCS_ANALYTICS_ENDPOINT` et `DOCS_ANALYTICS_SAMPLE_RATE`.
   artefact de version défaillante (`build/release.json`) میں۔
2. `DOCS_ANALYTICS_ENDPOINT` est un collecteur intermédiaire pour le point de collecte `npm run probe:portal` qui permet aux charges utiles du tracker d'émettre des charges utiles de suivi.
3. Les collecteurs sont en panne et `DOCS_ANALYTICS_ENDPOINT=""` est réglé pour reconstruire le court-circuit du tracker. Chronologie des incidents de fenêtre de panne
4. valider l'empreinte digitale `scripts/check-links.mjs` et l'empreinte digitale `checksums.sha256`
   (pannes d'analyse et validation du plan du site *bloc* en cours de fonctionnement)۔
5. Récupération du collecteur pour `npm run test:widgets` Les tests unitaires d'aide à l'analyse sont exécutés pour la réédition et la réédition

### واقعے کے بعد1. [`devportal/observability`](./observability) Limites des collecteurs et exigences d'échantillonnage pour les collecteurs
2. La politique relative aux données analytiques et la suppression ou la rédaction d'un avis de gouvernance ou d'un avis de gouvernance

## سہ ماہی استقامت کی مشقیں

دونوں exercices **ہر trimestre کے پہلے منگل** (Jan/Avr/Jul/Oct) کو چلائیں
Pour le changement d'infrastructure et pour le changement d'infrastructure artefacts
`artifacts/devportal/drills/<YYYYMMDD>/` est un appareil photo

| مشق | مراحل | شواہد |
| ----- | ----- | -------- |
| Restauration de l'alias | 1. تازہ ترین manifeste de production کے ساتھ "Échec du déploiement" rollback دوبارہ چلائیں۔2. les sondes passent ہونے کے بعد production پر re-liaison کریں۔3. `portal.manifest.submit.summary.json` Journaux de sonde et forage de forage | `rollback.submit.json`, sortie de sonde, répétition et étiquette de sortie |
| مصنوعی توثیق کا آڈٹ | 1. production et mise en scène کے خلاف `npm run probe:portal` اور `npm run probe:tryit-proxy` چلائیں۔2. `npm run check:links` چلائیں اور `build/link-report.json` archive کریں۔3. Panneaux Grafana avec captures d'écran/exportations jointes et confirmation du succès de la sonde | Journaux de sonde + `link-report.json` et empreinte digitale manifeste |

Exercices manqués avec le gestionnaire Docs/DevRel et l'examen de la gouvernance SRE et l'escalade avec la feuille de route et l'examen de la gouvernance SRE.
alias rollback et les sondes du portail sont des preuves déterministes et trimestrielles.

## PagerDuty et coordination d'astreinte- Service PagerDuty **Docs Portal Publishing** `dashboards/grafana/docs_portal.json` Alertes de sécurité et d'alertes en ligne
  Pour `DocsPortal/GatewayRefusals`, `DocsPortal/AliasCache`, et `DocsPortal/TLSExpiry` Docs/DevRel page principale
  اور Storage SRE secondaire ہوتا ہے۔
- la page `DOCS_RELEASE_TAG` contient des panneaux Grafana et des captures d'écran sont jointes pour l'atténuation des effets.
  sortie de vérification de sonde/lien et notes d'incident et lien
- atténuation (restauration et redéploiement) par `npm run probe:portal`, `npm run check:links` et par Grafana
  les instantanés capturent des mesures et des seuils et des mesures Voir les preuves de l'incident PagerDuty en pièce jointe
  قبل ازیں کہ اسے résoudre کیا جائے۔
- Il y a des alertes et des incendies (en cas d'expiration de TLS et d'arriérés), ainsi que des refus et un triage.
  (publication) et procédure de restauration pour le stockage SRE et le pont et TLS/backlog pour le stockage