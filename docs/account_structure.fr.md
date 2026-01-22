---
lang: fr
direction: ltr
source: docs/account_structure.md
status: complete
translator: manual
source_hash: 7e6a1321c6f8d71ac4b576a55146767fbc488b29c7e21d82bc2e1c55db89769c
source_last_modified: "2025-11-12T00:36:40.117854+00:00"
translation_last_reviewed: 2025-11-14
---

<!-- Traduction française de docs/account_structure.md (Account Structure RFC) -->

# RFC sur la Structure des Comptes

**Statut :** Accepté (ADDR‑1)  
**Public :** Équipes Modèle de données, Torii, Nexus, Wallet, Gouvernance  
**Tickets liés :** À définir

## Résumé

Ce document propose une refonte complète du schéma d’adressage des comptes afin
de fournir :

- Une adresse **Iroha Bech32m (IH‑B32)** lisible par les humains et protégée
  par checksum, qui lie un discriminant de chaîne au signataire du compte et
  offre des formes textuelles déterministes adaptées à l’interopérabilité.
- Des identifiants de domaine globalement uniques, appuyés par un registre
  interrogeable via Nexus pour le routage cross‑chain.
- Des couches de transition qui maintiennent les alias `alias@domain`
  fonctionnels pendant que nous migrons wallets, APIs et contrats vers le
  nouveau format.

## Motivation

Aujourd’hui, les wallets et les outils off‑chain s’appuient sur des alias de
routage bruts de la forme `alias@domain`. Cela pose deux problèmes majeurs :

1. **Aucun ancrage réseau.** La chaîne ne contient ni checksum ni préfixe de
   réseau, de sorte qu’un utilisateur peut coller une adresse provenant d’un
   autre réseau sans feedback immédiat. La transaction finira par être rejetée
   (écart de `chain_id`) ou, pire, pourrait réussir contre un compte
   inattendu si la destination existe localement.
2. **Collision de domaines.** Les domaines sont de simples espaces de noms et
   peuvent être réutilisés sur chaque chaîne. La fédération de services
   (dépositaires, bridges, workflows cross‑chain) devient fragile, car
   `finance` sur la chaîne A n’a aucun lien avec `finance` sur la chaîne B.

Nous avons besoin d’un format d’adresse lisible par les humains, qui protège
contre les erreurs de copier/coller, ainsi que d’un mapping déterministe entre
nom de domaine et chaîne autoritative.

## Objectifs

- Concevoir une enveloppe d’adresse Bech32m inspirée d’IH58 pour les comptes
  Iroha, tout en publiant des alias textuels canoniques CAIP‑10.
- Encoder le discriminant de chaîne configuré directement dans chaque adresse
  et définir son processus de gouvernance/registre.
- Expliquer comment introduire un registre global de domaines sans casser les
  déploiements existants et spécifier des règles de normalisation/anti‑spoofing.
- Documenter les attentes opérationnelles, les étapes de migration et les
  questions ouvertes.

## Hors périmètre

- Mettre en œuvre des transferts d’actifs cross‑chain. La couche de routage
  se limite à renvoyer la chaîne cible.
- Modifier la structure interne de `AccountId` (reste
  `DomainId + PublicKey`).
- Finaliser la gouvernance liée à l’émission de domaines globaux. Ce RFC se
  concentre sur le modèle de données et les primitives de transport.

## Contexte

### Alias de routage actuel

```text
AccountId {
    domain: DomainId,   // wrapper autour de Name (chaîne proche de l’ASCII)
    signatory: PublicKey // chaîne multihash (par ex. ed0120...)
}

Affichage / Parse : "<signatory multihash>@<domain name>"

Cette forme textuelle est désormais considérée comme un **alias de compte** :
une commodité de routage qui pointe vers l’[`AccountAddress`](#2-canonical-address-codecs)
canonique. Elle reste utile pour la lisibilité humaine et la gouvernance
scopée au domaine, mais n’est plus l’identifiant on‑chain autoritatif du
compte.
```

