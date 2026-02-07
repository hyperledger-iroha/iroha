---
lang: fr
direction: ltr
source: docs/source/jdg_sdn.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 1ee87ee60e2e8c9d9636b282231b33de3cf1fd7240c8d31d0a0a1673651dcef1
source_last_modified: "2026-01-03T18:07:58.621058+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

% d'attestations et de rotation JDG-SDN

Cette note capture le modèle d'application pour les attestations Secret Data Node (SDN).
utilisé par le flux Jurisdiction Data Guardian (JDG).

## Format d'engagement
- `JdgSdnCommitment` lie le scope (`JdgAttestationScope`), le chiffrement
  le hachage de la charge utile et la clé publique SDN. Les sceaux sont des signatures dactylographiées
  (`SignatureOf<JdgSdnCommitmentSignable>`) sur la charge utile balisée par domaine
  `iroha:jurisdiction:sdn:commitment:v1\x00 || norito(signable)`.
- La validation structurelle (`validate_basic`) applique :
  -`version == JDG_SDN_COMMITMENT_VERSION_V1`
  - plages de blocs valides
  - joints non vides
  - égalité de portée par rapport à l'attestation lorsqu'elle est exécutée via
    `JdgAttestation::validate_with_sdn`/`validate_with_sdn_registry`
- La déduplication est gérée par le validateur d'attestation (signataire + hachage de charge utile
  caractère unique) pour éviter les engagements retenus/dupliqués.

## Politique de registre et de rotation
- Les clés SDN résident dans `JdgSdnRegistry`, saisies par `(Algorithm, public_key_bytes)`.
- `JdgSdnKeyRecord` enregistre la hauteur d'activation, la hauteur de retrait optionnelle,
  et clé parent facultative.
- La rotation est régie par `JdgSdnRotationPolicy` (actuellement : `dual_publish_blocks`
  fenêtre de chevauchement). L'enregistrement d'une clé enfant met à jour la retraite du parent vers
  `child.activation + dual_publish_blocks`, avec garde-corps :
  - les parents disparus sont rejetés
  - les activations doivent être strictement croissantes
  - les chevauchements qui dépassent la fenêtre de grâce sont rejetés
- Les assistants du registre font apparaître les enregistrements installés (`record`, `keys`) pour connaître leur statut.
  et exposition aux API.

## Flux de validation
- `JdgAttestation::validate_with_sdn_registry` enveloppe la structure
  contrôles d’attestation et application du SDN. Fils `JdgSdnPolicy` :
  - `require_commitments` : imposer la présence des charges utiles PII/secrètes
  - `rotation` : fenêtre de grâce utilisée lors de la mise à jour de la retraite parentale
- Chaque engagement est vérifié :
  - validité structurelle + correspondance attestation-portée
  - présence clé enregistrée
  - fenêtre active couvrant la plage de blocs attestée (limites de retraite déjà
    inclure la grâce de double publication)
  - sceau valide sur le corps d'engagement étiqueté par domaine
- Des erreurs stables font apparaître l'index pour les preuves de l'opérateur :
  `MissingSdnCommitments`, `UnknownSdnKey`, `InactiveSdnKey`, `InvalidSeal`,
  ou des défaillances structurelles `Commitment`/`ScopeMismatch`.

## Runbook de l'opérateur
- **Provision :** enregistrez la première clé SDN avec `activated_at` au plus tard au
  hauteur du premier bloc secret. Publiez l’empreinte digitale de la clé aux opérateurs JDG.
- **Rotation :** génère la clé successeur, enregistrez-la avec `rotation_parent`
  en pointant sur la clé actuelle et confirmez que la retraite du parent est égale à
  `child_activation + dual_publish_blocks`. Resceller les engagements de charge utile avec
  la touche active pendant la fenêtre de chevauchement.
- **Audit :** expose les instantanés du registre (`record`, `keys`) via Torii/status
  surfaces afin que les auditeurs puissent confirmer la clé active et les fenêtres de retrait. Alerte
  si la plage attestée se situe en dehors de la fenêtre active.
- **Récupération :** `UnknownSdnKey` → assurez-vous que le registre inclut la clé de scellement ;
  `InactiveSdnKey` → faire pivoter ou ajuster les hauteurs d'activation ; `InvalidSeal` →
  refermer les charges utiles et actualiser les attestations.## Assistant d'exécution
- `JdgSdnEnforcer` (`crates/iroha_core/src/jurisdiction.rs`) regroupe une politique +
  registre et valide les attestations via `validate_with_sdn_registry`.
- Les registres peuvent être chargés à partir de bundles Norito codés en Norito (voir
  `JdgSdnEnforcer::from_reader`/`from_path`) ou assemblé avec
  `from_records`, qui applique les garde-corps de rotation lors de l'enregistrement.
- Les opérateurs peuvent conserver le bundle Norito comme preuve du statut Torii.
  faisant surface tandis que la même charge utile alimente l'exécuteur utilisé par l'admission et
  gardes du consensus. Un seul applicateur global peut être initialisé au démarrage via
  `init_enforcer_from_path` et `enforcer()`/`registry_snapshot()`/`sdn_registry_status()`
  exposer la politique en direct + les enregistrements clés pour les surfaces status/Torii.

## Tests
- Couverture de régression dans `crates/iroha_data_model/src/jurisdiction.rs` :
  `sdn_registry_accepts_active_commitment`, `sdn_registry_rejects_unknown_key`,
  `sdn_registry_rejects_inactive_key`, `sdn_registry_rejects_bad_signature`,
  `sdn_registry_sets_parent_retirement_window`,
  `sdn_registry_rejects_overlap_beyond_policy`, aux côtés de l'existant
  tests d'attestation structurelle/validation SDN.