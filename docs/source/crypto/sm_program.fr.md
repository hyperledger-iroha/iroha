---
lang: fr
direction: ltr
source: docs/source/crypto/sm_program.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 08e2e1e4a54390d9142d6788aad2385e93282a33423b9fc7f3418e3633f3f86a
source_last_modified: "2026-01-23T18:50:10.586502+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

//! Présentation de l'architecture d'activation SM2/SM3/SM4 pour Hyperledger Iroha v2.

# Résumé de l'architecture du programme SM

## Objectif
Définissez le plan technique, la posture de la chaîne d'approvisionnement et les limites de risque pour l'introduction de la cryptographie nationale chinoise (SM2/SM3/SM4) sur la pile Iroha v2 tout en préservant l'exécution déterministe et l'auditabilité.

## Portée
- **Chemins critiques par consensus :** `iroha_crypto`, `iroha`, `irohad`, hôte IVM, éléments intrinsèques Kotodama.
- **SDK client et outils :** Rust CLI, Kagami, SDK Python/JS/Swift, utilitaires Genesis.
- **Configuration et sérialisation :** Boutons `iroha_config`, balises de modèle de données Norito, gestion des manifestes, mises à jour multicodecs.
- **Tests et conformité :** suites unitaires/propriétés/interopérabilité, harnais Wycheproof, profilage des performances, conseils en matière d'exportation/réglementation. *(Statut : fusion de la pile SM basée sur RustCrypto ; suite fuzz `sm_proptest` en option et harnais de parité OpenSSL disponibles pour CI étendu.)*

Hors de portée : algorithmes PQ, accélération non déterministe de l'hôte dans les chemins de consensus ; Les builds wasm/`no_std` sont retirées.

## Entrées et livrables de l'algorithme
| Artefact | Propriétaire | À payer | Remarques |
|----------|-------|-----|-------|
| Conception des fonctionnalités de l'algorithme SM (`SM-P0`) | GT sur la cryptographie | 2025-02 | Contrôle des fonctionnalités, audit des dépendances, registre des risques. |
| Intégration de base de Rust (`SM-P1`) | GT Crypto / Modèle de données | 2025-03 | Aides de vérification/hachage/AEAD basées sur RustCrypto, extensions Norito, luminaires. |
| Signature + appels système VM (`SM-P2`) | IVM Programme de base/SDK | 2025-04 | Wrappers de signature déterministes, appels système, couverture Kotodama. |
| Activation facultative du fournisseur et des opérations (`SM-P3`) | Groupe de travail Opérations de plateforme / Performance | 2025-06 | Backend OpenSSL/Tongsuo, intrinsèques ARM, télémétrie, documentation. |

## Bibliothèques sélectionnées
- **Primaire :** Caisses RustCrypto (`sm2`, `sm3`, `sm4`) avec la fonctionnalité `rfc6979` activée et SM3 lié à des noms occasionnels déterministes.
- **FFI en option :** API du fournisseur OpenSSL 3.x ou Tongsuo pour les déploiements nécessitant des piles ou des moteurs matériels certifiés ; limité aux fonctionnalités et désactivé par défaut dans les binaires de consensus.### État d'intégration de la bibliothèque principale
- `iroha_crypto::sm` expose le hachage SM3, la vérification SM2 et les assistants SM4 GCM/CCM sous la fonctionnalité unifiée `sm`, avec des chemins de signature déterministes RFC6979 disponibles pour les SDK via `Sm2PrivateKey`.【crates/iroha_crypto/src/sm.rs:1049】【crates/iroha_crypto/src/sm.rs:1128】【crates/iroha_crypto/src/sm.rs:1236】
- Les balises Norito/Norito-JSON et les assistants multicodecs couvrent les clés/signatures publiques SM2 et les charges utiles SM3/SM4 afin que les instructions soient sérialisées de manière déterministe. hôtes.【crates/iroha_data_model/src/isi/registry.rs:407】【crates/iroha_data_model/tests/sm_norito_roundtrip.rs:12】
- Les suites à réponses connues valident l'intégration de RustCrypto (`sm3_sm4_vectors.rs`, `sm2_negative_vectors.rs`) et s'exécutent dans le cadre des tâches de fonctionnalité `sm` de CI, gardant la vérification déterministe pendant que les nœuds continuent de signer avec Ed25519.【crates/iroha_crypto/tests/sm3_sm4_vectors.rs:15】【crates/iroha_crypto/tests/sm2_negative_vectors.rs:1】
- Validation facultative de la construction des fonctionnalités `sm` : `cargo check -p iroha_crypto --features sm --locked` (froid 7,9 s / chaud 0,23 s) et `cargo check -p iroha_crypto --no-default-features --features "std sm" --locked` (1,0 s) réussissent tous deux ; l'activation de la fonctionnalité ajoute 11 caisses (`base64ct`, `ghash`, `opaque-debug`, `pem-rfc7468`, `pkcs8`, `polyval`, `primeorder`, `sm2`, `sm3`, `sm4`, `sm4-gcm`). Résultats enregistrés dans `docs/source/crypto/sm_rustcrypto_spike.md`.【docs/source/crypto/sm_rustcrypto_spike.md:1】
- Les appareils de vérification négative BouncyCastle/GmSSL fonctionnent sous `crates/iroha_crypto/tests/fixtures/sm/sm2_negative_vectors.json`, garantissant que les cas d'échec canoniques (r=0, s=0, inadéquation d'ID distinctif, clé publique falsifiée) restent alignés avec ceux largement déployés. fournisseurs.【crates/iroha_crypto/tests/sm2_negative_vectors.rs:1】【crates/iroha_crypto/tests/fixtures/sm/sm2_negative_vectors.json:1】
- `sm-ffi-openssl` compile désormais la chaîne d'outils OpenSSL 3.x fournie (fonctionnalité `openssl` crate `vendored`) afin que les versions d'aperçu et les tests ciblent toujours un fournisseur moderne compatible SM, même lorsque le système LibreSSL/OpenSSL ne dispose pas d'algorithmes SM. 【crates/iroha_crypto/Cargo.toml:59】
- `sm_accel` détecte désormais AArch64 NEON au moment de l'exécution et transmet les hooks SM3/SM4 via la répartition x86_64/RISC-V tout en respectant le bouton de configuration `crypto.sm_intrinsics` (`auto`/`force-enable`/`force-disable`). Lorsque les back-ends vectoriels sont absents, le répartiteur utilise toujours le chemin scalaire RustCrypto afin que les bancs et les bascules de politique se comportent de manière cohérente entre les hôtes.

### Norito Surfaces de schéma et de modèle de données| Norito type / consommateur | Représentation | Contraintes & remarques |
|------------------------|----------------|--------------------------|
| `Sm3Digest` (`iroha_crypto::Sm3Digest`) | Nu : blob de 32 octets · JSON : chaîne hexadécimale majuscule (`"4F4D..."`) | Enveloppement de tuple canonique Norito `[u8; 32]`. Le décodage JSON/Bare rejette les longueurs ≠ 32. Aller-retours couverts par `sm_norito_roundtrip::sm3_digest_norito_roundtrip`. |
| `Sm2PublicKey` / `Sm2Signature` | Blobs préfixés multicodecs (`0x1306` provisoire) | Les clés publiques codent les points SEC1 non compressés ; les signatures sont `(r∥s)` (32 octets chacune) avec des gardes d'analyse DER. |
| `Sm4Key` | Nu : blob de 16 octets | Wrapper de remise à zéro exposé à Kotodama/CLI. La sérialisation JSON est délibérément omise ; les opérateurs doivent transmettre les clés via des blobs (contrats) ou CLI hex (`--key-hex`). |
| Opérandes `sm4_gcm_seal/open` | Tuple de 4 blobs : `(key, nonce, aad, payload)` | Clé = 16 octets ; occasionnel = 12 octets ; longueur de balise fixée à 16 octets. Renvoie `(ciphertext, tag)` ; Kotodama/CLI émet des assistants hexadécimaux et base64.【crates/ivm/tests/sm_syscalls.rs:728】 |
| Opérandes `sm4_ccm_seal/open` | Tuple de 4 blobs (clé, nonce, aad, charge utile) + longueur de balise dans `r14` | Nonce 7 à 13 octets ; longueur de la balise ∈ {4,6,8,10,12,14,16}. La fonctionnalité `sm` expose le CCM derrière l’indicateur `sm-ccm`. |
| Intrinsèques Kotodama (`sm::hash`, `sm::seal_gcm`, `sm::open_gcm`, …) | Mapper les SCALL ci-dessus | La validation des entrées reflète les règles de l'hôte ; les tailles mal formées soulèvent `ExecutionError::Type`. |

