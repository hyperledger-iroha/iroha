---
lang: fr
direction: ltr
source: docs/source/nexus_operations.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 413953b2ca41875bb960be56995aa03dbaa58af4a30f927c24d1e8815c7da472
source_last_modified: "2025-11-08T16:26:57.335679+00:00"
translation_last_reviewed: 2026-01-01
---

# Runbook des operations Nexus (NX-14)

**Lien roadmap :** NX-14 - documentation Nexus et runbooks operateurs
**Statut :** Brouillon 2026-03-24 - aligne sur `docs/source/nexus_overview.md` et
le flux d'onboarding dans `docs/source/sora_nexus_operator_onboarding.md`.
**Audience :** Operateurs reseau, ingenieurs SRE/on-call, coordinateurs de gouvernance.

Ce runbook resume le cycle de vie operationnel des noeuds Sora Nexus (Iroha 3).
Il ne remplace pas la specification approfondie (`docs/source/nexus.md`) ni les
guides par lane (ex. `docs/source/cbdc_lane_playbook.md`), mais rassemble les
checklists concrets, les hooks de telemetrie et les exigences de preuve qui
doivent etre respectes avant d'admettre ou de mettre a niveau un noeud.

## 1. Cycle de vie operationnel

| Etape | Checklist | Preuve |
|-------|-----------|--------|
| **Pre-flight** | Valider les hashes/signatures des artefacts, confirmer `profile = "iroha3"`, et preparer les templates de config. | Sortie de `scripts/select_release_profile.py`, log de checksum, bundle de manifest signe. |
| **Alignement du catalogue** | Mettre a jour le catalogue lane + dataspace dans `[nexus]`, la policy de routage et les seuils DA pour correspondre au manifest emis par le conseil. | Sortie de `irohad --sora --config ... --trace-config` stockee avec le ticket. |
| **Smoke & cutover** | Executer `irohad --sora --config ... --trace-config`, lancer le smoke test CLI (ex. `FindNetworkStatus`), verifier les endpoints de telemetrie, puis demander l'admission. | Log de smoke-test + confirmation de silence Alertmanager. |
| **Etat stable** | Surveiller dashboards/alertes, faire tourner les cles selon la cadence de gouvernance, et garder configs + runbooks en sync avec les revisions de manifest. | Comptes rendus trimestriels, captures de dashboards liees, et IDs de tickets de rotation. |

Les instructions detaillees d'onboarding (y compris remplacement de cles, exemples
de policy de routage et validation du release profile) se trouvent dans
`docs/source/sora_nexus_operator_onboarding.md`. Referencez ce document quand
les formats d'artefacts ou les scripts changent.

## 2. Gestion des changements et hooks de gouvernance

1. **Mises a jour de release**
   - Suivre les annonces dans `status.md` et `roadmap.md`.
   - Chaque PR de release doit joindre la checklist remplie de
     `docs/source/sora_nexus_operator_onboarding.md`.
2. **Changements de lane manifest**
   - La gouvernance publie des bundles de manifest signes via Space Directory.
   - Les operateurs verifient les signatures, mettent a jour les entrees de catalogue et archivent
     les manifests dans `docs/source/project_tracker/nexus_config_deltas/`.
3. **Deltas de configuration**
   - Tout changement dans `config/config.toml` requiert un ticket qui reference le lane ID
     et l'alias de dataspace.
   - Conserver une copie redactee de la config effective dans le ticket lors de l'adhesion
     ou de la mise a niveau du noeud.
4. **Exercices de rollback**
   - Realiser des rehearsals trimestriels (arreter le noeud, restaurer le bundle precedent,
     rejouer la config, relancer le smoke). Enregistrer les resultats sous
     `docs/source/project_tracker/nexus_config_deltas/<date>-rollback.md`.
5. **Approbations de conformite**
   - Les lanes privees/CBDC doivent obtenir un sign-off conformite avant de changer
     la policy DA ou les knobs de redaction de telemetrie. Voir
     `docs/source/cbdc_lane_playbook.md#governance-hand-offs`.

## 3. Couverture telemetrie et SLO

Les dashboards et regles d'alerte sont versionnes sous `dashboards/` et documentes dans
`docs/source/nexus_telemetry_remediation_plan.md`. Les operateurs DOIVENT :

- Abonner les cibles PagerDuty/on-call a `dashboards/alerts/nexus_audit_rules.yml`
  et aux regles de sante lane dans `dashboards/alerts/torii_norito_rpc_rules.yml`
  (couvre le transport Torii/Norito).