`ChainId` vit en dehors de `AccountId`. Les nœuds comparent le `ChainId` de la
transaction avec la configuration lors de l’admission
(`AcceptTransactionFail::ChainIdMismatch`) et rejettent les transactions
provenant de chaînes étrangères, mais la chaîne de caractères du compte ne
porte aucun indice de réseau.

### Identifiants de domaine

`DomainId` encapsule un `Name` (chaîne normalisée) et est limité à la chaîne
locale. Chaque chaîne peut enregistrer `wonderland`, `finance`, etc. de manière
indépendante.

### Contexte Nexus

Nexus est responsable de la coordination cross‑component (lanes/data‑spaces).
À l’heure actuelle, il n’a aucune notion de routage de domaines entre chaînes.

## Conception proposée

### 1. Discriminant de chaîne déterministe

On étend `iroha_config::parameters::actual::Common` avec :

```rust
pub struct Common {
    pub chain: ChainId,
    pub chain_discriminant: u16, // nouveau, coordonné globalement
    // ... champs existants
}
```

- **Contraintes :**
  - Unicité par réseau actif ; géré via un registre public signé avec des
    plages réservées explicites (par exemple `0x0000–0x0FFF` pour test/dev,
    `0x1000–0x7FFF` pour les allocations communautaires,
    `0x8000–0xFFEF` pour les entrées approuvées par la gouvernance,
    `0xFFF0–0xFFFF` réservées).
  - Immuable pour une chaîne en fonctionnement. Toute modification requiert
    un hard fork et une mise à jour du registre.
- **Gouvernance & registre :** un ensemble de gouvernance multi‑signature
  maintient un registre JSON signé (épinglé sur IPFS) qui associe les
  discriminants à des alias humains et à des identifiants CAIP‑2. Les clients
  récupèrent et mettent en cache ce registre pour valider et afficher les
  métadonnées de chaîne.
- **Usage :** le discriminant est propagé dans l’admission d’état, Torii, les
  SDKs et les APIs de wallet, afin que chaque composant puisse l’encoder ou le
  vérifier. Il est exposé dans CAIP‑2 (par exemple `iroha:0x1234`).

<a id="2-canonical-address-codecs"></a>

### 2. Codecs d’adresse canoniques

Le modèle de données Rust expose une seule représentation binaire canonique
(`AccountAddress`) qui peut être rendue sous plusieurs formats
orientés‑utilisateur :
IH58 est le format de compte préféré pour le partage et la sortie canonique ;
la forme compressée `snx1` est une option de second rang, réservée à Sora, pour
l’UX.

- **IH58 (Iroha Base58)** – enveloppe Base58 qui embarque le discriminant de
  chaîne. Les décodeurs valident le préfixe avant de promouvoir la payload en
  forme canonique.
- **Vue compressée Sora :** alphabet spécifique à Sora de **105 symboles**,
  obtenu en ajoutant le poème イロハ en half‑width (incluant ヰ et ヱ) à
  l’alphabet IH58 (58 caractères). Les chaînes commencent par le sentinel
  `snx1`, incluent un checksum dérivé de Bech32m et omettent le préfixe de
  réseau (Sora Nexus est implicite via le sentinel).

  ```text
  IH58  : 123456789ABCDEFGHJKMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz
  Iroha : ｲﾛﾊﾆﾎﾍﾄﾁﾘﾇﾙｦﾜｶﾖﾀﾚｿﾂﾈﾅﾗﾑｳヰﾉｵｸﾔﾏｹﾌｺｴﾃｱｻｷﾕﾒﾐｼヱﾋﾓｾｽ
  ```
- **Hex canonique :** encodage `0x…` de la payload binaire canonique, adapté
  au debug.

`AccountAddress::parse_any` détecte automatiquement les entrées compressées,
IH58 ou hex canoniques et renvoie la payload décodée ainsi que
`AccountAddressFormat`. Torii appelle `parse_any` pour les adresses
supplémentaires ISO 20022 et stocke la forme hex canonique, ce qui garantit la
stabilité des métadonnées quelle que soit la représentation d’origine.

#### 2.1 Répartition du byte de header (ADDR‑1a)

