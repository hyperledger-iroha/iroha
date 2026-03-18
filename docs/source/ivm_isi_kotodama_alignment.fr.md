---
lang: fr
direction: ltr
source: docs/source/ivm_isi_kotodama_alignment.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 3f40329b9968530dea38745b49f7fee4d55aeb461e515e6f97b5b5986cb27e3f
source_last_modified: "2026-01-21T10:20:35.513444+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# IVM ⇄ ISI ⇄ Modèle de données ⇄ Kotodama — Révision de l'alignement

Ce document vérifie comment le jeu d'instructions de la machine virtuelle Iroha (IVM) et la surface d'appel système sont mappés aux instructions spéciales (ISI) Iroha et `iroha_data_model`, et comment Kotodama se compile dans cette pile. Il identifie les lacunes actuelles et propose des améliorations concrètes afin que les quatre couches s'emboîtent de manière déterministe et ergonomique.

Remarque sur la cible du bytecode : les contrats intelligents Kotodama se compilent en bytecode de la machine virtuelle Iroha (IVM) (`.to`). Ils ne ciblent pas « risc5 »/RISC‑V en tant qu'architecture autonome. Tous les codages de type RISC‑V référencés ici font partie du format d'instruction mixte de IVM et restent un détail d'implémentation.

## Portée et sources
- IVM : `crates/ivm/src/{instruction.rs,ivm.rs,syscalls.rs,host.rs,mock_wsv.rs}` et `crates/ivm/docs/*`.
- ISI/Modèle de données : `crates/iroha_data_model/src/isi/*`, `crates/iroha_core/src/smartcontracts/isi/*` et documents `docs/source/data_model_and_isi_spec.md`.
- Kotodama : `crates/kotodama_lang/src/*`, documents dans `crates/ivm/docs/*`.
- Intégration de base : `crates/iroha_core/src/{state.rs,executor.rs,smartcontracts/ivm/cache.rs}`.

Terminologie
- « ISI » fait référence aux types d'instructions intégrés qui modifient l'état du monde via l'exécuteur (par exemple, RegisterAccount, Mint, Transfer).
- « Syscall » fait référence à IVM `SCALL` avec un numéro de 8 bits qui délègue à l'hôte les opérations de grand livre.

---

## Cartographie actuelle (telle qu'implémentée)

### IVM Instructions
- Les assistants arithmétiques, mémoire, flux de contrôle, crypto, vectoriel et ZK sont définis dans `instruction.rs` et implémentés dans `ivm.rs`. Ces éléments sont autonomes et déterministes ; les chemins d'accélération (SIMD/Metal/CUDA) ont des replis de CPU.
- La limite système/hôte se fait via `SCALL` (opcode 0x60). Les numéros sont répertoriés dans `syscalls.rs` et incluent les opérations mondiales (enregistrement/désenregistrement de domaine/compte/actif, menthe/gravure/transfert, opérations de rôle/autorisation, déclencheurs) ainsi que les aides (`GET_PRIVATE_INPUT`, `COMMIT_OUTPUT`, `GET_MERKLE_PATH`, etc.).

### Couche hôte
- Le trait `IVMHost::syscall(number, &mut IVM)` réside dans `host.rs`.
- DefaultHost implémente uniquement les aides non liées au grand livre (allocation, croissance du tas, entrées/sorties, aides à la preuve ZK, découverte de fonctionnalités) — il n'effectue PAS de mutations d'état mondial.
- Une démo `WsvHost` existe dans `mock_wsv.rs` qui mappe un sous-ensemble d'opérations d'actifs (Transfer/Mint/Burn) à un petit WSV en mémoire à l'aide de `AccountId`/`AssetDefinitionId` via des mappages ad‑hoc entier → ID dans les registres x10..x13.

### ISI et modèle de données
- Les types et la sémantique ISI intégrés sont implémentés dans `iroha_core::smartcontracts::isi::*` et documentés dans `docs/source/data_model_and_isi_spec.md`.
- `InstructionBox` utilise un registre avec des « ID de fil » stables et un encodage Norito ; La répartition de l'exécution native est le chemin de code actuel dans le noyau.### Intégration de base de IVM
- `State::execute_trigger(..)` clone le `IVM` mis en cache, attache un `CoreHost::with_accounts_and_args`, puis appelle `load_program` + `run`.
- `CoreHost` implémente `IVMHost` : les appels système avec état sont décodés via la disposition pointeur-ABI TLV, mappés à l'ISI intégré (`InstructionBox`) et mis en file d'attente. Une fois la VM revenue, l'hôte transmet ces ISI à l'exécuteur habituel afin que les autorisations, les invariants, les événements et la télémétrie restent identiques à l'exécution native. Les appels système d’assistance qui ne touchent pas WSV délèguent toujours à `DefaultHost`.
- `executor.rs` continue d'exécuter l'ISI intégré de manière native ; la migration de l'exécuteur du validateur lui-même vers IVM reste un travail futur.

