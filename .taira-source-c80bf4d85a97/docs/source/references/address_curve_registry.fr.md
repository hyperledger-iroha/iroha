---
title: Registre des courbes de compte
description: Cartographie canonique entre les identifiants de courbe des contrôleurs de compte et les algorithmes de signature.
---

# Registre des courbes de compte

Les adresses de compte encodent leurs contrôleurs comme une charge utile
étiquetée qui commence par un identifiant de courbe sur 8 bits. Les validateurs,
les SDKs et les outils s’appuient sur un registre partagé afin que les
identifiants de courbe restent stables d’une version à l’autre et permettent un
décodage déterministe entre implémentations.

Le tableau ci‑dessous constitue la référence normative pour chaque `curve_id`
assigné. Une copie lisible par machine est publiée avec ce document dans
[`address_curve_registry.json`](address_curve_registry.json) ; les outils
automatisés DEVRAIENT consommer la version JSON et épingler son champ `version`
lors de la génération des fixtures.

## Courbes enregistrées

| ID (`curve_id`) | Algorithme | Garde de fonctionnalité | Statut | Encodage de la clé publique | Notes |
|-----------------|------------|-------------------------|--------|----------------------------|-------|
| `0x01` (1) | `ed25519` | — | Production | Clé Ed25519 compressée de 32 octets | Courbe canonique pour V1. Tous les SDKs DOIVENT prendre en charge cet identifiant. |
| `0x02` (2) | `ml-dsa` | — | Production (pilotée par configuration) | Clé publique Dilithium3 (1952 octets) | Disponible dans toutes les builds. Activer dans `crypto.allowed_signing` + `crypto.curves.allowed_curve_ids` avant d’émettre des payloads de contrôleur. |
| `0x03` (3) | `bls_normal` | `bls` | Production (pilotée par feature) | Clé publique G1 compressée de 48 octets | Requis pour les validateurs de consensus. L’admission accepte les contrôleurs BLS même si `allowed_signing`/`allowed_curve_ids` ne les listent pas. |
| `0x04` (4) | `secp256k1` | — | Production | Clé SEC1 compressée de 33 octets | ECDSA déterministe sur SHA‑256 ; les signatures utilisent la mise en page canonique `r∥s` de 64 octets. |
| `0x05` (5) | `bls_small` | `bls` | Production (pilotée par feature) | Clé publique G2 compressée de 96 octets | Profil BLS à signature compacte (signatures plus petites, clés publiques plus grandes). |
| `0x0A` (10) | `gost3410-2012-256-paramset-a` | `gost` | Réservé | Point TC26 param set A little‑endian de 64 octets | S’active avec la feature `gost` une fois le déploiement approuvé par la gouvernance. |
| `0x0B` (11) | `gost3410-2012-256-paramset-b` | `gost` | Réservé | Point TC26 param set B little‑endian de 64 octets | Reflète le paramètre TC26 B ; bloqué derrière la feature `gost`. |
| `0x0C` (12) | `gost3410-2012-256-paramset-c` | `gost` | Réservé | Point TC26 param set C little‑endian de 64 octets | Réservé pour une future approbation de gouvernance. |
| `0x0D` (13) | `gost3410-2012-512-paramset-a` | `gost` | Réservé | Point TC26 param set A little‑endian de 128 octets | Réservé en attente d’une demande pour les courbes GOST 512 bits. |
| `0x0E` (14) | `gost3410-2012-512-paramset-b` | `gost` | Réservé | Point TC26 param set B little‑endian de 128 octets | Réservé en attente d’une demande pour les courbes GOST 512 bits. |
| `0x0F` (15) | `sm2` | `sm` | Réservé | Longueur DistID (u16 BE) + octets DistID + clé SM2 SEC1 non compressée de 65 octets | Disponible quand la feature `sm` sortira de preview. |

### Recommandations d’usage

- **Fail closed :** Les encodeurs DOIVENT rejeter les algorithmes non pris en
  charge avec `ERR_UNSUPPORTED_ALGORITHM`. Les décodeurs DOIVENT lever
  `ERR_UNKNOWN_CURVE` pour tout identifiant absent de ce registre.
