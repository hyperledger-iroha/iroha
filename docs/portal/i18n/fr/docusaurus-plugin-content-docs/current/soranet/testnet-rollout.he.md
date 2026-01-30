---
lang: he
direction: rtl
source: docs/portal/i18n/fr/docusaurus-plugin-content-docs/current/soranet/testnet-rollout.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 09747c3cbcf61354c9e208252e4d6c0fbd6753c1f2774ed1d3098f5a7b6058cc
source_last_modified: "2025-11-14T04:43:22.507827+00:00"
translation_last_reviewed: 2026-01-30
---

:::note Source canonique
Cette page reflete le plan de rollout SNNet-10 dans `docs/source/soranet/testnet_rollout_plan.md`. Gardez les deux copies alignees jusqu'a la retraite des docs historiques.
:::

SNNet-10 coordonne l'activation par etapes de l'overlay d'anonymat SoraNet sur tout le reseau. Utilisez ce plan pour traduire le bullet de roadmap en livrables concrets, runbooks et gates de telemetrie afin que chaque operateur comprenne les attentes avant que SoraNet devienne le transport par defaut.

## Phases de lancement

| Phase | Calendrier (cible) | Portee | Artefacts requis |
|-------|-------------------|-------|--------------------|
| **T0 - Testnet ferme** | Q4 2026 | 20-50 relays sur >=3 ASNs operes par des contributeurs core. | Kit d'onboarding testnet, smoke suite de guard pinning, baseline de latence + metriques PoW, log de brownout drill. |
| **T1 - Beta publique** | Q1 2027 | >=100 relays, guard rotation activee, exit bonding impose, SDK betas par defaut sur SoraNet avec `anon-guard-pq`. | Kit d'onboarding mis a jour, checklist de verification operateur, SOP de publication du directory, pack de dashboards de telemetrie, rapports de rehearsal d'incident. |
| **T2 - Mainnet par defaut** | Q2 2027 (conditionne a la completion SNNet-6/7/9) | Le reseau de production passe par defaut sur SoraNet; transports obfs/MASQUE et enforcement du PQ ratchet actives. | Minutes d'approbation governance, procedure de rollback direct-only, alarmes de downgrade, rapport signe de metriques de succes. |

Il n'y a **aucune voie de saut** - chaque phase doit livrer la telemetrie et les artefacts de governance de l'etape precedente avant promotion.

## Kit d'onboarding testnet

Chaque operateur de relay recoit un package deterministe avec les fichiers suivants:

| Artefact | Description |
|----------|-------------|
| `01-readme.md` | Vue d'ensemble, points de contact et calendrier. |
| `02-checklist.md` | Checklist pre-flight (hardware, reachabilite reseau, verification de la guard policy). |
| `03-config-example.toml` | Configuration minimale relay + orchestrator SoraNet alignee sur les blocs de compliance SNNet-9, incluant un bloc `guard_directory` qui fixe le hash du dernier snapshot guard. |
| `04-telemetry.md` | Instructions pour cabler les dashboards de privacy metrics SoraNet et les seuils d'alerte. |
| `05-incident-playbook.md` | Procedure de reponse brownout/downgrade avec matrice d'escalade. |
| `06-verification-report.md` | Modele que les operateurs remplissent et retournent une fois les smoke tests valides. |

Une copie rendue est disponible dans `docs/examples/soranet_testnet_operator_kit/`. Chaque promotion rafraichit le kit; les numeros de version suivent la phase (par exemple, `testnet-kit-vT0.1`).

Pour les operateurs public-beta (T1), le brief concis dans `docs/source/soranet/snnet10_beta_onboarding.md` resume les prerequis, livrables de telemetrie et le workflow de soumission tout en renvoyant vers le kit deterministe et les helpers de validation.

`cargo xtask soranet-testnet-feed` genere le feed JSON qui agrege la fenetre de promotion, le roster de relays, le rapport de metriques, les preuves de drills et les hashes des pieces jointes referencees par le template stage-gate. Signez d'abord les logs de drill et les pieces jointes avec `cargo xtask soranet-testnet-drill-bundle` afin que le feed enregistre `drill_log.signed = true`.

## Metriques de succes

La promotion entre phases est conditionnee par la telemetrie suivante, collecte pendant au moins deux semaines:

- `soranet_privacy_circuit_events_total`: 95% des circuits se terminent sans brownout ni downgrade; les 5% restants sont limites par l'offre PQ.
- `sorafs_orchestrator_policy_events_total{outcome="brownout"}`: <1% des sessions de fetch par jour declenchent un brownout en dehors des drills planifies.
- `soranet_privacy_gar_reports_total`: variance dans +/-10% du mix attendu de categories GAR; les pics doivent etre expliques par des updates de policy approuves.
- Taux de succes des tickets PoW: >=99% dans la fenetre cible de 3 s; rapporte via `soranet_privacy_throttles_total{scope="congestion"}`.
- Latence (95e percentile) par region: <200 ms une fois les circuits completement etablis, capturee via `soranet_privacy_rtt_millis{percentile="p95"}`.

Les templates de dashboards et d'alertes vivent dans `dashboard_templates/` et `alert_templates/`; recopiez-les dans votre repo de telemetrie et ajoutez-les aux checks de lint CI. Utilisez `cargo xtask soranet-testnet-metrics` pour generer le rapport oriente governance avant de demander la promotion.

Les soumissions stage-gate doivent suivre `docs/source/soranet/snnet10_stage_gate_template.md`, qui renvoie vers le formulaire Markdown pret a copier sous `docs/examples/soranet_testnet_stage_gate/stage_gate_report_template.md`.

## Checklist de verification

Les operateurs doivent valider les points suivants avant d'entrer dans chaque phase:

- ✅ Relay advert signe avec l'admission envelope courant.
- ✅ Guard rotation smoke test (`tools/soranet-relay --check-rotation`) valide.
- ✅ `guard_directory` pointe vers le dernier artefact `GuardDirectorySnapshotV2` et `expected_directory_hash_hex` correspond au digest du comite (le demarrage du relay journalise le hash valide).
- ✅ Les metriques de PQ ratchet (`sorafs_orchestrator_pq_ratio`) restent au-dessus des seuils cibles pour l'etape demandee.
- ✅ La config de compliance GAR correspond au dernier tag (voir le catalogue SNNet-9).
- ✅ Simulation d'alarme downgrade (desactiver les collectors, attendre une alerte sous 5 min).
- ✅ Drill PoW/DoS execute avec des etapes de mitigation documentees.

Un modele pre-rempli est inclus dans le kit d'onboarding. Les operateurs soumettent le rapport complete au helpdesk governance avant de recevoir des credentials de production.

## Governance et reporting

- **Change control:** les promotions exigent une approbation du Governance Council enregistree dans les minutes du conseil et jointe a la page de status.
- **Status digest:** publier des mises a jour hebdomadaires resument le nombre de relays, le ratio PQ, les incidents brownout et les action items en attente (stocke dans `docs/source/status/soranet_testnet_digest.md` une fois la cadence lancee).
- **Rollbacks:** maintenir un plan de rollback signe qui ramene le reseau a la phase precedente en 30 minutes, incluant l'invalidation DNS/guard cache et des templates de communication client.

## Actifs de support

- `cargo xtask soranet-testnet-kit [--out <dir>]` materialise le kit d'onboarding depuis `xtask/templates/soranet_testnet/` vers le repertoire cible (par defaut `docs/examples/soranet_testnet_operator_kit/`).
- `cargo xtask soranet-testnet-metrics --input <metrics.json> [--out <path|->]` evalue les metriques de succes SNNet-10 et emet un rapport structure pass/fail adapte aux revues governance. Un snapshot d'exemple vit dans `docs/examples/soranet_testnet_metrics_sample.json`.
- Les templates Grafana et Alertmanager vivent sous `dashboard_templates/soranet_testnet_overview.json` et `alert_templates/soranet_testnet_rules.yml`; copiez-les dans votre repo de telemetrie ou branchez-les dans les checks de lint CI.
- Le template de communication downgrade pour les messages SDK/portal reside dans `docs/source/soranet/templates/downgrade_communication_template.md`.
- Les digests de status hebdomadaires doivent utiliser `docs/source/status/soranet_testnet_weekly_digest.md` comme forme canonique.

Les pull requests doivent mettre a jour cette page avec tout changement d'artefacts ou de telemetrie afin que le plan de rollout reste canonique.
