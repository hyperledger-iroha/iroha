<!-- Auto-generated stub for French (fr) translation. Replace this content with the full translation. -->

---
lang: fr
direction: ltr
source: docs/source/isi_extension_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 9648381ac7cc1716ffd3c48aca425ed17a6afe1ac73bdeff866ebbbd9147cf68
source_last_modified: "2026-03-30T18:22:55.972718+00:00"
translation_last_reviewed: 2026-04-02
translator: machine-google-reviewed
---

# Plan d'extension ISI (v1)

Cette note signe l'ordre de priorité pour les nouvelles instructions spéciales Iroha et capture
invariants non négociables pour chaque instruction avant la mise en œuvre. Les correspondances de commande
le risque de sécurité et d’opérabilité en premier, le débit UX ensuite.

## Pile prioritaire

1. **RotateAccountSignatory** – Requis pour une rotation hygiénique des clés sans migrations destructrices.
2. **DeactivateContractInstance** / **RemoveSmartContractBytes** – Fournir un contrat déterministe
   kill switch et récupération de stockage pour les déploiements compromis.
3. **SetAssetKeyValue** / **RemoveAssetKeyValue** – Étendre la parité des métadonnées à un actif concret
   les soldes afin que les outils d’observabilité puissent marquer les avoirs.
4. **BatchMintAsset** / **BatchTransferAsset** – Assistants de distribution déterministes pour conserver la taille de la charge utile
   et la pression de repli de la VM est gérable.

## Invariants d'instruction

### SetAssetKeyValue / RemoveAssetKeyValue
- Réutilisez l'espace de noms `AssetMetadataKey` (`state.rs`) afin que les clés WSV canoniques restent stables.
- Appliquez les limites de taille et de schéma JSON de la même manière aux assistants de métadonnées de compte.
- Émettre `AssetEvent::MetadataInserted` / `AssetEvent::MetadataRemoved` avec le `AssetId` concerné.
- Exiger les mêmes jetons d'autorisation que les modifications de métadonnées d'actifs existantes (propriétaire de la définition OU
  Subventions de type `CanModifyAssetMetadata`).
- Abandonner si l'enregistrement d'actif est manquant (pas de création implicite).### RotateAccountSignatoire
- Swap atomique du signataire en `AccountId` tout en préservant les métadonnées du compte et lié
  ressources (actifs, déclencheurs, rôles, autorisations, événements en attente).
- Vérifiez que le signataire actuel correspond à l'appelant (ou à l'autorité déléguée via un jeton explicite).
- Rejeter si la nouvelle clé publique soutient déjà un autre compte canonique.
- Mettez à jour toutes les clés canoniques qui intègrent l'ID de compte et invalidez les caches avant la validation.
- Émettre un `AccountEvent::SignatoryRotated` dédié avec les anciennes/nouvelles clés pour les pistes d'audit.
- Échafaudage de migration : s'appuyer sur `AccountAlias` + `AccountRekeyRecord` (voir `account::rekey`) donc
  les comptes existants peuvent conserver des liaisons d'alias stables lors d'une mise à niveau continue sans interruption de hachage.

### DésactiverContractInstance
- Supprimez ou désactivez la liaison `(namespace, contract_id)` tout en conservant les données de provenance
  (qui, quand, code anomalie) pour le dépannage.
- Exiger le même ensemble d'autorisations de gouvernance que l'activation, avec des crochets de politique à interdire
  désactivation des espaces de noms du système principal sans approbation élevée.
- Rejeter lorsque l'instance est déjà inactive pour conserver les journaux d'événements déterministes.
- Émettez un `ContractInstanceEvent::Deactivated` que les observateurs en aval peuvent consommer.### SupprimerSmartContractBytes
- Autoriser l'élagage du bytecode stocké par `code_hash` uniquement en l'absence de manifeste ou d'instance active
  référencer l'artefact ; sinon, échouez avec une erreur descriptive.
- Enregistrement des miroirs de porte d'autorisation (`CanRegisterSmartContractCode`) plus un niveau opérateur
  garde (par exemple, `CanManageSmartContractStorage`).
- Vérifiez que le `code_hash` fourni correspond au résumé du corps stocké juste avant la suppression pour éviter
  poignées rassis.
- Émettez `ContractCodeEvent::Removed` avec les métadonnées de hachage et de l'appelant.

### BatchMintAsset / BatchTransferAsset
- Sémantique tout ou rien : soit chaque tuple réussit, soit l'instruction abandonne sans côté
  effets.
- Les vecteurs d'entrée doivent être ordonnés de manière déterministe (pas de tri implicite) et délimités par la configuration
  (`max_batch_isi_items`).
- Émettre des événements d'actifs par article afin que la comptabilité en aval reste cohérente ; le contexte du lot est additif,
  pas un remplacement.
- Les contrôles d'autorisation réutilisent la logique d'élément unique existante par cible (propriétaire de l'actif, propriétaire de la définition,
  ou capacité accordée) avant la mutation d’état.
- Les ensembles d'accès consultatifs doivent regrouper toutes les clés de lecture/écriture pour maintenir la concurrence optimiste correcte.

## Échafaudage de mise en œuvre- Le modèle de données contient désormais les échafaudages `SetAssetKeyValue`/`RemoveAssetKeyValue` pour les métadonnées d'équilibre.
  modifications (`transparent.rs`).
- Les visiteurs de l'exécuteur exposent des espaces réservés qui permettront d'obtenir les autorisations une fois le câblage hôte atterri.
  (`default/mod.rs`).
- Les types de prototypes Rekey (`account::rekey`) fournissent une zone d'atterrissage pour les migrations progressives.
- L'état mondial inclut `account_rekey_records` saisi par `AccountAlias` afin que nous puissions créer un alias →
  migrations de signataires sans toucher à l’encodage historique `AccountId`.

## IVM Rédaction d'appel système

- Cales hôtes pour `DeactivateContractInstance` / `RemoveSmartContractBytes` expédiées comme
  `SYSCALL_DEACTIVATE_CONTRACT_INSTANCE` (0x43) et
  `SYSCALL_REMOVE_SMART_CONTRACT_BYTES` (0x44), tous deux consommant des TLV Norito qui reflètent le
  structures ISI canoniques.
- Étendre `abi_syscall_list()` uniquement après que les gestionnaires d'hôtes mettent en miroir les chemins d'exécution `iroha_core` à conserver
  Les hachages ABI sont stables pendant le développement.
- Mise à jour Kotodama en baisse une fois que les numéros d'appel système se stabilisent ; ajouter une couverture dorée pour l'étendue
  surface en même temps.

## Statut

L'ordre et les invariants ci-dessus sont prêts à être implémentés. Les branches de suivi doivent faire référence
ce document lors du câblage des chemins d'exécution et de l'exposition des appels système.