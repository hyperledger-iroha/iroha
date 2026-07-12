---
lang: fr
direction: ltr
source: docs/source/bridge_finality.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 1cbd248fe14e63d00f002f09e1663181f3ab9bd99124ffeb89c56763b784046b
source_last_modified: "2026-07-12"
translation_last_reviewed: 2026-07-12
---

<!--
SPDX-License-Identifier: Apache-2.0
-->

# Preuves de finalité du bridge

Ce document définit le format de la première version. Il transporte la preuve
durable exacte produite par Sumeragi v2. L'enveloppe de preuve a la version de
schéma `1`, mais le protocole de consensus qu'elle contient est la version `2`.
Il n'existe ni projection, ni décodeur, ni solution de repli Sumeragi v1.

## Format exact

`BridgeFinalityProof` (Norito ou Norito JSON) contient exactement trois champs :

```text
{ version, block_header, finality_artifact }
```

- `version` vaut `1` ;
- `block_header` est le `BlockHeader` canonique ;
- `finality_artifact` est le `V2FinalityArtifact` exact et immuable stocké par
  le chemin d'application Sumeragi v2 ; il incorpore durablement un PoP
  BLS-normal par entrée, dans l'ordre de son roster.

L'artéfact est l'unique source des faits de consensus. Il contient les versions
de format et de protocole, la hauteur, le `HeightContext` immuable complet, le
`BlockSubject` exact, le hash du bloc, le CommitQC et les PoP alignés au roster.
Le contexte fige le chain id, les bornes d'époque, le mode, le CommitQC parent,
le roster ordonné de
`ValidatorPower`, le `DualQuorum`, l'engagement Nexus/AMX, les paramètres DA et
la graine de leader. Le contexte du parent qui termine une époque incorpore
aussi le `next_epoch_snapshot` optionnel ; puisqu'il participe au context id,
le CommitQC du parent l'authentifie avant qu'il puisse autoriser le roster
enfant. Le snapshot finalisé lie aussi son `epoch_end_height` et les
`validator_set_pops` alignés du prochain roster, en plus de ses paramètres.
Le sujet lie `parent_block_hash`, `block_hash` et
`payload_hash`. Aucun champ dupliqué de hauteur, chaîne, hash, roster ou
certificat n'est accepté au niveau de la preuve.

## Source durable et vérification

Avant de publier la finalité ou d'évincer le corps, Kura écrit le header canonique exact
et l'archive SCCP authentifiée par la racine dans un enregistrement retained-block
immuable, puis conserve l'artéfact V2 exact dans un enregistrement de finalité immuable
distinct. Les deux écritures sont idempotentes et refusent tout conflit à la même
hauteur. `build_finality_proof` lit uniquement le header retenu et l'enregistrement de
finalité vérifié ; il ne lit jamais un corps historique et ne remplace jamais les PoP
par l'état mondial mutable. Au redémarrage, l'association header/archive/artifact/hash
est vérifiée de nouveau. L'éviction du corps ne rend pas une preuve valide indisponible ;
un enregistrement absent, corrompu, conflictuel ou invérifiable échoue de manière fermée.

`verify_bridge_finality_proof` impose :

1. le schéma `1`, le format d'artéfact `1` et le protocole Sumeragi `2` ;
2. un contexte, un roster pondéré, un quorum, un parent et une transition
   d'époque structurellement valides ;
3. l'égalité exacte entre hauteur, context id, sujet, hash répété et CommitQC,
   avec phase `Commit` ;
4. le chain id attendu et la hauteur, le hash, le prédécesseur et la view
   recalculés du header, tous exactement liés à l'artéfact ;
5. un PoP BLS-normal durable et valide dans l'artéfact pour chaque membre du roster ;
6. des indices de signataires strictement croissants et dans les limites ;
7. simultanément au moins `floor(2n/3) + 1` signataires distincts et une
   puissance signée strictement supérieure aux deux tiers du total ;
8. la signature BLS agrégée sur le préimage de vote v2 exact.

Le préimage est séparé par le domaine `iroha:sumeragi:v2:vote` et encode en
Norito `{ protocol_version: 2, round: { context_id, height, view }, phase:
Commit, subject: { parent_block_hash, block_hash, payload_hash } }`. L'indice et
la signature individuelle n'en font pas partie ; la liste ordonnée du CommitQC
sélectionne les clés et PoP. La vérification BLS/PoP est toujours obligatoire.

## Ancre de confiance et successeurs

Une preuve isolée établit sa cohérence cryptographique sous le roster qu'elle
transporte, mais ne prouve pas que ce roster est canonique. Le
`BridgeFinalityVerifier` exige donc un `HeightContextId` explicitement approuvé
avant la première preuve ; il ne déduit jamais la confiance de cette preuve.
Il n'accepte ensuite que la hauteur immédiatement suivante, vérifie le CommitQC
parent sous le contexte et les PoP précédents, puis impose les règles v2 de
transition. Dans une époque, l'enfant copie les PoP alignés de l'artéfact
précédent ; à la frontière, époque, roster, quorum, graine et PoP doivent
correspondre au `next_epoch_snapshot` du contexte parent, y compris son
`epoch_end_height`, le tout authentifié par le CommitQC parent.
Les hauteurs anciennes, sautées ou non liées sont rejetées.

## Frontière de confiance SCCP

`TairaSccpMessageProofV1.finality_proof` est l'encodage Norito du même type ;
SCCP n'a pas de second transcript ni de second calcul de quorum. Le header, la
racine SCCP et la branche Merkle authentifient le message, tandis que la preuve
brute n'établit que la cohérence sous son roster figé.

La confiance vient du `SccpSoraFinalityAnchorV1` gouverné : réseau Taira exact,
protocole `2`, hash du chain id, hauteur/hash du checkpoint,
`checkpoint_context_id` et hash séparé par domaine de l'artéfact durable. Le
circuit sémantique expose le hash de cette ancre comme dernier signal public.
L'admission doit authentifier l'artéfact du checkpoint et vérifier chaque
successeur immédiat jusqu'à l'artéfact du message, ou comparer les mêmes
artéfacts locaux approuvés. Une signature valide sous un roster fourni par le
message ne suffit pas à établir la finalité Taira.

## Bundle et API

`BridgeFinalityBundle` contient exactement `{ commitment, finality_proof }`.
L'engagement est exactement
`{ chain_id, height_context_id, block_height, block_hash }`. SCCP utilise sa
propre branche Merkle typée et son ancre gouvernée.

- `GET /v1/bridge/finality/{height}` renvoie `BridgeFinalityProof`.
- `GET /v1/bridge/finality/bundle/{height}` renvoie `BridgeFinalityBundle`.

Les deux routes échouent si le header canonique retenu ou l'artéfact v2 exact est absent
ou invalide. L'éviction du corps historique ne rend pas une preuve valide indisponible.
Les consommateurs de première version doivent rejeter toute forme ou version inconnue ;
aucune compatibilité de repli n'est prévue.
