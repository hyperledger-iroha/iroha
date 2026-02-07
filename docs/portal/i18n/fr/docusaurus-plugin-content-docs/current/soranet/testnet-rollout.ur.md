---
lang: fr
direction: ltr
source: docs/portal/docs/soranet/testnet-rollout.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : testnet-rollout
titre : Déploiement de SoraNet testnet (SNNet-10)
sidebar_label : déploiement de Testnet (SNNet-10)
la description : Il s'agit d'un plan d'activation, d'un kit d'intégration et de promotions SoraNet testnet pour les portes de télémétrie.
---

:::note Source canonique
Il s'agit du plan de déploiement `docs/source/soranet/testnet_rollout_plan.md` du SNNet-10. جب تک پرانا docs set retiré نہ ہو، دونوں کاپیاں رکھیں۔
:::

SNNet-10 est disponible pour la superposition d'anonymat SoraNet et l'activation et les coordonnées. Le plan, le plan, la feuille de route, les livrables concrets, les runbooks, et les portes de télémétrie sont pour l'opérateur et l'opérateur. Comment utiliser le transport par défaut SoraNet

## Phases de lancement

| Phases | Chronologie (cible) | Portée | Objets requis |
|-------|---------|-------|----------|
| **T0 – Réseau de test fermé** | T4 2026 | >=3 ASN pour 20 à 50 relais et les principaux contributeurs | Kit d'intégration Testnet, suite de fumée d'épinglage de garde, latence de base + métriques PoW, journal d'exercice de baisse de tension. |
| **T1 – Bêta publique** | T1 2027 | >=100 relais, rotation de garde activée, liaison de sortie appliquée, version bêta du SDK par défaut pour SoraNet et `anon-guard-pq`. | Kit d'intégration mis à jour, liste de contrôle de vérification de l'opérateur, publication d'annuaire SOP, pack de tableaux de bord de télémétrie, rapports de répétition d'incidents. |
| **T2 - Réseau principal par défaut** | T2 2027 (achèvement du SNNet-6/7/9 dans un cadre fermé) | Réseau de production SoraNet par défaut transports obfs/MASQUE et application du cliquet PQ activée | Procès-verbaux d'approbation de la gouvernance, procédure de restauration directe uniquement, alarmes de rétrogradation, rapport de mesures de réussite signé. |

** کوئی sauter le chemin نہیں** - ہر phase کو پچھلے مرحلے کی télémétrie et artefacts de gouvernance navire کرنا لازمی ہے قبل از promotion۔

## Kit d'intégration Testnet

Un opérateur de relais fournit un paquet déterministe pour:

| Artefact | Descriptif |
|--------------|-------------|
| `01-readme.md` | Aperçu, points de contact, et chronologie |
| `02-checklist.md` | Liste de contrôle avant le vol (matériel, accessibilité du réseau, vérification de la politique de garde). |
| `03-config-example.toml` | Les blocs de conformité SNNet-9 sont alignés avec la configuration minimale du relais SoraNet + orchestrateur, comme le bloc `guard_directory` et le hachage d'instantané de garde et la broche de hachage. |
| `04-telemetry.md` | Tableaux de bord des mesures de confidentialité SoraNet et seuils d'alerte filaires |
| `05-incident-playbook.md` | Procédure de réponse en cas de baisse de tension/déclassement et matrice d'escalade |
| `06-verification-report.md` | Modèle pour les tests de fumée des opérateurs |

Copie rendue `docs/examples/soranet_testnet_operator_kit/` میں موجود ہے۔ ہر promotion میں kit actualiser ہوتا ہے؛ la phase des numéros de version suit کرتے ہیں (مثال کے طور پر `testnet-kit-vT0.1`).

Les opérateurs bêta publics (T1) utilisent `docs/source/soranet/snnet10_beta_onboarding.md` avec de brèves conditions préalables d'intégration concises, les livrables de télémétrie et le flux de travail de soumission, ainsi qu'un kit déterministe et des aides aux validateurs. کرتا ہے۔`cargo xtask soranet-testnet-feed` Flux JSON pour une fenêtre de promotion, une liste de relais, un rapport de métriques, des preuves de forage et des hachages de pièces jointes et un agrégat de référence de modèle de porte de scène. پہلے `cargo xtask soranet-testnet-drill-bundle` pour les journaux de forage et les pièces jointes signent le flux `drill_log.signed = true` record کر سکے۔

## Indicateurs de réussite

Phases de promotion pour la télémétrie et la télémétrie fermée et pour la promotion de la télémétrie :