- Publier les boards Grafana suivants dans le portail operations :
  - `nexus_lanes.json` (hauteur de lane, backlog, parite DA).
  - `nexus_settlement.json` (latence de settlement, deltas de tresorerie).
  - `android_operator_console.json` / dashboards SDK quand la lane depend
    de la telemetrie mobile.
- Garder les exporters OTEL alignes sur `docs/source/torii/norito_rpc_telemetry.md`
  quand le transport binaire Torii est active.
- Executer la checklist de remediation telemetrie au moins trimestriellement
  (Section 5 dans `docs/source/nexus_telemetry_remediation_plan.md`) et joindre le
  formulaire rempli aux minutes de revue ops.

### Metriques cles

| Metrique | Description | Seuil d'alerte |
|--------|-------------|----------------|
| `nexus_lane_height{lane_id}` | Hauteur de tete par lane ; detecte les validateurs bloques. | Alerter si aucune hausse pendant 3 slots consecutifs. |
| `nexus_da_backlog_chunks{lane_id}` | Chunks DA non traites par lane. | Alerter au-dessus de la limite configuree (defaut : 64 public, 8 prive). |
| `nexus_settlement_latency_seconds{lane_id}` | Temps entre commit de lane et settlement global. | Alerter >900 ms P99 (public) ou >1200 ms (prive). |
| `torii_request_failures_total{scheme="norito_rpc"}` | Compteur d'erreurs Norito RPC. | Alerter si le ratio d'erreurs 5 minutes >2%. |
| `telemetry_redaction_override_total` | Overrides emis pour la redaction telemetrie. | Alerter immediatement (Sev 2) et exiger un ticket conformite. |

## 4. Reponse aux incidents

| Severite | Definition | Actions requises |
|----------|------------|-----------------|
| **Sev 1** | Breche d'isolation de data space, arret de settlement >15 min, ou corruption du vote de gouvernance. | Page Nexus Primary + Release Engineering + Compliance. Geler l'admission de lane, collecter metriques/logs, publier la communication d'incident sous 60 min, ouvrir un RCA <=5 jours ouvrables. |
| **Sev 2** | Backlog de lane au-dela du SLA, angle mort telemetrie >30 min, rollout de manifest echoue. | Page Nexus Primary + SRE, mitigation sous 4 h, consigner les issues de suivi sous 2 jours ouvrables. |
| **Sev 3** | Regressions non bloquantes (drift docs, alerte mal declenchee). | Enregistrer dans le tracker, planifier un correctif dans le sprint. |

Les tickets d'incident doivent inclure :

1. IDs de lane/data-space affectes et hashes de manifest.
2. Timeline (UTC) avec detection, mitigation, recuperation et communications.
3. Metriques/captures soutenant la detection.
4. Taches de suivi (avec owners/dates) et si des mises a jour d'automatisation/runbooks
   sont necessaires.

## 5. Preuves et audit trail

- **Archive des artefacts :** Stocker bundles, manifests et exports de telemetrie sous
  `artifacts/nexus/<lane>/<date>/`.
- **Snapshots de config :** `config.toml` redacte + sortie `trace-config` pour
  chaque release.
- **Lien gouvernance :** Notes de conseil et decisions signees referencees
  dans le ticket d'onboarding ou d'incident.
- **Exports de telemetrie :** Snapshots hebdomadaires des chunks TSDB Prometheus
  lies a la lane, attaches au partage d'audit pendant 12 mois minimum.
- **Versionnage du runbook :** Tout changement significatif de ce fichier doit
  inclure une entree de changelog dans `docs/source/project_tracker/nexus_config_deltas/README.md`
  afin que les auditeurs puissent suivre quand les exigences ont change.

## 6. Ressources associees

- `docs/source/nexus_overview.md` - architecture/resume haut niveau.
- `docs/source/nexus.md` - specification technique complete.
- `docs/source/nexus_lanes.md` - geometrie des lanes.
- `docs/source/nexus_transition_notes.md` - roadmap de migration.
- `docs/source/cbdc_lane_playbook.md` - politiques CBDC.
- `docs/source/sora_nexus_operator_onboarding.md` - flux release/onboarding.
- `docs/source/nexus_telemetry_remediation_plan.md` - guardrails telemetrie.

Garder ces references a jour lorsque l'item NX-14 avance ou lorsque de nouvelles
classes de lane, regles de telemetrie ou hooks de gouvernance sont introduits.
