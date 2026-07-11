---
lang: fr
direction: ltr
source: docs/source/bridge_proofs.md
status: needs-review
generator: scripts/sync_docs_i18n.py
source_hash: 465d8cf704022986b169ab93133517428f8cf2ffe01a498cbda458f4a5b2e69b
source_last_modified: "2026-07-11"
translation_last_reviewed: 2026-07-11
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