Chaque payload canonique est structurée comme `header · domain selector · controller`.
Le `header` est un octet unique qui détermine les règles de parse applicables
aux octets suivants :

```text
bit index:   7        5 4      3 2      1 0
             ┌─────────┬────────┬────────┬────┐
payload bit: │version  │ class  │  norm  │ext │
             └─────────┴────────┴────────┴────┘
```

Ce premier octet empaquette la métadonnée de schéma pour les décodeurs :

| Bits | Champ         | Valeurs autorisées | Erreur en cas de violation                             |
|------|---------------|--------------------|--------------------------------------------------------|
| 7‑5  | `addr_version` | `0` (v1). Les valeurs `1‑7` sont réservées à de futures révisions. | Valeurs hors de `0‑7` → `AccountAddressError::InvalidHeaderVersion` ; les implémentations DOIVENT traiter toute valeur non nulle comme non supportée aujourd’hui. |
| 4‑3  | `addr_class`  | `0` = clé unique, `1` = multisig. | Autres valeurs → `AccountAddressError::UnknownAddressClass`. |
| 2‑1  | `norm_version` | `1` (Norm v1). Les valeurs `0`, `2`, `3` sont réservées. | Valeurs hors de `0‑3` → `AccountAddressError::InvalidNormVersion`. |
| 0    | `ext_flag`    | DOIT être `0`.    | Bit positionné → `AccountAddressError::UnexpectedExtensionFlag`. |

L’encoder Rust écrit toujours `0x02` (version 0, classe “single‑key”, version
de normalisation 1, bit d’extension à zéro).

#### 2.2 Encodage du sélecteur de domaine (ADDR‑1a)

Le sélecteur de domaine suit immédiatement le header et est une union
taguée :

| Tag   | Signification               | Payload   | Remarques |
|-------|-----------------------------|-----------|-----------|
| `0x00` | Domaine par défaut implicite | aucune    | Correspond au `default_domain_name()` configuré. |
| `0x01` | Digest de domaine local    | 12 octets | Digest = `blake2s_mac(key = "SORA-LOCAL-K:v1", canonical_label)[0..12]`. |
| `0x02` | Entrée du registre global  | 4 octets  | `registry_id` en big‑endian ; réservé jusqu’au déploiement du registre global. |

Les labels de domaine sont normalisés (UTS‑46 + STD3 + NFC) avant le hash.
Les tags inconnus produisent `AccountAddressError::UnknownDomainTag`. Lors de
la validation d’une adresse par rapport à un domaine, un sélecteur qui ne
correspond pas déclenche `AccountAddressError::DomainMismatch`.

```text
domain selector
┌──────────┬──────────────────────────────────────────────┐
│ tag (u8) │ payload (depends on selector kind, see table)│
└──────────┴──────────────────────────────────────────────┘
```

Le sélecteur est immédiatement adjacent à la payload du contrôleur, ce qui
permet au décodeur de parcourir le format on‑wire dans l’ordre : lire l’octet
de tag, lire la payload spécifique, puis passer aux octets du contrôleur.

**Exemples de sélecteur**

- *Défaut implicite* (`tag = 0x00`). Pas de payload. Exemple d’hex canonique
  pour le domaine par défaut avec la clé de test déterministe :
  `0x0200000120641297079357229f295938a4b5a333de35069bf47b9d0704e45805713d13c201`.
- *Digest local* (`tag = 0x01`). La payload est le digest 12 octets. Exemple
  (`treasury`, seed `0x01`) :
  `0x0201b18fe9c1abbac45b3e38fc5d0001203b77a042f1de02f6d5f418f36a20fd68c8329fe3bbfbecd26a2d72878cd827f8`.
- *Registre global* (`tag = 0x02`). La payload est un `registry_id:u32`
  big‑endian. Les octets suivant cette payload sont identiques au cas de
  domaine implicite ; le sélecteur remplace simplement le label normalisé par
  un pointeur de registre. Exemple avec `registry_id = 0x0000_002A` (42 en
  décimal) et le contrôleur déterministe par défaut :
  `0x02020000002a000120641297079357229f295938a4b5a333de35069bf47b9d0704e45805713d13c201`.

