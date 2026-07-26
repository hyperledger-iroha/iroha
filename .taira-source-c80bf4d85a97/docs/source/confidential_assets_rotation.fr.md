---
lang: fr
direction: ltr
source: docs/source/confidential_assets_rotation.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: fd1e43316c492cc96ed107f6318841ad8db160735d4698c4f05562ff6127fda9
source_last_modified: "2026-01-22T15:38:30.658859+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

//! Playbook confidentiel de rotation des actifs référencé par `roadmap.md:M3`.

# Runbook de rotation des actifs confidentiels

Ce playbook explique comment les opérateurs planifient et exécutent les actifs confidentiels
rotations (ensembles de paramètres, vérification des clés et transitions de politique) tandis que
garantissant que les portefeuilles, les clients Torii et les gardes mempool restent déterministes.

## Cycle de vie et statuts

Jeux de paramètres confidentiels (`PoseidonParams`, `PedersenParams`, vérification des clés)
treillis et assistant utilisés pour dériver l'état effectif à une hauteur donnée
`crates/iroha_core/src/state.rs:7540`–`7561`. Balayage des assistants d'exécution en attente
effectue les transitions dès que la hauteur cible est atteinte et enregistre les échecs pour plus tard
rediffusions (`crates/iroha_core/src/state.rs:6725`–`6765`).

Intégration des stratégies d'actifs
`pending_transition { transition_id, new_mode, effective_height, conversion_window }`
afin que la gouvernance puisse planifier des mises à niveau via
`ScheduleConfidentialPolicyTransition` et annulez-les si nécessaire. Voir
`crates/iroha_data_model/src/asset/definition.rs:320` et miroirs DTO Torii
(`crates/iroha_torii/src/routing.rs:1539`–`1580`).

## Flux de travail de rotation

1. **Publier de nouveaux ensembles de paramètres.** Les opérateurs soumettent
   Instructions `PublishPedersenParams`/`PublishPoseidonParams` (CLI
   `iroha app zk params publish ...`) pour mettre en scène de nouveaux groupes électrogènes avec métadonnées,
   fenêtres d’activation/dépréciation et marqueurs d’état. L'exécuteur rejette
   ID en double, versions non croissantes ou transitions de statut incorrectes par
   `crates/iroha_core/src/smartcontracts/isi/world.rs:2499`–`2635`, et le
   les tests de registre couvrent les modes de défaillance (`crates/iroha_core/tests/confidential_params_registry.rs:93` – `226`).
2. **Mises à jour d'enregistrement/vérification des clés.** `RegisterVerifyingKey` applique le backend,
   engagement et contraintes de circuit/version avant qu'une clé puisse entrer dans le
   registre (`crates/iroha_core/src/smartcontracts/isi/world.rs:2067`–`2137`).
   La mise à jour d'une clé rend automatiquement obsolète l'ancienne entrée et efface les octets en ligne,
   tel qu'exercé par `crates/iroha_core/tests/zk_vk_deprecate_marks_status.rs:1`.
3. **Planifiez les transitions entre les stratégies d'actifs.** Une fois les nouveaux ID de paramètres activés,
   la gouvernance appelle `ScheduleConfidentialPolicyTransition` avec le
   mode, fenêtre de transition et hachage d’audit. L'exécuteur refuse les conflits
   transitions ou actifs avec une offre transparente exceptionnelle. Des tests tels que
   `crates/iroha_core/tests/confidential_policy_gates.rs:300`–`384` vérifier que
   les transitions interrompues effacent `pending_transition`, tandis que
   `confidential_policy_transition_reaches_shielded_only_on_schedule` à
   les lignes 385 à 433 confirment que les mises à niveau programmées passent à `ShieldedOnly` exactement à
   la hauteur efficace.
4. **Application de stratégie et protection du pool de mémoire.** L'exécuteur de bloc balaie tous les éléments en attente
   transitions au début de chaque bloc (`apply_policy_if_due`) et émet
   télémétrie en cas d'échec d'une transition afin que les opérateurs puissent la reprogrammer. Lors de l'admission
   le mempool refuse les transactions dont la politique effective changerait en cours de bloc,
   assurer une inclusion déterministe tout au long de la fenêtre de transition
   (`docs/source/confidential_assets.md:60`).

## Exigences du portefeuille et du SDK- Swift et d'autres SDK mobiles exposent les assistants Torii pour récupérer la politique active
  ainsi que toute transition en attente, afin que les portefeuilles puissent avertir les utilisateurs avant de signer. Voir
  `IrohaSwift/Sources/IrohaSwift/ToriiClient.swift:309` (DTO) et les associés
  tests à `IrohaSwift/Tests/IrohaSwiftTests/ToriiClientTests.swift:591`.
- La CLI reflète les mêmes métadonnées via `iroha ledger assets data-policy get` (assistant dans
  `crates/iroha_cli/src/main.rs:1497`–`1670`), permettant aux opérateurs d'auditer le
  ID de politique/paramètre connectés à une définition d'actif sans spéléologie
  bloquer le magasin.

## Couverture des tests et de la télémétrie

- `crates/iroha_core/tests/zk_ledger_scaffold.rs:288`–`345` vérifie cette stratégie
  les transitions se propagent dans des instantanés de métadonnées et s’effacent une fois appliquées.
- `crates/iroha_core/tests/zk_dedup.rs:1` prouve que le cache `Preverify`
  rejette les doubles dépenses/doubles preuves, y compris les scénarios de rotation où
  les engagements diffèrent.
- `crates/iroha_core/tests/zk_confidential_events.rs` et
  `zk_shield_transfer_audit.rs` couvrir blindage bout à bout → transfert → non blindage
  flux, garantissant que la piste d’audit survit à travers les rotations de paramètres.
- `dashboards/grafana/confidential_assets.json` et
  `docs/source/confidential_assets.md:401` documente l'EngagementTree &
  des jauges de cache de vérificateur qui accompagnent chaque exécution d’étalonnage/rotation.

## Propriété du Runbook

- **DevRel / Wallet SDK Leads :** conserve les extraits du SDK + les démarrages rapides qui s'affichent
  comment faire apparaître les transitions en attente et rejouer la menthe → transférer → révéler
  tests localement (suivis sous `docs/source/project_tracker/confidential_assets_phase_c.md:M3.2`).
- **Gestion du programme / Actifs confidentiels TL :** approuve les demandes de transition, conserve
  `status.md` mis à jour avec les rotations à venir et assurez-vous que les dérogations (le cas échéant) sont
  enregistré à côté du registre d’étalonnage.