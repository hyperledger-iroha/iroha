---
lang: fr
direction: ltr
source: docs/source/error_mapping.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: cba8780bcec4ebf562dc9c5725f328b0ea2d9009517efa5b5a504e2fb6be81fe
source_last_modified: "2026-01-18T05:31:56.950113+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Guide de mappage des erreurs

Dernière mise à jour : 2025-08-21

Ce guide mappe les modes de défaillance courants dans Iroha aux catégories d'erreurs stables révélées par le modèle de données. Utilisez-le pour concevoir des tests et rendre prévisible la gestion des erreurs client.

Principes
- Les chemins d'instructions et de requêtes émettent des énumérations structurées. Évitez les paniques ; signalez une catégorie spécifique dans la mesure du possible.
- Les catégories sont stables, les messages peuvent évoluer. Les clients doivent correspondre sur des catégories, et non sur des chaînes de forme libre.

Catégories
- InstructionExecutionError::Find : Entité manquante (actif, compte, domaine, NFT, rôle, déclencheur, autorisation, clé publique, bloc, transaction). Exemple : la suppression d'une clé de métadonnées inexistante donne Find(MetadataKey).
- InstructionExecutionError::Repetition : enregistrement en double ou ID conflictuel. Contient le type d’instruction et l’IdBox répétée.
- InstructionExecutionError::Mintability : invariant de mintabilité violé (`Once` épuisé deux fois, `Limited(n)` à découvert ou tentatives de désactivation de `Infinitely`). Exemples : frapper deux fois un actif défini comme `Once` donne `Mintability(MintUnmintable)` ; la configuration de `Limited(0)` donne `Mintability(InvalidMintabilityTokens)`.
- InstructionExecutionError::Math : erreurs de domaine numérique (débordement, division par zéro, valeur négative, quantité insuffisante). Exemple : brûler plus que la quantité disponible donne Math(NotEnoughQuantity).
- InstructionExecutionError::InvalidParameter : paramètre d'instruction ou configuration non valide (par exemple, déclenchement temporel dans le passé). À utiliser pour les charges utiles de contrat mal formé.
- InstructionExecutionError::Evaluate : incompatibilité DSL/spécifications pour la forme ou les types d'instructions. Exemple : une spécification numérique incorrecte pour une valeur d'actif donne Evaluate(Type(AssetNumericSpec(..))).
- InstructionExecutionError::InvariantViolation : Violation d'un invariant système qui ne peut pas être exprimé dans d'autres catégories. Exemple : tentative de suppression du dernier signataire.
- InstructionExecutionError::Query : Wrapping de QueryExecutionFail lorsqu'une requête échoue lors de l'exécution d'une instruction.

Échec de l'exécution de la requête
- Rechercher : entité manquante dans le contexte de la requête.
- Conversion : Mauvais type attendu par une requête.
- NotFound : curseur de requête en direct manquant.
- CursorMismatch / CursorDone : Erreurs de protocole du curseur.
- FetchSizeTooBig : limite appliquée par le serveur dépassée.
- GasBudgetExceeded : l'exécution de la requête a dépassé le budget de gaz/matérialisation.
- InvalidSingularParameters : paramètres non pris en charge pour les requêtes singulières.
-CapacityLimit : capacité du magasin de requêtes en direct atteinte.

Conseils de test
- Préférer les tests unitaires proches de l'origine d'une erreur. Par exemple, une inadéquation des spécifications numériques des actifs peut être générée dans les tests de modèles de données.
- Les tests d'intégration doivent couvrir la cartographie de bout en bout pour les cas représentatifs (par exemple, registre en double, clé manquante lors de la suppression, transfert sans propriété).
- Gardez les assertions résilientes en faisant correspondre les variantes d'énumération au lieu des sous-chaînes de message.