### Kotodama → IVM
- Des éléments frontend existent (lexer/analyseur/sémantique minimale/IR/regalloc).
- Codegen (`kotodama::compiler`) émet un sous-ensemble d'opérations IVM et utilise `SCALL` pour les opérations sur les actifs :
  - `MintAsset` → définir x10=compte, x11=actif, x12=&NoritoBytes (Numérique) ; `SCALL SYSCALL_MINT_ASSET`.
  - `BurnAsset`/`TransferAsset` similaire (montant transmis comme pointeur NoritoBytes (numérique)).
- Les démos `koto_*_demo.rs` montrent l'utilisation de `WsvHost` avec des indices entiers mappés sur des identifiants pour des tests rapides.

---

## Lacunes et discordances

1) Couverture et parité des hôtes de base
- Statut : `CoreHost` existe désormais dans le noyau et traduit de nombreux appels système du grand livre en ISI qui s'exécutent via le chemin standard. La couverture est encore incomplète (par exemple, certains appels système de rôles/autorisations/déclencheurs sont des stubs) et des tests de parité sont nécessaires pour garantir que l'ISI mis en file d'attente produit les mêmes états/événements que l'exécution native.

2) Surface Syscall par rapport à la dénomination et à la couverture du modèle ISI/Data
- NFT : les appels système exposent désormais les noms canoniques `SYSCALL_NFT_*` alignés sur `iroha_data_model::nft`.
- Rôles/Autorisations/Déclencheurs : une liste d'appels système existe, mais aucune implémentation de référence ni table de mappage liant chaque appel à un ISI concret dans le noyau.
- Paramètres/sémantique : certains appels système ne spécifient pas l'encodage des paramètres (identifiants saisis par rapport aux pointeurs) ni la sémantique du gaz ; La sémantique ISI est bien définie.

3) ABI pour transmettre des données saisies à travers la frontière VM/hôte
- Les TLV Pointer‑ABI sont désormais décodées dans `CoreHost` (`decode_tlv_typed`), donnant un chemin déterministe pour les ID, les métadonnées et les charges utiles JSON. Il reste du travail pour garantir que chaque appel système documente les types de pointeurs attendus et que Kotodama émet les TLV corrects (y compris la gestion des erreurs lorsque la politique rejette un type).

4) Cohérence de la cartographie des gaz et des erreurs
- Les codes d'opération IVM facturent du gaz par opération ; CoreHost renvoie désormais du gaz supplémentaire pour les appels système ISI en utilisant le programme de gaz natif (y compris les transferts par lots et le pont ISI du fournisseur), et ZK vérifie que les appels système réutilisent le programme de gaz confidentiel. DefaultHost conserve toujours des coûts minimes pour la couverture des tests.
- Les surfaces d'erreur diffèrent : IVM renvoie `VMError::{OutOfGas,PermissionDenied,...}` ; ISI renvoie les catégories `InstructionExecutionError` (`Find`, `Repetition`, `InvariantViolation`, `Math`, `Type`, `Mintability`, `InvalidParameter`).5) Déterminisme sur les chemins d'accélération
- IVM vector/CUDA/Metal ont des replis CPU, mais certaines opérations restent réservées (`SETVL`, PARBEGIN/PAREND) et ne font pas encore partie du noyau déterministe.
- Les arbres Merkle diffèrent entre IVM et le nœud (`ivm::merkle_tree` vs `iroha_crypto::MerkleTree`) — un élément d'unification apparaît déjà dans `roadmap.md`.

6) Surface du langage Kotodama par rapport à la sémantique prévue du grand livre
- Le compilateur émet un petit sous-ensemble ; la plupart des fonctionnalités du langage (états/structures, déclencheurs, autorisations, paramètres/retours saisis) ne sont pas encore connectées au modèle hôte/ISI.
- Aucun typage de capacité/effet pour garantir que les appels système sont légaux pour l'autorité.

---

## Recommandations (étapes concrètes)

### A. Implémenter un hôte de production IVM dans le noyau
- Ajout du module `iroha_core::smartcontracts::ivm::host` implémentant `ivm::host::IVMHost`.
- Pour chaque appel système dans `ivm::syscalls` :
  - Décoder les arguments via un ABI canonique (voir B.), construire l'ISI intégré correspondant ou appeler directement la même logique de base, l'exécuter sur `StateTransaction` et mapper les erreurs de manière déterministe vers un code de retour IVM.
  - Chargez le gaz de manière déterministe à l'aide d'une table d'appel système définie dans le noyau (et exposée à IVM via `SYSCALL_GET_PARAMETER` si nécessaire à l'avenir). Dans un premier temps, restituez le gaz supplémentaire fixe de l'hôte pour chaque appel.