- **Garde de fonctionnalité :** BLS/GOST/SM2 restent derrière les features
  listées à la compilation. Les opérateurs doivent activer les entrées
  correspondantes dans `iroha_config.crypto.allowed_signing` et les features de
  build avant d’émettre des adresses avec ces courbes.
- **Exceptions d’admission :** Les contrôleurs BLS sont autorisés pour les
  validateurs de consensus même si `allowed_signing`/`allowed_curve_ids` ne les
  listent pas.
- **Parité config + manifest :** Utilisez `iroha_config.crypto.allowed_curve_ids`
  (et le `ManifestCrypto.allowed_curve_ids` correspondant) pour publier les
  identifiants de courbe acceptés par le cluster ; l’admission applique désormais
  cette liste en plus de `allowed_signing`.
- **Encodage déterministe :** Les clés publiques sont encodées exactement comme
  renvoyées par l’implémentation de signature (octets Ed25519 compressés, clés
  publiques ML‑DSA, points BLS compressés, etc.). Les SDKs doivent exposer les
  erreurs de validation avant de soumettre des payloads malformés.
- **Parité des manifests :** Les manifests de genèse et de contrôleur DOIVENT
  utiliser les mêmes identifiants afin que l’admission puisse rejeter les
  contrôleurs dépassant les capacités du cluster.

## Annonce du bitmap de capacités

`GET /v1/node/capabilities` expose désormais la liste `allowed_curve_ids` ainsi
que le tableau `allowed_curve_bitmap` empaqueté sous `crypto.curves`. Le bitmap
est little‑endian sur des lanes 64 bits (jusqu’à quatre valeurs pour couvrir
l’espace d’identifiants `u8` 0–255). Un bit `i` à 1 signifie que l’identifiant de
courbe `i` est autorisé par la politique d’admission du cluster.

- Exemple : `{ allowed_curve_ids: [1, 15] }` ⇒ `allowed_curve_bitmap: [32770]`
  car `(1 << 1) | (1 << 15) = 32770`.
- Les courbes au‑delà de `63` positionnent des bits dans des lanes ultérieures.
  Les lanes finales à zéro sont omises pour garder des payloads courts ; ainsi,
  une configuration qui active aussi `curve_id = 130` émettrait
  `allowed_curve_bitmap = [32768, 0, 4]` (bits 15 et 130 activés).

Préférez le bitmap pour les dashboards et checks de santé : un simple test de bit
répond aux questions de capacité sans parcourir le tableau complet, tandis que
les outils nécessitant des identifiants ordonnés peuvent continuer à utiliser
`allowed_curve_ids`. Exposer les deux vues satisfait l’exigence **ADDR-3** du
roadmap visant à publier des bitmaps de capacités déterministes pour les
opérateurs et les SDKs.

## Liste de validation

Chaque composant qui ingère des contrôleurs (Torii, admission, encodeurs SDK,
outils hors‑ligne) doit appliquer les mêmes vérifications déterministes avant
d’accepter une charge utile. Les étapes ci‑dessous sont obligatoires :

1. **Résoudre la politique du cluster :** Analysez l’octet `curve_id` en tête du
   payload du compte et rejetez le contrôleur si l’identifiant n’est pas présent
   dans `iroha_config.crypto.allowed_curve_ids` (et le miroir
   `ManifestCrypto.allowed_curve_ids`). Les contrôleurs BLS sont l’exception :
   lorsqu’ils sont compilés, l’admission les autorise indépendamment des allowlists
   afin que les clés de validateurs de consensus continuent de fonctionner. Cela
   empêche un cluster d’accepter des courbes en preview non activées.
2. **Imposer la longueur d’encodage :** Comparez la longueur du payload avec la
   taille canonique de l’algorithme avant de tenter de décompresser ou étendre
   la clé. Rejetez toute valeur qui échoue à ce contrôle pour éliminer les
   entrées malformées tôt.
3. **Décodage spécifique à l’algorithme :** Utilisez les mêmes décodeurs
   canoniques que `iroha_crypto` (`ed25519_dalek`, `pqcrypto_mldsa`,
   `w3f_bls`/`blstrs`, `sm2`, les helpers TC26, etc.) afin que toutes les
   implémentations partagent le même comportement de validation de sous‑groupe
   et de points.