#### 2.3 Encodage de la payload de contrôleur (ADDR‑1a)

La payload de contrôleur est une autre union taguée ajoutée après le
sélecteur de domaine :

| Tag   | Contrôleur | Layout | Remarques |
|-------|------------|--------|-----------|
| `0x00` | Clé unique | `curve_id:u8` · `key_len:u8` · `key_bytes` | `curve_id = 0x01` correspond aujourd’hui à Ed25519. `key_len` est borné à `u8` ; des valeurs supérieures produisent `AccountAddressError::KeyPayloadTooLong`. |
| `0x01` | Multisig   | `version:u8` · `threshold:u16` · `member_count:u8` · (`curve_id:u8` · `weight:u16` · `key_len:u16` · `key_bytes`)\* | Supporte jusqu’à 255 membres (`CONTROLLER_MULTISIG_MEMBER_MAX`). Les courbes inconnues déclenchent `AccountAddressError::UnknownCurve` ; les politiques malformées remontent sous la forme `AccountAddressError::InvalidMultisigPolicy`. |

Les politiques multisig exposent également une map CBOR de style CTAP2 et un
digest canonique, afin que hosts et SDKs puissent vérifier le contrôleur de
manière déterministe. Voir
`docs/source/references/address_norm_v1.md` (ADDR‑1c) pour le schéma, la
procédure de hachage et les fixtures de référence.

Tous les octets de clé sont encodés exactement comme retournés par
`PublicKey::to_bytes` ; les décodeurs reconstruisent les instances
`PublicKey` et lèvent `AccountAddressError::InvalidPublicKey` si les bytes ne
correspondent pas à la courbe déclarée.

> **Application canonique Ed25519 (ADDR‑3a) :** les clés avec
> `curve_id = 0x01` doivent se décoder exactement en la séquence d’octets
> émise par le signataire et ne doivent pas appartenir au sous‑groupe de
> petit ordre. Les nœuds rejettent les encodages non canoniques (par exemple
> des valeurs réduites modulo `2^255‑19`) ainsi que les points faibles comme
> l’élément identité ; les SDKs doivent refléter les mêmes erreurs de
> validation avant de soumettre des adresses.

##### 2.3.1 Registre des identifiants de courbe (ADDR‑1d)

| ID (`curve_id`) | Algorithme | Feature gate | Remarques |
|-----------------|-----------|--------------|-----------|
| `0x00` | Réservé | — | NE DOIT PAS être émis ; les décodeurs renvoient `ERR_UNKNOWN_CURVE`. |
| `0x01` | Ed25519 | Toujours actif | Algorithme canonique v1 (`Algorithm::Ed25519`) ; seul id activé aujourd’hui en production. |
| `0x02` | ML‑DSA (preview) | `ml-dsa` | Réservé pour le déploiement ADDR‑3 ; désactivé par défaut tant que la voie de signature ML‑DSA n’est pas disponible. |
| `0x03` | Marqueur GOST | `gost` | Réservé pour l’approvisionnement/la négociation ; tout payload portant cet id doit être rejeté tant que la gouvernance n’a pas approuvé de profil TC26 concret. |
| `0x0A` | GOST R 34.10‑2012 (256, set A) | `gost` | Réservé ; activé avec le feature cryptographique `gost`. |
| `0x0B` | GOST R 34.10‑2012 (256, set B) | `gost` | Réservé pour approbation de gouvernance future. |
| `0x0C` | GOST R 34.10‑2012 (256, set C) | `gost` | Réservé pour approbation de gouvernance future. |
| `0x0D` | GOST R 34.10‑2012 (512, set A) | `gost` | Réservé pour approbation de gouvernance future. |
| `0x0E` | GOST R 34.10‑2012 (512, set B) | `gost` | Réservé pour approbation de gouvernance future. |
| `0x0F` | SM2 | `sm` | Réservé ; sera disponible lorsque le feature crypto SM sera stabilisé. |