Référence rapide d'utilisation :
- **Hachage SM3 dans les contrats/tests :** `Sm3Digest::hash(b"...")` (Rust) ou Kotodama `sm::hash(input_blob)`. JSON attend 64 caractères hexadécimaux.
- **SM4 AEAD via CLI :** `iroha tools crypto sm4 gcm-seal --key-hex <32 hex> --nonce-hex <24 hex> --plaintext-hex …` produit des paires de texte chiffré hexadécimal/base64+tag. Décryptez avec `gcm-open` correspondant.
- **Chaînes multicodecs :** Les clés/signatures publiques SM2 analysent depuis/vers la chaîne multibase acceptée par `PublicKey::from_str`/`Signature::from_bytes`, permettant aux manifestes Norito et aux ID de compte de transporter les signataires SM.

Les consommateurs de modèles de données doivent traiter les clés et balises SM4 comme des blobs transitoires ; ne conservez jamais les clés brutes sur la chaîne. Les contrats doivent stocker uniquement les résultats de texte chiffré/de balise ou les résumés dérivés (par exemple, SM3 de la clé) lorsqu'un audit est requis.

### Chaîne d'approvisionnement et licences
| Composant | Licence | Atténuation |
|-----------|---------|---------------|
| `sm2`, `sm3`, `sm4` | Apache-2.0 / MIT | Suivez les validations en amont, le fournisseur si des versions verrouillées sont requises, planifiez un audit tiers avant que le validateur ne signe GA. |
| `rfc6979` | Apache-2.0 / MIT | Déjà utilisé dans d'autres algorithmes ; confirmer la liaison déterministe `k` avec le résumé SM3. |
| OpenSSL/Tongsuo en option | Apache-2.0 / Style BSD | Conservez la fonctionnalité `sm-ffi-openssl`, exigez l'adhésion explicite de l'opérateur et la liste de contrôle d'emballage. |### Indicateurs de fonctionnalités et propriété
| Surfaces | Par défaut | Mainteneur | Remarques |
|---------|---------|------------|-------|
| `iroha_crypto/sm-core`, `sm-ccm`, `sm` | Désactivé | GT sur la cryptographie | Active les primitives RustCrypto SM ; `sm` regroupe les assistants CCM pour les clients nécessitant un chiffrement authentifié. |
| `ivm/sm` | Désactivé | IVM Équipe principale | Construit des appels système SM (`sm3_hash`, `sm2_verify`, `sm4_gcm_*`, `sm4_ccm_*`). Le déclenchement de l'hôte dérive de `crypto.allowed_signing` (présence de `sm2`). |
| `iroha_crypto/sm_proptest` | Désactivé | GT QA / Crypto | Harnais de test de propriété couvrant les signatures/balises mal formées. Activé uniquement dans CI étendu. |
| `crypto.allowed_signing` + `default_hash` | `["ed25519"]`, `blake2b-256` | GT Config / GT Opérateurs | La présence du hachage `sm2` plus `sm3-256` permet les appels système/signatures SM ; la suppression de `sm2` revient au mode de vérification uniquement. |
| `sm-ffi-openssl` en option (aperçu) | Désactivé | Opérations de plateforme | Fonctionnalité d'espace réservé pour l'intégration du fournisseur OpenSSL/Tongsuo ; reste désactivé jusqu'à ce que les SOP de certification et d'emballage arrivent. |