4. **Vérifier la taille des signatures :** L’admission et les SDKs doivent
   imposer les longueurs de signature ci‑dessous et rejeter toute payload avec
   une signature tronquée ou trop longue avant la vérification.

| Algorithme | `curve_id` | Octets clé publique | Octets signature | Vérifications critiques |
|-----------|------------|---------------------|------------------|-------------------------|
| `ed25519` | `0x01` | 32 | 64 | Rejeter les points compressés non canoniques, forcer l’élimination du cofacteur (pas de points d’ordre réduit) et assurer `s < L` lors de la validation des signatures. |
| `ml-dsa` (Dilithium3) | `0x02` | 1952 | 3309 | Rejeter tout payload dont la longueur n’est pas exactement 1952 octets avant décodage ; parser la clé publique Dilithium3 et vérifier les signatures avec pqcrypto‑dilithium et des longueurs canoniques. |
| `bls_normal` | `0x03` | 48 | 96 | N’accepter que des clés publiques G1 compressées canoniques et des signatures G2 compressées ; rejeter les points identité et les encodages non canoniques. |
| `secp256k1` | `0x04` | 33 | 64 | N’accepter que des points SEC1 compressés ; décompresser et rejeter les points non canoniques/invalides, et vérifier les signatures avec l’encodage canonique `r∥s` de 64 octets (normalisation low‑`s` appliquée par le signataire). |
| `bls_small` | `0x05` | 96 | 48 | N’accepter que des clés publiques G2 compressées canoniques et des signatures G1 compressées ; rejeter les points identité et les encodages non canoniques. |
| `gost3410-2012-256-paramset-a` | `0x0A` | 64 | 64 | Interpréter le payload comme des coordonnées `(x||y)` little‑endian, s’assurer que chaque coordonnée `< p`, rejeter le point identité et imposer des limbs `r`/`s` canoniques de 32 octets lors de la vérification. |
| `gost3410-2012-256-paramset-b` | `0x0B` | 64 | 64 | Même validation que le paramètre A mais avec les paramètres TC26 B. |
| `gost3410-2012-256-paramset-c` | `0x0C` | 64 | 64 | Même validation que le paramètre A mais avec les paramètres TC26 C. |
| `gost3410-2012-512-paramset-a` | `0x0D` | 128 | 128 | Interpréter `(x||y)` comme des limbs de 64 octets, s’assurer que `< p`, rejeter le point identité et exiger des limbs `r`/`s` de 64 octets pour les signatures. |
| `gost3410-2012-512-paramset-b` | `0x0E` | 128 | 128 | Même validation que le paramètre A mais avec les paramètres TC26 B 512 bits. |
| `sm2` | `0x0F` | 2 + distid + 65 | 64 | Décoder la longueur distid (u16 BE), valider les octets DistID, parser le point SEC1 non compressé, appliquer les règles de sous‑groupe GM/T 0003, appliquer le DistID configuré et exiger des limbs `(r, s)` canoniques selon SM2. |

Chaque ligne correspond à l’objet `validation` dans
[`address_curve_registry.json`](address_curve_registry.json). Les outils
consommant l’export JSON peuvent s’appuyer sur les champs `public_key_bytes`,
`signature_bytes` et `checks` pour automatiser les mêmes étapes de validation
ci‑dessus ; les encodages à longueur variable (par exemple SM2) définissent
`public_key_bytes` à null et documentent la règle de longueur dans `checks`.

## Demander un nouvel identifiant de courbe

1. Rédigez la spécification de l’algorithme (encodage, validation, gestion des
   erreurs) et obtenez l’approbation de gouvernance pour le déploiement.
2. Soumettez une pull request mettant à jour ce document et
   `address_curve_registry.json`. Les nouveaux identifiants doivent être uniques
   et appartenir à la plage inclusive `0x01..=0xFE`.
3. Mettez à jour les SDKs, les fixtures Norito et la documentation opérateur
   avec le nouvel identifiant avant de déployer en production.
4. Coordonnez‑vous avec les responsables sécurité et observabilité afin que la
   télémétrie, les runbooks et les politiques d’admission reflètent le nouvel
   algorithme.