Les slots `0x04–0x09` restent libres pour de futures courbes ; introduire un
nouvel algorithme nécessite une mise à jour de roadmap et une couverture
coordonnée côté SDK/host. Les encodeurs DOIVENT rejeter tout algorithme non
supporté via `ERR_UNSUPPORTED_ALGORITHM`, et les décodeurs DOIVENT échouer
rapidement sur les ids inconnus avec `ERR_UNKNOWN_CURVE` pour conserver un
comportement fail‑closed.

Le registre canonique (y compris un export JSON lisible par machine) se trouve
dans [`docs/source/references/address_curve_registry.md`](source/references/address_curve_registry.md).
Les outils DEVRAIENT le consommer directement afin de garder les identifiants
de courbe cohérents entre SDKs et workflows opérateur.

- **Gating SDK :** les SDKs se limitent par défaut à Ed25519 uniquement. Swift
  expose des flags de compilation (`IROHASWIFT_ENABLE_MLDSA`,
  `IROHASWIFT_ENABLE_GOST`, `IROHASWIFT_ENABLE_SM`) ; le SDK Java requiert
  `AccountAddress.configureCurveSupport(...)`, et le SDK JavaScript utilise
  `configureCurveSupport({ allowMlDsa: true, allowGost: true, allowSm2: true })`
  afin que les intégrateurs doivent faire un opt‑in explicite avant d’émettre
  des adresses utilisant d’autres courbes.
- **Gating host :** `Register<Account>` rejette les contrôleurs dont les
  signataires utilisent des algorithmes absents de
  `crypto.allowed_signing` **ou** des ids de courbe absents de
  `crypto.curves.allowed_curve_ids` ; les clusters doivent annoncer le support
  (configuration + genesis) avant d’autoriser des contrôleurs ML‑DSA/GOST/SM.

#### 2.4 Règles d’échec (ADDR‑1a)

- Les payloads plus courtes que le header + sélecteur obligatoires, ou
  contenant des octets restants, entraînent
  `AccountAddressError::InvalidLength` ou
  `AccountAddressError::UnexpectedTrailingBytes`.
- Les headers qui positionnent le `ext_flag` réservé ou annoncent des
  versions/classes non supportées DOIVENT être rejetés via
  `UnexpectedExtensionFlag`, `InvalidHeaderVersion` ou
  `UnknownAddressClass`.
- Des tags inconnus de sélecteur/contrôleur produisent
  `UnknownDomainTag` ou `UnknownControllerTag`.
- Un matériel de clé surdimensionné ou malformé génère
  `KeyPayloadTooLong` ou `InvalidPublicKey`.
- Les contrôleurs multisig dépassant 255 membres déclenchent
  `MultisigMemberOverflow`.
- Conversions IME/NFKC : les kana Sora en half‑width peuvent être normalisées
  en full‑width sans casser le décodage, mais le sentinel ASCII `snx1` et les
  chiffres/lettres IH58 DOIVENT rester ASCII. Des sentinels en full‑width ou
  soumis à un case‑folding entraînent `ERR_MISSING_COMPRESSED_SENTINEL` ;
  des payloads ASCII en full‑width entraînent `ERR_INVALID_COMPRESSED_CHAR`,
  et les mismatches de checksum se traduisent par `ERR_CHECKSUM_MISMATCH`.
  Des property tests dans
  `crates/iroha_data_model/src/account/address.rs` couvrent ces chemins afin
  que SDKs et wallets puissent se reposer sur des échecs déterministes.
- Le parse des alias `address@domain` dans Torii et les SDKs renvoie
  désormais les mêmes codes `ERR_*` lorsque les entrées IH58/compressées
  échouent avant le fallback vers l’alias (par exemple checksum invalide,
  digest de domaine invalide), ce qui permet aux clients de relayer des
  raisons structurées plutôt que d’inférer à partir de messages libres.

#### 2.5 Vecteurs binaires normatifs

- **Domaine par défaut implicite (`default`, byte seed `0x00`)**  
  Hex canonique :
  `0x0200000120641297079357229f295938a4b5a333de35069bf47b9d0704e45805713d13c201`.  
  Décomposition : header `0x02`, sélecteur `0x00` (défaut implicite), tag de
  contrôleur `0x00`, id de courbe `0x01` (Ed25519), longueur `0x20` puis les
  32 octets de la clé.