La stratégie réseau expose désormais `network.require_sm_handshake_match` et
`network.require_sm_openssl_preview_match` (les deux par défaut sont `true`). Effacer l'un ou l'autre des drapeaux permet
déploiements mixtes où les observateurs Ed25519 uniquement se connectent à des validateurs compatibles SM ; les discordances sont
connecté à `WARN`, mais les nœuds de consensus doivent conserver les valeurs par défaut activées pour éviter toute erreur accidentelle.
divergence entre les pairs conscients du SM et ceux handicapés par le SM.
La CLI fait apparaître ces bascules via la mise à jour de la poignée de main de l'application `iroha_cli sorafs
--allow-sm-handshake-mismatch` and `--allow-sm-openssl-preview-mismatch`, or the matching `--require-*`
drapeaux pour rétablir une application stricte.#### Aperçu d'OpenSSL/Tongsuo (`sm-ffi-openssl`)
- **Portée.** Crée un shim de fournisseur en version préliminaire uniquement (`OpenSslProvider`) qui valide la disponibilité du runtime OpenSSL et expose le hachage SM3 soutenu par OpenSSL, la vérification SM2 et le cryptage/déchiffrement SM4-GCM tout en restant opt-in. Les binaires Consensus doivent continuer à utiliser le chemin RustCrypto ; le backend FFI est strictement opt-in pour les pilotes de vérification/signature de bord.
- **Conditions préalables de construction.** Compilez avec `cargo build -p iroha_crypto --features "sm sm-ffi-openssl"` et assurez les liens de la chaîne d'outils avec OpenSSL/Tongsuo 3.0+ (`libcrypto` avec prise en charge SM2/SM3/SM4). Les liens statiques sont déconseillés ; privilégiez les bibliothèques dynamiques gérées par l'opérateur.
- **Test de fumée du développeur.** Exécutez `scripts/sm_openssl_smoke.sh` pour exécuter `cargo check -p iroha_crypto --features "sm sm-ffi-openssl"` suivi de `cargo test -p iroha_crypto --features "sm sm-ffi-openssl" --test sm_openssl_smoke -- --nocapture` ; l'assistant saute automatiquement lorsque les en-têtes de développement OpenSSL ≥3 ne sont pas disponibles (ou que `pkg-config` est manquant) et affiche la sortie de fumée afin que les développeurs puissent voir si la vérification SM2 a été exécutée ou est revenue à l'implémentation de Rust.
- **Échafaudage Rust.** Le module `openssl_sm` achemine désormais le hachage SM3, la vérification SM2 (préhachage ZA + ECDSA SM2) et le chiffrement/déchiffrement SM4 GCM via OpenSSL avec des erreurs structurées couvrant les bascules d'aperçu et les longueurs de clé/nonce/tag non valides ; Le SM4 CCM reste purement rouillé jusqu'à ce que des cales FFI supplémentaires atterrissent.
- **Comportement de saut.** Lorsque les en-têtes ou les bibliothèques OpenSSL ≥3.0 sont absents, le test de fumée imprime une bannière de saut (via `-- --nocapture`) mais se termine toujours avec succès afin que CI puisse distinguer les lacunes de l'environnement des véritables régressions.
- **Garde-corps d'exécution.** L'aperçu d'OpenSSL est désactivé par défaut ; activez-le via la configuration (`crypto.enable_sm_openssl_preview` / `OpenSslProvider::set_preview_enabled(true)`) avant d'essayer d'utiliser le chemin FFI. Conservez les clusters de production en mode vérification uniquement (omettez `sm2` de `allowed_signing`) jusqu'à ce que le fournisseur obtienne son diplôme, comptez sur la solution de secours déterministe RustCrypto et limitez les pilotes de signature à des environnements isolés.
- **Liste de contrôle d'emballage.** Documentez la version du fournisseur, le chemin d'installation et les hachages d'intégrité dans les manifestes de déploiement. Les opérateurs doivent fournir des scripts d'installation qui installent la version OpenSSL/Tongsuo approuvée, l'enregistrent auprès du magasin de confiance du système d'exploitation (si nécessaire) et épinglent les mises à niveau derrière les fenêtres de maintenance.
- **Prochaines étapes.** Les prochaines étapes ajouteront des liaisons déterministes SM4 CCM FFI, des tâches de fumée CI (voir `ci/check_sm_openssl_stub.sh`) et la télémétrie. Suivez les progrès sous SM-P3.1.x dans `roadmap.md`.

#### Instantané de propriété du code
- **Crypto WG :** `iroha_crypto`, luminaires SM, documentation de conformité.
- **IVM Core :** implémentations d'appels système, éléments intrinsèques Kotodama, contrôle de l'hôte.
- **Config WG :** קונפיגורציית `crypto.allowed_signing`/`default_hash`, מניפסט, חיווט קבלה.
- **Programme SDK :** Outils compatibles SM sur CLI/Kagami/SDK, appareils partagés.
- **Platform Ops & Performance WG :** crochets d'accélération, télémétrie, activation des opérateurs.

## Manuel de migration de configurationLes opérateurs passant des réseaux Ed25519 uniquement aux déploiements compatibles SM doivent
suivez le processus par étapes dans
[`sm_config_migration.md`](sm_config_migration.md). Le guide couvre la construction
validation, superposition `iroha_config` (`defaults` → `user` → `actual`), genèse
régénération via les remplacements `kagami` (par exemple `kagami genesis generate --allowed-signing sm2 --default-hash sm3-256`), la validation avant vol et la restauration
planification afin que les instantanés de configuration et les manifestes restent cohérents dans l'ensemble
flotte.

## Politique déterministe
- Appliquer les noms occasionnels dérivés de la RFC6979 pour tous les chemins de signature SM2 dans les SDK et la signature facultative de l'hôte ; les vérificateurs acceptent uniquement les codages r∥s canoniques.
- La communication du plan de contrôle (streaming) reste Ed25519 ; SM2 limité aux signatures du plan de données, à moins que la gouvernance n'approuve l'expansion.
- Intrinsèques (ARM SM3/SM4) limités aux opérations de vérification/hachage déterministes avec détection des fonctionnalités d'exécution et repli logiciel.

## Norito et plan d'encodage
1. Étendre les énumérations d'algorithmes dans `iroha_data_model` avec `Sm2PublicKey`, `Sm2Signature`, `Sm3Digest`, `Sm4Key`.
2. Sérialisez les signatures SM2 sous forme de tableaux `r∥s` big-endian à largeur fixe (32+32 octets) pour éviter les ambiguïtés DER ; conversions gérées dans les adaptateurs. *(Terminé : implémenté dans les assistants `Sm2Signature` ; allers-retours Norito/JSON en place.)*
3. Enregistrez les identifiants multicodecs (`sm3-256`, `sm2-pub`, `sm4-key`) si vous utilisez des multiformats, mettez à jour les appareils et les documents. *(Progression : code provisoire `sm2-pub` `0x1306` désormais validé avec des clés dérivées ; codes SM3/SM4 en attente d'affectation finale, suivis via `sm_known_answers.toml`.)*
4. Mise à jour des tests d'or Norito couvrant les allers-retours et le rejet des encodages mal formés (r ou s courts/longs, paramètres de courbe invalides).## Plan d'intégration hôte et VM (SM-2)
1. Implémentez l'appel système `sm3_hash` côté hôte reflétant la cale de hachage GOST existante ; réutilisez `Sm3Digest::hash` et exposez les chemins d'erreur déterministes. *(Landed : l'hôte renvoie le TLV Blob ; voir l'implémentation `DefaultHost` et la régression `sm_syscalls.rs`.)*
2. Étendez la table d'appel système de la VM avec `sm2_verify` qui accepte les signatures canoniques r∥s, valide les ID distinctifs et mappe les échecs aux codes de retour déterministes. *(Terminé : l'hôte + les éléments intrinsèques Kotodama renvoient `1/0` ; la suite de régression couvre désormais les signatures tronquées, les clés publiques mal formées, les TLV non-blob et les charges utiles `distid` UTF-8/vides/inadaptées.)*
3. Fournissez les appels système `sm4_gcm_seal`/`sm4_gcm_open` (et éventuellement CCM) avec un dimensionnement explicite des noms occasionnels/tags (RFC 8998). *(Terminé : GCM utilise des noms occasionnels fixes de 12 octets + des balises de 16 octets ; CCM prend en charge les noms occasionnels de 7 à 13 octets avec des longueurs de balises {4,6,8,10,12,14,16} contrôlées via `r14` ; Kotodama les expose comme `sm::seal_gcm/open_gcm` et `sm::seal_ccm/open_ccm`.) Documentez la politique de réutilisation occasionnelle dans le manuel du développeur.*
4. Câblez les contrats de fumée Kotodama et les tests d'intégration IVM couvrant les cas positifs et négatifs (tags altérés, signatures mal formées, algorithmes non pris en charge). *(Fait via la mise en miroir des régressions d'hôte `crates/ivm/tests/kotodama_sm_syscalls.rs` pour SM3/SM2/SM4.)*
5. Mettez à jour les listes autorisées d'appels système, les stratégies et les documents ABI (`crates/ivm/docs/syscalls.md`) et actualisez les manifestes hachés après avoir ajouté les nouvelles entrées.

### État de l'intégration de l'hôte et de la VM
- DefaultHost, CoreHost et WsvHost exposent les appels système SM3/SM2/SM4 et les déclenchent sur `sm_enabled`, renvoyant `PermissionDenied` lorsque l'indicateur d'exécution est faux.【crates/ivm/src/host.rs:915】【crates/ivm/src/core_host.rs:833】【crates/ivm/src/mock_wsv.rs:2307】
- Le déclenchement `crypto.allowed_signing` est transmis via le pipeline/l'exécuteur/l'état afin que les nœuds de production optent de manière déterministe via la configuration ; l'ajout de `sm2` active la disponibilité de l'assistant SM.`【crates/iroha_core/src/smartcontracts/ivm/host.rs:170】【crates/iroha_core/src/state.rs:7673】【crates/iroha_core/src/executor.rs:683】
- La couverture de régression exerce les chemins activés et désactivés (DefaultHost/CoreHost/WsvHost) pour le hachage SM3, la vérification SM2 et le sceau/ouverture SM4 GCM/CCM flux.【crates/ivm/tests/sm_syscalls.rs:129】【crates/ivm/tests/sm_syscalls.rs:733】【crates/ivm/tests/sm_syscalls.rs:1036】

