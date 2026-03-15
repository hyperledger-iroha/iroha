# RFC sur la structure du compte

**Statut :** Accepté (ADDR-1)  
**Public :** Modèle de données, Torii, Nexus, Wallet, équipes de gouvernance  
**Problèmes connexes :** À déterminer

## Résumé

Ce document décrit la pile d'adressage des comptes d'expédition implémentée dans
`AccountAddress` (`crates/iroha_data_model/src/account/address.rs`) et le
outillage compagnon. Il fournit :

- Une **adresse Iroha Base58 (I105)** avec somme de contrôle et face humaine produite par
  `AccountAddress::to_i105` qui lie une chaîne discriminante au compte
  contrôleur et propose des formes textuelles déterministes et conviviales pour l’interopérabilité.
- Sélecteurs de domaine pour les domaines par défaut implicites et les résumés locaux, avec un
  balise de sélection de registre global réservée pour le futur routage basé sur Nexus (la
  la recherche dans le registre n'est **pas encore expédiée**).

## Motivation

Les portefeuilles et les outils hors chaîne s'appuient aujourd'hui sur des alias de routage bruts `alias@domain` (rejected legacy form). Ceci
présente deux inconvénients majeurs :

1. **Aucune liaison réseau.** La chaîne n'a pas de somme de contrôle ni de préfixe de chaîne, donc les utilisateurs
   peut coller une adresse provenant du mauvais réseau sans retour immédiat. Le
   la transaction sera finalement rejetée (inadéquation de la chaîne) ou, pire, réussira
   contre un compte involontaire si la destination existe localement.
2. **Collision de domaines.** Les domaines sont réservés à l'espace de noms et peuvent être réutilisés sur chaque
   chaîne. Fédération de services (dépositaires, ponts, workflows cross-chain)
   devient fragile car `finance` sur la chaîne A n'a aucun rapport avec `finance` sur
   chaîne B.

Nous avons besoin d’un format d’adresse convivial qui protège contre les erreurs de copier/coller
et une cartographie déterministe du nom de domaine à la chaîne faisant autorité.

## Objectifs

- Décrire l'enveloppe I105 Base58 implémentée dans le modèle de données et le
  règles canoniques d'analyse/alias que `AccountId` et `AccountAddress` suivent.
- Encodez le discriminant de chaîne configuré directement dans chaque adresse et
  définir son processus de gouvernance/registre.
- Décrire comment introduire un registre de domaine mondial sans rompre le courant
  déploiements et spécifier des règles de normalisation/anti-usurpation d'identité.

## Non-objectifs

- Mise en œuvre de transferts d'actifs inter-chaînes. La couche de routage renvoie uniquement le
  chaîne cible.
- Finaliser la gouvernance pour l'émission de domaines mondiaux. Cette RFC se concentre sur les données
  primitives de modèle et de transport.

## Contexte

### Alias de routage actuel

```
AccountId {
    domain: DomainId,   // wrapper over Name (ASCII-ish string)
    controller: AccountController // single PublicKey or multisig policy
}

Display: canonical I105 literal (no `@domain` suffix)
Parse accepts:
- Encoded account identifiers only: I105.
- Runtime parsers reject canonical hex (`0x...`), any `@<domain>` suffix, and alias literals such as `label@domain`.

Multihash hex is canonical: varint bytes are lowercase hex, payload bytes are uppercase hex,
and `0x` prefixes are not accepted.

This text form is now treated as an **account alias**: a routing convenience
that points to the canonical [`AccountAddress`](#2-canonical-address-codecs).
It remains useful for human readability and domain-scoped governance, but it is
no longer considered the authoritative account identifier on-chain.
```

`ChainId` vit à l'extérieur de `AccountId`. Les nœuds vérifient le `ChainId` de la transaction
contre configuration lors de l'admission (`AcceptTransactionFail::ChainIdMismatch`)
et rejeter les transactions étrangères, mais la chaîne de compte elle-même ne comporte aucun
indice de réseau.

### Identifiants de domaine

`DomainId` encapsule un `Name` (chaîne normalisée) et s'étend à la chaîne locale.
Chaque chaîne peut enregistrer `wonderland`, `finance`, etc. indépendamment.

### Contexte Nexus

Nexus est responsable de la coordination entre les composants (voies/espaces de données). Il
n'a actuellement aucun concept de routage de domaine inter-chaînes.

## Conception proposée

### 1. Discriminant de chaîne déterministe

`iroha_config::parameters::actual::Common` expose désormais :

```rust
pub struct Common {
    pub chain: ChainId,
    pub chain_discriminant: u16, // globally coordinated
    // ... existing fields
}
```

- **Contraintes :**
  - Unique par réseau actif ; géré via un registre public signé avec
    plages réservées explicites (par exemple, `0x0000–0x0FFF` test/dev, `0x1000–0x7FFF`
    allocations communautaires, `0x8000–0xFFEF` approuvées par la gouvernance, `0xFFF0–0xFFFF`
    réservé).
  - Immuable pour une chaîne en cours d'exécution. Le changer nécessite un hard fork et un
    mise à jour du registre.
- **Gouvernance et registre (prévu) :** Un ensemble de gouvernance multi-signatures
  maintenir un registre JSON signé mappant les discriminants aux alias humains et
  Identifiants CAIP-2. Ce registre ne fait pas encore partie du runtime livré.
- **Utilisation :** Enfilé via l'admission d'État, Torii, les SDK et les API de portefeuille, donc
  chaque composant peut l'intégrer ou le valider. L’exposition au CAIP-2 reste un avenir
  tâche d'interopérabilité.

### 2. Codecs d'adresses canoniques

Le modèle de données Rust expose une seule représentation canonique de la charge utile
(`AccountAddress`) qui peut être émis sous plusieurs formats destinés aux humains. I105 est
le format de compte préféré pour le partage et la sortie canonique ; le compressé
Le formulaire `sora` est une option de deuxième choix, réservée à Sora, pour l'UX où l'alphabet kana
ajoute de la valeur. L'hexagone canonique reste une aide au débogage.

- **I105 (Iroha Base58)** – une enveloppe Base58 qui intègre la chaîne
  discriminant. Les décodeurs valident le préfixe avant de promouvoir la charge utile vers
  la forme canonique.
- **Vue compressée Sora** – un alphabet Sora uniquement de **105 symboles** construit par
  ajouter le poème イロハ demi-chasse (comprenant ヰ et ヱ) aux 58 caractères
  Ensemble I105. Les chaînes commencent par la sentinelle `sora`, intègrent un dérivé de Bech32m
  somme de contrôle et omettez le préfixe réseau (Sora Nexus est implicite par la sentinelle).

```
  I105  : 123456789ABCDEFGHJKMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz
  Iroha : ｲﾛﾊﾆﾎﾍﾄﾁﾘﾇﾙｦﾜｶﾖﾀﾚｿﾂﾈﾅﾗﾑｳヰﾉｵｸﾔﾏｹﾌｺｴﾃｱｻｷﾕﾒﾐｼヱﾋﾓｾｽ
  ```
- **Canonical hex** – un encodage `0x…` convivial pour le débogage de l'octet canonique
  enveloppe.

`AccountAddress::parse_encoded` détecte automatiquement I105 (de préférence), compressé (`sora`, deuxième meilleur) ou hexadécimal canonique
(`0x...` uniquement ; l'hexagone nu est rejeté) saisit et renvoie à la fois la charge utile décodée et la charge détectée.
`AccountAddress`. Torii appelle désormais `parse_encoded` pour la norme ISO 20022 supplémentaire
traite et stocke la forme hexadécimale canonique afin que les métadonnées restent déterministes
quelle que soit la représentation originale.

#### 2.1 Disposition des octets d'en-tête (ADDR-1a)

Chaque charge utile canonique est présentée comme `header · controller`. Le
`header` est un seul octet qui indique quelles règles d'analyseur s'appliquent aux octets qui
suivre :

```
bit index:   7        5 4      3 2      1 0
             ┌─────────┬────────┬────────┬────┐
payload bit: │version  │ class  │  norm  │ext │
             └─────────┴────────┴────────┴────┘
```

Le premier octet contient donc les métadonnées du schéma pour les décodeurs en aval :

| Morceaux | Champ | Valeurs autorisées | Erreur en cas de violation |
|------|-------|----------------|----------|
| 7-5 | `addr_version` | `0` (v1). Les valeurs `1-7` sont réservées pour les révisions futures. | Les valeurs en dehors de `0-7` déclenchent `AccountAddressError::InvalidHeaderVersion` ; les implémentations DOIVENT traiter les versions non nulles comme non prises en charge aujourd'hui. |
| 4-3 | `addr_class` | `0` = clé unique, `1` = multisig. | Les autres valeurs augmentent `AccountAddressError::UnknownAddressClass`. |
| 2-1 | `norm_version` | `1` (Norme v1). Les valeurs `0`, `2`, `3` sont réservées. | Les valeurs en dehors de `0-3` augmentent `AccountAddressError::InvalidNormVersion`. |
| 0 | `ext_flag` | DOIT être `0`. | Le bit activé augmente `AccountAddressError::UnexpectedExtensionFlag`. |

L'encodeur Rust écrit `0x02` pour les contrôleurs à touche unique (version 0, classe 0,
norme v1, indicateur d'extension effacé) et `0x0A` pour les contrôleurs multisig (version 0,
classe 1, norme v1, drapeau d'extension effacé).

#### 2.2 Domainless payload semantics

Canonical payload bytes are domainless: the wire layout is `header · controller`
with no selector segment, no implicit default-domain reconstruction, and no
public decode fallback for legacy scoped-account literals.

Explicit domain context is modeled separately as `ScopedAccountId { account,
domain }` or separate API fields; it is not encoded into `AccountId` payload
bytes.

| Tag | Meaning | Payload | Notes |
|-----|---------|---------|-------|
| `0x00` | Domainless canonical scope | none | Canonical account payloads are domainless; explicit domain context lives outside the address payload. |
| `0x01` | Local domain digest | 12 bytes | Digest = `blake2s_mac(key = "SORA-LOCAL-K:v1", canonical_label)[0..12]`. |
| `0x02` | Global registry entry | 4 bytes | Big-endian `registry_id`; reserved until the global registry ships. |

Domain labels are canonicalised (UTS-46 + STD3 + NFC) before hashing. Unknown tags raise `AccountAddressError::UnknownDomainTag`. When validating an address against a domain, mismatched selectors raise `AccountAddressError::DomainMismatch`.

```
legacy selector segment
┌──────────┬──────────────────────────────────────────────┐
│ tag (u8) │ payload (depends on selector kind, see table)│
└──────────┴──────────────────────────────────────────────┘
```

When present, the selector is immediately adjacent to the controller payload, so
a decoder can walk the wire format in order: read the tag byte, read the
tag-specific payload, then move on to the controller bytes.

**Legacy selector examples**

- *Implicit default* (`tag = 0x00`). No payload. Example canonical hex for the default
  domain using the deterministic test key:
  `0x020001203b6a27bcceb6a42d62a3a8d02a6f0d73653215771de243a63ac048a18b59da29`.
- *Local digest* (`tag = 0x01`). Payload is the 12-byte digest. Example (`treasury` seed
  `0x01`): `0x0201b18fe9c1abbac45b3e38fc5d0001208a88e3dd7409f195fd52db2d3cba5d72ca6709bf1d94121bf3748801b40f6f5c`.
- *Global registry* (`tag = 0x02`). Payload is a big-endian `registry_id:u32`. The bytes
  that follow the payload are identical to the implicit-default case; the selector simply
  replaces the normalised domain string with a registry pointer. Example using
  `registry_id = 0x0000_002A` (decimal 42) and the deterministic default controller:
  `0x02020000002a000120641297079357229f295938a4b5a333de35069bf47b9d0704e45805713d13c201`.

#### 2.3 Encodages de la charge utile du contrôleur (ADDR-1a)

La charge utile du contrôleur est une autre union balisée ajoutée après le sélecteur de domaine :

| Étiquette | Contrôleur | Mise en page | Remarques |
|-----|------------|--------|-------|
| `0x00` | Clé unique | `curve_id:u8` · `key_len:u8` · `key_bytes` | `curve_id=0x01` correspond à Ed25519 aujourd'hui. `key_len` est limité à `u8` ; des valeurs plus grandes augmentent `AccountAddressError::KeyPayloadTooLong` (de sorte que les clés publiques ML‑DSA à clé unique, qui font >255 octets, ne peuvent pas être codées et doivent utiliser multisig). |
| `0x01` | Multisignature | `version:u8` · `threshold:u16` · `member_count:u8` · (`curve_id:u8` · `weight:u16` · `key_len:u16` · `key_bytes`)\* | Prend en charge jusqu'à 255 membres (`CONTROLLER_MULTISIG_MEMBER_MAX`). Les courbes inconnues augmentent `AccountAddressError::UnknownCurve` ; les politiques mal formulées apparaissent sous le nom de `AccountAddressError::InvalidMultisigPolicy`. |

Les politiques Multisig exposent également une carte CBOR de style CTAP2 et un résumé canonique afin
les hôtes et les SDK peuvent vérifier le contrôleur de manière déterministe. Voir
`docs/source/references/multisig_policy_schema.md` (ADDR-1c) pour le schéma,
règles de validation, procédure de hachage et luminaires dorés.

Tous les octets clés sont codés exactement comme renvoyé par `PublicKey::to_bytes` ; les décodeurs reconstruisent les instances de `PublicKey` et lèvent `AccountAddressError::InvalidPublicKey` si les octets ne correspondent pas à la courbe déclarée.

> **Application canonique Ed25519 (ADDR-3a) :** les clés de courbe `0x01` doivent décoder la chaîne d'octets exacte émise par le signataire et ne doivent pas se trouver dans le sous-groupe de petit ordre. Les nœuds rejettent désormais les codages non canoniques (par exemple, les valeurs réduites modulo `2^255-19`) et les points faibles tels que l'élément d'identité, de sorte que les SDK doivent faire apparaître des erreurs de validation de correspondance avant de soumettre des adresses.

##### 2.3.1 Registre des identifiants de courbe (ADDR-1d)

| ID (`curve_id`) | Algorithme | Porte de fonctionnalités | Remarques |
|-----------------|-----------|--------------|-------|
| `0x00` | Réservé | — | NE DOIT PAS être émis ; surface des décodeurs `ERR_UNKNOWN_CURVE`. |
| `0x01` | Ed25519 | — | Algorithme canonique v1 (`Algorithm::Ed25519`); activé dans la configuration par défaut. |
| `0x02` | ML‑DSA (Dilithium3) | — | Utilise les octets de la clé publique Dilithium3 (1952 octets). Les adresses à clé unique ne peuvent pas coder ML‑DSA car `key_len` est `u8` ; multisig utilise les longueurs `u16`. |
| `0x03` | BLS12‑381 (normal) | `bls` | Clés publiques en G1 (48 octets), signatures en G2 (96 octets). |
| `0x04` | secp256k1 | — | ECDSA déterministe sur SHA‑256 ; les clés publiques utilisent la forme compressée SEC1 de 33 octets et les signatures utilisent la disposition canonique `r∥s` de 64 octets. |
| `0x05` | BLS12‑381 (petit) | `bls` | Clés publiques en G2 (96 octets), signatures en G1 (48 octets). |
| `0x0A` | GOST R 34.10‑2012 (256, ensemble A) | `gost` | Disponible uniquement lorsque la fonctionnalité `gost` est activée. |
| `0x0B` | GOST R 34.10‑2012 (256, ensemble B) | `gost` | Disponible uniquement lorsque la fonctionnalité `gost` est activée. |
| `0x0C` | GOST R 34.10‑2012 (256, ensemble C) | `gost` | Disponible uniquement lorsque la fonctionnalité `gost` est activée. |
| `0x0D` | GOST R 34.10‑2012 (512, ensemble A) | `gost` | Disponible uniquement lorsque la fonctionnalité `gost` est activée. |
| `0x0E` | GOST R 34.10‑2012 (512, ensemble B) | `gost` | Disponible uniquement lorsque la fonctionnalité `gost` est activée. |
| `0x0F` | SM2 | `sm` | Longueur DistID (u16 BE) + octets DistID + clé SM2 non compressée SEC1 de 65 octets ; disponible uniquement lorsque `sm` est activé. |

Les emplacements `0x06–0x09` restent non attribués pour des courbes supplémentaires ; introduisant un nouveau
L’algorithme nécessite une mise à jour de la feuille de route et une couverture SDK/hôte correspondante. Encodeurs
DOIT rejeter tout algorithme non pris en charge avec `ERR_UNSUPPORTED_ALGORITHM`, et
les décodeurs DOIVENT échouer rapidement sur des identifiants inconnus avec `ERR_UNKNOWN_CURVE` pour préserver
comportement de fermeture en cas d'échec.

Le registre canonique (y compris une exportation JSON lisible par machine) réside sous
[`docs/source/references/address_curve_registry.md`](source/references/address_curve_registry.md).
L'outillage DEVRAIT consommer cet ensemble de données directement afin que les identifiants de courbe restent
cohérent entre les SDK et les flux de travail des opérateurs.

- **SDK gating :** Les SDK sont par défaut sur la validation/l'encodage Ed25519 uniquement. Swift expose
  indicateurs de compilation (`IROHASWIFT_ENABLE_MLDSA`, `IROHASWIFT_ENABLE_GOST`,
  `IROHASWIFT_ENABLE_SM`); le SDK Java/Android nécessite
  `AccountAddress.configureCurveSupport(...)`; le SDK JavaScript utilise
  `configureCurveSupport({ allowMlDsa: true, allowGost: true, allowSm2: true })`.
  La prise en charge de secp256k1 est disponible mais n'est pas activée par défaut dans JS/Android
  SDK ; les appelants doivent s’inscrire explicitement lorsqu’ils émettent des contrôleurs non Ed25519.
- **Host gating :** `Register<Account>` rejette les contrôleurs dont les signataires utilisent des algorithmes
  manquant dans la liste `crypto.allowed_signing` du nœud **ou** identifiants de courbe absents de
  `crypto.curves.allowed_curve_ids`, les clusters doivent donc annoncer la prise en charge (configuration +
  Genesis) avant que les contrôleurs ML‑DSA/GOST/SM puissent être enregistrés. Contrôleur BLS
  les algorithmes sont toujours autorisés une fois compilés (les clés de consensus en dépendent),
  et la configuration par défaut active Ed25519 + secp256k1.【crates/iroha_core/src/smartcontracts/isi/domain.rs:32】

##### 2.3.2 Guidage du contrôleur Multisig

`AccountController::Multisig` sérialise les politiques via
`crates/iroha_data_model/src/account/controller.rs` et applique le schéma
documenté dans [`docs/source/references/multisig_policy_schema.md`](source/references/multisig_policy_schema.md).
Détails clés de la mise en œuvre :

- Les politiques sont normalisées et validées par `MultisigPolicy::validate()` avant
  étant incorporé. Les seuils doivent être ≥1 et ≤Σ poids ; les membres en double sont
  supprimé de manière déterministe après tri par `(algorithm || 0x00 || key_bytes)`.
- La charge utile du contrôleur binaire (`ControllerPayload::Multisig`) code
  `version:u8`, `threshold:u16`, `member_count:u8`, puis celui de chaque membre
  `(curve_id, weight:u16, key_len:u16, key_bytes)`. C'est exactement ce que
  `AccountAddress::canonical_bytes()` écrit sur les charges utiles I105 (préféré)/sora (deuxième meilleur).
- Hashing (`MultisigPolicy::digest_blake2b256()`) utilise Blake2b-256 avec le
  `iroha-ms-policy` chaîne de personnalisation afin que les manifestes de gouvernance puissent se lier à un
  ID de stratégie déterministe qui correspond aux octets du contrôleur intégrés dans I105.
- La couverture des luminaires réside dans `fixtures/account/address_vectors.json` (cas
  `addr-multisig-*`). Les portefeuilles et les SDK doivent affirmer les chaînes canoniques I105
  ci-dessous pour confirmer que leurs encodeurs correspondent à l'implémentation de Rust.

| Numéro d'identification du cas | Seuil / membres | Littéral I105 (préfixe `0x02F1`) | Sora compressé (`sora`) littéral | Remarques |
|---------|----------|--------------------------------|-------------------------|-------|
| `addr-multisig-council-threshold3` | `≥3` poids, membres `(2,1,1)` | `SRfSHsrH3tEmYaaAYyD248F3vfT1oQ3WEGS22MaD8W9bLefF7rsoKLYGcpbcM9EcSus5ZhCAZU7ztn2BCsyeCAdfRncAVmVsipd4ibk6CBLF3Nrzcw8P7VKJg6mtFgEhWVTjfDkUMoc63oeEmaWyV6cyiphwk8ZgKAJUe4TyVtmKm1WWcg7qZ6i` | `sora3vﾑ2zkaoUwﾋﾅGﾘﾚyﾂe3ﾖfﾙヰｶﾘﾉwｷnoWﾛYicaUr3ﾔｲﾖ2Ado3TﾘYQﾉJqﾜﾇｳﾑﾐd8dDjRGｦ3Vﾃ9HcﾀMヰR8ﾎﾖgEqGｵEｾDyc5ﾁ1ﾔﾉ31sUﾑﾀﾖaｸxﾘ3ｲｷMEuFｺｿﾉBQSVQnxﾈeJzrXLヰhｿｹ5SEEﾅPﾂﾗｸdヰﾋ1bUGHｲVXBWNNJ6K` | Quorum de gouvernance du domaine du Conseil. |
| `addr-multisig-wonderland-threshold2` | `≥2`, membres `(1,2)` | `3xsmkps1KPBn9dtpE5qHRhHEZCpiAe8d9j6H9A42TV6kc1TpaqdwnSksKgQrsSEHznqvWKBMc1os69BELzkLjsR7EV2gjV14d9JMzo97KEmYoKtxCrFeKFAcy7ffQdboV1uRt` | `sora2ﾖZﾘeｴAdx3ﾂﾉﾔXhnｹﾀ2ﾉｱﾋxﾅﾄﾌヱwﾐmﾊvEﾐCﾏﾎｦ1ﾑHﾋso2GKﾔﾕﾁwﾂﾃP6ﾁｼﾙﾖｺ9ｻｦbﾈ4wFdﾑFヰ3HaﾘｼMｷﾌHWtｷﾋLﾙﾖQ4D3XﾊﾜXmpktﾚｻ5ﾅﾅﾇ1gkﾏsCFQGH9` | Exemple de pays des merveilles à double signature (poids 1 + 2). |
| `addr-multisig-default-quorum3` | `≥3`, membres `(1,1,1,1)` | `nA2bDNhMqXz7ERkHNoEWbvJGyR1aDRsw32LaUWLgbK3vcpzohmdFCLvdotxUWWDY3aZeX4ptLk4Z6TjF5ossnJm8VrNo6daxmGTkqUyP4MxJxiNyPFxsEE5DLnsoLWUcxaWNpZ76tmkbiGS31Gv8tejKpuiHUMaQ1s5ohWyZvDnpycNkBK8AEfGJqn5yc9zAzfWbVhpDwkPj8ScnzvH1Echr5` | `soraﾐ38ﾅｴｸﾜ8ﾃzwBrqﾘｺ4yﾄv6kqJp1ｳｱﾛｿrzﾄﾃﾘﾒRﾗtV9ｼﾔPｽcヱEﾌVVVｼﾘｲZAｦﾓﾅｦeﾒN76vﾈcuｶuﾛL54rzﾙﾏX2zMﾌRLﾃﾋpﾚpｲcHﾑﾅﾃﾔzｵｲVfAﾃﾚﾎﾚCヰﾔｲｽｦw9ﾔﾕ8bGGkﾁ6sNｼaｻRﾖﾜYﾕﾚU18ﾅHヰﾌuMeﾊtﾂrｿj95Ft8ﾜ3fﾄkNiｴuﾈrCﾐQt8ヱｸｸmﾙﾒgUbﾑEKTTCM` | Quorum de domaine implicite par défaut utilisé pour la gouvernance de base.

#### 2.4 Règles de défaillance (ADDR-1a)

- Les charges utiles plus courtes que l'en-tête + le sélecteur requis ou avec des octets restants émettent `AccountAddressError::InvalidLength` ou `AccountAddressError::UnexpectedTrailingBytes`.
- Les en-têtes qui définissent le `ext_flag` réservé ou annoncent des versions/classes non prises en charge DOIVENT être rejetés en utilisant `UnexpectedExtensionFlag`, `InvalidHeaderVersion` ou `UnknownAddressClass`.
- Les balises de sélecteur/contrôleur inconnues génèrent `UnknownDomainTag` ou `UnknownControllerTag`.
- Les éléments de clé surdimensionnés ou mal formés soulèvent `KeyPayloadTooLong` ou `InvalidPublicKey`.
- Les contrôleurs Multisig dépassant 255 membres génèrent `MultisigMemberOverflow`.
- Conversions IME/NFKC : les Sora kana demi-largeur peuvent être normalisés dans leur forme pleine largeur sans interrompre le décodage, mais la sentinelle ASCII `sora` et les chiffres/lettres I105 DOIVENT rester ASCII. Les sentinelles pleine largeur ou pliées font surface `ERR_MISSING_COMPRESSED_SENTINEL`, les charges utiles ASCII pleine largeur augmentent `ERR_INVALID_COMPRESSED_CHAR` et les discordances de somme de contrôle apparaissent sous la forme `ERR_CHECKSUM_MISMATCH`. Les tests de propriété dans `crates/iroha_data_model/src/account/address.rs` couvrent ces chemins afin que les SDK et les portefeuilles puissent s'appuyer sur des échecs déterministes.
- L'analyse Torii et SDK des alias `address@domain` (rejected legacy form) émettent désormais les mêmes codes `ERR_*` lorsque les entrées I105 (préféré)/sora (deuxième meilleur) échouent avant le repli de l'alias (par exemple, non-concordance de somme de contrôle, non-concordance de résumé de domaine), afin que les clients puissent relayer des raisons structurées sans deviner à partir de chaînes de prose.
- Les charges utiles du sélecteur local de moins de 12 octets apparaissent `ERR_LOCAL8_DEPRECATED`, préservant un basculement définitif à partir des anciens résumés Local‑8.
- Domainless canonical I105 literals decode directly to a domainless `AccountId`. Use `ScopedAccountId` only when an interface requires explicit domain context.

#### 2.5 Vecteurs binaires normatifs

- **Domaine implicite par défaut (`default`, octet de départ `0x00`)**  
  Hex canonique : `0x020001203b6a27bcceb6a42d62a3a8d02a6f0d73653215771de243a63ac048a18b59da29`.  
  Répartition : `0x02` en-tête, `0x00` sélecteur (par défaut implicite), `0x00` balise de contrôleur, `0x01` identifiant de courbe (Ed25519), `0x20` longueur de clé, suivi de la charge utile de clé de 32 octets.
- **Résumé de domaine local (`treasury`, octet de départ `0x01`)**  
  Hex canonique : `0x0201b18fe9c1abbac45b3e38fc5d0001208a88e3dd7409f195fd52db2d3cba5d72ca6709bf1d94121bf3748801b40f6f5c`.  
  Répartition : `0x02` en-tête, balise de sélection `0x01` plus résumé `b1 8f e9 c1 ab ba c4 5b 3e 38 fc 5d`, suivi de la charge utile à clé unique (`0x00` balise, `0x01` identifiant de courbe, `0x20` longueur, 32 octets Ed25519 clé).

Les tests unitaires (`account::address::tests::parse_encoded_accepts_all_formats`) affirment les vecteurs V1 ci-dessous via `AccountAddress::parse_encoded`, garantissant que les outils peuvent s'appuyer sur la charge utile canonique sur les formulaires hexadécimaux, I105 (de préférence) et compressés (`sora`, deuxième meilleur). Régénérez le jeu de luminaires étendu avec `cargo run -p iroha_data_model --example address_vectors`.

| Domaine | Octet de départ | Hex canonique | Compressé (`sora`) |
|-------------|-----------|-------------------------------------------------------------------------------------------------------|------------|
| par défaut | `0x00` | `0x020001203b6a27bcceb6a42d62a3a8d02a6f0d73653215771de243a63ac048a18b59da29` | `sorauﾛ1NﾗhBUd2BﾂｦﾄiﾔﾆﾂﾇKSﾃaﾘﾒﾓQﾗrﾒoﾘﾅnｳﾘbQｳQJﾆLJ5HSE` |
| trésor | `0x01` | `0x0201b18fe9c1abbac45b3e38fc5d0001208a88e3dd7409f195fd52db2d3cba5d72ca6709bf1d94121bf3748801b40f6f5c` | `sora5ｻu6rﾀCヰTGwﾏ1ﾅヱﾌQｲﾖﾇqCｦヰﾓZQCZRDSSﾅMｱﾙヱｹﾁｸ8ｾeﾄﾛ6C8bZuwﾗｹCZｦRSLQFU` |
| pays des merveilles | `0x02` | `0x0201b8ae571b79c5a80f5834da2b0001208139770ea87d175f56a35466c34c7ecccb8d8a91b4ee37a25df60f5b8fc9b394` | `sora5ｻwﾓyRｿqﾏnMﾀﾙヰKoﾒﾇﾓQｺﾛyｼ3ｸFHB2F5LyPﾐTMZkｹｼw67ﾋVﾕｻr8ﾉGﾇeEnｻVRNKCS` |
| Iroha | `0x03` | `0x0201de8b36819700c807083608e2000120ed4928c628d1c2c6eae90338905995612959273a5c63f93636c14614ac8737d1` | `sora5ｻﾜxﾀ7Vｱ7QFeｷMﾂLﾉﾃﾏﾓﾀTﾚgSav3Wnｱｵ4ｱCKｷﾛMﾘzヰHiﾐｱ6ﾃﾉﾁﾐZmﾇ2fiﾎX21P4L` |
| alpha | `0x04` | `0x020146be2154ae86826a3fef0ec0000120ca93ac1705187071d67b83c7ff0efe8108e8ec4530575d7726879333dbdabe7c` | `sora5ｻ9JヱﾈｿuwU6ｴpﾔﾂﾈRqRTds1HﾃﾐｶLVﾍｳ9ﾔhｾNｵVｷyucEﾒGﾈﾏﾍ9sKeﾉDzrｷﾆ742WG1` |
| oméga | `0x05` | `0x0201390d946885bc8416b3d30c9d0001206e7a1cdd29b0b78fd13af4c5598feff4ef2a97166e3ca6f2e4fbfccd80505bf1` | `sora5ｻ3zrﾌuﾚﾄJﾑXQhｸTyN8pzwRkWxmjVﾗbﾚﾕヰﾈoｽｦｶtEEﾊﾐ6GPｿﾓﾊｾEhvPｾｻ3XAJ73F` |
| gouvernance | `0x06` | `0x0201989eb45a80940d187e2c908f0001208a875fff1eb38451577acd5afee405456568dd7c89e090863a0557bc7af49f17` | `sora5ｻiｵﾁyVﾕｽbFpDHHuﾇﾉdﾗｲﾓﾄRﾋAW3frUCｾ5ｷﾘTwdﾚnｽtQiLﾏｼｶﾅXgｾZmﾒヱH58H4KP` |
| validateurs | `0x07` | `0x0201e4ffa58704c69afaeb7cc2d7000120ea4a6c63e29c520abef5507b132ec5f9954776aebebe7b92421eea691446d22c` | `sora5ｻﾀLDH6VYﾑNAｾgﾉVﾜtxﾊRXLｹﾍﾔﾌLd93GﾔGeｴﾄYrs1ﾂHｸkYxｹwｿyZﾗxyﾎZoXT1S4N` |
| explorateur | `0x08` | `0x02013b35422c65c2a83c99c523ad0001201398f62c6d1a457c51ba6a4b5f3dbd2f69fca93216218dc8997e416bd17d93ca` | `sora5ｻ4nmｻaﾚﾚPvNLgｿｱv6MHDeEyﾀovﾉJcpvrﾖ6ﾈCQcCNﾇﾜhﾚﾖyFdTwｸｶHEｱ9rWU8FMB` |
| soranet | `0x09` | `0x0201047d9ea7f5d5dbec3f7bfc58000120fd1724385aa0c75b64fb78cd602fa1d991fdebf76b13c58ed702eac835e9f618` | `sora5ｱｸヱVQﾂcﾁヱRﾓcApｲﾁﾅﾒvﾌﾏfｾNnﾛRJsｿDhﾙuHaﾚｺｦﾌﾍﾈeﾆﾎｺN1UUDｶ6ﾎﾄﾛoRH8JUL` |
| kitsune | `0x0A` | `0x0201e91933de397fd7723dc9a76c00012043a72e714401762df66b68c26dfbdf2682aaec9f2474eca4613e424a0fbafd3c` | `sora5ｻﾚｺヱkfFJfSﾁｼJwﾉLvbpSｷﾔMWFMrbｳｸｲｲyヰKGJﾉｻ4ｹﾕrｽhｺｽzSDヰXAN62AD7RGNS` |
| ça | `0x0B` | `0x02016838cf5bb0ce0f3d4f380e1c00012066be7e332c7a453332bd9d0a7f7db055f5c5ef1a06ada66d98b39fb6810c473a` | `sora5ｻNﾒ5SﾐRﾉﾐﾃ62ｿ1ｶｷWFKyF1BcAﾔvｼﾐHqﾙﾐPﾏｴヰ5tｲﾕvnﾙT6ﾀW7mﾔ7ﾇﾗﾂｳ25CXS93` |

Révisé par : Groupe de travail sur le modèle de données, Groupe de travail sur la cryptographie — portée approuvée pour ADDR-1a.

##### Alias ​​de référence Sora Nexus

Les réseaux Sora Nexus sont par défaut `chain_discriminant = 0x02F1`
(`iroha_config::parameters::defaults::common::CHAIN_DISCRIMINANT`). Le
Les helpers `AccountAddress::to_i105` et `to_i105` émettent donc
des formes textuelles cohérentes pour chaque charge utile canonique. Luminaires sélectionnés de
`fixtures/account/address_vectors.json` (généré via
`cargo xtask address-vectors`) sont présentés ci-dessous pour référence rapide :

| Compte / sélecteur | Littéral I105 (préfixe `0x02F1`) | Sora compressé (`sora`) littéral |
|------------------------|--------------------------------|-----------------------------|
| `default` domaine (sélecteur implicite, graine `0x00`) | `6cmzPVPX5jDQFNfiz6KgmVfm1fhoAqjPhoPFn4nx9mBWaFMyUCwq4cw` | `sorauﾛ1NﾗhBUd2BﾂｦﾄiﾔﾆﾂﾇKSﾃaﾘﾒﾓQﾗrﾒoﾘﾅnｳﾘbQｳQJﾆLJ5HSE` (suffixe `@default` facultatif lors de la fourniture d'indications de routage explicites) |
| `treasury` (sélecteur de résumé local, graine `0x01`) | `34mSYnCXkCzHXm31UDHh7SJfGvC4QPEhwim8z7sys2iHqXpCwCQkjL8KHvkFLSs1vZdJcb37r` | `sora5ｻu6rﾀCヰTGwﾏ1ﾅヱﾌQｲﾖﾇqCｦヰﾓZQCZRDSSﾅMｱﾙヱｹﾁｸ8ｾeﾄﾛ6C8bZuwﾗｹCZｦRSLQFU` |
| Pointeur de registre global (`registry_id = 0x0000_002A`, équivalent à `treasury`) | `3oE9sLeRGP49Cu7mQ1nF4wtKAm29BG4TGLiRsaXe7mhbMP5WZ113nNW1N6RbqF` | `sorakXｹ6NｻﾍﾀﾖSﾜﾖｱ3ﾚ5WﾘﾋQﾅｷｦxgﾛｸcﾁｵﾋkﾋvﾏ8SPﾓﾀｹdｴｴｲW9iCM6AEP` |

Ces chaînes correspondent à celles émises par la CLI (`iroha tools address convert`), Torii
réponses (`canonical I105 literal rendering`) et assistants SDK, donc copier/coller UX
les flux peuvent s’appuyer sur eux textuellement. Ajoutez `<address>@<domain>` (rejected legacy form) uniquement lorsque vous avez besoin d'un indice de routage explicite ; le suffixe ne fait pas partie de la sortie canonique.

#### 2.6 Alias textuels pour l'interopérabilité (prévu)

- **Style d'alias de chaîne :** `ih:<chain-alias>:<alias@domain>` pour les journaux et les humains
  entrée. Les portefeuilles doivent analyser le préfixe, vérifier la chaîne intégrée et bloquer
  inadéquations.
- **Formulaire CAIP-10 :** `iroha:<caip-2-id>:<i105-addr>` pour les chaînes indépendantes
  intégrations. Ce mappage n'est **pas encore implémenté** dans la version livrée
  chaînes d'outils.
- **Aide machine :** Publiez des codecs pour Rust, TypeScript/JavaScript, Python,
  et Kotlin couvrant I105 et les formats compressés (`AccountAddress::to_i105`,
  `AccountAddress::parse_encoded` et leurs équivalents SDK). Les assistants CAIP-10 sont
  travaux futurs.

#### 2.7 Alias ​​déterministe I105

- **Mappage de préfixe :** Réutilisez le `chain_discriminant` comme préfixe réseau I105.
  `encode_i105_prefix()` (voir `crates/iroha_data_model/src/account/address.rs`)
  émet un préfixe de 6 bits (un seul octet) pour les valeurs `<64` et un préfixe de 14 bits sur deux octets
  forme pour les réseaux plus grands. Les missions faisant autorité vivent dans
  [`address_prefix_registry.md`](source/references/address_prefix_registry.md) ;
  Les SDK DOIVENT maintenir le registre JSON correspondant synchronisé pour éviter les collisions.
- **Matériel du compte :** I105 encode la charge utile canonique créée par
  `AccountAddress::canonical_bytes()` : octet d'en-tête, sélecteur de domaine et
  charge utile du contrôleur. Il n’y a aucune étape de hachage supplémentaire ; I105 intègre le
  charge utile du contrôleur binaire (clé unique ou multisig) telle que produite par Rust
  encodeur, pas la carte CTAP2 utilisée pour les résumés de politique multisig.
- **Encodage :** `encode_i105()` concatène les octets du préfixe avec le canonique
  charge utile et ajoute une somme de contrôle de 16 bits dérivée de Blake2b-512 avec le fixe
  préfixe `I105PRE` (`b"I105PRE" || prefix || payload`). Le résultat est codé en Base58 via `bs58`.
  Les assistants CLI/SDK exposent la même procédure et `AccountAddress::parse_encoded`
  l'inverse via `decode_i105`.

#### 2.8 Vecteurs de tests textuels normatifs

`fixtures/account/address_vectors.json` contient un I105 complet (de préférence) et compressé (`sora`, deuxième meilleur)
des littéraux pour chaque charge utile canonique. Points forts :

- **`addr-single-default-ed25519` (Sora Nexus, préfixe `0x02F1`).**  
  I105 `6cmzPVPX5jDQFNfiz6KgmVfm1fhoAqjPhoPFn4nx9mBWaFMyUCwq4cw`, compressé (`sora`)
  `sora2QG…U4N5E5`. Torii émet ces chaînes exactes à partir de `AccountId`
  `Display` implémentation (canonique I105) et `AccountAddress::to_i105`.
- **`addr-global-registry-002a` (sélecteur de registre → trésorerie).**  
  I105 `3oE9sLeRGP49Cu7mQ1nF4wtKAm29BG4TGLiRsaXe7mhbMP5WZ113nNW1N6RbqF`, compressé (`sora`)
  `sorakX…CM6AEP`. Démontre que les sélecteurs de registre décodent toujours en
  la même charge utile canonique que le résumé local correspondant.
- **Cas d'échec (`i105-prefix-mismatch`).**  
  Analyse d'un littéral I105 codé avec le préfixe `NETWORK_PREFIX + 1` sur un nœud
  s'attendre à ce que le préfixe par défaut donne
  `AccountAddressError::UnexpectedNetworkPrefix { expected: 753, found: 754 }`
  avant que le routage de domaine ne soit tenté. Le luminaire `i105-checksum-mismatch`
  exerce une détection de falsification sur la somme de contrôle Blake2b.

#### 2.9 Calendriers de conformité

ADDR‑2 est livré avec un pack de luminaires rejouables couvrant les aspects positifs et négatifs
scénarios sur l'hexagone canonique, I105 (préféré), compressé (`sora`, demi-/pleine largeur), implicite
sélecteurs par défaut, alias de registre global et contrôleurs multisignatures. Le
Le JSON canonique réside dans `fixtures/account/address_vectors.json` et peut être
régénéré avec :

```
cargo xtask address-vectors --out fixtures/account/address_vectors.json
# verify without writing:
cargo xtask address-vectors --verify
```

Pour les expériences ad hoc (différents chemins/formats), l'exemple de binaire est toujours
disponible :

```
cargo run -p iroha_data_model --example account_address_vectors > fixtures/account/address_vectors.json
```

Tests unitaires Rust dans `crates/iroha_data_model/tests/account_address_vectors.rs`
et `crates/iroha_torii/tests/account_address_vectors.rs`, avec le JS,
Swift et Android exploitent (`javascript/iroha_js/test/address.test.js`,
`IrohaSwift/Tests/IrohaSwiftTests/AccountAddressTests.swift`,
`java/iroha_android/src/test/java/org/hyperledger/iroha/android/address/AccountAddressTests.java`),
consommez le même appareil pour garantir la parité des codecs entre les SDK et l'admission Torii.

### 3. Domaines et normalisation uniques au monde

Voir aussi : [`docs/source/references/address_norm_v1.md`](source/references/address_norm_v1.md)
pour le pipeline canonique Norm v1 utilisé dans Torii, le modèle de données et les SDK.

Redéfinissez `DomainId` en tant que tuple balisé :

```
DomainId {
    name: Name,
    authority: GlobalDomainAuthority, // new enum
}

enum GlobalDomainAuthority {
    LocalChain,                  // default for the local chain
    External { chain_discriminant: u16 },
}
```

`LocalChain` encapsule le nom existant pour les domaines gérés par la chaîne actuelle.
Lorsqu'un domaine est enregistré via le registre mondial, nous conservons la propriété
discriminant de la chaîne. L'affichage/analyse reste inchangé pour l'instant, mais le
la structure élargie permet des décisions de routage.

#### 3.1 Normalisation et défenses contre l'usurpation d'identité

La norme v1 définit le pipeline canonique que chaque composant doit utiliser avant un domaine.
le nom est conservé ou intégré dans un `AccountAddress`. La procédure complète
vit à [`docs/source/references/address_norm_v1.md`](source/references/address_norm_v1.md) ;
le résumé ci-dessous capture les étapes que les portefeuilles, Torii, les SDK et la gouvernance
les outils doivent mettre en œuvre.

1. **Validation d'entrée.** Rejetez les chaînes vides, les espaces et les espaces réservés
   délimiteurs `@`, `#`, `$`. Cela correspond aux invariants imposés par
   `Name::validate_str`.
2. **Composition Unicode NFC.** Appliquer la normalisation NFC soutenue par ICU de manière canonique
   les séquences équivalentes s'effondrent de manière déterministe (par exemple, `e\u{0301}` → `é`).
3. **Normalisation UTS-46.** Exécutez la sortie NFC via UTS‑46 avec
   `use_std3_ascii_rules = true`, `transitional_processing = false` et
   Application de la longueur DNS activée. Le résultat est une séquence d’étiquette A minuscule ;
   les entrées qui violent les règles STD3 échouent ici.
4. **Limites de longueur.** Appliquer les limites de style DNS : chaque étiquette DOIT être comprise entre 1 et 63.
    octets et le domaine complet NE DOIT PAS dépasser 255 octets après l'étape 3.
5. **Politique facultative pouvant être confondue.** Les vérifications de script UTS‑39 sont suivies pour
   Norme v2 ; les opérateurs peuvent les activer plus tôt, mais en cas d'échec du contrôle, ils doivent abandonner
   traitement.

Si chaque étape réussit, la chaîne d'étiquette A minuscule est mise en cache et utilisée pour
codage d'adresses, configuration, manifestes et recherches dans le registre. Résumé local
les sélecteurs dérivent leur valeur de 12 octets sous la forme `blake2s_mac(key = "SORA-LOCAL-K:v1",
canonical_label)[0..12]` à l'aide du résultat de l'étape 3. Toutes les autres tentatives (mixtes
casse, majuscules, entrée Unicode brute) sont rejetés avec des
`ParseError`s à la limite où le nom a été fourni.

Appareils canoniques démontrant ces règles - y compris les allers-retours en punycode
et les séquences STD3 invalides - sont répertoriées dans
`docs/source/references/address_norm_v1.md` et sont reflétés dans le SDK CI
suites vectorielles suivies sous ADDR‑2.

### 4. Registre et routage de domaines Nexus

- **Schéma de registre :** Nexus gère une carte signée `DomainName -> ChainRecord`
  où `ChainRecord` inclut les métadonnées facultatives discriminantes en chaîne (RPC
  points de terminaison) et une preuve d’autorité (par exemple, gouvernance multi-signature).
- **Mécanisme de synchronisation :**
  - Les chaînes soumettent des revendications de domaine signées à Nexus (soit pendant la genèse, soit via
    instruction de gouvernance).
  - Nexus publie des manifestes périodiques (JSON signé plus racine Merkle facultative)
    via HTTPS et le stockage adressé par contenu (par exemple, IPFS). Les clients épinglent le
    dernier manifeste et vérifier les signatures.
- **Flux de recherche :**
  - Torii reçoit une transaction référençant `DomainId`.
  - Si le domaine est inconnu localement, Torii interroge le manifeste Nexus mis en cache.
  - Si le manifeste indique une chaîne étrangère, la transaction est rejetée avec
    une erreur déterministe `ForeignDomain` et les informations sur la chaîne distante.
  - Si le domaine est absent de Nexus, Torii renvoie `UnknownDomain`.
- **Ancres de confiance et rotation :** Les clés de gouvernance signent les manifestes ; rotation ou
  la révocation est publiée en tant que nouvelle entrée du manifeste. Les clients appliquent le manifeste
  TTL (par exemple, 24h) et refusez de consulter les données obsolètes au-delà de cette fenêtre.
- **Modes d'échec :** Si la récupération du manifeste échoue, Torii revient en cache
  données dans TTL ; passé TTL, il émet `RegistryUnavailable` et refuse
  routage inter-domaines pour éviter les états incohérents.

### 4.1 Immuabilité du registre, alias et pierres tombales (ADDR-7c)

Nexus publie un **manifeste en ajout uniquement** afin que chaque attribution de domaine ou d'alias
peut être audité et rejoué. Les opérateurs doivent traiter le lot décrit dans le
[runbook du manifeste d'adresse](source/runbooks/address_manifest_ops.md) comme
seule source de vérité : si un manifeste est manquant ou échoue à la validation, Torii doit
refuser de résoudre le domaine concerné.

Prise en charge de l'automatisation : `cargo xtask address-manifest verify --bundle <current_dir> --previous <previous_dir>`
rejoue la somme de contrôle, le schéma et les vérifications du résumé précédent énoncés dans le
runbook. Incluez le résultat de la commande dans les tickets de modification pour afficher le `sequence`
et la liaison `previous_digest` a été validée avant de publier le bundle.

#### En-tête du manifeste et contrat de signature

| Champ | Exigence |
|-------|-------------|
| `version` | Actuellement `1`. Bump uniquement avec une mise à jour des spécifications correspondante. |
| `sequence` | Incrémentez de **exactement** un par publication. Les caches Torii refusent les révisions comportant des lacunes ou des régressions. |
| `generated_ms` + `ttl_hours` | Établir la fraîcheur du cache (24 h par défaut). Si la durée de vie expire avant la prochaine publication, Torii passe à `RegistryUnavailable`. |
| `previous_digest` | BLAKE3 digest (hex) du corps manifeste précédent. Les vérificateurs le recalculent avec `b3sum` pour prouver l'immuabilité. |
| `signatures` | Les manifestes sont signés via Sigstore (`cosign sign-blob`). Les opérateurs doivent exécuter `cosign verify-blob --bundle manifest.sigstore manifest.json` et appliquer les contraintes d'identité de gouvernance/d'émetteur avant le déploiement. |

L'automatisation de la version émet `manifest.sigstore` et `checksums.sha256`
aux côtés du corps JSON. Conservez les fichiers ensemble lors de la mise en miroir sur SoraFS ou
Points de terminaison HTTP afin que les auditeurs puissent rejouer textuellement les étapes de vérification.

#### Types d'entrées

| Tapez | Objectif | Champs obligatoires |
|------|---------|-----------------|
| `global_domain` | Déclare qu'un domaine est enregistré globalement et doit correspondre à un discriminant de chaîne et à un préfixe I105. | `{ "domain": "<label>", "chain": "sora:nexus:global", "i105_prefix": 753, "selector": "global" }` |
| `tombstone` | Supprime définitivement un alias/sélecteur. Requis lors de l’effacement des résumés Local‑8 ou de la suppression d’un domaine. | `{ "selector": {…}, "reason_code": "LOCAL8_RETIREMENT" \| …, "ticket": "<governance id>", "replaces_sequence": <number> }` |

Les entrées `global_domain` peuvent éventuellement inclure un `manifest_url` ou un `sorafs_cid`
pour pointer les portefeuilles vers les métadonnées de la chaîne signée, mais le tuple canonique reste
`{domain, chain, discriminant/i105_prefix}`. `tombstone` enregistrements **doivent** citer
le sélecteur étant retiré et l'artefact de ticket/gouvernance qui a autorisé
le changement afin que la piste d'audit soit reconstructible hors ligne.

#### Flux de travail et télémétrie d'alias/tombstone

1. **Détecter la dérive.** Utilisez `torii_address_local8_total{endpoint}`,
   `torii_address_local8_domain_total{endpoint,domain}`,
   `torii_address_collision_total{endpoint,kind="local12_digest"}`,
   `torii_address_collision_domain_total{endpoint,domain}`,
   `torii_address_domain_total{endpoint,domain_kind}`, et
   `torii_address_invalid_total{endpoint,reason}` (rendu en
   `dashboards/grafana/address_ingest.json`) pour confirmer les soumissions locales et
   Les collisions locales-12 restent à zéro avant de proposer une pierre tombale. Le
   les compteurs par domaine permettent aux propriétaires de prouver que seuls les domaines de développement/test émettent du Local‑8
   trafic (et que les collisions Local‑12 correspondent à des domaines de transit connus) tandis que
   inclut le panneau **Domain Kind Mix (5m)** afin que les SRE puissent représenter graphiquement la quantité
   Le trafic `domain_kind="local12"` demeure et le `AddressLocal12Traffic`
   l'alerte se déclenche chaque fois que la production voit toujours des sélecteurs Local-12 malgré le
   porte de retraite.
2. **Dériver des résumés canoniques.** Exécuter
   `iroha tools address convert <address> --format json --expect-prefix 753`
   (ou consommez `fixtures/account/address_vectors.json` via
   `scripts/account_fixture_helper.py`) pour capturer le `digest_hex` exact.
   La CLI accepte les littéraux I105, `i105` et canoniques `0x…` ; ajouter
   `@<domain>` uniquement lorsque vous devez conserver une étiquette pour les manifestes.
   Le résumé JSON fait apparaître ce domaine via le champ `input_domain`, et
   `legacy  suffix` relit l'encodage converti sous la forme `<address>@<domain>` (rejected legacy form) pour
   différences manifestes (ce suffixe est une métadonnée, pas un identifiant de compte canonique).
   Pour les exportations orientées vers une nouvelle ligne, utilisez
   `iroha tools address normalize --input <file> legacy-selector input mode` pour convertir en masse Local
   sélecteurs en formats canoniques I105 (de préférence), compressés (`sora`, deuxième meilleur), hexadécimaux ou JSON tout en sautant
   lignes non locales. Lorsque les auditeurs ont besoin de preuves sous forme de tableur, exécutez
   `iroha tools address audit --input <file> --format csv` pour émettre un résumé CSV
   (`input,status,format,domain_kind,…`) qui met en avant les sélecteurs locaux,
   encodages canoniques et analyser les échecs dans le même fichier.
3. **Ajouter les entrées du manifeste.** Rédigez l'enregistrement `tombstone` (et le suivi
   `global_domain` enregistrement lors de la migration vers le registre global) et validez
   le manifeste avec `cargo xtask address-vectors` avant de demander des signatures.
4. **Vérifier et publier.** Suivez la liste de contrôle du runbook (hachages, Sigstore,
   monotonie de la séquence) avant de mettre en miroir le bundle sur SoraFS. Torii maintenant
   canonise les littéraux I105 (préféré)/sora (deuxième meilleur) immédiatement après l'atterrissage du bundle.
5. **Surveiller et restaurer.** Gardez les panneaux de collision Local‑8 et Local‑12 à
   zéro pendant 30 jours ; si des régressions apparaissent, republier le manifeste précédent
   uniquement dans l'environnement de non-production concerné jusqu'à ce que la télémétrie se stabilise.

Toutes les étapes ci-dessus constituent des preuves obligatoires pour l’ADDR‑7c : manifeste sans
le paquet de signatures `cosign` ou sans correspondance avec les valeurs `previous_digest` doivent
être rejeté automatiquement, et les opérateurs doivent joindre les journaux de vérification à
leurs billets de change.

### 5. Ergonomie du Wallet & des API

- **Affichage par défaut :** Les portefeuilles affichent l'adresse I105 (courte, avec somme de contrôle)
  plus le domaine résolu sous forme d'étiquette extraite du registre. Les domaines sont
  clairement marqué comme métadonnées descriptives susceptibles de changer, tandis que I105 est le
  adresse stable.
- **Canonique d'entrée :** Torii et les SDK acceptent I105 (préféré)/sora (deuxième meilleur)/0x
  adresses plus `alias@domain` (rejected legacy form), `uaid:…` et
  `opaque:…` formulaires, puis canonisez-les en I105 pour la sortie. Il n'y a pas
  bascule en mode strict ; les identifiants bruts de téléphone/e-mail doivent être conservés hors grand livre
  via des mappages UAID/opaques.
- **Prévention des erreurs :** Les portefeuilles analysent les préfixes I105 et appliquent la discrimination en chaîne
  attentes. Les incompatibilités de chaîne déclenchent des pannes matérielles avec des diagnostics exploitables.
- **Bibliothèques de codecs :** Rust officiel, TypeScript/JavaScript, Python et Kotlin
  les bibliothèques fournissent un encodage/décodage I105 ainsi qu'un support compressé (`sora`) pour
  éviter les implémentations fragmentées. Les conversions CAIP-10 ne sont pas encore expédiées.

#### Conseils sur l'accessibilité et le partage sécurisé

- Les conseils de mise en œuvre pour les surfaces de produits sont suivis en direct
  `docs/portal/docs/reference/address-safety.md`; faire référence à cette liste de contrôle lorsque
  adapter ces exigences au portefeuille ou à l'explorateur UX.
- **Flux de partage sécurisé :** Les surfaces qui copient ou affichent des adresses utilisent par défaut le formulaire I105 et exposent une action « partager » adjacente qui présente à la fois la chaîne complète et un code QR dérivé de la même charge utile afin que les utilisateurs puissent vérifier la somme de contrôle visuellement ou par numérisation. Lorsque la troncature est inévitable (par exemple, petits écrans), conservez le début et la fin de la chaîne, ajoutez des points de suspension clairs et gardez l'adresse complète accessible via la copie dans le presse-papiers pour éviter tout écrêtage accidentel.
- **Garanties IME :** Les entrées d'adresse DOIVENT rejeter les artefacts de composition des claviers de style IME/IME. Appliquez l'entrée ASCII uniquement, présentez un avertissement en ligne lorsque des caractères pleine chasse ou Kana sont détectés et proposez une zone de collage de texte brut qui supprime les marques combinées avant la validation afin que les utilisateurs japonais et chinois puissent désactiver leur IME sans perdre leur progression.
- **Prise en charge des lecteurs d'écran :** fournissez des étiquettes visuellement masquées (`aria-label`/`aria-describedby`) qui décrivent les principaux chiffres du préfixe Base58 et divisent la charge utile I105 en groupes de 4 ou 8 caractères, afin que la technologie d'assistance lise les caractères groupés au lieu d'une chaîne d'exécution. Annoncez le succès de la copie/partage via des régions en direct polies et assurez-vous que les aperçus QR incluent un texte alternatif descriptif (« adresse I105 pour <alias> sur la chaîne 0x02F1 »).
- **Utilisation compressée pour Sora uniquement :** Étiquetez toujours la vue compressée `i105` comme « Sora uniquement » et placez-la derrière une confirmation explicite avant de la copier. Les SDK et les portefeuilles doivent refuser d'afficher une sortie compressée lorsque le discriminant de chaîne n'est pas la valeur de Sora Nexus et doivent rediriger les utilisateurs vers I105 pour les transferts inter-réseaux afin d'éviter un acheminement erroné des fonds.

## Liste de contrôle de mise en œuvre

- **Enveloppe I105 :** Le préfixe encode le `chain_discriminant` en utilisant le compact
  Schéma 6/14 bits de `encode_i105_prefix()`, le corps est constitué des octets canoniques
  (`AccountAddress::canonical_bytes()`), et la somme de contrôle correspond aux deux premiers octets
  de Blake2b-512 (`b"I105PRE"` || préfixe || corps). La charge utile complète est Base58-
  encodé via `bs58`.
- **Contrat de registre :** Publication JSON signée (et racine Merkle en option)
  `{discriminant, i105_prefix, chain_alias, endpoints}` avec 24h TTL et
  touches de rotation.
- **Politique de domaine :** ASCII `Name` aujourd'hui ; si vous activez i18n, appliquez UTS-46 pour
  normalisation et UTS-39 pour les contrôles confus. Appliquer l'étiquette max (63) et
  longueurs totales (255).
- **Aide textuelle :** Expédiez les codecs I105 ↔ compressés (`i105`) dans Rust,
  TypeScript/JavaScript, Python et Kotlin avec vecteurs de test partagés (CAIP-10
  les cartographies restent des travaux futurs).
- **Outils CLI :** Fournissez un flux de travail déterministe pour l'opérateur via `iroha tools address convert`
  (voir `crates/iroha_cli/src/address.rs`), qui accepte les littéraux I105/`0x…` et
  étiquettes facultatives `<address>@<domain>` (rejected legacy form), par défaut la sortie I105 en utilisant le préfixe Sora Nexus (`753`),
  et n'émet l'alphabet compressé Sora uniquement que lorsque les opérateurs le demandent explicitement avec
  `--format i105` ou le mode résumé JSON. La commande applique les attentes de préfixe sur
  analyser, enregistre le domaine fourni (`input_domain` en JSON) et l'indicateur `legacy  suffix`
  relit l'encodage converti sous la forme `<address>@<domain>` (rejected legacy form) afin que les différences manifestes restent ergonomiques.
- **Wallet/explorer UX :** Suivez les [consignes d'affichage de l'adresse](source/sns/address_display_guidelines.md)
  livré avec ADDR-6 : offre des boutons de double copie, conserve I105 comme charge utile QR et avertit
  aux utilisateurs que le formulaire compressé `i105` est uniquement Sora et sensible aux réécritures IME.
- **Intégration Torii :** Cache Nexus se manifeste en respectant le TTL, émet
  `ForeignDomain`/`UnknownDomain`/`RegistryUnavailable` de manière déterministe, et
  keep strict account-literal parsing canonical-I105-only (reject compressed and any `@domain` suffix) with canonical I105 output.

### Formats de réponse Torii

- `GET /v1/accounts` accepte un paramètre de requête `canonical I105 rendering` facultatif et
  `POST /v1/accounts/query` accepte le même champ à l'intérieur de l'enveloppe JSON.
  Les valeurs prises en charge sont :
  - `i105` (par défaut) — les réponses émettent des charges utiles I105 Base58 canoniques (par exemple,
    `6cmzPVPX5jDQFNfiz6KgmVfm1fhoAqjPhoPFn4nx9mBWaFMyUCwq4cw`).
  - `i105_default` — les réponses émettent la vue compressée `i105` Sora uniquement pendant
    garder les paramètres de filtres/chemin canoniques.
- Les valeurs non valides renvoient `400` (`QueryExecutionFail::Conversion`). Cela permet
  les portefeuilles et les explorateurs pour demander des chaînes compressées pour l'UX Sora uniquement tout en
  en gardant I105 comme valeur par défaut interopérable.
- Listes des détenteurs d'actifs (`GET /v1/assets/{definition_id}/holders`) et leur JSON
  la contrepartie de l'enveloppe (`POST …/holders/query`) honore également `canonical I105 rendering`.
  Le champ `items[*].account_id` émet des littéraux compressés chaque fois que le
  Le champ paramètre/enveloppe est défini sur `i105_default`, reflétant les comptes
  points de terminaison afin que les explorateurs puissent présenter une sortie cohérente dans tous les répertoires.
- **Test :** Ajout de tests unitaires pour les allers-retours encodeur/décodeur, mauvaise chaîne
  échecs et recherches manifestes ; ajouter une couverture d'intégration dans Torii et les SDK
  pour I105, les flux sont de bout en bout.

## Registre des codes d'erreur

Les encodeurs et décodeurs d'adresses exposent les échecs via
`AccountAddressError::code_str()`. Les tableaux suivants fournissent les codes stables
que les SDK, les portefeuilles et les surfaces Torii devraient apparaître aux côtés de lisibles par l'homme
messages, ainsi que des conseils de résolution recommandés.

### Construction canonique

| Codes | Échec | Correction recommandée |
|------|---------|------------------------------|
| `ERR_UNSUPPORTED_ALGORITHM` | L'encodeur a reçu un algorithme de signature non pris en charge par les fonctionnalités de registre ou de build. | Limitez la construction de compte aux courbes activées dans le registre et la configuration. |
| `ERR_KEY_PAYLOAD_TOO_LONG` | La longueur de la charge utile de la clé de signature dépasse la limite prise en charge. | Les contrôleurs à clé unique sont limités aux longueurs `u8` ; utilisez le multisig pour les grandes clés publiques (par exemple, ML‑DSA). |
| `ERR_INVALID_HEADER_VERSION` | La version de l’en-tête d’adresse est en dehors de la plage prise en charge. | Émettre la version d'en-tête `0` pour les adresses V1 ; mettre à niveau les encodeurs avant d’adopter de nouvelles versions. |
| `ERR_INVALID_NORM_VERSION` | L'indicateur de version de normalisation n'est pas reconnu. | Utilisez la version de normalisation `1` et évitez de basculer les bits réservés. |
| `ERR_INVALID_I105_PREFIX` | Le préfixe réseau I105 demandé ne peut pas être codé. | Choisissez un préfixe dans la plage inclusive `0..=16383` publiée dans le registre de la chaîne. |
| `ERR_CANONICAL_HASH_FAILURE` | Le hachage canonique de la charge utile a échoué. | Réessayez l'opération ; si l'erreur persiste, traitez-la comme un bug interne dans la pile de hachage. |

### Décodage de format et détection automatique

| Codes | Échec | Correction recommandée |
|------|---------|------------------------------|
| `ERR_INVALID_I105_ENCODING` | La chaîne I105 contient des caractères en dehors de l'alphabet. | Assurez-vous que l'adresse utilise l'alphabet I105 publié et n'a pas été tronquée lors du copier/coller. |
| `ERR_INVALID_LENGTH` | La longueur de la charge utile ne correspond pas à la taille canonique attendue pour le sélecteur/contrôleur. | Fournissez la charge utile canonique complète pour le sélecteur de domaine et la disposition du contrôleur sélectionnés. |
| `ERR_CHECKSUM_MISMATCH` | La validation de la somme de contrôle I105 (de préférence) ou compressée (`sora`, deuxième meilleur) a échoué. | Régénérez l'adresse à partir d'une source fiable ; cela indique généralement une erreur de copier/coller. |
| `ERR_INVALID_I105_PREFIX_ENCODING` | Les octets du préfixe I105 sont mal formés. | Ré-encoder l'adresse avec un encodeur conforme ; ne modifiez pas manuellement les principaux octets Base58. |
| `ERR_INVALID_HEX_ADDRESS` | La forme hexadécimale canonique n'a pas pu être décodée. | Fournissez une chaîne hexadécimale de longueur paire avec préfixe `0x` et produite par l'encodeur officiel. |
| `ERR_MISSING_COMPRESSED_SENTINEL` | Le formulaire compressé ne commence pas par `sora`. | Préfixez les adresses Sora compressées avec la sentinelle requise avant de les transmettre aux décodeurs. |
| `ERR_COMPRESSED_TOO_SHORT` | La chaîne compressée ne contient pas suffisamment de chiffres pour la charge utile et la somme de contrôle. | Utilisez la chaîne compressée complète émise par l'encodeur au lieu d'extraits tronqués. |
| `ERR_INVALID_COMPRESSED_CHAR` | Caractère extérieur à l’alphabet compressé rencontré. | Remplacez le caractère par un glyphe Base‑105 valide issu des tableaux demi-largeur/pleine largeur publiés. |
| `ERR_INVALID_COMPRESSED_BASE` | L'encodeur a tenté d'utiliser une base non prise en charge. | Déposer un bug contre l'encodeur ; l'alphabet compressé est fixé à la base 105 dans la V1. |
| `ERR_INVALID_COMPRESSED_DIGIT` | La valeur du chiffre dépasse la taille de l'alphabet compressé. | Assurez-vous que chaque chiffre se trouve dans `0..105)`, en régénérant l'adresse si nécessaire. |
| `ERR_UNSUPPORTED_ADDRESS_FORMAT` | La détection automatique n'a pas pu reconnaître le format d'entrée. | Fournissez des chaînes hexadécimales I105 (de préférence), compressées (`sora`) ou canoniques `0x` lors de l'appel des analyseurs. |

### Validation de domaine et de réseau

| Codes | Échec | Correction recommandée |
|------|---------|------------------------------|
| `ERR_DOMAIN_MISMATCH` | Le sélecteur de domaine ne correspond pas au domaine attendu. | Utilisez une adresse émise pour le domaine prévu ou mettez à jour les attentes. |
| `ERR_INVALID_DOMAIN_LABEL` | Les vérifications de normalisation de l'étiquette de domaine ont échoué. | Canonisez le domaine à l'aide du traitement non transitionnel UTS-46 avant l'encodage. |
| `ERR_UNEXPECTED_NETWORK_PREFIX` | Le préfixe réseau I105 décodé diffère de la valeur configurée. | Basculez vers une adresse de la chaîne cible ou ajustez le discriminant/préfixe attendu. |
| `ERR_UNKNOWN_ADDRESS_CLASS` | Les bits de classe d'adresse ne sont pas reconnus. | Mettez à niveau le décodeur vers une version qui comprend la nouvelle classe ou évitez de falsifier les bits d'en-tête. |
| `ERR_UNKNOWN_DOMAIN_TAG` | La balise de sélection de domaine est inconnue. | Mettez à jour vers une version prenant en charge le nouveau type de sélecteur ou évitez d'utiliser des charges utiles expérimentales sur les nœuds V1. |
| `ERR_UNEXPECTED_EXTENSION_FLAG` | Le bit d'extension réservé a été activé. | Effacer les bits réservés ; ils restent fermés jusqu'à ce qu'un futur ABI les présente. |
| `ERR_UNKNOWN_CONTROLLER_TAG` | Balise de charge utile du contrôleur non reconnue. | Mettez à niveau le décodeur pour reconnaître les nouveaux types de contrôleurs avant de les analyser. |
| `ERR_UNEXPECTED_TRAILING_BYTES` | La charge utile canonique contenait des octets de fin après le décodage. | Régénérer la charge utile canonique ; seule la longueur documentée doit être présente. |

### Validation de la charge utile du contrôleur

| Codes | Échec | Correction recommandée |
|------|---------|------------------------------|
| `ERR_INVALID_PUBLIC_KEY` | Les octets clés ne correspondent pas à la courbe déclarée. | Assurez-vous que les octets clés sont codés exactement comme requis pour la courbe sélectionnée (par exemple, Ed25519 de 32 octets). |
| `ERR_UNKNOWN_CURVE` | L'identifiant de courbe n'est pas enregistré. | Utilisez l’ID de courbe `1` (Ed25519) jusqu’à ce que des courbes supplémentaires soient approuvées et publiées dans le registre. |
| `ERR_MULTISIG_MEMBER_OVERFLOW` | Le contrôleur Multisig déclare plus de membres que ce qui est pris en charge. | Réduisez l’adhésion multisig à la limite documentée avant l’encodage. |
| `ERR_INVALID_MULTISIG_POLICY` | La validation de la charge utile de la stratégie Multisig a échoué (seuil/poids/schéma). | Reconstruisez la stratégie afin qu'elle satisfasse au schéma CTAP2, aux limites de pondération et aux contraintes de seuil. |

## Alternatives envisagées

- **Pure Base58Check (style Bitcoin).** Somme de contrôle plus simple mais détection d'erreurs plus faible
  que la somme de contrôle I105 dérivée de Blake2b (`encode_i105` tronque un hachage de 512 bits)
  et manque de sémantique de préfixe explicite pour les discriminants 16 bits.
- **Intégration du nom de la chaîne dans la chaîne du domaine (par exemple, `finance@chain`).** Ruptures
- **Comptez uniquement sur le routage Nexus sans changer d'adresse.** Les utilisateurs continueraient
  copier/coller des chaînes ambiguës ; nous voulons que l'adresse elle-même porte le contexte.
- **Enveloppe Bech32m.** Compatible QR et offre un préfixe lisible par l'homme, mais
  s'écarterait de l'implémentation I105 d'expédition (`AccountAddress::to_i105`)
  et nécessitent de recréer tous les appareils/SDK. La feuille de route actuelle conserve I105 +
  support compressé (`sora`) tout en poursuivant la recherche sur l'avenir
  Couches Bech32m/QR (le mappage CAIP-10 est différé).

## Questions ouvertes

- Confirmer que les `u16` discriminants plus les plages réservées couvrent la demande à long terme ;
  sinon, évaluez `u32` avec un encodage varint.
- Finaliser le processus de gouvernance multi-signature pour les mises à jour du registre et comment
  les révocations/allocations expirées sont traitées.
- Définir le schéma exact de signature du manifeste (par exemple, Ed25519 multi-sig) et
  sécurité du transport (épinglage HTTPS, format de hachage IPFS) pour la distribution Nexus.
- Déterminer s'il faut prendre en charge les alias/redirections de domaine pour les migrations et comment
  les faire ressortir sans rompre avec le déterminisme.
- Spécifiez comment les contrats Kotodama/IVM accèdent aux assistants I105 (`to_address()`,
  `parse_address()`) et si le stockage en chaîne doit un jour exposer CAIP-10
  mappages (aujourd’hui I105 est canonique).
- Explorer l'enregistrement des chaînes Iroha dans des registres externes (par exemple, registre I105,
  Répertoire d’espaces de noms CAIP) pour un alignement plus large de l’écosystème.

## Prochaines étapes

1. L'encodage I105 a atterri dans `iroha_data_model` (`AccountAddress::to_i105`,
   `parse_encoded`); continuez à porter les appareils/tests sur chaque SDK et purgez tout
   Espaces réservés Bech32m.
2. Étendez le schéma de configuration avec `chain_discriminant` et dérivez-le de manière raisonnable
  valeurs par défaut pour les configurations de test/développement existantes. **(Fait : `common.chain_discriminant`
  est désormais livré en `iroha_config`, par défaut `0x02F1` avec par réseau
  remplacements.)**
3. Rédigez le schéma de registre Nexus et l'éditeur du manifeste de validation de principe.
4. Recueillir les commentaires des fournisseurs de portefeuilles et des dépositaires sur les aspects humains
   (Nom HRP, formatage de l'affichage).
5. Mettez à jour la documentation (`docs/source/data_model.md`, documentation de l'API Torii) une fois le
   le chemin de mise en œuvre est engagé.
6. Expédiez les bibliothèques de codecs officielles (Rust/TS/Python/Kotlin) avec test normatif
   vecteurs couvrant les cas de réussite et d’échec.
