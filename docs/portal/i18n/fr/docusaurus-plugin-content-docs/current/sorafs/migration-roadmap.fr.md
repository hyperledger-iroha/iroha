---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/migration-roadmap.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
titre : "Feuille de route de migration SoraFS"
---

> Adapter de [`docs/source/sorafs/migration_roadmap.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/migration_roadmap.md).

# Feuille de route de migration SoraFS (SF-1)

Ce document opérationnalise les directives de migration capturées dans
`docs/source/sorafs_architecture_rfc.md`. Il développe les livrables SF-1 fr
jalons prêts à être exécutés, critères de passage et checklists des responsables afin
que les équipes de stockage, de gouvernance, DevRel et SDK coordonnent la transition du

La feuille de route est volontairement déterministe : chaque jalon nomme les
artefacts requis, les invocations de commandes et les étapes d'attestation pour
que les pipelines en aval produisent des sorties identiques et que la
la gouvernance conserve une trace auditable.

## Vue d'ensemble des jalons

| Jalón | Fenêtre | Objectifs principaux | Doit livrer | Propriétaires |
|-------|---------|------------|-------------|--------|
| **M1 - Enforcement déterministe** | Semaines 7-12 | Exiger des luminaires signés et préparer les preuves d'alias pendant que les pipelines adoptent les drapeaux d'attente. | Vérification nocturne des luminaires, manifestes signes par le conseil, entrées mise en scène du registre d'alias. | Stockage, gouvernance, SDK |

Le statut des jalons est suivi dans `docs/source/sorafs/migration_ledger.md`. Toutes
les modifications de cette feuille de route DOIVENT mettre à jour le registre afin
que la gouvernance et l’ingénierie des versions restent synchronisées.## Pistes de travail

### 2. Adoption du pinning déterministe

| Étape | Jalón | Descriptif | Propriétaire(s) | Sortie |
|-------|-------|-------------|---------|--------|
| Répétitions de rencontres | M0 | Dry-runs hebdomadaires comparent les digests locaux de chunks avec `fixtures/sorafs_chunker`. Publier un rapport sous `docs/source/sorafs/reports/`. | Fournisseurs de stockage | `determinism-<date>.md` avec matrice réussite/échec. |
| Exiger les signatures | M1 | `ci/check_sorafs_fixtures.sh` + `.github/workflows/sorafs-fixtures-nightly.yml` font écho aux signatures ou manifestes dérivés. Les overrides de dev exigent une renonciation à la gouvernance attachée au PR. | GT Outillage | Log CI, lien vers ticket de renonciation (si applicable). |
| Indicateurs d'attente | M1 | Les pipelines appelant `sorafs_manifest_stub` avec des attentes explicites pour figer les sorties : | Documents CI | Les scripts doivent être à jour référençant les attentes flags (voir bloc de commande ci-dessous). |
| Épingler le registre en premier | M2 | `sorafs pin propose` et `sorafs pin approve` enveloppent les soumissions de manifeste; la CLI par défaut utilise `--require-registry`. | Opérations de gouvernance | Log d'audit du registre CLI, télémétrie des propositions notées. |
| Observabilité parite | M3 | Des tableaux de bord Prometheus/Grafana alertent lorsque les inventaires de morceaux divergent des registres des manifestes ; alertes branchées sur l'astreinte ops. | Observabilité | Lien vers le tableau de bord, les identifiants des règles d'alerte, les résultats GameDay. |

#### Commande canonique de publication```bash
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

Remplacez les valeurs de digest, taille et CID par les références attendues
recensees dans l'entree du registre de migration pour l'artefact.

### 3. Transition des alias et des communications

| Étape | Jalón | Descriptif | Propriétaire(s) | Sortie |
|-------|-------|-------------|---------|--------|
| Preuves d'alias en mise en scène | M1 | Enregistrer les revendications d'alias dans le Pin Registry staging et attacher des preuves Merkle aux manifestes (`--alias`). | Gouvernance, Docs | Bundle de preuves stocke a cote du manifeste + commentaire du registre avec le nom d'alias. |
| Application des preuves | M2 | Les gateways rejettent les manifests sans headers `Sora-Proof` recents; CI ajoute l'étape `sorafs alias verify` pour récupérer les preuves. | Réseautage | Patch de config gateway + sortie CI capturant la vérification russe. |

### 4. Communication et audit

- **Discipline du registre:** chaque changement d'état (dérive de luminaires, registre de soumission,
  activation d'alias) doit ajouter une note datée dans
  `docs/source/sorafs/migration_ledger.md`.
- **Minutes de gouvernance:** les séances du conseil approuvant les changements du pin registre ou
  les politiques d'alias doivent référencer cette feuille de route et le registre.
- **Comms externes:** DevRel publie des mises à jour à chaque jalon (blog + extrait de changelog)
  mettant en avant les garanties déterministes et les calendriers d'alias.## Dépendances et risques

| Dépendance | Impact | Atténuation |
|------------|--------|------------|
| Disponibilité du contrat Pin Registry | Bloquer le déploiement M2 pin-first. | Préparer le contrat avant M2 avec des tests de replay; maintenir une enveloppe de repli jusqu'à la stabilité. |
| Clés de signature du conseil | Requises pour les enveloppes de manifeste et les approbations registre. | Cérémonie de signature documentée dans `docs/source/sorafs/signing_ceremony.md` ; rotation avec chevauchement et note dans le registre. |
| Cadence de sortie du SDK | Les clients doivent honorer les preuves d'alias avant M3. | Aligner les fenêtres de release SDK sur les portes des jalons ; ajouter des checklists de migration aux modèles de release. |

Les risques résiduels et atténuations sont reprennent dans `docs/source/sorafs_architecture_rfc.md`
et doivent être récupérés lors des ajustements.

## Checklist des critères de sortie

| Jalón | Critères |
|-------|--------------|
| M1 | - Job nightly des luminaires vert pendant sept jours consécutifs.  - Preuves d'alias staging vérifiées en CI.  - La gouvernance ratifie la politique d'attente flags. |

##Gestion du changement1. Proposer des ajustements via PR mettant à jour ce fichier **et**
   `docs/source/sorafs/migration_ledger.md`.
2. Lier les minutes de gouvernance et les preuves CI dans la description du PR.
3. Après la fusion, notifier la liste storage + DevRel avec un CV et les actions
   attendues des opérateurs.

Suivre cette procédure garantit que le déploiement SoraFS reste déterministe,
auditable et transparent entre les équipes participant au lancement Nexus.