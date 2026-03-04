<!--
  SPDX-License-Identifier: Apache-2.0
-->

---
lang: fr
direction: ltr
source: docs/source/nexus_overview.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: bda1352ff13cc866cd02a08f9db6be962798b547e905f2fccf236cd803eb0eda
source_last_modified: "2025-11-08T16:26:32.878050+00:00"
translation_last_reviewed: 2026-01-01
---

# Vue d ensemble Nexus et contexte operateur

**Lien de roadmap:** NX-14 - documentation Nexus et runbooks operateurs  
**Statut:** Redige le 2026-03-24 (en paire avec `docs/source/nexus_operations.md`)  
**Public:** responsables de programme, ingenieurs operations et equipes partenaires qui
ont besoin d un resume d une page de l architecture Sora Nexus (Iroha 3) avant
de plonger dans les specifications detaillees (`docs/source/nexus.md`,
`docs/source/nexus_lanes.md`, `docs/source/nexus_transition_notes.md`).

## 1. Lignes de release et outillage partage

- **Iroha 2** reste la voie auto-hebergee pour les deploiements de consortium.
- **Iroha 3 / Sora Nexus** introduit l execution multi-lane, les data spaces et
  une gouvernance partagee. Le meme depot, l outillage et les pipelines CI produisent
  les deux lignes de release, donc les correctifs apportes a l IVM, au compilateur Kotodama
  ou aux SDK s appliquent automatiquement a Nexus.
- **Artefacts:** `iroha3-<version>-<os>.tar.zst` bundles et images OCI contiennent
  les binaires, des configs d exemple et les metadonnees du profil Nexus. Les operateurs
  se referent a `docs/source/sora_nexus_operator_onboarding.md` pour le flux de validation
  des artefacts de bout en bout.
- **Surface SDK partagee:** Les SDK Rust, Python, JS/TS, Swift et Android consomment
  les memes schemas Norito et fixtures d adresse (`fixtures/account/address_vectors.json`)
  pour que les wallets et l automatisation puissent basculer entre Iroha 2 et Nexus sans
  forks de format.

## 2. Blocs de construction architecturale

| Composant | Description | References cle |
|----------|-------------|----------------|
| **Data Space (DS)** | Domaine d execution porte par la gouvernance qui definit la composition des validateurs, la classe de confidentialite, la politique de frais et le profil de disponibilite des donnees. Chaque DS possede une ou plusieurs lanes. | `docs/source/nexus.md`, `docs/source/nexus_transition_notes.md` |
| **Lane** | Shard deterministe d execution et d etat. Les manifestes de lane declarent les ensembles de validateurs, les hooks de settlement, les metadonnees de telemetrie et les permissions de routage. L anneau de consensus global ordonne les commitments de lane. | `docs/source/nexus_lanes.md` |
| **Space Directory** | Contrat de registre (et helpers CLI) qui stocke les manifestes DS, les rotations de validateurs et les grants de capacites. Il conserve des manifestes historiques signes pour que les auditeurs puissent reconstruire l etat. | `docs/source/nexus.md#space-directory` |
| **Lane Catalog** | Section de configuration (`[nexus]` dans `config.toml`) qui mappe les IDs de lane vers des alias, des politiques de routage et des knobs de retention. Les operateurs peuvent inspecter le catalogue effectif via `irohad --sora --config ... --trace-config`. | `docs/source/sora_nexus_operator_onboarding.md` |
| **Settlement Router** | Routage des mouvements XOR entre lanes (par exemple, lanes CBDC privees <-> lanes de liquidite publiques). Les politiques par defaut sont dans `docs/source/cbdc_lane_playbook.md`. | `docs/source/cbdc_lane_playbook.md` |
| **Telemetry & SLOs** | Des dashboards et regles d alerte sous `dashboards/grafana/nexus_*.json` capturent la hauteur des lanes, le backlog DA, la latence de settlement et la profondeur des files de gouvernance. Le plan de remediation est suivi dans `docs/source/nexus_telemetry_remediation_plan.md`. | `dashboards/grafana/nexus_lanes.json`, `dashboards/alerts/nexus_audit_rules.yml` |

