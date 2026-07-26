---
lang: fr
direction: ltr
source: docs/source/bridge_proofs.md
status: needs-review
generator: scripts/sync_docs_i18n.py
source_hash: 74e29801129deccb6d5640d414289c47cf13fa9e0229fb55212b6c7710d7c5f7
source_last_modified: "2026-07-12T07:38:49.568351+00:00"
translation_last_reviewed: 2026-07-12
translator: machine-assisted
---

> Cette page est un résumé traduit et abrégé, et non une traduction intégrale.
> La [page canonique en anglais](bridge_proofs.md) reste la source normative
> exacte pour la gouvernance, les API, la sémantique des preuves et les
> exigences de publication.

# Preuves de pont SCCP V1 — résumé abrégé

## Périmètre de la première version

SCCP V1 est un protocole fermé pour la première version. Les seules sources
externes prises en charge sont `ethereum-mainnet`, `bsc-mainnet` et
`tron-mainnet`, et l'unique destination SORA est `sora-taira`. Solana, TON, les
réseaux personnalisés et toute autre destination SORA ne sont pas pris en
charge et sont rejetés de manière sûre.

Dans cette version, `SubmitBridgeProof` n'accepte que les preuves typées
`NativeProtocol` et `SccpDestination`. La soumission de preuves génériques
`Ics` ou `TransparentZk` n'est pas disponible et reste rejetée tant qu'il
n'existe pas de vérificateur faisant autorité sur la chaîne.

## Registre typé et protection contre le rejeu

`SccpRegistryV1` est un registre typé, lié à chaque lane et en ajout seul
(append-only). Chaque lane conserve au plus 64 révisions de route et 4 096
native trust anchors. Les entrées historiques ne sont jamais évincées
implicitement ; à la limite, l'ajout suivant est rejeté atomiquement sans
modifier l'état.

Les intervalles d'anchor utilisent une coordonnée authentifiée de progression
du consensus : Ethereum emploie le finalized beacon slot, tandis que BSC et
TRON emploient la hauteur du native block finalisé. Un ancien anchor reste
valide jusqu'au checkpoint successeur inclus ; le dernier anchor courant est
ouvert. Le finality cutoff d'une route terminale doit être exactement égal au
checkpoint successeur de l'anchor historique.

L'enregistrement inbound durable conserve séparément la hauteur de finalité de
l'événement/source et l'`anchor_interval_height` vérifié. Un index high-water
durable, indexé par lane et hash d'anchor, interdit à la gouvernance de choisir
un checkpoint successeur inférieur à une coordonnée déjà admise. L'hydratation
d'un snapshot recalcule cet index depuis les enregistrements durables et exige
une égalité exacte ; un index absent, périmé, mal formé ou sans justification
est rejeté. Les identifiants de messages consommés restent également durables
afin d'empêcher le rejeu.

La route source TRON utilise l’ABI exacte
`transferToTaira(bytes,uint256,uint64 expectedNonce)`. L’exécution ne réussit
que si `expectedNonce == transferNonce`, puis écrit cette même valeur dans le
canonical payload avant d’incrémenter le storage. L’admission native reconstruit
l’appel ABI complet à partir du recipient du payload, du montant mis à l’échelle
et du nonce. Le selector retiré à deux arguments, un nonce ancien ou futur et
un nonce `uint64` épuisé sont donc tous rejetés de manière sûre.

## Vérification en un seul passage et limites de travail

Les preuves destination et native sont structurées une fois, liées une fois et
réservent le travail déterministe avant toute cryptographie coûteuse. Le chemin
destination vérifie une seule fois le pairing-product BN254 et une seule fois
la finalité BLS locale. Les chemins native exigent le préfixe canonique le plus
court : au plus 1 004 headers pour BSC et 54 pour TRON.

`[zk.sccp]` impose des limites non nulles par transaction et par bloc sur le
nombre et les octets des preuves, les native headers/bytes, les mises à jour du
light client Ethereum, les récupérations secp256k1, les vérifications BLS
aggregate et contributions de clés, ainsi que les vérifications de pairing
BN254. Ces limites d'admission sont liées au consensus : tous les validateurs
doivent employer les mêmes valeurs de fichier de configuration et aucune
substitution par variable d'environnement n'existe.

Les limites par défaut de la première version sont :

| Dimension de travail | Transaction | Bloc |
|---|---:|---:|
| proofs | 1 | 4 |
| canonical proof bytes | 8 MiB | 32 MiB |
| BSC/TRON continuation headers | 1 004 | 4 016 |
| Ethereum light-client updates | 128 | 512 |
| framed native-finality bytes | 8 MiB | 32 MiB |
| secp256k1 recoveries | 1 005 | 4 020 |
| BLS aggregate checks | 1 004 | 4 016 |
| BLS key/contribution work items | 131 713 | 526 852 |
| BN254 pairing-product checks | 1 | 4 |

Une proof peut contenir au plus 8 MiB de canonical bytes. Le travail réservé
par une transaction abandonnée ou rejetée ne se propage pas au bloc.

## Engagement outbound, rétention et découverte

Chaque message outbound réussi reçoit un `commitment_index` dense dans l'ordre
d'exécution du bloc (`0..=511`). V1 fixe les limites immuables à 512 messages par
bloc et 4 096 octets de payload canonique par message. `[zk.sccp]` borne
conjointement les payloads en attente avec `max_pending_outbound_messages`
(valeur par défaut `65536`) et `max_pending_outbound_payload_bytes` (valeur par
défaut `268435456`).

Avant de publier la finalité ou d'évincer le corps du bloc, Kura conserve de façon
immuable le header canonique exact et l'archive SCCP authentifiée par la racine. La
reconstruction des proofs, bundles, proof requests et de l'historique récent ne lit
ni le corps historique ni une copie mutable du payload dans le WSV. L'acceptation
de la destination proof supprime atomiquement le payload en attente et sa charge,
puis laisse un descripteur terminal de taille fixe avec son locator/index. L'état
en attente est borné ; les enregistrements terminaux et l'historique Kura immuable
croissent volontairement pour la protection permanente contre le rejeu.
`GET /v1/sccp/messages/recent` emploie le curseur composé
`{ from, after_index }`. Les preuves immuables comptent dans l'usage disque total
et opérateur, mais pas dans le budget des corps évictables.

## Limites Torii et HTTP

Torii applique une limite de corps JSON propre à chaque endpoint SCCP avant de
lire le corps, d'allouer de la mémoire ou d'effectuer une vérification
cryptographique. Un `Content-Length` ou un corps chunked trop grand est rejeté
avec HTTP `413`. Le client lit aussi la réponse HTTP décodée sous une limite
fixe ; un `Content-Length` absent ou mensonger ne peut donc pas la contourner.

Toutes les entrées JSON, base64 et Norito doivent être canoniques. Les champs
inconnus, clés dupliquées, network/route/anchor incorrects, rejeux, dépassements
de quota de travail ou échecs de vérification sont rejetés sans modification
partielle de l'état.