- `soranet_privacy_circuit_events_total` : baisse de tension de 95 % des circuits et déclassement Pour un approvisionnement de 5 % en PQ
- `sorafs_orchestrator_policy_events_total{outcome="brownout"}` : sessions de récupération planifiées pour <1 % d'exercices planifiés et déclencheur de baisse de tension
- `soranet_privacy_gar_reports_total` : mélange de catégories GAR et variance +/-10 % pics et mises à jour de politiques approuvées et expliquer
- Taux de réussite des tickets PoW : 3 s dans la fenêtre cible >=99 % ; `soranet_privacy_throttles_total{scope="congestion"}` rapport de recherche en ligne
- Latence (95e percentile) par région : circuits de capture <200 ms, `soranet_privacy_rtt_millis{percentile="p95"}` pour capture et capture

Modèles d'alerte du tableau de bord `dashboard_templates/` et `alert_templates/` Il s'agit d'un référentiel de télémétrie et d'un miroir ainsi que de contrôles de charpie CI. Promotion du rapport sur la gouvernance en cours de promotion `cargo xtask soranet-testnet-metrics` استعمال کریں۔

Soumissions Stage-gate comme `docs/source/soranet/snnet10_stage_gate_template.md` pour le formulaire Markdown et `docs/examples/soranet_testnet_stage_gate/stage_gate_report_template.md` comme formulaire Markdown prêt à copier pour le formulaire de demande de paiement

## Liste de contrôle de vérification

La phase de démarchage des opérateurs et les opérateurs de phase sont chargés de l'approbation du projet :

- ✅ Annonce relais موجودہ enveloppe d'admission کے ساتھ signée ہو۔
- ✅ Test de fumée de rotation de garde (`tools/soranet-relay --check-rotation`) ici
- ✅ `guard_directory` تازہ ترین `GuardDirectorySnapshotV2` artefact کی طرف اشارہ کرے اور `expected_directory_hash_hex` résumé du comité سے match (journal de hachage validé au démarrage du relais کرتا ہے)۔
- ✅ Métriques à cliquet PQ (`sorafs_orchestrator_pq_ratio`) pour l'étape et les seuils cibles.
- ✅ La configuration de conformité GAR est une balise qui correspond à (catalogue SNNet-9)
- ✅ Simulation d'alarme de déclassement (les collecteurs désactivent l'alerte de 5 minutes, attendez کریں)۔
- ✅ Étapes d'atténuation documentées de l'exercice PoW/DoS pour exécuter et exécuter

Kit d'intégration de modèles pré-remplis میں شامل ہے۔ Les opérateurs travaillent au service d'assistance sur la gouvernance et soumettent des informations d'identification de production.

## Gouvernance et reporting

- **Contrôle des changements :** promotions et approbation du Conseil de gouvernance et procès-verbaux du conseil et page d'état en pièce jointe.
- **Résumé du statut :** Plusieurs mises à jour et nombre de relais, ratio PQ, incidents de baisse de tension et éléments d'action en suspens (cadence et nombre de relais) `docs/source/status/soranet_testnet_digest.md` میں محفوظ)۔
- **Annulations :** Un plan de restauration signé a été signé pour une phase de 30 minutes de lecture. Invalidation du cache DNS/guard et modèles de communication client ici

## Actifs pris en charge- `cargo xtask soranet-testnet-kit [--out <dir>]` `xtask/templates/soranet_testnet/` est un kit d'intégration et un répertoire cible qui se matérialise (par défaut `docs/examples/soranet_testnet_operator_kit/`).
- Les mesures de réussite `cargo xtask soranet-testnet-metrics --input <metrics.json> [--out <path|->]` SNNet-10 évaluent les examens de gouvernance et les rapports structurés de réussite/échec émettent des rapports de réussite/échec. Exemple d'instantané `docs/examples/soranet_testnet_metrics_sample.json` میں موجود ہے۔
- Grafana et modèles Alertmanager `dashboard_templates/soranet_testnet_overview.json` et `alert_templates/soranet_testnet_rules.yml` pour les modèles de gestionnaire d'alertes Référentiel de télémétrie et copie des contrôles de charpie CI et fil de fer
- Messagerie SDK/portail et modèle de communication de rétrogradation `docs/source/soranet/templates/downgrade_communication_template.md` میں ہے۔
- Résumés d'état hebdomadaires sous forme canonique sous `docs/source/status/soranet_testnet_weekly_digest.md` sous forme canonique

Pull request pour les artefacts et la télémétrie pour les mises à jour et le plan de déploiement canonique