- **Digest de domaine local (`treasury`, byte seed `0x01`)**  
  Hex canonique :
  `0x0201b18fe9c1abbac45b3e38fc5d0001203b77a042f1de02f6d5f418f36a20fd68c8329fe3bbfbecd26a2d72878cd827f8`.  
  Décomposition : header `0x02`, tag de sélecteur `0x01` plus le digest
  `b1 8f e9 c1 ab ba c4 5b 3e 38 fc 5d`, suivi de la payload de clé unique
  (`0x00` comme tag de contrôleur, `0x01` comme id de courbe, `0x20` comme
  longueur et les 32 octets de la clé Ed25519).

Les tests unitaires (`account::address::tests::parse_any_accepts_all_formats`)
valident ces vecteurs V1 via `AccountAddress::parse_any`, garantissant que les
outils peuvent se fier à la même payload canonique, que ce soit en hex, IH58
ou compressée. L’ensemble étendu de fixtures peut être regénéré avec :

```text
cargo xtask address-vectors --out fixtures/account/address_vectors.json
```

### 3. Domaines globalement uniques et normalisation

Voir également
[`docs/source/references/address_norm_v1.md`](source/references/address_norm_v1.md)
pour la pipeline Norm v1 canonique utilisée dans Torii, le modèle de données
et les SDKs.

On redéfinit `DomainId` comme tuple tagué :

```text
DomainId {
    name: Name,
    authority: GlobalDomainAuthority, // nouvel enum
}

enum GlobalDomainAuthority {
    LocalChain,                  // valeur par défaut pour la chaîne locale
    External { chain_discriminant: u16 },
}
```

`LocalChain` encapsule le `Name` existant pour les domaines gérés par la
chaîne courante. Lorsqu’un domaine est enregistré via le registre global, on
persiste le discriminant de la chaîne détentrice. L’affichage/parse reste
inchangé pour le moment, mais la structure étendue permet des décisions de
routing.

#### 3.1 Normalisation et défenses anti‑spoofing

Norm v1 définit la pipeline canonique que chaque composant doit utiliser avant
de persister un nom de domaine ou de l’intégrer dans un `AccountAddress`. La
description détaillée se trouve dans
`docs/source/references/address_norm_v1.md` ; le résumé ci‑dessous capture les
étapes que wallets, Torii, SDKs et outils de gouvernance doivent implémenter.

1. **Validation d’entrée.** Rejeter les chaînes vides, les espaces blancs et
   les délimiteurs réservés `@`, `#`, `$`. Cela correspond aux invariants
   appliqués par `Name::validate_str`.
2. **Composition Unicode NFC.** Appliquer la normalisation NFC (via ICU) pour
   réduire de manière déterministe les séquences canoniquement équivalentes
   (par exemple `e\u{0301}` → `é`).
3. **Normalisation UTS‑46.** Appliquer UTS‑46 avec
   `use_std3_ascii_rules = true`, `transitional_processing = false` et
   l’application des limites de longueur DNS. Le résultat est une suite
   d’A‑labels en minuscules ; les entrées qui violent les règles STD3 échouent
   ici.
4. **Limites de longueur.** Imposer les mêmes bornes que DNS : chaque label
   DOIT faire entre 1 et 63 octets et le domaine complet NE DOIT PAS dépasser
   255 octets après l’étape 3.
5. **Politique optionnelle sur les confusables.** Les contrôles de scripts
   UTS‑39 sont suivis pour Norm v2 ; les opérateurs peuvent les activer en
   avance, mais un échec à ce niveau doit interrompre le traitement.

Si toutes les étapes réussissent, la chaîne d’A‑labels en minuscules est
mise en cache et utilisée pour l’encodage d’adresse, la configuration, les
manifests et les requêtes au registre. Les sélecteurs de digest local dérivent
leur valeur 12 octets comme
`blake2s_mac(key = "SORA-LOCAL-K:v1", canonical_label)[0..12]` à partir de la
sortie de l’étape 3. Toute autre forme d’entrée (mélange de casse, Unicode
brut, etc.) est rejetée avec des `ParseError`s structurés au point où le nom
est fourni.

