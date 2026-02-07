---
lang: fr
direction: ltr
source: docs/source/examples/smt_update.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 788902cfafc6c7db6d52d4237b46ffe78193efd57852bc3427a16d7f3cda2f9c
source_last_modified: "2026-01-03T18:08:00.438859+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Exemple de mise à jour Sparse Merkle

Cet exemple concret illustre comment la trace FASTPQ Stage 2 code un
témoin de non-appartenance en utilisant la colonne `neighbour_leaf`. L'arbre Merkle clairsemé
est binaire sur les éléments du champ Poséidon2. Les clés sont converties en canoniques
Chaînes petit-boutiste de 32 octets, hachées en un élément de champ, et la plupart
les bits significatifs sélectionnent la branche à chaque niveau.

## Scénario

- Congés préalables
  - `asset::alice::rose` -> clé hachée `0x12b7...` avec la valeur `0x0000_0000_0000_0005`.
  - `asset::bob::rose` -> clé hachée `0x1321...` avec la valeur `0x0000_0000_0000_0003`.
- Demande de mise à jour : insérer `asset::carol::rose` avec la valeur 2.
- Le hachage de clé canonique pour Carol s'étend au préfixe 5 bits `0b01011`. Le
  les voisins existants ont les préfixes `0b01010` (Alice) et `0b01101` (Bob).

Puisqu’il n’existe aucune feuille dont le préfixe correspond à `0b01011`, le prouveur doit fournir
preuve supplémentaire que l'intervalle `(alice, bob)` est vide. L'étape 2 se remplit
la ligne de trace dans les colonnes `path_bit_{level}`, `sibling_{level}`,
`node_in_{level}` et `node_out_{level}` (avec `level` dans `[0, 31]`). Toutes les valeurs
sont des éléments du champ Poséidon2 codés sous forme petit-boutiste :

| niveau | `path_bit_level` | `sibling_level` | `node_in_level` | `node_out_level` | Remarques |
| ----- | ---------------- | -------------------------------- | ------------------------------------ | ------------------------------------ | ----- |
| 0 | 1 | `0x241f...` (hachage de feuilles d'Alice) | `0x0000...` | `0x4b12...` (`value_2 = 2`) | Insérer : recommencer à zéro, stocker une nouvelle valeur. |
| 1 | 1 | `0x7d45...` (droit vide) | Poséidon2(`node_out_0`, `sibling_0`) | Poséidon2(`sibling_1`, `node_out_1`) | Suivez le bit de préfixe 1. |
| 2 | 0 | `0x03ae...` (branche Bob) | Poséidon2(`node_out_1`, `sibling_1`) | Poséidon2(`node_in_2`, `sibling_2`) | La branche s'inverse car bit = 0. |
| 3 | 1 | `0x9bc4...` | Poséidon2(`node_out_2`, `sibling_2`) | Poséidon2(`sibling_3`, `node_out_3`) | Les niveaux plus élevés continuent de monter. |
| 4 | 0 | `0xe112...` | Poséidon2(`node_out_3`, `sibling_3`) | Poséidon2(`node_in_4`, `sibling_4`) | Niveau racine ; le résultat est la racine post-état. |

La colonne `neighbour_leaf` de cette ligne contient la feuille de Bob
(`key = 0x1321...`, `value = 3`, `hash = Poseidon2(key, value) = 0x03ae...`). Quand
En vérifiant, l'AIR vérifie que :

1. Le voisin fourni correspond au frère utilisé au niveau 2.
2. La clé voisine est lexicographiquement supérieure à la clé insérée et la
   le frère de gauche (Alice) est lexicographiquement plus petit.
3. Le remplacement de la feuille insérée par la voisine reproduit la racine pré-étatique.Ensemble, ces vérifications prouvent qu'aucune feuille n'existait pour l'intervalle `(0b01010,
0b01101)` avant la mise à jour. Les implémentations générant des traces FASTPQ peuvent utiliser
cette mise en page textuellement ; les constantes numériques ci-dessus sont illustratives. Pour un plein
Témoin JSON, émettez les colonnes exactement telles qu'elles apparaissent dans le tableau ci-dessus (avec
suffixes numériques par niveau), en utilisant des chaînes d'octets little-endian sérialisées avec
Norito Aides JSON.