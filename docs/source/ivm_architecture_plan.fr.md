---
lang: fr
direction: ltr
source: docs/source/ivm_architecture_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: da8a99adbbcf1d8b209a25da32e256c0dad2860633f373d7410a3a91d790c938
source_last_modified: "2026-01-21T10:21:48.087325+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# IVM Plan de refactorisation de l'architecture

Ce plan capture les étapes à court terme pour remodeler la machine virtuelle Iroha.
(IVM) en couches plus claires tout en préservant les caractéristiques de sécurité et de performances.
Il se concentre sur l'isolement des responsabilités, rendant les intégrations hôtes plus sûres et
préparer la pile de langage Kotodama pour l'extraction dans une caisse autonome.

## Objectifs

1. **Façade d'exécution en couches** – introduisez une interface d'exécution explicite pour que la VM
   le noyau peut être intégré derrière un trait étroit et des frontaux alternatifs peuvent évoluer
   sans toucher aux modules internes.
2. **Renforcement des limites hôte/appel système** : acheminez la répartition des appels système via un
   adaptateur dédié qui applique la politique ABI et la validation du pointeur avant tout hôte
   le code s'exécute.
3. **Séparation langue/outils** – déplacez le code spécifique Kotodama vers une nouvelle caisse et
   conserver uniquement la surface d'exécution du bytecode dans `ivm`.
4. **Cohésion de la configuration** : unifiez l'accélération et les bascules de fonctionnalités afin qu'elles soient
   piloté via `iroha_config`, supprimant les boutons basés sur l'environnement en production
   chemins.

## Répartition des phases

### Phase 1 – Runtime façade (en cours)
- Ajout d'un module `runtime` qui définit un trait `VmEngine` décrivant le cycle de vie
  opérations (`load_program`, `execute`, plomberie hôte).
- Apprenez à `IVM` à implémenter le trait.  Cela conserve la structure existante mais permet
  les consommateurs (et les tests futurs) dépendent de l'interface plutôt que du béton
  genres.
- Commencez à supprimer les réexportations directes de modules depuis `lib.rs` afin que les appelants importent via le
  façade lorsque cela est possible.

**Impact sécurité/performance** : La façade restreint l'accès direct à l'intérieur
état; seuls les points d’entrée sûrs sont exposés.  Cela facilite l'audit de l'hôte
interactions et raison concernant la manipulation du gaz ou du TLV.

### Phase 2 – Répartiteur Syscall
- Introduire un composant `SyscallDispatcher` qui encapsule `IVMHost` et applique ABI
  validation de la politique et du pointeur une fois, en un seul endroit.
- Migrer l'hôte par défaut et les hôtes fictifs pour utiliser le répartiteur, en supprimant
  logique de validation dupliquée.
- Rendre le répartiteur enfichable afin que les hôtes puissent fournir une instrumentation personnalisée sans
  contourner les contrôles de sécurité.
- Fournir un assistant `SyscallDispatcher::shared(...)` pour que les machines virtuelles clonées puissent être transférées
  appels système via un hôte `Arc<Mutex<..>>` partagé sans que chaque travailleur ne construise
  emballages sur mesure.

**Impact sur la sécurité/les performances** : le contrôle centralisé protège contre les hôtes qui
oubliez d'appeler `is_syscall_allowed`, et cela permet la mise en cache future du pointeur
validations pour les appels système répétés.

### Phase 3 – Extraction Kotodama
- Compilateur Kotodama extrait vers `crates/kotodama_lang` (de `crates/ivm/src/kotodama`).
- Fournir une API de bytecode minimale consommée par la VM (`compile_to_ivm_bytecode`).

**Impact sur la sécurité/les performances** : le découplage réduit la surface d'attaque de la VM
noyau et permet l’innovation linguistique sans risquer de régressions des interprètes.### Phase 4 – Consolidation des configurations
- Options d'accélération des threads via les préréglages `iroha_config` (par exemple, activation des backends GPU) tout en conservant les remplacements d'environnement existants (`IVM_DISABLE_CUDA`, `IVM_DISABLE_METAL`) comme kill switch d'exécution.
- Exposez un objet `RuntimeConfig` à travers la nouvelle façade afin que les hôtes sélectionnent
  explicitement des politiques d’accélération déterministes.

**Impact sur la sécurité/les performances** : l'élimination des bascules basées sur l'environnement évite le silence
dérive de configuration et garantit un comportement déterministe à travers les déploiements.

## Prochaines étapes immédiates

- Terminer la phase 1 en ajoutant le trait de façade et en mettant à jour les sites d'appels de haut niveau pour
  en dépendent.
- Auditer les réexportations publiques pour garantir uniquement la façade et les API volontairement publiques
  fuite hors de la caisse.
- Prototyper l'API du répartiteur syscall dans un module séparé et migrer le
  hôte par défaut une fois validé.

Les progrès de chaque phase seront suivis dans `status.md` une fois la mise en œuvre terminée.
en cours.