Des fixtures canoniques illustrant ces règles — y compris des allers‑retours
en punycode et des séquences invalides au regard de STD3 — figurent dans
`docs/source/references/address_norm_v1.md` et sont répliquées dans les
vecteurs de tests CI des SDKs suivis sous ADDR‑2.

### 4. Registre de domaines Nexus et routage

- **Schéma de registre :** Nexus maintient une map signée
  `DomainName -> ChainRecord`, où `ChainRecord` inclut le discriminant de
  chaîne, des métadonnées optionnelles (endpoints RPC) et une preuve
  d’autorité (par exemple une multi‑signature de gouvernance).
- **Mécanisme de synchronisation :**
  - Les chaînes soumettent à Nexus des revendications de domaine signées (au
    moment du genesis ou via des instructions de gouvernance).
  - Nexus publie périodiquement des manifests (JSON signé + racine Merkle
    optionnelle) via HTTPS et un stockage adressé par contenu (par exemple
    IPFS). Les clients conservent le manifest le plus récent et vérifient les
    signatures.
- **Flux de résolution :**
  - Torii reçoit une transaction référant un `DomainId`.
  - Si le domaine est inconnu localement, Torii consulte le manifest Nexus
    mis en cache.
  - Si le manifest indique que le domaine appartient à une chaîne externe, la
    transaction est rejetée avec une erreur déterministe `ForeignDomain` et
    les informations de la chaîne distante.
  - Si le domaine n’apparaît pas dans Nexus, Torii renvoie `UnknownDomain`.
- **Ancrages de confiance & rotation :** des clés de gouvernance signent les
  manifests ; toute rotation ou révocation est publiée comme un nouveau
  manifest. Les clients appliquent un TTL (par exemple 24 h) et refusent de
  s’appuyer sur des données expirées au‑delà de cette fenêtre.
- **Modes de panne :** si la récupération du manifest échoue, Torii retombe
  sur le manifest en cache tant que le TTL reste valide ; au‑delà, il émet
  `RegistryUnavailable` et refuse le routage inter‑domaines afin d’éviter des
  états incohérents.

#### 4.1 Immutabilité du registre, alias et tombstones (ADDR‑7c)

Nexus publie un **manifest append‑only**, de sorte que chaque affectation de
domaine ou d’alias puisse être auditée et rejouée. Les opérateurs doivent
traiter le bundle décrit dans
`docs/source/runbooks/address_manifest_ops.md` comme l’unique source de
vérité : si un manifest manque ou échoue à la validation, Torii doit refuser
de résoudre le domaine concerné.

Support d’automatisation :
`cargo xtask address-manifest verify --bundle <current_dir> --previous <previous_dir>`
rejoue les vérifications de checksum, de schéma et de `previous_digest`
détaillées dans le runbook. Inclure la sortie de cette commande dans les
tickets de change permet de démontrer que le lien `sequence` +
`previous_digest` a été validé avant la publication du bundle.

##### Header de manifest et contrat de signature

| Champ               | Exigence |
|---------------------|----------|
| `version`           | Actuellement `1`. Ne doit être incrémentée qu’avec une mise à jour correspondante du spec. |
| `sequence`          | Doit être incrémenté de **exactement** 1 à chaque publication. Torii refuse les révisions avec trous ou régressions. |
| `generated_ms` + `ttl_hours` | Définissent la fraîcheur du cache (par défaut 24 h). Si le TTL expire avant le manifest suivant, Torii bascule en `RegistryUnavailable`. |
| `previous_digest`   | Digest BLAKE3 (hex) du corps du manifest précédent. Les vérificateurs le recalculent via `b3sum` pour prouver l’immutabilité. |
| `signatures`        | Les manifests sont signés via Sigstore (`cosign sign-blob`). Les opérations doivent exécuter `cosign verify-blob --bundle manifest.sigstore manifest.json` et vérifier l’identité/l’émetteur de gouvernance avant le déploiement. |