- Enfilez `authority: &AccountId` et `&mut StateTransaction` dans l'hôte afin que les vérifications d'autorisation et les événements soient identiques à l'ISI natif.
- Mettez à jour `State::execute_trigger(ExecutableRef::Ivm)` pour attacher cet hôte avant `vm.run()` et renvoyer la même sémantique `ExecutionStep` que ISI (les événements sont déjà émis dans le noyau ; un comportement cohérent doit être validé).

### B. Définir une ABI VM/hôte déterministe pour les valeurs saisies
- Utilisez Norito côté VM pour les arguments structurés :
  - Transmettez des pointeurs (en x10..x13, etc.) vers les régions de mémoire de la machine virtuelle contenant des valeurs codées en Norito pour des types tels que `AccountId`, `AssetDefinitionId`, `Numeric`, `Metadata`.
  - L'hôte lit les octets via les assistants de mémoire `IVM` et décode avec Norito (`iroha_data_model` dérive déjà `Encode/Decode`).
- Ajoutez un minimum d'aides dans le codegen Kotodama pour sérialiser les identifiants littéraux dans des pools de code/constants ou pour préparer des trames d'appel en mémoire.
- Les montants sont `Numeric` et sont transmis sous forme de pointeurs NoritoBytes ; d'autres types complexes passent également par un pointeur.
- Documentez cela dans `crates/ivm/docs/calling_convention.md` et ajoutez des exemples.### C. Aligner la dénomination et la couverture des appels système avec ISI/Data Model
- Renommez les appels système liés à NFT pour plus de clarté : les noms canoniques suivent désormais le modèle `SYSCALL_NFT_*` (`SYSCALL_NFT_MINT_ASSET`, `SYSCALL_NFT_SET_METADATA`, etc.).
- Publier une table de mappage (doc + commentaires de code) de chaque appel système vers la sémantique principale d'ISI, comprenant :
  - Paramètres (registres et pointeurs), conditions préalables attendues, événements et mappages d'erreurs.
  - Charges de gaz.
- Assurez-vous qu'il existe un appel système pour chaque ISI intégré qui doit être invocable à partir de Kotodama (domaines, comptes, actifs, rôles/autorisations, déclencheurs, paramètres). Si un ISI doit rester privilégié, documentez-le et appliquez-le via des contrôles d'autorisation dans l'hôte.

### D. Unifier les erreurs et les gaz
- Ajoutez une couche de traduction dans l'hôte : mappez `InstructionExecutionError::{Find,Repetition,InvariantViolation,Math,Type,Mintability,InvalidParameter}` à des codes `VMError` spécifiques ou à une convention de résultat étendue (par exemple, définissez `x10=0/1` et utilisez un `VMError::HostRejected { code }` bien défini).
- Introduire une table de gaz dans le noyau pour les appels système ; mettez-le en miroir dans la documentation IVM ; garantir que les coûts sont prévisibles et indépendants de la plateforme.

