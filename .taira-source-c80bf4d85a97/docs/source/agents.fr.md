---
lang: fr
direction: ltr
source: docs/source/agents.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 7f35a28d00188a3e1f3db76b56e6b29c708dbb75afa3dd009d416b7cd4314754
source_last_modified: "2026-01-03T18:08:01.361022+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Guide d'exécution de l'agent d'automatisation

Cette page résume les garde-fous opérationnels pour tout agent d'automatisation
travaillant dans l’espace de travail Hyperledger Iroha. Cela reflète le canonique
Les conseils `AGENTS.md` et les références de la feuille de route pour la construction, la documentation et
les changements télémétriques se ressemblent tous, qu'ils aient été produits par un humain ou
un contributeur automatisé.

Chaque tâche devrait générer du code déterministe ainsi que des documents, tests,
et des preuves opérationnelles. Considérez les sections ci-dessous comme une référence prête avant
toucher les éléments `roadmap.md` ou répondre à des questions de comportement.

## Commandes de démarrage rapide

| Actions | Commande |
|--------|---------|
| Construire l'espace de travail | `cargo build --workspace` |
| Exécutez la suite de tests complète | `cargo test --workspace` *(prend généralement plusieurs heures)* |
| Exécutez Clippy avec des avertissements de refus par défaut | `cargo clippy --workspace --all-targets -- -D warnings` |
| Formater le code Rust | `cargo fmt --all` *(édition 2024)* |
| Testez une seule caisse | `cargo test -p <crate>` |
| Exécuter un test | `cargo test -p <crate> <test_name> -- --nocapture` |
| Tests du SDK Swift | À partir de `IrohaSwift/`, exécutez `swift test` |

## Principes fondamentaux du flux de travail

- Lisez les chemins de code pertinents avant de répondre aux questions ou de modifier la logique.
- Divisez les éléments volumineux de la feuille de route en commits traitables ; ne rejetez jamais catégoriquement le travail.
- Restez dans l'adhésion à l'espace de travail existant, réutilisez les caisses internes et faites
  **ne pas** modifier `Cargo.lock`, sauf indication explicite.
- Utilisez les indicateurs de fonctionnalités et les bascules de capacités uniquement lorsque le matériel l'exige
  accélérateurs; gardez des solutions de secours déterministes disponibles sur chaque plateforme.
- Mettre à jour la documentation et les références Markdown parallèlement à tout changement fonctionnel
  donc les documents décrivent toujours le comportement actuel.
- Ajoutez au moins un test unitaire pour chaque fonction nouvelle ou modifiée. Préférez en ligne
  Modules `#[cfg(test)]` ou dossier `tests/` de la caisse selon la portée.
- Une fois le travail terminé, mettez à jour `status.md` avec un bref résumé et une référence.
  fichiers pertinents ; gardez `roadmap.md` concentré sur les éléments qui nécessitent encore du travail.

## Garde-fous de mise en œuvre

### Sérialisation et modèles de données
- Utiliser partout le codec Norito (binaire via `norito::{Encode, Decode}`,
  JSON via `norito::json::*`). N’ajoutez pas d’utilisation directe serde/`serde_json`.
- Les charges utiles Norito doivent annoncer leur disposition (octet de version ou indicateurs d'en-tête),
  et les nouveaux formats nécessitent des mises à jour de documentation correspondantes (par exemple,
  `norito.md`, `docs/source/da/*.md`).
- Les données Genesis, les manifestes et les charges utiles de mise en réseau doivent rester déterministes
  ainsi deux pairs avec les mêmes entrées produisent des hachages identiques.

### Configuration et comportement d'exécution
- Préférez les boutons résidant dans `crates/iroha_config` aux nouvelles variables d'environnement.
  Thread valeurs explicitement via des constructeurs ou une injection de dépendances.
- Ne déclenchez jamais les appels système IVM ou le comportement de l'opcode : ABI v1 est livré partout.
- Lorsque de nouvelles options de configuration sont ajoutées, mettez à jour les valeurs par défaut, la documentation et tout élément associé.
  modèles (`peer.template.toml`, `docs/source/configuration*.md`, etc.).### ABI, appels système et types de pointeurs
- Traitez la politique d'ABI comme inconditionnelle. Ajout/suppression d'appels système ou de types de pointeurs
  nécessite une mise à jour :
  -`ivm::syscalls::abi_syscall_list` et `crates/ivm/tests/abi_syscall_list_golden.rs`
  - `ivm::pointer_abi::PointerType` plus les tests d'or
  - `crates/ivm/tests/abi_hash_versions.rs` chaque fois que le hachage ABI change
- Les appels système inconnus doivent correspondre à `VMError::UnknownSyscall` et les manifestes doivent
  conserver les contrôles d'égalité `abi_hash` signés dans les tests d'admission.

### Accélération matérielle et déterminisme
- Les nouvelles primitives cryptographiques ou les calculs mathématiques lourds doivent être livrés avec une accélération matérielle
  chemins (METAL/NEON/SIMD/CUDA) tout en conservant des replis déterministes.
- Eviter les réductions parallèles non déterministes ; la priorité est des sorties identiques sur
  chaque homologue, même lorsque le matériel diffère.
- Gardez les appareils Norito et FASTPQ reproductibles afin que SRE puisse auditer l'ensemble de la flotte
  télémétrie.

### Documentation et preuves
- Mettre en miroir toute modification de document destinée au public dans le portail (`docs/portal/...`) lorsque
  applicable afin que le site de documentation reste à jour avec les sources Markdown.
- Lorsque de nouveaux workflows sont introduits, ajoutez des runbooks, des notes de gouvernance ou
  des listes de contrôle expliquant comment répéter, annuler et capturer des preuves.
- Lors de la traduction de contenu en akkadien, fournir des rendus sémantiques écrits
  en translittérations cunéiformes plutôt que phonétiques.

### Attentes en matière de tests et d'outillage
- Exécuter localement les suites de tests pertinentes (`cargo test`, `swift test`,
  harnais d'intégration) et documentez les commandes dans la section de tests PR.
- Gardez les scripts CI guard (`ci/*.sh`) et les tableaux de bord synchronisés avec la nouvelle télémétrie.
- Pour les macros proc, associez les tests unitaires aux tests de l'interface utilisateur `trybuild` pour verrouiller les diagnostics.

## Liste de contrôle prête à être expédiée

1. Le code se compile et `cargo fmt` n'a produit aucune différence.
2. Les documents mis à jour (espace de travail Markdown et miroirs de portail) décrivent le nouveau
   comportement, de nouveaux indicateurs CLI ou des boutons de configuration.
3. Les tests couvrent chaque nouveau chemin de code et échouent de manière déterministe en cas de régression
   apparaître.
4. La télémétrie, les tableaux de bord et les définitions d'alerte font référence à toute nouvelle métrique ou
   codes d'erreur.
5. `status.md` comprend un bref résumé faisant référence aux fichiers pertinents et
   section feuille de route.

Le respect de cette liste de contrôle permet de vérifier l'exécution de la feuille de route et garantit que chaque
L’agent apporte des preuves auxquelles les autres équipes peuvent faire confiance.