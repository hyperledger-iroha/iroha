---
lang: fr
direction: ltr
source: docs/source/examples/lookup_grand_product.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 6f6421d420a704c5c4af335741e309adf641702ddb8c291dce94ea5581557a66
source_last_modified: "2026-01-03T18:08:00.673232+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Exemple de grand-produit de recherche

Cet exemple développe l'argument de recherche d'autorisation FASTPQ mentionné dans
`fastpq_plan.md`.  Dans le pipeline de l'étape 2, le prouveur évalue le sélecteur
(`s_perm`) et colonnes témoins (`perm_hash`) sur l'extension bas degré (LDE)
domaine, met à jour un grand produit en cours d'exécution `Z_i` et valide enfin l'intégralité
séquence avec Poséidon.  L'accumulateur haché est annexé à la transcription
sous le domaine `fastpq:v1:lookup:product`, tandis que le `Z_i` final correspond toujours
le produit de table d'autorisations validées `T`.

Nous considérons un petit lot avec les valeurs de sélecteur suivantes :

| rangée | `s_perm` | `perm_hash` |
| --- | -------- | ---------------------------------------------- |
| 0 | 1 | `0x019a...` (rôle d'attribution = auditeur, perm = transfer_asset) |
| 1 | 0 | `0xabcd...` (aucune modification d'autorisation) |
| 2 | 1 | `0x42ff...` (rôle de révocation = auditeur, perm = burn_asset) |

Soit `gamma = 0xdead...` le défi de recherche Fiat-Shamir dérivé du
transcription.  Le prouveur initialise `Z_0 = 1` et plie chaque ligne :

```
Z_0 = 1
Z_1 = Z_0 * (perm_hash_0 + gamma)^(s_perm_0) = 1 * (0x019a... + gamma)
Z_2 = Z_1 * (perm_hash_1 + gamma)^(s_perm_1) = Z_1 (selector is zero)
Z_3 = Z_2 * (perm_hash_2 + gamma)^(s_perm_2)
```

Les lignes où `s_perm = 0` ne modifient pas l'accumulateur.  Après avoir traité le
trace, le prouveur Poséidon hache la séquence `[Z_1, Z_2, ...]` pour la transcription
mais publie également `Z_final = Z_3` (le produit final en cours d'exécution) pour correspondre au tableau
condition aux limites.

Du côté de la table, l'arbre Merkle des autorisations validées code le déterministe
ensemble d'autorisations actives pour l'emplacement.  Le vérificateur (ou le prouveur lors
génération de témoins) calcule

```
T = product over entries: (entry.hash + gamma)
```

Le protocole applique la contrainte de limite `Z_final / T = 1`.  Si la trace
introduit une autorisation qui n'est pas présente dans le tableau (ou en a omis une qui
est), le rapport du grand produit s'écarte de 1 et le vérificateur rejette.  Parce que
les deux côtés se multiplient par `(value + gamma)` à l'intérieur du champ Boucle d'or, le rapport
reste stable sur les backends CPU/GPU.

Pour sérialiser l'exemple en tant que Norito JSON pour les appareils, enregistrez le tuple de
`perm_hash`, sélecteur et accumulateur après chaque ligne, par exemple :

```json
{
  "gamma": "0xdead...",
  "rows": [
    {"s_perm": 1, "perm_hash": "0x019a...", "z_after": "0x5f10..."},
    {"s_perm": 0, "perm_hash": "0xabcd...", "z_after": "0x5f10..."},
    {"s_perm": 1, "perm_hash": "0x42ff...", "z_after": "0x9a77..."}
  ],
  "table_product": "0x9a77..."
}
```

Les espaces réservés hexadécimaux (`0x...`) peuvent être remplacés par des boucles d'or en béton
éléments de terrain lors de la génération de tests automatisés.  Calendrier de l'étape 2 également
enregistrer le hachage Poséidon de l'accumulateur en cours d'exécution mais conserver la même forme JSON,
l'exemple peut donc servir de modèle pour les futurs vecteurs de test.