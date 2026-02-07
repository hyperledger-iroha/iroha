---
lang: fr
direction: ltr
source: docs/source/kotodama_error_codes.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 5e0e4f16000f6a578fe9c9d6e204c01087e987ac3b46d70537a15b072df48a13
source_last_modified: "2026-01-03T18:08:01.373878+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Codes d'erreur du compilateur Kotodama

Le compilateur Kotodama émet des codes d'erreur stables afin que les utilisateurs d'outils et de CLI puissent
comprendre rapidement la cause d'une panne. Utiliser `koto_compile --explain <code>`
pour imprimer l'indice correspondant.

| Codes | Descriptif | Correction typique |
|-------|-------------|-------------|
| `E0001` | La cible de branchement est hors de portée pour le codage de saut IVM. | Divisez les fonctions très volumineuses ou réduisez l’inline afin que les distances des blocs de base restent inférieures à ± 1 Mo. |
| `E0002` | Les sites d'appel font référence à une fonction qui n'a jamais été définie. | Recherchez les fautes de frappe, les modificateurs de visibilité ou les indicateurs de fonctionnalité qui ont supprimé l'appelé. |
| `E0003` | Des appels système d’état durable ont été émis sans ABI v1 activé. | Définissez `CompilerOptions::abi_version = 1` ou ajoutez `meta { abi_version: 1 }` dans le contrat `seiyaku`. |
| `E0004` | Les appels système liés aux actifs recevaient des pointeurs non littéraux. | Utilisez `account_id(...)`, `asset_definition(...)`, etc., ou transmettez 0 sentinelles pour les valeurs par défaut de l'hôte. |
| `E0005` | L'initialiseur de boucle `for` est plus complexe que celui pris en charge aujourd'hui. | Déplacez la configuration complexe avant la boucle ; seuls les initialiseurs simples `let`/expression sont actuellement acceptés. |
| `E0006` | La clause d’étape de boucle `for` est plus complexe que celle prise en charge aujourd’hui. | Mettez à jour le compteur de boucles avec une expression simple (par exemple `i = i + 1`). |