### E. Déterminisme et primitives partagées
- Unification complète de l'arbre Merkle (voir feuille de route) et suppression/alias `ivm::merkle_tree` à `iroha_crypto` avec des feuilles et des preuves identiques.
- Gardez `SETVL`/PARBEGIN/PAREND` réservé jusqu'à ce que des contrôles de déterminisme de bout en bout et une stratégie de planification déterministe soient en place ; document selon lequel IVM ignore ces indices aujourd'hui.
- S'assurer que les chemins d'accélération produisent des sorties identiques octet par octet ; lorsque cela n'est pas possible, protégez les fonctionnalités avec un test garantissant l'équivalence de secours du processeur.

### F. Câblage du compilateur Kotodama
- Étendre codegen à l'ABI canonique (B.) pour les identifiants et les paramètres complexes ; arrêtez d'utiliser les cartes de démonstration entier → ID.
- Ajoutez le mappage intégré directement aux appels système ISI au-delà des actifs (domaines/comptes/rôles/autorisations/déclencheurs) avec des noms clairs.
- Ajoutez des contrôles de capacité au moment de la compilation et des annotations `permission(...)` facultatives ; repli sur les erreurs de l'hôte d'exécution lorsque la preuve statique n'est pas possible.
- Ajoutez des tests unitaires dans `crates/ivm/tests/kotodama.rs` qui compilent et exécutent de petits contrats de bout en bout à l'aide d'un hôte de test qui décode les arguments Norito et mute un WSV temporaire.

### G. Documentation et ergonomie développeurs
- Mettez à jour `docs/source/data_model_and_isi_spec.md` avec la table de mappage d'appels système et les notes ABI.
- Ajout d'un nouveau document « Guide d'intégration de l'hôte IVM » dans `crates/ivm/docs/` décrivant comment implémenter un `IVMHost` sur un `StateTransaction` réel.
- Clarifiez dans `README.md` et les documents de caisse que Kotodama cible le bytecode IVM `.to` et que les appels système sont le pont vers l'état mondial.

---

## Tableau de mappage suggéré (version initiale)

Sous-ensemble représentatif : finaliser et développer pendant la mise en œuvre de l'hôte.- SYSCALL_REGISTER_DOMAIN (id : ptr DomainId) → Registre ISI
- SYSCALL_REGISTER_ACCOUNT (id : ptr AccountId) → Registre ISI
- SYSCALL_REGISTER_ASSET(id : ptr AssetDefinitionId, mintable : u8) → Registre ISI
- SYSCALL_MINT_ASSET (compte : ptr AccountId, actif : ptr AssetDefinitionId, montant : ptr NoritoBytes (Numeric)) → ISI Mint
- SYSCALL_BURN_ASSET (compte : ptr AccountId, actif : ptr AssetDefinitionId, montant : ptr NoritoBytes (Numeric)) → ISI Burn
- SYSCALL_TRANSFER_ASSET (de : ptr AccountId, à : ptr AccountId, actif : ptr AssetDefinitionId, montant : ptr NoritoBytes(Numeric)) → ISI Transfer
- SYSCALL_TRANSFER_V1_BATCH_BEGIN() / SYSCALL_TRANSFER_V1_BATCH_END() → ISI TransferAssetBatch (ouvrir/fermer la portée ; les entrées individuelles sont réduites via `transfer_asset`)
- SYSCALL_TRANSFER_V1_BATCH_APPLY(&NoritoBytes) → Soumettre un lot pré-codé lorsque les contrats ont déjà sérialisé les entrées hors chaîne
- SYSCALL_NFT_MINT_ASSET (id : ptr NftId, propriétaire : ptr AccountId) → Registre ISI
- SYSCALL_NFT_TRANSFER_ASSET (de : ptr AccountId, à : ptr AccountId, id : ptr NftId) → ISI Transfer
- SYSCALL_NFT_SET_METADATA (id : ptr NftId, contenu : ptr Metadata) → ISI SetKeyValue
- SYSCALL_NFT_BURN_ASSET(id : ptr NftId) → ISI Unregister
- SYSCALL_CREATE_ROLE (id : ptr RoleId, rôle : ptr Role) → ISI Register
- SYSCALL_GRANT_ROLE (compte : ptr AccountId, rôle : ptr RoleId) → ISI Grant
- SYSCALL_REVOKE_ROLE (compte : ptr AccountId, rôle : ptr RoleId) → ISI Revoke
- SYSCALL_SET_PARAMETER (param : paramètre ptr) → ISI SetParameter

Remarques
- « ptr T » désigne un pointeur dans un registre vers les octets codés Norito pour T, stockés dans la mémoire de la VM ; l'hôte le décode dans le type `iroha_data_model` correspondant.
- Convention de retour : le succès définit `x10=1` ; l'échec définit `x10=0` et peut générer `VMError::HostRejected` en cas d'erreurs fatales.

---

## Risques et plan de déploiement
- Commencez par câbler l'hôte pour un ensemble restreint (Actifs + Comptes) et ajoutez des tests ciblés.
- Conserver l'exécution ISI native comme chemin faisant autorité pendant que la sémantique de l'hôte mûrit ; exécutez les deux chemins en « mode ombre » dans les tests pour affirmer des effets finaux et des événements identiques.
- Une fois la parité validée, activer l'hôte IVM pour les déclencheurs IVM en production ; envisagez plus tard d'acheminer également les transactions régulières via IVM.

---

## Travail exceptionnel
- Finalisez les assistants Kotodama qui transmettent les pointeurs codés Norito (`crates/ivm/src/kotodama_std.rs`) et faites-les apparaître via la CLI du compilateur.
- Publiez la table des gaz d'appel système (y compris les appels système d'assistance) et maintenez l'application/les tests de CoreHost alignés sur elle.
- ✅ Ajout de luminaires aller-retour Norito couvrant l'ABI pointeur-argument ; voir `crates/iroha_data_model/tests/norito_pointer_abi_roundtrip.rs` pour la couverture du manifeste et du pointeur NFT conservée dans CI.