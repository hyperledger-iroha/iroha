---
lang: fr
direction: ltr
source: docs/source/domain_endorsements.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 7c337150e6de1efa9f9480ba8126ecd5ada4ed8ee7ee8b70a95fd7f6348f9016
source_last_modified: "2026-01-03T18:08:00.700192+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Approbations de domaine

Les approbations de domaine permettent aux opérateurs de contrôler la création et la réutilisation de domaines dans le cadre d’une déclaration signée par le comité. La charge utile d'approbation est un objet Norito enregistré sur la chaîne afin que les clients puissent vérifier qui a attesté quel domaine et quand.

## Forme de la charge utile

- `version` : `DOMAIN_ENDORSEMENT_VERSION_V1`
- `domain_id` : identifiant canonique de domaine
- `committee_id` : étiquette du comité lisible par l'homme
-`statement_hash` : `Hash::new(domain_id.to_string().as_bytes())`
- `issued_at_height` / `expires_at_height` : validité des limites des hauteurs de bloc
- `scope` : espace de données en option plus une fenêtre `[block_start, block_end]` en option (incluse) qui **doit** couvrir la hauteur du bloc d'acceptation
- `signatures` : signatures sur `body_hash()` (avenant avec `signatures = []`)
- `metadata` : métadonnées facultatives Norito (identifiants de proposition, liens d'audit, etc.)

## Application

- Des approbations sont requises lorsque Nexus est activé et `nexus.endorsement.quorum > 0`, ou lorsqu'une stratégie par domaine marque le domaine comme requis.
- La validation applique la liaison de hachage de domaine/instruction, la version, la fenêtre de blocage, l'appartenance à l'espace de données, l'expiration/l'âge et le quorum du comité. Les signataires doivent disposer de clés de consensus en direct avec le rôle `Endorsement`. Les rediffusions sont rejetées par `body_hash`.
- Les mentions attachées à l'enregistrement du domaine utilisent la clé de métadonnées `endorsement`. Le même chemin de validation est utilisé par l'instruction `SubmitDomainEndorsement`, qui enregistre les approbations pour l'audit sans enregistrer un nouveau domaine.

## Comités et politiques

- Les comités peuvent être enregistrés en chaîne (`RegisterDomainCommittee`) ou dérivés des paramètres par défaut (`nexus.endorsement.committee_keys` + `nexus.endorsement.quorum`, id = `default`).
- Les politiques par domaine sont configurées via `SetDomainEndorsementPolicy` (identifiant du comité, `max_endorsement_age`, indicateur `required`). En cas d'absence, les valeurs par défaut Nexus sont utilisées.

## assistants CLI

- Créer/signer une approbation (sort le JSON Norito sur la sortie standard) :

  ```
  iroha endorsement prepare \
    --domain wonderland \
    --committee-id default \
    --issued-at-height 5 \
    --expires-at-height 25 \
    --block-start 5 \
    --block-end 15 \
    --signer-key <PRIVATE_KEY> --signer-key <PRIVATE_KEY>
  ```

- Soumettre une approbation :

  ```
  iroha endorsement submit --file endorsement.json
  # or: cat endorsement.json | iroha endorsement submit
  ```

- Gérer la gouvernance :
  -`iroha endorsement register-committee --committee-id jdga --quorum 2 --member <PK> --member <PK> [--metadata path]`
  -`iroha endorsement set-policy --domain wonderland --committee-id jdga --max-endorsement-age 1000 --required`
  -`iroha endorsement policy --domain wonderland`
  -`iroha endorsement committee --committee-id jdga`
  -`iroha endorsement list --domain wonderland`

Les échecs de validation renvoient des chaînes d'erreur stables (incompatibilité de quorum, approbation périmée/expirée, incompatibilité de portée, espace de données inconnu, comité manquant).