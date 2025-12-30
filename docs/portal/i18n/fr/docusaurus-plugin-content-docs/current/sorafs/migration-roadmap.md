<!-- Auto-generated stub for French (fr) translation. Replace this content with the full translation. -->

---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/migration-roadmap.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

> Adapte de [`docs/source/sorafs/migration_roadmap.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/migration_roadmap.md).

# Feuille de route de migration SoraFS (SF-1)

Ce document operationalise les directives de migration capturees dans
`docs/source/sorafs_architecture_rfc.md`. Il developpe les livrables SF-1 en
jalons prets a executer, criteres de passage et checklists des responsables afin
que les equipes storage, governance, DevRel et SDK coordonnent la transition du
hosting d'artefacts legacy vers une publication adossee a SoraFS.

La feuille de route est volontairement deterministe: chaque jalon nomme les
artefacts requis, les invocations de commandes et les etapes d'attestation pour
que les pipelines downstream produisent des sorties identiques et que la
governance conserve une trace auditable.

## Vue d'ensemble des jalons

| Jalon | Fenetre | Objectifs principaux | Doit livrer | Owners |
|-------|---------|----------------------|-------------|--------|
| **M0 - Bootstrap** | Semaines 1-6 | Publier des fixtures de chunker deterministes et publier en double (legacy + SoraFS). | Fixtures `sorafs_chunker`, integration CLI `sorafs_manifest_stub`, entrees du registre de migration. | Docs, DevRel, Storage |
| **M1 - Enforcement deterministe** | Semaines 7-12 | Exiger des fixtures signees et preparer les preuves d'alias pendant que les pipelines adoptent les expectation flags. | Verification nightly des fixtures, manifests signes par le conseil, entrees staging du registre d'alias. | Storage, Governance, SDKs |
| **M2 - Registry d'abord** | Semaines 13-20 | Router les pins via le registry, geler les bundles legacy et exposer la telemetrie de parite. | Contrat Pin Registry + CLI (`sorafs pin propose/approve`), dashboards d'observabilite, runbooks operateurs. | Governance, Ops, Observability |
| **M3 - Alias uniquement** | Semaine 21+ | Decommissionner le hosting legacy et exiger des preuves d'alias pour la recuperation. | Gateways alias-only, alertes de parite, defaults SDK mis a jour, avis de retrait legacy. | Ops, Networking, SDKs |

Le statut des jalons est suivi dans `docs/source/sorafs/migration_ledger.md`. Toutes
les modifications de cette feuille de route DOIVENT mettre a jour le registre afin
que governance et release engineering restent synchronises.

## Pistes de travail

### 1. Reconditionnement des donnees legacy

| Etape | Jalon | Description | Owner(s) | Sortie |
|-------|-------|-------------|----------|--------|
| Inventaire et etiquetage | M0 | Exporter les digests SHA3-256 des bundles legacy et les consigner dans le registre de migration (append-only). | Docs, DevRel | Entrees du registre avec `source_path`, `sha3_digest`, `owner`, `planned_manifest_cid`. |
| Reconstruction deterministe | M0-M1 | Invoquer `sorafs_manifest_stub` pour chaque artefact de release et persister CAR, manifest, envelope de signatures et plan de fetch dans `artifacts/<team>/<alias>/<timestamp>/`. | Docs, CI | Bundles CAR + manifest reproductibles par version. |
| Boucle de validation | M1 | Rejouer `sorafs_fetch` contre les gateways de staging pour confirmer que les limites/digests de chunks correspondent aux fixtures. Noter pass/fail dans les commentaires du registre. | Governance QA | Rapport de verification staging + issue GitHub pour le drift. |
| Cut-over registry | M2 | Basculer le statut du registre vers `Pinned` une fois le digest du manifest on-chain; le bundle legacy passe en lecture seule (servir mais ne pas modifier). | Governance, Ops | Hash de transaction registry, ticket read-only pour le stockage legacy. |
| Decommission | M3 | Supprimer les entrees du CDN legacy apres 30 jours de grace, archiver les approvals de changement DNS, publier le post-mortem. | Ops | Checklist de decommission, enregistrement changement DNS, cloture de ticket incident. |

### 2. Adoption du pinning deterministe

| Etape | Jalon | Description | Owner(s) | Sortie |
|-------|-------|-------------|----------|--------|
| Repetitions de fixtures | M0 | Dry-runs hebdomadaires comparant les digests locaux de chunks avec `fixtures/sorafs_chunker`. Publier un rapport sous `docs/source/sorafs/reports/`. | Storage Providers | `determinism-<date>.md` avec matrice pass/fail. |
| Exiger les signatures | M1 | `ci/check_sorafs_fixtures.sh` + `.github/workflows/sorafs-fixtures-nightly.yml` echouent si signatures ou manifests derivent. Les overrides de dev exigent un waiver governance attache au PR. | Tooling WG | Log CI, lien vers ticket de waiver (si applicable). |
| Expectation flags | M1 | Les pipelines appellent `sorafs_manifest_stub` avec des expectations explicites pour figer les sorties: | Docs CI | Scripts mis a jour referencant les expectation flags (voir bloc de commande ci-dessous). |
| Pinning registry-first | M2 | `sorafs pin propose` et `sorafs pin approve` enveloppent les soumissions de manifest; le CLI par defaut utilise `--require-registry`. | Governance Ops | Log d'audit du CLI registry, telemetrie des propositions ratees. |
| Parite observabilite | M3 | Des dashboards Prometheus/Grafana alertent quand les inventaires de chunks divergent des manifests registry; alertes branchees sur l'astreinte ops. | Observability | Lien dashboard, IDs des regles d'alerte, resultats GameDay. |

#### Commande canonique de publication

```bash
cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- docs/book \
  --manifest-out artifacts/docs/book/2025-11-01/docs.manifest \
  --manifest-signatures-out artifacts/docs/book/2025-11-01/docs.manifest_signatures.json \
  --car-out artifacts/docs/book/2025-11-01/docs.car \
  --chunk-fetch-plan-out artifacts/docs/book/2025-11-01/docs.fetch_plan.json \
  --car-digest=13fa919c67e55a2e95a13ff8b0c6b40b2e51d6ef505568990f3bc7754e6cc482 \
  --car-size=429391872 \
  --root-cid=f40101... \
  --dag-codec=0x71
```

Remplacez les valeurs de digest, taille et CID par les references attendues
recensees dans l'entree du registre de migration pour l'artefact.

### 3. Transition des alias et communications

| Etape | Jalon | Description | Owner(s) | Sortie |
|-------|-------|-------------|----------|--------|
| Preuves d'alias en staging | M1 | Enregistrer les claims d'alias dans le Pin Registry staging et attacher des preuves Merkle aux manifests (`--alias`). | Governance, Docs | Bundle de preuves stocke a cote du manifest + commentaire du registre avec le nom d'alias. |
| DNS double + notification | M1-M2 | Operer le DNS legacy et Torii/SoraDNS en parallele; publier des avis de migration aux operateurs et canaux SDK. | Networking, DevRel | Post d'annonce + ticket de changement DNS. |
| Enforcement des preuves | M2 | Les gateways rejettent les manifests sans headers `Sora-Proof` recents; CI ajoute l'etape `sorafs alias verify` pour recuperer les preuves. | Networking | Patch de config gateway + sortie CI capturant la verification reussie. |
| Rollout alias-only | M3 | Supprimer le DNS legacy, mettre a jour les defaults SDK pour s'appuyer sur Torii/SoraDNS + preuves d'alias, documenter la fenetre de rollback. | SDK Maintainers, Ops | Notes de release SDK, mise a jour du runbook ops, plan de rollback. |

### 4. Communication et audit

- **Discipline du registre:** chaque changement d'etat (drift de fixtures, soumission registry,
  activation d'alias) doit ajouter une note datee dans
  `docs/source/sorafs/migration_ledger.md`.
- **Minutes de gouvernance:** les sessions du conseil approuvant les changements du pin registry ou
  les politiques d'alias doivent referencer cette feuille de route et le registre.
- **Comms externes:** DevRel publie des mises a jour a chaque jalon (blog + extrait de changelog)
  mettant en avant les garanties deterministes et les calendriers d'alias.

## Dependances et risques

| Dependance | Impact | Mitigation |
|------------|--------|------------|
| Disponibilite du contrat Pin Registry | Bloque le rollout M2 pin-first. | Preparer le contrat avant M2 avec des tests de replay; maintenir un fallback envelope jusqu'a stabilite. |
| Cles de signature du conseil | Requises pour les envelopes de manifest et les approbations registry. | Ceremony de signature documentee dans `docs/source/sorafs/signing_ceremony.md`; rotation avec chevauchement et note dans le registre. |
| Tooling de parite gateway | Necessaire pour imposer les preuves d'alias et la parite des chunks. | Livrer les mises a jour gateway en M1, garder le comportement legacy derriere un feature flag jusqu'aux criteres M2. |
| Cadence de release SDK | Les clients doivent honorer les preuves d'alias avant M3. | Aligner les fenetres de release SDK sur les gates des jalons; ajouter des checklists de migration aux templates de release. |

Les risques residuels et mitigations sont reprennent dans `docs/source/sorafs_architecture_rfc.md`
et doivent etre recoupes lors des ajustements.

## Checklist des criteres de sortie

| Jalon | Criteres |
|-------|----------|
| M0 | - Tous les artefacts cibles reconstruits via `sorafs_manifest_stub` avec expectation flags. <br /> - Registre de migration renseigne pour chaque famille d'artefacts. <br /> - Publication double (legacy + SoraFS) active. |
| M1 | - Job nightly des fixtures vert pendant sept jours consecutifs. <br /> - Preuves d'alias staging verifiees en CI. <br /> - Governance ratifie la politique d'expectation flags. |
| M2 | - 100% des nouveaux manifests routes via Pin Registry. <br /> - Stockage legacy marque en lecture seule; playbook incident approuve. <br /> - Dashboards d'observabilite en ligne avec seuils d'alerte. |
| M3 | - Gateways alias-only en production. <br /> - DNS legacy supprime et reflechi dans les tickets de changement. <br /> - Defaults SDK mis a jour et publies. <br /> - Statut final ajoute au registre de migration. |

## Gestion du changement

1. Proposer des ajustements via PR mettant a jour ce fichier **et**
   `docs/source/sorafs/migration_ledger.md`.
2. Lier les minutes de gouvernance et les preuves CI dans la description du PR.
3. Apres merge, notifier la liste storage + DevRel avec un resume et les actions
   attendues des operateurs.

Suivre cette procedure garantit que le rollout SoraFS reste deterministe,
auditable et transparent entre les equipes participant au lancement Nexus.
