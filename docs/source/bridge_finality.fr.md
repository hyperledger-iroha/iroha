---
lang: fr
direction: ltr
source: docs/source/bridge_finality.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 2e4c6ed5974f623906f51259a634bcad5df703bcec899630ae29f4669b289ab6
source_last_modified: "2026-01-08T21:52:45.509525+00:00"
translation_last_reviewed: 2026-01-08
---

<!--
SPDX-License-Identifier: Apache-2.0
-->

# Preuves de finalite bridge

Ce document decrit la surface initiale des preuves de finalite bridge pour Iroha.
L objectif est de permettre aux chaines externes ou aux light clients de verifier
qu un bloc Iroha est finalise sans calcul off-chain ni relays de confiance.

## Format de preuve

`BridgeFinalityProof` (Norito/JSON) contient:

- `height`: hauteur de bloc.
- `chain_id`: identifiant de chaine Iroha pour prevenir les replays inter-chain.
- `block_header`: `BlockHeader` canonique.
- `block_hash`: hash du header (les clients le recomputent pour valider).
- `commit_certificate`: ensemble des validateurs + signatures qui finalisent le bloc.
- `validator_set_pops`: preuves de possession (PoP) alignees avec l ordre du validator set
  (requises pour la verification BLS agregée).

La preuve est auto-contenue; aucun manifest externe ou blob opaque n est requis.
Retention: Torii sert les preuves de finalite pour la fenetre recente des commit certificates
(borne par le cap d historique configure; 512 entrees par defaut via
`sumeragi.commit_cert_history_cap` / `SUMERAGI_COMMIT_CERT_HISTORY_CAP`). Les clients
devraient mettre en cache ou ancrer les preuves s ils ont besoin d horizons plus longs.
Le tuple canonique est `(block_header, block_hash, commit_certificate)`: le hash du header
doit correspondre au hash dans le commit certificate, et le chain id lie la preuve a un
seul ledger. Les serveurs rejettent et journalisent un `CommitCertificateHashMismatch`
lorsque le certificate pointe vers un hash de bloc different.

## Commitment bundle

`BridgeFinalityBundle` (Norito/JSON) etend la preuve de base avec un commitment et une
justification explicites:

- `commitment`: `{ chain_id, authority_set { id, validator_set, validator_set_hash, validator_set_hash_version }, block_height, block_hash, mmr_root?, mmr_leaf_index?, mmr_peaks?, next_authority_set? }`
- `justification`: signatures du authority set sur le payload de commitment
  (reutilise les signatures du commit certificate).
- `block_header`, `commit_certificate`: identiques a la preuve de base.

Placeholder actuel: `mmr_root`/`mmr_peaks` sont derives en recomputant en memoire un MMR de
block-hash; les preuves d inclusion ne sont pas encore retournees. Les clients peuvent
encore verifier le meme hash via le payload de commitment aujourd hui.

MMR peaks are ordered left to right. Recompute `mmr_root` by bagging peaks
from right to left: `root = H(p_n, H(p_{n-1}, ... H(p_1, p_0)))`.

API: `GET /v2/bridge/finality/bundle/{height}` (Norito/JSON).

La verification est analogue a la preuve de base: recomputer `block_hash` depuis le header,
verifier les signatures du commit certificate, et verifier que les champs du commitment
correspondent au certificate et au hash du bloc. Le bundle ajoute un wrapper commitment/
justification pour les protocoles bridge qui preferent la separation.

## Etapes de verification

1. Recompute `block_hash` depuis `block_header`; rejeter en cas de mismatch.
2. Verifier que `commit_certificate.block_hash` correspond au `block_hash` recompute;
   rejeter les paires header/commit certificate non correspondantes.
3. Verifier que `chain_id` correspond a la chaine Iroha attendue.
4. Recompute `validator_set_hash` depuis `commit_certificate.validator_set` et verifier
   qu il correspond au hash/version enregistres.
5. Verifier que la longueur de `validator_set_pops` correspond au validator set et valider
   chaque PoP contre sa cle publique BLS.
6. Verifier les signatures du commit certificate contre le hash du header en utilisant les
   cles publiques et indices de validateurs references; appliquer le quorum
   (`2f+1` quand `n>3`, sinon `n`) et rejeter les indices dupliques/hors plage.
7. Optionnellement lier a un checkpoint de confiance en comparant le hash du validator set
   a une valeur ancree (anchor de weak-subjectivity).
8. Optionnellement lier a un anchor d epoch attendu pour rejeter les preuves d epochs
   plus anciens/nouveaux jusqu a rotation intentionnelle de l anchor.

`BridgeFinalityVerifier` (dans `iroha_data_model::bridge`) applique ces controles, rejetant
chain-id/height drift, mismatch hash/version du validator set, PoPs manquantes ou invalides,
signataires dupliques/hors plage, signatures invalides, et epochs inattendus avant de
compter le quorum, afin que les light clients puissent reutiliser un seul verificateur.

## Verificateur de reference

`BridgeFinalityVerifier` accepte un `chain_id` attendu plus des anchors optionnels de
validator set et d epoch. Il applique le tuple header/block-hash/commit-certificate,
valide le hash/version du validator set, verifie signatures/quorum contre le roster de
validateurs annonce, et suit la derniere hauteur pour rejeter les preuves stale/sautees.
Lorsque des anchors sont fournis il rejette les replays entre epochs/rosters avec des
erreurs `UnexpectedEpoch`/`UnexpectedValidatorSet`; sans anchors il adopte le hash de
validator set et l epoch de la premiere preuve avant de continuer a appliquer des
erreurs deterministes pour signatures dupliquees/hors plage/insuffisantes.

## Surface API

- `GET /v2/bridge/finality/{height}` - renvoie `BridgeFinalityProof` pour la hauteur de bloc
  demandee. La negociation de contenu via `Accept` supporte Norito ou JSON.
- `GET /v2/bridge/finality/bundle/{height}` - renvoie `BridgeFinalityBundle`
  (commitment + justification + header/certificate) pour la hauteur demandee.

## Notes et suites

- Les preuves sont actuellement derivees des commit certificates stockes. L historique borne
  suit la fenetre de retention des commit certificates; les clients doivent mettre en cache
  des preuves d ancrage s ils ont besoin d horizons plus longs. Les requetes hors fenetre
  renvoient `CommitCertificateNotFound(height)`; exposez l erreur et revenez a un checkpoint
  ancre.
- Une preuve rejouee ou forgee avec un `block_hash` mismatch (header vs. certificate) est
  rejetee avec `CommitCertificateHashMismatch`; les clients doivent effectuer la meme
  verification de tuple avant la verification des signatures et rejeter les payloads
  mismatch.
- Le travail futur peut ajouter des chains de commitment MMR/authority-set pour reduire la
  taille des preuves pour de tres longues histories. Le commit certificate est enveloppe
  dans des enveloppes de commitment plus riches.
