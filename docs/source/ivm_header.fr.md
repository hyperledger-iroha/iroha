---
lang: fr
direction: ltr
source: docs/source/ivm_header.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 779174437b1a7e57b371d3b41d1cab780d94700acf6642b1356cdb75504ae5fa
source_last_modified: "2026-01-21T10:30:30.084677+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# IVM En-tête de bytecode


Magie
- 4 octets : ASCII `IVM\0` au offset 0.

Mise en page (actuelle)
- Décalages et tailles (17 octets au total) :
  - 0..4 : magique `IVM\0`
  - 4 : `version_major: u8`
  - 5 : `version_minor: u8`
  - 6 : `mode: u8` (bits de fonctionnalité ; voir ci-dessous)
  - 7 : `vector_length: u8`
  - 8..16 : `max_cycles: u64` (petit-boutiste)
  - 16 : `abi_version: u8`

Bits de mode
- `ZK = 0x01`, `VECTOR = 0x02`, `HTM = 0x04` (réservé/fonctionnel).

Champs (signification)
- `abi_version` : table d'appel système et version du schéma pointeur‑ABI.
- `mode` : bits de fonctionnalité pour le traçage ZK/VECTOR/HTM.
- `vector_length` : longueur de vecteur logique pour les opérations vectorielles (0 → non défini).
- `max_cycles` : limite de remplissage d'exécution utilisée en mode ZK et admission.

Remarques
- L'endianisme et la mise en page sont définis par l'implémentation et liés à `version`. La disposition filaire ci-dessus reflète l'implémentation actuelle dans `crates/ivm_abi/src/metadata.rs`.
- Un lecteur minimal peut s'appuyer sur cette disposition pour les artefacts actuels et doit gérer les modifications futures via le portail `version`.
- L'accélération matérielle (SIMD/Metal/CUDA) est optionnelle par hôte. Le moteur d'exécution lit les valeurs `AccelerationConfig` à partir de `iroha_config` : `enable_simd` force les replis scalaires lorsqu'ils sont faux, tandis que `enable_metal` et `enable_cuda` contrôlent leurs backends respectifs même lorsqu'ils sont compilés. Ces bascules sont appliquées via `ivm::set_acceleration_config`. avant la création de la VM.
- Les SDK mobiles (Android/Swift) présentent les mêmes boutons ; `IrohaSwift.AccelerationSettings`
  appelle `connect_norito_set_acceleration_config` pour que les versions macOS/iOS puissent opter pour Metal /
  NEON tout en gardant des replis déterministes.
- Les opérateurs peuvent également forcer la désactivation de backends spécifiques pour les diagnostics en exportant `IVM_DISABLE_METAL=1` ou `IVM_DISABLE_CUDA=1`. Ces remplacements d'environnement ont priorité sur la configuration et maintiennent la VM sur le chemin déterministe du processeur.

Aides d'état durables et surface ABI
- Les appels système d'assistance d'état durable (0x50–0x5A : STATE_{GET,SET,DEL}, ENCODE/DECODE_INT, BUILD_PATH_* et JSON/SCHEMA encode/decode) font partie de l'ABI V1 et sont inclus dans le calcul `abi_hash`.
- CoreHost connecte STATE_{GET,SET,DEL} à l'état de contrat intelligent durable soutenu par WSV ; Les hôtes de développement/test peuvent utiliser des superpositions ou une persistance locale mais doivent conserver le même comportement observable.

Validation
- L'admission de nœud accepte uniquement les en-têtes `version_major = 1` et `version_minor = 0`.
- `mode` ne doit contenir que des bits connus : `ZK`, `VECTOR`, `HTM` (les bits inconnus sont rejetés).
- `vector_length` est consultatif et peut être différent de zéro même si le bit `VECTOR` n'est pas activé ; l'admission impose uniquement une limite supérieure.
- Valeurs `abi_version` prises en charge : la première version accepte uniquement `1` (V1) ; les autres valeurs sont rejetées à l'admission.

### Politique (générée)
Le résumé de la politique suivant est généré à partir de la mise en œuvre et ne doit pas être modifié manuellement.<!-- BEGIN GENERATED HEADER POLICY -->
| Champ | Politique |
|---|---|
| version_major | 1 |
| version_mineure | 0 |
| mode (bits connus) | 0x07 (ZK=0x01, VECTEUR=0x02, HTM=0x04) |
| abi_version | 1 |
| longueur_vecteur | 0 ou 1..=64 (consultatif ; indépendant du bit VECTOR) |
<!-- END GENERATED HEADER POLICY -->

### Hachages ABI (générés)
Le tableau suivant est généré à partir de l'implémentation et répertorie les valeurs canoniques `abi_hash` pour les stratégies prises en charge.

<!-- BEGIN GENERATED ABI HASHES -->
| Politique | abi_hash (hexadécimal) |
|---|---|
| ABI v1 | ba1786031c3d0cdbd607debdae1cc611a0807bf9cf49ed349a0632855724969f |
<!-- END GENERATED ABI HASHES -->

- Des mises à jour mineures peuvent ajouter des instructions derrière `feature_bits` et un espace opcode réservé ; les mises à jour majeures peuvent modifier les codages ou supprimer/réutiliser uniquement avec une mise à niveau du protocole.
- Les plages Syscall sont stables ; inconnu pour le `abi_version` actif donne `E_SCALL_UNKNOWN`.
- Les horaires de gaz sont liés au `version` et nécessitent des vecteurs dorés lors du changement.

Inspection des artefacts
- Utilisez `ivm_tool inspect <file.to>` pour une vue stable des champs d'en-tête.
- Pour le développement, des exemples/incluent une petite cible Makefile `examples-inspect` qui exécute une inspection sur les artefacts construits.

Exemple (Rust) : magie minimale + contrôle de taille

```rust
use std::fs::File;
use std::io::{Read};

fn is_ivm_artifact(path: &std::path::Path) -> std::io::Result<bool> {
    let mut f = File::open(path)?;
    let mut magic = [0u8; 4];
    if f.read(&mut magic)? != 4 { return Ok(false); }
    if &magic != b"IVM\0" { return Ok(false); }
    let meta = std::fs::metadata(path)?;
    Ok(meta.len() >= 64)
}
```

Remarque : La disposition exacte de l'en-tête au-delà de la magie est versionnée et définie par l'implémentation ; préférez `ivm_tool inspect` pour les noms et valeurs de champs stables.