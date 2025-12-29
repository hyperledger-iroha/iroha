---
lang: fr
direction: ltr
source: docs/norito_bridge_release.md
status: complete
translator: manual
source_hash: bc7c766ff5fb0504f4da43a017bf294758800b9c815affc8f97b9bcc94ae8e15
source_last_modified: "2025-11-02T04:40:28.805628+00:00"
translation_last_reviewed: 2025-11-14
---

<!-- Traduction française de docs/norito_bridge_release.md (NoritoBridge Release Packaging) -->

# Packaging de release NoritoBridge

Ce guide décrit les étapes nécessaires pour publier les bindings Swift `NoritoBridge` sous
forme de XCFramework consommable via Swift Package Manager et CocoaPods. Le workflow
maintient les artefacts Swift strictement alignés sur les releases du crate Rust qui
embarque le codec Norito d’Iroha. Pour un tutoriel de bout en bout sur la consommation de
ces artefacts dans une application (câblage Xcode, usage de ChaChaPoly, etc.), voir
`docs/connect_swift_integration.md`.

> **Note :** L’automatisation CI pour ce flux sera ajoutée une fois que des builders macOS
> avec l’outillage Apple requis seront disponibles (suivi dans le backlog des builders
> macOS de Release Engineering). D’ici là, les étapes ci‑dessous doivent être exécutées
> manuellement sur un Mac de développement.

## Prérequis

- Machine macOS avec la dernière version stable des Xcode Command Line Tools installée.
- Toolchain Rust conforme à `rust-toolchain.toml` à la racine du workspace.
- Toolchain Swift 5.7 ou plus récent.
- CocoaPods (via Ruby gems) si publication dans le dépôt central de specs.
- Accès aux clés de signature de release Hyperledger Iroha pour taguer les artefacts
  Swift.

## Modèle de versionnage

1. Déterminer la version du crate Rust pour le codec Norito (`crates/norito/Cargo.toml`).
2. Taguer le workspace avec l’identifiant de release (par ex. `v2.1.0`).
3. Utiliser la même version sémantique pour le package Swift et le podspec CocoaPods.
4. Lorsque le crate Rust incrémente sa version, répéter le processus et publier un
   artefact Swift correspondant. Pendant les tests, les versions peuvent inclure un
   suffixe de metadata (par ex. `-alpha.1`).

## Étapes de build

1. Depuis la racine du repository, appeler le script helper pour assembler le XCFramework :

   ```bash
   ./scripts/build_norito_xcframework.sh --workspace-root "$(pwd)" \
       --output "artifacts/NoritoBridge.xcframework" \
       --profile release
   ```

   Le script compile la bibliothèque bridge Rust pour les cibles iOS et macOS, puis
   regroupe les bibliothèques statiques résultantes dans un répertoire XCFramework unique.

2. Compresser le XCFramework pour distribution :

   ```bash
   ditto -c -k --sequesterRsrc --keepParent \
     artifacts/NoritoBridge.xcframework \
     artifacts/NoritoBridge.xcframework.zip
   ```

3. Mettre à jour le manifest du package Swift (`IrohaSwift/Package.swift`) pour pointer
   vers la nouvelle version et son checksum :

   ```bash
   swift package compute-checksum artifacts/NoritoBridge.xcframework.zip
   ```

   Reporter ce checksum dans `Package.swift` lors de la définition de la binary target.

4. Mettre à jour `IrohaSwift/IrohaSwift.podspec` avec la nouvelle version, le checksum et
   l’URL de l’archive.

5. **Régénérer les headers si le bridge expose de nouveaux symboles.** Le bridge Swift
   expose désormais `connect_norito_set_acceleration_config` afin que
   `AccelerationSettings` puisse activer les backends Metal/GPU. Vérifier que
   `NoritoBridge.xcframework/**/Headers/connect_norito_bridge.h` reflète bien
   `crates/connect_norito_bridge/include/connect_norito_bridge.h` avant la compression.

6. Exécuter la suite de validation Swift avant de créer la tag :

   ```bash
   swift test --package-path IrohaSwift
   make swift-ci
   ```

   La première commande garantit que le package Swift (incluant `AccelerationSettings`)
   reste vert ; la seconde valide la parité des fixtures, génère les dashboards de
   parité/CI et applique les mêmes contrôles de télémétrie que ceux de Buildkite (y
   compris le metadata `ci/xcframework-smoke:<lane>:device_tag`).

7. Commiter les artefacts générés dans une branche de release, puis taguer le commit.

## Publication

### Swift Package Manager

- Pousser la tag vers le repository Git public.
- Vérifier que la tag est accessible via l’index de packages (Apple ou miroir
  communautaire).
- Les consommateurs peuvent désormais déclarer une dépendance :
  `.package(url: "https://github.com/hyperledger/iroha", from: "<version>")`.

### CocoaPods

1. Valider le pod localement :

   ```bash
   pod lib lint IrohaSwift.podspec --allow-warnings
   ```

2. Pousser le podspec à jour :

   ```bash
   pod trunk push IrohaSwift.podspec
   ```

3. Confirmer que la nouvelle version apparaît dans l’index CocoaPods.

## Considérations CI

- Créer un job macOS qui exécute le script de packaging, archive les artefacts et publie
  le checksum généré comme output de workflow.
- Conditionner les releases à la capacité de l’application de démonstration Swift à
  construire contre le framework fraîchement produit.
- Conserver les logs de build pour faciliter le diagnostic en cas d’échec.

## Idées d’automatisation supplémentaires

- Utiliser `xcodebuild -create-xcframework` directement lorsque toutes les cibles
  nécessaires sont exposées.
- Intégrer la signature/notarisation pour une distribution en dehors des machines de
  développement.
- Garder les tests d’intégration synchronisés avec la version packagée en fixant la
  dépendance SPM sur la tag de release.