## Fils de configuration
- Ajoutez `crypto.allowed_signing`, `crypto.default_hash`, `crypto.sm2_distid_default` et le `crypto.enable_sm_openssl_preview` en option à `iroha_config`. Assurez-vous que la plomberie des fonctionnalités du modèle de données reflète la caisse de chiffrement (`iroha_data_model` expose `sm` → `iroha_crypto/sm`).
- Connectez la configuration aux politiques d'admission afin que les fichiers manifestes/genèse définissent les algorithmes autorisés ; le plan de contrôle reste Ed25519 par défaut.### Travail CLI et SDK (SM-3)
1. **Torii CLI** (`crates/iroha_cli`) : ajoutez la génération/importation/exportation de clés SM2 (compatible distid), les assistants de hachage SM3 et les commandes de chiffrement/déchiffrement SM4 AEAD. Mettez à jour les invites et les documents interactifs.
2. **Outils Genesis** (`xtask`, `scripts/`) : permet aux manifestes de déclarer les algorithmes de signature autorisés et les hachages par défaut, échouent rapidement si SM est activé sans les boutons de configuration correspondants. *(Terminé : `RawGenesisTransaction` comporte désormais un bloc `crypto` avec `default_hash`/`allowed_signing`/`sm2_distid_default` ; `ManifestCrypto::validate` et `kagami genesis validate` rejettent les paramètres SM incohérents et Le manifeste defaults/genesis annonce l'instantané.)*
3. **Surfaces du SDK** :
   - Rust (`iroha_client`) : expose les aides à la signature/vérification SM2, le hachage SM3, les wrappers SM4 AEAD avec des valeurs par défaut déterministes.
   - Python/JS/Swift : miroir de l'API Rust ; réutilisez les appareils mis en scène dans `sm_known_answers.toml` pour les tests multilingues.
4. Documentez le flux de travail de l'opérateur pour activer SM dans les démarrages rapides CLI/SDK et assurez-vous que les configurations JSON/YAML acceptent les nouvelles balises d'algorithme.

#### Progression de la CLI
- `cargo run -p iroha_cli --features sm -- crypto sm2 keygen --distid CN12345678901234` émet désormais une charge utile JSON décrivant la paire de clés SM2 ainsi qu'un extrait `client.toml` (`public_key_config`, `private_key_hex`, `distid`). La commande accepte `--seed-hex` pour la génération déterministe et reflète la dérivation RFC 6979 utilisée par les hôtes.
- `cargo xtask sm-operator-snippet --distid CN12345678901234` encapsule le flux keygen/export, en écrivant les mêmes sorties `sm2-key.json`/`client-sm2.toml` en une seule étape. Utilisez `--json-out <path|->` / `--snippet-out <path|->` pour rediriger les fichiers ou les diffuser vers la sortie standard, en supprimant la dépendance `jq` pour l'automatisation.
- `iroha_cli tools crypto sm2 import --private-key-hex <hex> [--distid ...]` dérive les mêmes métadonnées du matériel existant afin que les opérateurs puissent valider les identifiants distinctifs avant l'admission.
- `iroha_cli tools crypto sm2 export --private-key-hex <hex> --emit-json` imprime l'extrait de configuration (y compris les instructions `allowed_signing`/`sm2_distid_default`) et réémet éventuellement l'inventaire des clés JSON pour les scripts.
- `iroha_cli tools crypto sm3 hash --data <string>` hache les charges utiles arbitraires ; `--data-hex` / `--file` couvrent les entrées binaires et la commande signale les résumés hexadécimaux et base64 pour les outils de manifeste.
- `iroha_cli tools crypto sm4 gcm-seal --key-hex <KEY> --nonce-hex <NONCE> --plaintext-hex <PT>` (et `gcm-open`) encapsulent les assistants SM4-GCM de l'hôte et font apparaître les charges utiles `ciphertext_hex`/`tag_hex` ou en texte brut. `sm4 ccm-seal` / `sm4 ccm-open` fournissent la même UX pour CCM avec une validation de longueur de nonce (7 à 13 octets) et de longueur de balise (4,6,8,10,12,14,16) intégrée ; les deux commandes émettent éventuellement des octets bruts sur le disque.## Stratégie de test
### Tests unitaires/à réponses connues
- Vecteurs GM/T 0004 & GB/T 32905 pour SM3 (par exemple, `"abc"`).
- Vecteurs GM/T 0002 & RFC 8998 pour SM4 (bloc + GCM/CCM).
- Exemples GM/T 0003/GB/T 32918 pour SM2 (valeur Z, vérification de signature), y compris l'exemple 1 de l'annexe avec l'ID `ALICE123@YAHOO.COM`.
- Dossier intermédiaire de montage : `crates/iroha_crypto/tests/fixtures/sm_known_answers.toml`.
- La suite de régression SM2 dérivée de Wycheproof (`crates/iroha_crypto/tests/sm2_wycheproof.rs`) comporte désormais un corpus de 52 cas qui superpose des appareils déterministes (annexe D, graines du SDK) avec des négatifs de bit-flip, de falsification de message et de signature tronquée. Le JSON nettoyé se trouve dans `crates/iroha_crypto/tests/fixtures/wycheproof_sm2.json`, et `sm2_fuzz.rs` le consomme directement afin que les scénarios de chemin heureux et de falsification restent alignés lors des exécutions de fuzz/propriétés. Il s'agit d'une annexe de l'annexe `Sm2PublicKey` de BigInt. Je suis en train de le faire.
- `cargo xtask sm-wycheproof-sync --input <wycheproof-sm2.json>` (ou `--input-url <https://…>`) coupe de manière déterministe toute chute en amont (étiquette de générateur en option) et réécrit `crates/iroha_crypto/tests/fixtures/wycheproof_sm2.json`. Jusqu'à ce que C2SP publie le corpus officiel, téléchargez les forks manuellement et alimentez-les via l'assistant ; il normalise les clés, les décomptes et les indicateurs afin que les réviseurs puissent raisonner sur les différences.
- Aller-retours SM2/SM3 Norito validés en `crates/iroha_data_model/tests/sm_norito_roundtrip.rs`.
- Régression des appels système de l'hôte SM3 dans `crates/ivm/tests/sm_syscalls.rs` (fonctionnalité SM).
- SM2 vérifie la régression des appels système dans `crates/ivm/tests/sm_syscalls.rs` (cas de réussite + échec).

### Tests de propriété et de régression
- Proptest pour SM2 rejetant les courbes invalides, les r/s non canoniques et la réutilisation des noms occasionnels. *(Disponible dans `crates/iroha_crypto/tests/sm2_fuzz.rs`, fermé derrière `sm_proptest` ; activé via `cargo test -p iroha_crypto --features "sm sm_proptest"`.)*
- Vecteurs Wycheproof SM4 (mode bloc/AES) adaptés à des modes variés ; suivre en amont les ajouts de SM2. `sm3_sm4_vectors.rs` exerce désormais des retournements de bits de balises, des balises tronquées et une falsification de texte chiffré pour GCM et CCM.

### Interopérabilité et performances
- RustCrypto ↔ Suite de parité OpenSSL/Tongsuo pour la signature/vérification SM2, les résumés SM3 et SM4 ECB/GCM réside dans `crates/iroha_crypto/tests/sm_cli_matrix.rs` ; invoquez-le avec `scripts/sm_interop_matrix.sh`. Les vecteurs de parité CCM s'exécutent désormais dans `sm3_sm4_vectors.rs` ; La prise en charge de la matrice CLI suivra une fois que les CLI en amont exposeront les assistants CCM.
- L'assistant SM3 NEON exécute désormais le chemin de compression/rembourrage Armv8 de bout en bout avec un contrôle d'exécution via `sm_accel::is_sm3_enabled` (fonctionnalité + remplacements d'environnement reflétés sur SM3/SM4). Les résumés dorés (zéro/`"abc"`/bloc long + longueurs aléatoires) et les tests de désactivation forcée maintiennent la parité avec le backend scalaire RustCrypto, et le micro-bench Criterion (`crates/sm3_neon/benches/digest.rs`) capture le débit scalaire vs NEON sur les hôtes AArch64.
- Harnais de performance miroir `scripts/gost_bench.sh` pour comparer Ed25519/SHA-2 vs SM2/SM3/SM4 et valider les seuils de tolérance.#### Arm64 Baseline (Apple Silicon local ; critère `sm_perf`, actualisé le 05/12/2025)
- `scripts/sm_perf.sh` exécute désormais le banc Criterion et applique les médianes par rapport à `crates/iroha_crypto/benches/sm_perf_baseline.json` (enregistré sur aarch64 macOS ; tolérance de 25 % par défaut, les métadonnées de base capturent le triple hôte). Le nouvel indicateur `--mode` permet aux ingénieurs de capturer des points de données scalaires, NEON et `sm-neon-force` sans modifier le script ; le bundle de capture actuel (JSON brut + résumé agrégé) réside sous `artifacts/sm_perf/2026-03-lab/m3pro_native/` et tamponne chaque charge utile avec `cpu_label="m3-pro-native"`.
- Les modes d'accélération sélectionnent désormais automatiquement la ligne de base scalaire comme cible de comparaison. `scripts/sm_perf.sh` fait passer `--compare-baseline/--compare-tolerance/--compare-label` à `sm_perf_check`, émettant des deltas par référence par rapport à la référence scalaire et échouant lorsque le ralentissement dépasse le seuil configuré. Les tolérances par référence par rapport à la ligne de base déterminent la garde de comparaison (SM3 est plafonné à 12 % sur la ligne de base scalaire d'Apple, tandis que le delta de comparaison SM3 autorise désormais jusqu'à 70 % par rapport à la référence scalaire pour éviter les battements) ; les lignes de base Linux réutilisent la même carte de comparaison car elles sont exportées à partir de la capture `neoverse-proxy-macos`, et nous les resserrerons après une exécution Neoverse nue si les médianes diffèrent. Transmettez explicitement `--compare-tolerance` lors de la capture de limites plus strictes (par exemple, `--compare-tolerance 0.20`) et utilisez `--compare-label` pour annoter des hôtes de référence alternatifs.
- Les lignes de base enregistrées sur la machine de référence CI sont désormais disponibles dans `crates/iroha_crypto/benches/sm_perf_baseline_aarch64_macos_scalar.json`, `sm_perf_baseline_aarch64_macos_auto.json` et `sm_perf_baseline_aarch64_macos_neon_force.json`. Actualisez-les avec `scripts/sm_perf.sh --mode scalar --write-baseline`, `--mode auto --write-baseline` ou `--mode neon-force --write-baseline` (définissez `SM_PERF_CPU_LABEL` avant la capture) et archivez le JSON généré avec les journaux d'exécution. Conservez la sortie d'assistance agrégée (`artifacts/.../aggregated.json`) avec le PR afin que les réviseurs puissent auditer chaque échantillon. Les lignes de base Linux/Neoverse sont désormais livrées dans `sm_perf_baseline_aarch64_unknown_linux_gnu_{mode}.json`, promu à partir de `artifacts/sm_perf/2026-03-lab/neoverse-proxy-macos/aggregated.json` (étiquette CPU `neoverse-proxy-macos`, SM3 compare la tolérance 0,70 pour aarch64 macOS/Linux) ; réexécutez sur les hôtes Neoverse nus lorsqu'ils sont disponibles pour resserrer les tolérances.
- Les fichiers JSON de base peuvent désormais contenir un objet `tolerances` facultatif pour renforcer les garde-corps par benchmark. Exemple :
  ```json
  {
    "benchmarks": { "...": 12.34 },
    "tolerances": {
      "sm4_vs_chacha20poly1305_encrypt/sm4_gcm_encrypt": 0.08,
      "sm3_vs_sha256_hash/sm3_hash": 0.12
    }
  }
  ```
  `sm_perf_check` applique ces limites fractionnaires (8 % et 12 % dans l'exemple) tout en utilisant la tolérance CLI globale pour tous les benchmarks non répertoriés.
- Les gardes de comparaison peuvent également honorer `compare_tolerances` dans la référence de comparaison. Utilisez ceci pour permettre un delta plus lâche par rapport à la référence scalaire (par exemple, `\"sm3_vs_sha256_hash/sm3_hash\": 0.70` dans la ligne de base scalaire) tout en conservant le `tolerances` principal strict pour les vérifications directes de la ligne de base.- Les lignes de base Apple Silicon enregistrées sont désormais livrées avec des garde-corps en béton : les opérations SM2/SM4 permettent une dérive de 12 à 20 % en fonction de la variance, tandis que les comparaisons SM3/ChaCha se situent entre 8 et 12 %. La tolérance `sm3` de la ligne de base scalaire est désormais resserrée à 0,12 ; les fichiers `unknown_linux_gnu` reflètent l'exportation `neoverse-proxy-macos` avec la même carte de tolérance (SM3 comparé à 0,70) et des notes de métadonnées indiquant qu'ils sont expédiés pour la porte Linux jusqu'à ce qu'une réexécution Neoverse sans système d'exploitation soit disponible.
- Signature SM2 : 298 µs par opération (Ed25519 : 32 µs) ⇒ ~9,2× plus lente ; vérification : 267µs (Ed25519 : 41µs) ⇒ ~6,5× plus lent.
- Hachage SM3 (charge utile 4KiB) : 11,2µs, parité effective avec SHA-256 à 11,3µs (≈356MiB/s vs 353MiB/s).
- SM4-GCM sceau/ouvert (charge utile de 1 Ko, nonce de 12 octets) : 15,5 µs contre ChaCha20-Poly1305 à 1,78 µs (≈64 Mo/s contre 525 Mo/s).
- Artefacts de référence (`target/criterion/sm_perf*`) capturés pour la reproductibilité ; les lignes de base Linux proviennent de `artifacts/sm_perf/2026-03-lab/neoverse-proxy-macos/` (étiquette CPU `neoverse-proxy-macos`, tolérance de comparaison SM3 0,70) et peuvent être actualisées sur des hôtes Neoverse nus (`SM-4c.1`) une fois le temps de laboratoire ouvert pour resserrer les tolérances.

#### Liste de contrôle de capture inter-architectures
- Exécutez `scripts/sm_perf_capture_helper.sh` **sur la machine cible** (poste de travail x86_64, serveur Neoverse ARM, etc.). Transmettez `--cpu-label <host>` pour tamponner les captures et (lors de l'exécution en mode matrice) pour pré-remplir le plan/les commandes générés pour la planification du laboratoire. L'assistant imprime les commandes spécifiques au mode qui :
  1. exécutez la suite Criterion avec l'ensemble de fonctionnalités correct, et
  2. écrivez les médianes dans `crates/iroha_crypto/benches/sm_perf_baseline_${arch}_${os}_${mode}.json`.
- Capturez d'abord la ligne de base scalaire, puis réexécutez l'assistant pour `auto` (et `neon-force` sur les plates-formes AArch64). Utilisez un `SM_PERF_CPU_LABEL` significatif afin que les réviseurs puissent retracer les détails de l'hôte dans les métadonnées JSON.
- Après chaque exécution, archivez le répertoire brut `target/criterion/sm_perf*` et incluez-le dans le PR avec les lignes de base générées. Resserrez les tolérances par référence dès que deux exécutions consécutives se stabilisent (voir `sm_perf_baseline_aarch64_macos_*.json` pour le formatage de référence).
- Enregistrez les médianes + tolérances dans cette section et mettez à jour `status.md`/`roadmap.md` lorsqu'une nouvelle architecture est couverte. Les lignes de base Linux sont désormais archivées à partir de la capture `neoverse-proxy-macos` (les métadonnées notent l'exportation vers la porte aarch64-unknown-linux-gnu) ; réexécuter sur des hôtes Neoverse/x86_64 nus à titre de suivi lorsque ces emplacements de laboratoire sont disponibles.

#### ARMv8 SM3/SM4 intrinsèques vs chemins scalaires
`sm_accel` (voir `crates/iroha_crypto/src/sm.rs:739`) fournit la couche de répartition d'exécution pour les assistants SM3/SM4 pris en charge par NEON. La fonctionnalité est protégée à trois niveaux :| Couche | Contrôle | Remarques |
|-------|--------|-------|
| Temps de compilation | `--features sm` (extrait désormais automatiquement `sm-neon` sur `aarch64`) ou `sm-neon-force` (tests/benchmarks) | Construit les modules NEON et relie `sm3-neon`/`sm4-neon`. |
| Détection automatique de l'exécution | `sm4_neon::is_supported()` | Uniquement vrai sur les processeurs qui exposent des équivalents AES/PMULL (par exemple, Apple série M, Neoverse V1/N2). Les machines virtuelles qui masquent NEON ou FEAT_SM4 reviennent au code scalaire. |
| Remplacement de l'opérateur | `crypto.sm_intrinsics` (`auto`/`force-enable`/`force-disable`) | Répartition basée sur la configuration appliquée au démarrage ; utilisez `force-enable` uniquement pour le profilage dans des environnements de confiance et préférez `force-disable` lors de la validation des solutions de secours scalaires. |

**Enveloppe de performances (Apple M3 Pro ; médianes enregistrées dans `sm_perf_baseline_aarch64_macos_{mode}.json`) :**

| Mode | Résumé SM3 (4 Ko) | Sceau SM4-GCM (1 Ko) | Remarques |
|------|---------|------------|-------|
| Scalaire | 11,6 µs | 15,9 µs | Chemin déterministe de RustCrypto ; utilisé partout où la fonctionnalité `sm` est compilée mais NEON n'est pas disponible. |
| NÉON automobile | ~2,7 fois plus rapide que le scalaire | ~2,3 fois plus rapide que le scalaire | Les noyaux NEON actuels (SM-5a.2c) élargissent la planification de quatre mots à la fois et utilisent une répartition à double file d'attente ; les médianes exactes varient selon l'hôte, consultez donc les métadonnées JSON de base. |
| Force NÉON | Met en miroir NEON auto mais désactive entièrement le repli | Identique à NEON auto | Exercé via `scripts/sm_perf.sh --mode neon-force` ; maintient CI honnête même sur les hôtes qui passeraient par défaut en mode scalaire. |

**Déterminisme et conseils de déploiement**
- Les intrinsèques ne modifient jamais les résultats observables : `sm_accel` renvoie `None` lorsque le chemin accéléré n'est pas disponible et que l'assistant scalaire s'exécute. Les chemins de code consensuels restent donc déterministes tant que l’implémentation scalaire est correcte.
- Ne contrôlez **pas** la logique métier pour savoir si le chemin NEON a été utilisé. Traitez l'accélération uniquement comme un indice de performances et exposez l'état via la télémétrie uniquement (par exemple, jauge `sm_intrinsics_enabled`).
- Exécutez toujours `ci/check_sm_perf.sh` (ou `make check-sm-perf`) après avoir touché le code SM afin que le faisceau Criterion valide les chemins scalaires et accélérés en utilisant les tolérances intégrées dans chaque JSON de base.
- Lors de l'analyse comparative ou du débogage, préférez le bouton de configuration `crypto.sm_intrinsics` aux indicateurs de compilation ; la recompilation avec `sm-neon-force` désactive entièrement le repli scalaire, alors que `force-enable` pousse simplement la détection d'exécution.
- Documentez la politique choisie dans les notes de version : les versions de production doivent laisser la politique dans `Auto`, permettant à chaque validateur de découvrir les capacités matérielles indépendamment tout en partageant les mêmes artefacts binaires.
- Évitez d'expédier des fichiers binaires qui mélangent des éléments intrinsèques de fournisseurs liés statiquement (par exemple, des bibliothèques SM4 tierces) à moins qu'ils ne respectent le même flux de répartition et de test. Dans le cas contraire, les régressions de performances ne seront pas détectées par nos outils de base.#### x86_64 Base de référence Rosetta (Apple M3 Pro ; capturé le 01/12/2025)
- Les lignes de base résident dans `crates/iroha_crypto/benches/sm_perf_baseline_x86_64_macos_{scalar,auto,neon_force}.json` (cpu_label=`m3-pro-rosetta`), avec des captures brutes + agrégées sous `artifacts/sm_perf/2026-03-lab/m3pro_rosetta/`.
- Les tolérances par référence sur x86_64 sont définies à 20 % pour SM2, 15 % pour Ed25519/SHA-256 et 12 % pour SM4/ChaCha. `scripts/sm_perf.sh` définit désormais par défaut la tolérance de comparaison d'accélération à 25 % sur les hôtes non-AArch64 afin que scalaire contre auto reste serré tout en laissant la marge de 5,25 sur AArch64 pour la ligne de base `m3-pro-native` partagée jusqu'à ce qu'une réexécution de Neoverse arrive.

| Référence | Scalaire | Automobile | Néon-Force | Auto vs Scalaire | Néon vs Scalaire | Néon contre Auto |
|---------------|--------|------|------------|----------------|---------------|--------------|
| sm2_vs_ed25519_sign/ed25519_sign |    57.43 |  57.12 |      55,77 |          -0,53% |         -2,88% |        -2,36% |
| sm2_vs_ed25519_sign/sm2_sign |   572,76 | 568.71 |     557,83 |          -0,71% |         -2,61% |        -1,91% |
| sm2_vs_ed25519_verify/verify/ed25519 |    69.03 |  68.42 |      66.28 |          -0,88% |         -3,97% |        -3,12% |
| sm2_vs_ed25519_verify/verify/sm2 |   521.73 | 514,50 |     502.17 |          -1,38% |         -3,75% |        -2,40% |
| sm3_vs_sha256_hash/sha256_hash |    16h78 |  16h58 |      16h16 |          -1,19% |         -3,69% |        -2,52% |
| sm3_vs_sha256_hash/sm3_hash |    15h78 |  15h51 |      15.04 |          -1,71% |         -4,69% |        -3,03% |
| sm4_vs_chacha20poly1305_decrypt/chacha20poly1305_decrypt |     1,96 |   1,97 |       1,97 |           0,39% |          0,16% |        -0,23% |
| sm4_vs_chacha20poly1305_decrypt/sm4_gcm_decrypt |    16h26 |  16h38 |      16h26 |           0,72% |         -0,01% |        -0,72% |
| sm4_vs_chacha20poly1305_encrypt/chacha20poly1305_encrypt |     1,96 |   2h00 |       1,93 |           2,23% |         -1,14% |        -3,30% |
| sm4_vs_chacha20poly1305_encrypt/sm4_gcm_encrypt |    16h60 |  16h58 |      16h15 |          -0,10% |         -2,66% |        -2,57% |

#### x86_64 / autres cibles non-aarch64
- Les versions actuelles ne fournissent toujours que le chemin scalaire déterministe RustCrypto sur x86_64 ; gardez `sm` activé mais n'injectez **pas** de noyaux AVX2/VAES externes jusqu'à ce que SM-4c.1b atterrisse. La politique d'exécution reflète ARM : par défaut, `Auto`, honore `crypto.sm_intrinsics` et affiche les mêmes jauges de télémétrie.
- Les captures Linux/x86_64 restent à enregistrer ; réutilisez l'assistant sur ce matériel et déposez les médianes dans `sm_perf_baseline_x86_64_unknown_linux_gnu_{mode}.json` aux côtés des lignes de base Rosetta et de la carte de tolérance ci-dessus.**Pièges courants**
1. **Instances ARM virtualisées :** De nombreux cloud exposent NEON mais masquent les extensions SM4/AES vérifiées par `sm4_neon::is_supported()`. Attendez-vous au chemin scalaire dans ces environnements et capturez les lignes de base de performances en conséquence.
2. **Remplacements partiels :** Le mélange des valeurs `crypto.sm_intrinsics` persistantes entre les exécutions entraîne des lectures de performances incohérentes. Documentez le remplacement prévu dans le ticket d'expérience et réinitialisez la configuration avant de capturer de nouvelles lignes de base.
3. **Parité CI :** Certains exécuteurs macOS n'autorisent pas l'échantillonnage de performances basé sur un compteur lorsque NEON est actif. Conservez les sorties `scripts/sm_perf_capture_helper.sh` attachées aux PR afin que les réviseurs puissent confirmer que le chemin accéléré a été exercé même si le coureur cache ces compteurs.
4. **Futures variantes ISA (SVE/SVE2) :** Les noyaux actuels adoptent des formes de voie NEON. Avant le portage vers SVE/SVE2, étendez `sm_accel::NeonPolicy` avec une variante dédiée afin que nous puissions garder les boutons CI, télémétrie et opérateur alignés.

Les éléments d'action suivis sous SM-5a/SM-4c.1 garantissent que CI capture des preuves de parité pour chaque nouvelle architecture, et la feuille de route reste à 🈺 jusqu'à ce que les lignes de base Neoverse/x86 et les tolérances NEON-vs-scalaire convergent.

## Notes de conformité et de réglementation

### Normes et références normatives
- **GM/T 0002-2012** (SM4), **GM/T 0003-2012** + **GB/T 32918 series** (SM2), **GM/T 0004-2012** + **GB/T 32905/32907** (SM3) et **RFC 8998** régissent les définitions d'algorithmes, les vecteurs de test et les liaisons KDF que nos appareils consommer.【docs/source/crypto/sm_vectors.md#L79】
- La note de conformité `docs/source/crypto/sm_compliance_brief.md` relie ces normes aux responsabilités de dépôt/export des équipes d'ingénierie, SRE et juridiques ; gardez ce dossier à jour chaque fois que le catalogue GM/T est révisé.

### Flux de travail réglementaire en Chine continentale
1. **Dépôt du produit (开发备案) :** Avant d'expédier des binaires compatibles SM depuis la Chine continentale, soumettez le manifeste de l'artefact, les étapes de construction déterministes et la liste des dépendances à l'administration provinciale de cryptographie. Les modèles de dépôt et la liste de contrôle de conformité se trouvent dans `docs/source/crypto/sm_compliance_brief.md` et dans le répertoire des pièces jointes (`sm_product_filing_template.md`, `sm_sales_usage_filing_template.md`, `sm_export_statement_template.md`).
2. **Dépôt de ventes/utilisation (销售/使用备案) :** Les opérateurs exécutant des nœuds compatibles SM à terre doivent enregistrer leur étendue de déploiement, leur posture de gestion des clés et leur plan de télémétrie. Joignez les manifestes signés ainsi que les instantanés de métriques `iroha_sm_*` lors du dépôt.
3. **Tests accrédités :** Les opérateurs d'infrastructures critiques peuvent exiger des rapports de laboratoire certifiés. Fournissez des scripts de construction reproductibles, des exportations SBOM et des artefacts Wycheproof/interop (voir ci-dessous) afin que les auditeurs en aval puissent reproduire les vecteurs sans modifier le code.
4. **Suivi du statut :** Enregistrez les dépôts terminés dans le ticket de sortie et `status.md` ; les dépôts manquants bloquent la promotion des pilotes de vérification uniquement aux pilotes de signature.### Posture d'exportation et de distribution
- Traitez les fichiers binaires compatibles SM comme des éléments contrôlés en vertu de la **US EAR Category 5 Part 2** et du **Règlement UE 2021/821 Annexe 1 (5D002)**. La publication des sources continue de bénéficier des exclusions open source/ENC, mais la redistribution vers des destinations sous embargo nécessite toujours un examen juridique.
- Les manifestes de version doivent regrouper une déclaration d'exportation faisant référence à la base ENC/TSU et répertorier les identifiants de build OpenSSL/Tongsuo si l'aperçu FFI est packagé.
- Préférer les emballages région-locale (par exemple, miroirs continentaux) lorsque les opérateurs ont besoin d'une distribution terrestre pour éviter les problèmes de transfert transfrontalier.

### Documentation et preuves de l'opérateur
- Associez cette note d'architecture à la liste de contrôle de déploiement dans `docs/source/crypto/sm_operator_rollout.md` et au guide de dépôt de conformité dans `docs/source/crypto/sm_compliance_brief.md`.
- Gardez le démarrage rapide de la genèse/de l'opérateur synchronisé sur `docs/genesis.md`, `docs/genesis.he.md` et `docs/genesis.ja.md` ; Dans le workflow CLI SM2/SM3, il s'agit de la source de vérité destinée à l'opérateur pour l'amorçage des manifestes `crypto`.
- Archivez la provenance OpenSSL/Tongsuo, la sortie `scripts/sm_openssl_smoke.sh` et les journaux de parité `scripts/sm_interop_matrix.sh` avec chaque ensemble de versions afin que les partenaires de conformité et d'audit disposent d'artefacts déterministes.
- Mettez à jour `status.md` chaque fois que la portée de la conformité change (nouvelles juridictions, dépôts terminés ou décisions d'exportation) pour que l'état du programme reste détectable.
- Suivez les examens de préparation par étapes (`SM-RR1` – `SM-RR3`) capturés dans `docs/source/release_dual_track_runbook.md` ; la promotion entre les phases de vérification uniquement, pilote et de signature GA nécessite les artefacts qui y sont énumérés.

## Recettes d'interopérabilité

### RustCrypto ↔ Matrice OpenSSL/Tongsuo
1. Assurez-vous que les CLI OpenSSL/Tongsuo sont disponibles (`IROHA_SM_CLI="openssl /opt/tongsuo/bin/openssl"` permet la sélection explicite des outils).
2. Exécutez `scripts/sm_interop_matrix.sh` ; il invoque `cargo test -p iroha_crypto --test sm_cli_matrix --features sm` et exerce la signature/vérification SM2, les résumés SM3 et les flux SM4 ECB/GCM sur chaque fournisseur, en ignorant toute CLI absente.【scripts/sm_interop_matrix.sh#L1】
3. Archivez les fichiers `target/debug/deps/sm_cli_matrix*.log` résultants avec les artefacts de version.

### Aperçu d'OpenSSL Smoke (Packaging Gate)
1. Installez les en-têtes de développement OpenSSL ≥3.0 et assurez-vous que `pkg-config` peut les localiser.
2. Exécutez `scripts/sm_openssl_smoke.sh` ; l'assistant exécute `cargo check`/`cargo test --test sm_openssl_smoke`, exerçant le hachage SM3, la vérification SM2 et les allers-retours SM4-GCM via le backend FFI (le harnais de test active explicitement l'aperçu).【scripts/sm_openssl_smoke.sh#L1】
3. Traitez tout échec non sauté comme un bloqueur de version ; capturez la sortie de la console pour les preuves d’audit.

### Actualisation déterministe des luminaires
- Régénérez les appareils SM (`sm_vectors.md`, `fixtures/sm/…`) avant chaque dépôt de conformité, puis réexécutez la matrice de parité et le faisceau de fumée afin que les auditeurs reçoivent de nouvelles transcriptions déterministes parallèlement aux dépôts.## Préparation de l'audit externe
- `docs/source/crypto/sm_audit_brief.md` regroupe le contexte, la portée, le calendrier et les contacts pour l'examen externe.
- Les artefacts d'audit se trouvent sous `docs/source/crypto/attachments/` (journal de fumée OpenSSL, instantané de l'arbre de chargement, exportation de métadonnées de fret, provenance de la boîte à outils) et `fuzz/sm_corpus_manifest.json` (graines de fuzz SM déterministes provenant de vecteurs de régression existants). Sur macOS, le journal de fumée enregistre actuellement une exécution ignorée car le cycle de dépendance de l'espace de travail empêche `cargo check` ; Les versions Linux sans le cycle exerceront pleinement le backend de prévisualisation.
- Distribué aux responsables de Crypto WG, Platform Ops, Security et Docs/DevRel le 2026-01-30 pour alignement avant l'envoi de la RFQ.

### Statut de la mission d'audit

- **Trail of Bits (pratique de cryptographie du CN)** — Énoncé des travaux exécuté le **2026-02-21**, coup d'envoi **2026-02-24**, fenêtre de travail sur le terrain **2026-02-24–2026-03-22**, rapport final attendu le **2026-04-15**. Point de contrôle de statut hebdomadaire tous les mercredis à 09h00 UTC avec le responsable du Crypto WG et la liaison avec l'ingénierie de sécurité. Voir [`sm_audit_brief.md`](sm_audit_brief.md#engagement-status) pour les contacts, les livrables et les pièces jointes de preuves.
- **NCC Group APAC (créneau de contingence)** — Réservé la fenêtre de mai 2026 comme examen de suivi/parallèle si des conclusions supplémentaires ou des demandes des régulateurs nécessitent un deuxième avis. Les détails de l’engagement et les crochets d’escalade sont enregistrés à côté de l’entrée Trail of Bits dans `sm_audit_brief.md`.

## Risques et atténuations

Registre complet : voir [`sm_risk_register.md`](sm_risk_register.md) pour plus de détails
notation de probabilité/impact, déclencheurs de surveillance et historique d’approbation. Le
Le résumé ci-dessous suit les principaux éléments évoqués lors de l'ingénierie de la version.
| Risque | Gravité | Propriétaire | Atténuation |
|------|----------|-------|------------|
| Absence d'audit externe pour les caisses RustCrypto SM | Élevé | GT sur la cryptographie | Contrat Trail of Bits/NCC Group, conserver uniquement la vérification jusqu'à ce que le rapport d'audit soit accepté. |
| Régressions déterministes occasionnelles dans les SDK | Élevé | Responsables du programme SDK | Partagez des appareils sur le SDK CI ; appliquer le codage canonique r∥s ; ajouter des tests d'intégration cross-SDK (suivis dans SM-3c). |
| Bogues spécifiques à ISA dans les intrinsèques | Moyen | GT sur les performances | Les fonctionnalités intrinsèques nécessitent une couverture CI sur ARM et maintiennent le repli logiciel. Matrice de validation matérielle conservée dans `sm_perf.md`. |
| L'ambiguïté en matière de conformité retarde l'adoption | Moyen | Documents et liaison juridique | Publier le dossier de conformité et la liste de contrôle de l'opérateur (SM-6a/SM-6b) avant l'AG ; recueillir des avis juridiques. Liste de contrôle de dépôt expédiée dans `sm_compliance_brief.md`. |
| Dérive du backend FFI avec les mises à jour du fournisseur | Moyen | Opérations de plateforme | Épinglez les versions du fournisseur, ajoutez des tests de parité, conservez l'opt-in du backend FFI jusqu'à ce que l'emballage se stabilise (SM-P3). |## Questions ouvertes / suivis
1. Sélectionnez des partenaires d'audit indépendants expérimentés avec les algorithmes SM dans Rust.
   - **Réponse (2026-02-24) :** Le cabinet de cryptographie CN de Trail of Bits a signé le SOW d'audit principal (coup d'envoi le 2026-02-24, livraison le 2026-04-15) et NCC Group APAC dispose d'un créneau d'urgence en mai afin que les régulateurs puissent demander un deuxième examen sans rouvrir les achats. La portée de l'engagement, les contacts et les listes de contrôle se trouvent dans [`sm_audit_brief.md`](sm_audit_brief.md#engagement-status) et sont reflétés dans `sm_audit_vendor_landscape.md`.
2. Continuer le suivi en amont pour un ensemble de données officiel Wycheproof SM2 ; l'espace de travail fournit actuellement une suite organisée de 52 cas (appareils déterministes + cas de sabotage synthétisés) et l'alimente dans `sm2_wycheproof.rs`/`sm2_fuzz.rs`. Mettez à jour le corpus via `cargo xtask sm-wycheproof-sync` une fois le JSON en amont atterri.
   - Suivre les suites de vecteurs négatifs Bouncy Castle et GmSSL ; importer dans `sm2_fuzz.rs` une fois la licence obtenue pour compléter le corpus existant.
3. Définissez la télémétrie de base (métriques, journalisation) pour la surveillance de l'adoption de SM.
4. Décidez si la valeur par défaut de SM4 AEAD est GCM ou CCM pour l'exposition Kotodama/VM.
5. Suivez la parité RustCrypto/OpenSSL pour l'exemple 1 de l'annexe (ID `ALICE123@YAHOO.COM`) : confirmez la prise en charge de la bibliothèque pour la clé publique publiée et `(r, s)` afin que les appareils puissent être promus aux tests de régression.

## Éléments d'action
- [x] Finaliser l'audit des dépendances et la capture dans le tracker de sécurité.
- [x] Confirmer l'engagement du partenaire d'audit pour les caisses RustCrypto SM (suivi SM-P0). Trail of Bits (pratique de cryptographie du CN) est propriétaire de l'examen principal avec des dates de lancement/livraison enregistrées dans `sm_audit_brief.md`, et NCC Group APAC a retenu un créneau d'urgence en mai 2026 pour satisfaire aux suivis des régulateurs ou de la gouvernance.
- [x] Extension de la couverture Wycheproof pour les boîtiers d'inviolabilité SM4 CCM (SM-4a).
- [x] Atterrissez les appareils de signature canoniques SM2 sur les SDK et connectez-les à CI (SM-3c/SM-1b.1) ; gardé par `scripts/check_sm2_sdk_fixtures.py` (voir `ci/check_sm2_sdk_fixtures.sh`).

## Annexe de conformité (cryptographie commerciale d'État)

- **Classification :** Navire SM2/SM3/SM4 sous le régime chinois de *cryptographie commerciale d'État* (loi sur la cryptographie de la RPC, art. 3). L'envoi de ces algorithmes dans le logiciel Iroha ne place **pas** le projet dans les niveaux principaux/communs (secret d'État), mais les opérateurs qui les utilisent dans les déploiements en RPC doivent respecter les obligations de dépôt de crypto-monnaie commerciale et de MLPS.
- **Lignée des normes :** Alignez la documentation publique avec les conversions GB/T officielles des spécifications GM/T :

| Algorithme | Référence GB/T | Origine OGM/T | Remarques |
|---------------|----------------|-------------|-------|
| SM2 | GB/T32918 (toutes les pièces) | GM/T0003 | Signature numérique ECC + échange de clés ; Iroha expose la vérification dans les nœuds principaux et la signature déterministe aux SDK. |
| SM3 | GB/T32905 | GM/T0004 | Hachage de 256 bits ; hachage déterministe sur des chemins accélérés scalaires et ARMv8. |
| SM4 | GB/T32907 | GM/T0002 | Chiffrement par bloc de 128 bits ; Iroha fournit des assistants GCM/CCM et garantit la parité big-endian entre les implémentations. |- **Manifeste de capacité :** Le point de terminaison Torii `/v2/node/capabilities` annonce la forme JSON suivante afin que les opérateurs et les outils puissent utiliser le manifeste SM par programme :

```json
{
  "supported_abi_versions": [1],
  "default_compile_target": 1,
  "data_model_version": 1,
  "crypto": {
    "sm": {
      "enabled": true,
      "default_hash": "sm3-256",
      "allowed_signing": ["ed25519"],
      "sm2_distid_default": "1234567812345678",
      "openssl_preview": false,
      "acceleration": {
        "scalar": true,
        "neon_sm3": false,
        "neon_sm4": false,
        "policy": "auto"
      }
    }
  }
}
```

La sous-commande CLI `iroha runtime capabilities` fait apparaître la même charge utile localement, en imprimant un résumé d'une ligne à côté de la publicité JSON pour la collecte de preuves de conformité.

- **Livrables de documentation :** publier des notes de version et des SBOM qui identifient les algorithmes/normes ci-dessus, et conserver le dossier de conformité complet (`sm_chinese_crypto_law_brief.md`) avec les artefacts de version afin que les opérateurs puissent le joindre aux documents déposés par les provinces.【docs/source/crypto/sm_chinese_crypto_law_brief.md:59】
- **Transfert de l'opérateur :** rappelle aux déployeurs que MLPS2.0/GB/T39786-2021 nécessite des évaluations d'applications de chiffrement, des SOP de gestion des clés SM et une conservation des preuves ≥ 6 ans ; dirigez-les vers la liste de contrôle de l'opérateur dans le dossier de conformité.

## Plan de communication
- **Public :** Membres principaux du Crypto WG, Release Engineering, comité d'examen de la sécurité, responsables du programme SDK.
- **Artefacts :** `sm_program.md`, `sm_lock_refresh_plan.md`, `sm_vectors.md`, `sm_wg_sync_template.md`, extrait de feuille de route (SM-0 .. SM-7a).
- **Canal :** Agenda hebdomadaire de synchronisation du Crypto WG + e-mail de suivi résumant les éléments d'action et demandant l'approbation de l'actualisation du verrouillage et de la prise en compte des dépendances (projet diffusé le 19/01/2025).
- **Propriétaire :** Responsable du groupe de travail Crypto (délégué acceptable).