L’automatisation de release émet `manifest.sigstore` et `checksums.sha256`
aux côtés du corps JSON. Il faut garder ces fichiers ensemble lors de la
réplication vers SoraFS ou des endpoints HTTP, afin que les auditeurs puissent
rejouer les étapes de vérification telles quelles.

##### Types d’entrée

| Type          | Objectif | Champs requis |
|---------------|----------|---------------|
| `global_domain` | Déclare qu’un domaine est enregistré globalement et doit être mappé à un discriminant de chaîne et un préfixe IH58. | `{ "domain": "<label>", "chain": "sora:nexus:global", "ih58_prefix": 753, "selector": "global" }` |
| `local_alias`   | Suit les sélecteurs hérités (`Local-12`) qui routent encore localement. Ajoute le digest 12 octets et un `alias_label` optionnel. | `{ "domain": "<label>", "selector": { "kind": "local", "digest_hex": "<12-byte-hex>" }, "alias_label": "<optional>" }` |
| `tombstone`     | Retire un alias/sélecteur de manière permanente. Obligatoire pour l’effacement de digests Local‑8 ou la suppression d’un domaine. | `{ "selector": {…}, "reason_code": "LOCAL8_RETIREMENT" \| …, "ticket": "<governance id>", "replaces_sequence": <number> }` |

Les entrées `global_domain` peuvent inclure optionnellement un `manifest_url`
ou `sorafs_cid` pointant vers des métadonnées de chaîne signées, mais le
triplet canonique reste `{domain, chain, discriminant/ih58_prefix}`. Les
enregistrements `tombstone` **doivent** référencer le sélecteur retiré ainsi
que le ticket/artefact de gouvernance ayant autorisé le changement, afin que
la piste d’audit puisse être reconstruite offline.

##### Workflow alias/tombstone et télémétrie

1. **Détecter la dérive.** Utiliser
   `torii_address_local8_total{endpoint}` et
   `torii_address_invalid_total{endpoint,reason}`
   (affichés dans `dashboards/grafana/address_ingest.json`) pour confirmer que
   les chaînes Local‑8 ne sont plus acceptées en production avant de proposer
   un `tombstone`.
2. **Dériver les digests canoniques.** Lancer
   `iroha tools address convert <address-or-account_id> --format json --expect-prefix 753`
   (ou consommer `fixtures/account/address_vectors.json` via
   `scripts/account_fixture_helper.py`) pour capturer exactement le champ
   `digest_hex`. Le CLI accepte des entrées comme `snx1...@wonderland` ; le
   résumé JSON expose le domaine via `input_domain` et l’option
   `--append-domain` rejoue l’encodage converti sous la forme
   `<ih58>@wonderland` pour la mise à jour du manifest. Pour des exports
   ligne‑par‑ligne, utiliser
   afin de convertir massivement des sélecteurs Local en formes IH58
   canoniques (ou compressées/hex/JSON) en ignorant les lignes non locales.
   Pour une preuve exploitable sous forme de feuille de calcul, exécuter
   pour obtenir un CSV (`input,status,format,domain_kind,…`) mettant en
   évidence sélecteurs locaux, encodages canoniques et échecs de parse.
3. **Ajouter les entrées au manifest.** Préparer l’entrée `tombstone` (et le
   `global_domain` correspondant lors de la migration vers le registre global)
   et valider le manifest avec `cargo xtask address-manifest verify` avant de
   demander les signatures.
4. **Vérifier et publier.** Suivre la checklist du runbook (hashes, Sigstore,
   monotonie de `sequence`) avant de répliquer le bundle vers SoraFS. Torii
   les clusters de production exigent immédiatement des littéraux
   IH58/compressés canoniques après la mise à jour du bundle.
5. **Surveiller et, si besoin, revenir en arrière.** Garder les panneaux
   Local‑8 à zéro pendant 30 jours ; en cas de régression, republier le bundle
   de manifests précédent et, uniquement sur les environnements non
   stabilisation de la télémétrie.
