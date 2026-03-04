---
lang: fr
direction: ltr
source: docs/source/jdg_attestations.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 459e8ed4612da7cfa68053e4e299b2f68e7620d4f3b98a8a721ebf8327829ea1
source_last_modified: "2026-01-09T07:05:10.922933+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Attestations JDG : garde, rotation et rétention

Cette note documente la garde d'attestation JDG v1 qui est désormais livrée dans `iroha_core`.

- **Manifestes du comité :** Les bundles `JdgCommitteeManifest` codés en Norito effectuent une rotation par espace de données.
  plannings (`committee_id`, membres ordonnés, seuil, `activation_height`, `retire_height`).
  Les manifestes sont chargés avec `JdgCommitteeSchedule::from_path` et appliquent strictement une augmentation
  hauteurs d'activation avec un chevauchement de grâce en option (`grace_blocks`) entre le retrait/l'activation
  comités.
- **Attestation guard :** `JdgAttestationGuard` applique la liaison, l'expiration et les limites obsolètes de l'espace de données.
  correspondance d'identifiant/seuil de comité, adhésion de signataire, schémas de signature pris en charge et facultatif
  Validation SDN via `JdgSdnEnforcer`. Les tailles maximales, le décalage maximum et les schémas de signature autorisés sont
  paramètres du constructeur ; `validate(attestation, dataspace, current_height)` renvoie l'actif
  comité ou une erreur structurée.
  - `scheme_id = 1` (`simple_threshold`) : signatures par signataire, bitmap de signataire facultatif.
  - `scheme_id = 2` (`bls_normal_aggregate`) : signature BLS-normale pré-agrégée unique sur le
    hachage d'attestation ; Bitmap du signataire facultatif, par défaut tous les signataires de l'attestation. BLS
    la validation globale nécessite un PoP valide par membre du comité dans le manifeste ; manquant ou
    les PoP invalides rejettent l’attestation.
  Configurez la liste verte via `governance.jdg_signature_schemes`.
- **Magasin de rétention :** `JdgAttestationStore` suit les attestations par espace de données avec un paramètre configurable
  plafond par espace de données, éliminant les entrées les plus anciennes lors de l'insertion. Appelez `for_dataspace` ou
  `for_dataspace_and_epoch` pour récupérer les bundles d'audit/relecture.
- **Tests :** La couverture unitaire exerce désormais une sélection de comité valide, le rejet des signataires inconnus, périmé
  rejet d’attestation, identifiants de schéma non pris en charge et élagage de rétention. Voir
  `crates/iroha_core/src/jurisdiction.rs`.

Le gardien rejette les programmes en dehors de la liste autorisée configurée.