### Classes de lane et de data space

- `default_public` lanes ancrent des charges totalement publiques sous le Parlement Sora.
- `public_custom` lanes permettent des economies par programme tout en restant transparentes.
- `private_permissioned` lanes servent les CBDC ou les apps de consortium; elles exportent seulement des commitments et proofs.
- `hybrid_confidential` lanes combinent des preuves a divulgation nulle avec des hooks de divulgation selective.

Chaque lane declare:

1. **Manifeste de lane:** metadonnees approuvees par la gouvernance et suivies dans Space Directory.
2. **Politique de disponibilite des donnees:** parametres d erasure coding, hooks de recuperation et exigences d audit.
3. **Profil de telemetrie:** dashboards et runbooks on-call a mettre a jour a chaque changement de gouvernance.

## 3. Instantane du calendrier de deploiement

| Phase | Focus | Criteres de sortie |
|-------|-------|--------------------|
| **N0 - Closed beta** | Registraire gere par le conseil, namespace `.sora` uniquement, onboarding operateur manuel. | Manifestes DS signes, catalogue de lanes statique, rehearsals de gouvernance enregistres. |
| **N1 - Public launch** | Ajoute les suffixes `.nexus`, des encheres et un registraire en libre-service. Les settlements se raccordent a la tresorerie XOR. | Tests de synchronisation resolver/gateway au vert, dashboards de reconciliation de facturation en ligne, exercice de dispute termine. |
| **N2 - Expansion** | Active `.dao`, APIs de reseller, analytique, portail de litiges, scorecards de stewards. | Artefacts de conformite versionnes, toolkit de policy-jury en ligne, rapports de transparence de tresorerie publies. |
| **NX-12/13/14 gate** | Le moteur de conformite, les dashboards de telemetrie et la documentation doivent arriver ensemble avant l ouverture du pilote partenaires. | `docs/source/nexus_overview.md` + `docs/source/nexus_operations.md` publies, dashboards cables avec alertes, moteur de politique relie a la gouvernance. |

## 4. Responsabilites operateur

| Responsabilite | Description | Preuve |
|---------------|-------------|-------|
| Hygiene de config | Garder `config/config.toml` aligne avec le catalogue publie des lanes et dataspaces; consigner les changements dans des tickets. | Sortie `irohad --sora --config ... --trace-config` archivee avec les artefacts de release. |
| Suivi des manifestes | Surveiller les mises a jour du Space Directory et rafraichir caches/allowlists locaux. | Bundle de manifestes signe conserve avec le ticket d onboarding. |
| Couverture telemetrie | S assurer que les dashboards de la Section 2 sont accessibles, que les alertes sont reliees a PagerDuty, et que les revues trimestrielles sont enregistrees. | Compte rendu d astreinte + export Alertmanager. |
| Rapport d incident | Suivre la matrice de severite definie dans `docs/source/nexus_operations.md` et soumettre des rapports post-incident sous cinq jours ouvrables. | Modele post-incident archive par ID d incident. |
| Preparation gouvernance | Participer aux votes du conseil Nexus lorsque les changements de politique de lane affectent le deploiement; repeter les instructions de rollback chaque trimestre. | Presence au conseil + checklist de repetition sous `docs/source/project_tracker/nexus_config_deltas/`. |

## 5. Carte de documentation associee

- **Specification detaillee:** `docs/source/nexus.md`
- **Geometrie des lanes et stockage:** `docs/source/nexus_lanes.md`
- **Plan de transition et routage temporaire:** `docs/source/nexus_transition_notes.md`
- **Onboarding operateur:** `docs/source/sora_nexus_operator_onboarding.md`
- **Politique des lanes CBDC et plan de settlement:** `docs/source/cbdc_lane_playbook.md`
- **Remediation telemetrie et carte des dashboards:** `docs/source/nexus_telemetry_remediation_plan.md`
- **Runbook / processus d incident:** `docs/source/nexus_operations.md`

Garder ce resume aligne avec l item de roadmap NX-14 lorsque des changements substantiels
arrivent dans les documents lies ou lorsque de nouvelles classes de lanes ou des flux de gouvernance
sont introduits.
