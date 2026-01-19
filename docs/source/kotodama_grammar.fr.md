---
lang: fr
direction: ltr
source: docs/source/kotodama_grammar.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: f9d64b88546c924258ef054d8071b38230f3f19c8a7d920f9594b0ecb84252ce
source_last_modified: "2025-12-04T09:32:10.286919+00:00"
translation_last_reviewed: 2026-01-05
---

# Grammaire et sémantique du langage Kotodama

Ce document spécifie la syntaxe du langage Kotodama (lexing et grammaire), les règles de typage, la sémantique déterministe et la façon dont les programmes sont abaissés en bytecode IVM (`.to`) avec les conventions pointer-ABI de Norito. Les sources Kotodama utilisent l’extension `.ko`. Le compilateur émet du bytecode IVM (`.to`) et peut éventuellement retourner un manifeste.

Sommaire
- Vue d’ensemble et objectifs
- Structure lexicale
- Types et littéraux
- Déclarations et modules
- Conteneur de contrat et métadonnées
- Fonctions et paramètres
- Instructions
- Expressions
- Builtins et constructeurs pointer-ABI
- Collections et maps
- Itération déterministe et bornes
- Erreurs et diagnostics
- Mappage de génération de code vers IVM
- ABI, en-tête et manifeste
- Feuille de route

## Vue d’ensemble et objectifs

- Déterminisme : les programmes doivent produire des résultats identiques quel que soit le matériel ; pas de flottants ni de sources non déterministes. Toutes les interactions avec l’hôte passent par des syscalls avec des arguments encodés en Norito.
- Portabilité : cible le bytecode Iroha Virtual Machine (IVM), pas une ISA physique. Les encodages de type RISC‑V visibles dans le dépôt sont des détails d’implémentation du décodeur IVM et ne doivent pas changer le comportement observable.
- Auditable : sémantique petite et explicite ; mappage clair de la syntaxe vers les opcodes IVM et les syscalls de l’hôte.
- Bornes : les boucles sur des données non bornées doivent porter des limites explicites. L’itération de map suit des règles strictes pour garantir le déterminisme.

## Structure lexicale

Espaces blancs et commentaires
- Les espaces blancs séparent les tokens et sont sinon insignifiants.
- Les commentaires de ligne commencent par `//` et vont jusqu’à la fin de ligne.
- Les commentaires de bloc `/* ... */` ne sont pas imbriqués.

Identifiants
- Commencent par `[A-Za-z_]` puis continuent avec `[A-Za-z0-9_]*`.
- Sensibles à la casse ; `_` est un identifiant valide mais déconseillé.

Mots-clés (réservés)
- `seiyaku`, `hajimari`, `kotoage`, `kaizen`, `state`, `struct`, `fn`, `let`, `const`, `return`, `if`, `else`, `while`, `for`, `in`, `break`, `continue`, `true`, `false`, `permission`, `kotoba`.

Opérateurs et ponctuation
- Arithmétique : `+ - * / %`
- Bit à bit : `& | ^ ~`, décalages `<< >>`
- Comparaison : `== != < <= > >=`
- Logique : `&& || !`
- Affectation : `= += -= *= /= %= &= |= ^= <<= >>=`
- Divers : `: , ; . :: ->`
- Parenthèses : `() [] {}`

Littéraux
- Entier : décimal (`123`), hex (`0x2A`), binaire (`0b1010`). Tous les entiers sont signés 64 bits à l’exécution ; les littéraux sans suffixe sont typés par inférence ou comme `int` par défaut.
- Chaîne : entre guillemets doubles avec échappements `\` ; UTF‑8.
- Booléen : `true`, `false`.

## Types et littéraux

Types scalaires
- `int` : 64 bits en complément à deux ; l’arithmétique s’enroule modulo 2^64 pour add/sub/mul ; la division a des variantes signée/non signée définies dans IVM ; le compilateur choisit l’opération appropriée.
- `bool` : valeur logique ; abaissée en `0`/`1`.
- `string` : chaîne UTF‑8 immuable ; représentée en TLV Norito lors des syscalls ; dans la VM, on utilise des tranches d’octets et une longueur.
- `bytes` : payload Norito brut ; alias du type `Blob` du pointer-ABI pour les entrées de hash/crypto/preuve et les overlays durables.

Types composés
- `struct Name { field: Type, ... }` types produit définis par l’utilisateur. Les constructeurs utilisent la syntaxe d’appel `Name(a, b, ...)` dans les expressions. L’accès aux champs `obj.field` est pris en charge et abaissé en champs positionnels de type tuple en interne. L’ABI de l’état durable on-chain est encodée en Norito ; le compilateur émet des overlays qui reflètent l’ordre du struct et des tests récents (`crates/iroha_core/tests/kotodama_struct_overlay.rs`) verrouillent la mise en page entre versions.
- `Map<K, V>` : map associative déterministe ; la sémantique restreint l’itération et les mutations pendant l’itération (voir ci-dessous).
- `Tuple (T1, T2, ...)` : type produit anonyme avec champs positionnels ; utilisé pour les retours multiples.

Types spéciaux pointer-ABI (côté hôte)
- `AccountId`, `AssetDefinitionId`, `Name`, `Json`, `NftId`, `Blob` et similaires ne sont pas des types de première classe à l’exécution. Ce sont des constructeurs qui produisent des pointeurs typés et immuables vers la région INPUT (enveloppes TLV Norito) et ne peuvent être utilisés que comme arguments de syscalls ou déplacés entre variables sans mutation.

Inférence de type
- Les liaisons locales `let` infèrent le type à partir de l’initialiseur. Les paramètres de fonction doivent être explicitement typés. Les types de retour peuvent être inférés sauf ambiguïté.

## Déclarations et modules

Éléments de niveau supérieur
- Contrats : `seiyaku Name { ... }` contiennent fonctions, état, structs et métadonnées.
- Plusieurs contrats par fichier sont autorisés mais déconseillés ; un `seiyaku` principal est utilisé comme entrée par défaut dans les manifestes.
- Les déclarations `struct` définissent des types utilisateur au sein d’un contrat.

Visibilité
- `kotoage fn` désigne un point d’entrée public ; la visibilité affecte les permissions du dispatcher, pas la génération de code.

## Conteneur de contrat et métadonnées

Syntaxe
```
seiyaku Name {
  meta {
    abi_version: 1,
    vector_length: 0,
    max_cycles: 0,
    features: ["zk", "simd"],
  }

  state int counter;

  hajimari() { counter = 0; }

  kotoage fn inc() { counter = counter + 1; }
}
```

Sémantique
- `meta { ... }` surcharge les valeurs par défaut du compilateur pour l’en-tête IVM émis : `abi_version`, `vector_length` (0 signifie non défini), `max_cycles` (0 signifie la valeur par défaut du compilateur), `features` bascule des bits de fonctionnalité (tracé ZK, annonce vectorielle). Les fonctionnalités non prises en charge sont ignorées avec un avertissement. Lorsque `meta {}` est omis, le compilateur émet `abi_version = 1` et utilise les valeurs par défaut d’options pour les autres champs.
- `features: ["zk", "simd"]` (alias : `"vector"`) demande explicitement les bits d’en-tête correspondants. Les chaînes de fonctionnalités inconnues produisent désormais une erreur de parseur au lieu d’être ignorées.
- `state` déclare les variables du contrat. Aujourd’hui le compilateur les abaisse en stockage éphémère par exécution (alloué à l’entrée de fonction) ; les overlays durables côté hôte et le suivi des conflits restent TODO. Pour des lectures/écritures côté hôte, utilisez les helpers explicites `state_get/state_set/state_del` et les helpers de map `get_or_insert_default` ; ils passent par des TLV Norito et maintiennent des noms/ordres de champs stables pour la persistance future.
- Les identifiants `state` sont réservés ; masquer un nom `state` dans des paramètres ou des `let` est rejeté (`E_STATE_SHADOWED`).
- Les valeurs des maps d’état ne sont pas de première classe : utilisez l’identifiant de l’état directement pour les opérations et l’itération. Lier ou passer des maps d’état à des fonctions définies par l’utilisateur est rejeté (`E_STATE_MAP_ALIAS`).
- Les maps d’état durables supportent actuellement uniquement les types de clé `int` et pointer-ABI ; les autres types de clé sont rejetés à la compilation.
- Les champs d’état durable doivent être `int`, `bool`, `Json`, `Blob`/`bytes` ou des types pointer-ABI (y compris des structs/tuples composés de ces champs) ; `string` n’est pas pris en charge pour l’état durable.

## Déclarations de déclencheurs

Les déclarations de déclencheurs attachent des métadonnées de planification aux manifestes des
points d’entrée et sont enregistrées automatiquement lors de l’activation d’une instance de
contrat (supprimées lors de la désactivation). Elles sont analysées dans un bloc `seiyaku`.

Syntaxe
```
register_trigger wake {
  call run;
  on time pre_commit;
  repeats 2;
  metadata { tag: "alpha"; count: 1; enabled: true; }
}
```

Notes
- `call` doit référencer une entrée publique `kotoage fn` dans le même contrat ; un
  `namespace::entrypoint` optionnel est enregistré dans le manifeste mais les callbacks
  inter-contrats sont rejetés pour l’instant (callbacks locaux uniquement).
- Filtres supportés : `time pre_commit` et `time schedule(start_ms, period_ms?)`, plus
  `execute trigger <name>` pour les triggers par appel. Les filtres data/pipeline ne sont pas
  encore supportés.
- Les valeurs de métadonnées doivent être des littéraux JSON (`string`, `number`, `bool`, `null`)
  ou `json!(...)`.
- Clés de métadonnées injectées par le runtime : `contract_namespace`, `contract_id`,
  `contract_entrypoint`, `contract_code_hash`, `contract_trigger_id`.

## Fonctions et paramètres

Syntaxe
- Déclaration : `fn name(param1: Type, param2: Type, ...) -> Ret { ... }`
- Publique : `kotoage fn name(...) { ... }`
- Initialiseur : `hajimari() { ... }` (invoqué au déploiement par le runtime, pas par la VM elle-même).
- Hook de mise à niveau : `kaizen(args...) permission(Role) { ... }`.

Paramètres et retours
- Les arguments passent dans les registres `r10..r22` comme valeurs ou pointeurs INPUT (TLV Norito) selon l’ABI ; les arguments supplémentaires sont déversés sur la pile.
- Les fonctions retournent zéro ou un scalaire ou un tuple. La valeur principale est dans `r10` pour un scalaire ; les tuples sont matérialisés sur la pile/OUTPUT par convention.

## Instructions

- Liaisons de variables : `let x = expr;`, `let mut x = expr;` (la mutabilité est une vérification à la compilation ; la mutation à l’exécution n’est autorisée que pour les locales).
- Affectation : `x = expr;` et formes composées `x += 1;` etc. Les cibles doivent être des variables ou des indices de map ; les champs de tuple/struct sont immuables.
- Contrôle : `if (cond) { ... } else { ... }`, `while (cond) { ... }`, `for (init; cond; step) { ... }` à la C.
  - Les initialiseurs et étapes de `for` doivent être des `let name = expr` simples ou des instructions d’expression ; le destructuring complexe est rejeté (`E0005`, `E0006`).
  - Portée des `for` : les liaisons de la clause init sont visibles dans la boucle et après ; les liaisons créées dans le corps ou l’étape ne sortent pas de la boucle.
- L’égalité (`==`, `!=`) est prise en charge pour `int`, `bool`, `string`, scalaires pointer-ABI (p. ex., `AccountId`, `Name`, `Blob`/`bytes`, `Json`) ; tuples, structs et maps ne sont pas comparables.
- Boucle de map : `for (k, v) in map { ... }` (déterministe ; voir ci-dessous).
- Flux : `return expr;`, `break;`, `continue;`.
- Appel : `name(args...);` ou `call name(args...);` (les deux acceptés ; le compilateur normalise en instructions d’appel).
- Assertions : `assert(cond);`, `assert_eq(a, b);` mappent vers `ASSERT*` de IVM en builds non‑ZK ou des contraintes ZK en mode ZK.

## Expressions

Précédence (haut → bas)
1. Membre/index : `a.b`, `a[b]`
2. Unaire : `! ~ -`
3. Multiplicatif : `* / %`
4. Additif : `+ -`
5. Décalages : `<< >>`
6. Relationnel : `< <= > >=`
7. Égalité : `== !=`
8. AND/XOR/OR bit à bit : `& ^ |`
9. AND/OR logique : `&& ||`
10. Ternaire : `cond ? a : b`

Appels et tuples
- Les appels utilisent des arguments positionnels : `f(a, b, c)`.
- Littéral de tuple : `(a, b, c)` et destructuring : `let (x, y) = pair;`.
- Le destructuring de tuple exige des types tuple/struct de même arité ; les mismatches sont rejetés.

Chaînes et bytes
- Les chaînes sont UTF‑8 ; les fonctions qui requièrent des octets bruts acceptent des pointeurs `Blob` via les constructeurs (voir Builtins).

## Builtins et constructeurs pointer-ABI

Constructeurs de pointeurs (émettent un TLV Norito dans INPUT et renvoient un pointeur typé)
- `account_id(string) -> AccountId*`
- `asset_definition(string) -> AssetDefinitionId*`
- `asset_id(string) -> AssetId*`
- `domain(string) | domain_id(string) -> DomainId*`
- `name(string) -> Name*`
- `json(string) -> Json*`
- `nft_id(string) -> NftId*`
- `blob(bytes|string) -> Blob*`
- `norito_bytes(bytes|string) -> NoritoBytes*`
- `dataspace_id(string|0xhex) -> DataSpaceId*`
- `axt_descriptor(string|0xhex) -> AxtDescriptor*`
- `asset_handle(string|0xhex) -> AssetHandle*`
- `proof_blob(string|0xhex) -> ProofBlob*`

Les macros du prélude fournissent des alias plus courts et une validation en ligne pour ces constructeurs :
- `account!("alice@wonderland")`, `account_id!("alice@wonderland")`
- `asset_definition!("rose#wonderland")`, `asset_id!("rose#wonderland")`
- `domain!("wonderland")`, `domain_id!("wonderland")`
- `name!("example")`
- `json!("{\"hello\":\"world\"}")` ou des littéraux structurés tels que `json!{ hello: "world" }`
- `nft_id!("dragon#demo")`, `blob!("bytes")`, `norito_bytes!("...")`

Les macros se développent vers les constructeurs ci-dessus et rejettent les littéraux invalides à la compilation.

Statut d’implémentation
- Implémenté : les constructeurs ci-dessus acceptent des arguments de chaîne littérale et sont abaissés en enveloppes TLV Norito typées placées dans la région INPUT. Ils renvoient des pointeurs typés immuables utilisables comme arguments de syscalls. Les expressions de chaîne non littérales sont rejetées ; utilisez `Blob`/`bytes` pour les entrées dynamiques. `blob`/`norito_bytes` acceptent aussi des valeurs `bytes` à l’exécution sans macros.
- Formes étendues :
  - `json(Blob[NoritoBytes]) -> Json*` via syscall `JSON_DECODE`.
  - `name(Blob[NoritoBytes]) -> Name*` via syscall `NAME_DECODE`.
  - Décodage de pointeurs depuis Blob/NoritoBytes : tout constructeur de pointeur (y compris les types AXT) accepte un payload `Blob`/`NoritoBytes` et est abaissé en `POINTER_FROM_NORITO` avec l’identifiant de type attendu.
  - Passage direct pour les formes pointer : `name(Name) -> Name*`, `blob(Blob) -> Blob*`, `norito_bytes(Blob) -> Blob*`.
  - Sucre de méthode pris en charge : `s.name()`, `s.json()`, `b.blob()`, `b.norito_bytes()`.

Builtins host/syscall (mappés sur SCALL ; numéros exacts dans ivm.md)
- `mint_asset(AccountId*, AssetDefinitionId*, numeric)`
- `burn_asset(AccountId*, AssetDefinitionId*, numeric)`
- `transfer_asset(AccountId*, AccountId*, AssetDefinitionId*, numeric)`
- `set_account_detail(AccountId*, Name*, Json*)`
- `nft_mint_asset(NftId*, AccountId*)`
- `nft_transfer_asset(AccountId*, NftId*, AccountId*)`
- `nft_set_metadata(NftId*, Json*)`
- `nft_burn_asset(NftId*)`
- `authority() -> AccountId*`
- `register_domain(DomainId*)`
- `unregister_domain(DomainId*)`
- `transfer_domain(AccountId*, DomainId*, AccountId*)`
- `vrf_verify(Blob, Blob, Blob, int variant) -> Blob`
- `vrf_verify_batch(Blob) -> Blob`
- `axt_begin(AxtDescriptor*)`
- `axt_touch(DataSpaceId*, Blob[NoritoBytes]? manifest)`
- `verify_ds_proof(DataSpaceId*, ProofBlob?)`
- `use_asset_handle(AssetHandle*, Blob[NoritoBytes], ProofBlob?)`
- `axt_commit()`
- `contains(Map<K,V>, K) -> bool`

Builtins utilitaires
- `info(string|int)` : émet un événement/message structuré via OUTPUT.
- `hash(blob) -> Blob*` : renvoie un hash encodé Norito sous forme de Blob.
- `build_submit_ballot_inline(election_id, ciphertext, nullifier32, backend, proof, vk) -> Blob*` et `build_unshield_inline(asset, to, amount, inputs32, backend, proof, vk) -> Blob*` : constructeurs ISI en ligne ; tous les arguments doivent être des littéraux à la compilation (littéraux de chaîne ou constructeurs de pointeur à partir de littéraux). `nullifier32` et `inputs32` doivent faire exactement 32 octets (chaîne brute ou hex `0x`), et `amount` doit être non négatif.
- `schema_info(Name*) -> Json* { "id": "<hex>", "version": N }`
- `pointer_to_norito(ptr) -> NoritoBytes*` : enveloppe un TLV pointer-ABI existant en NoritoBytes pour stockage ou transport.
- `isqrt(int) -> int` : racine carrée entière (`floor(sqrt(x))`) implémentée comme opcode IVM.
- `min(int, int) -> int`, `max(int, int) -> int`, `abs(int) -> int`, `div_ceil(int, int) -> int`, `gcd(int, int) -> int`, `mean(int, int) -> int` — helpers arithmétiques fusionnés appuyés par des opcodes natifs IVM (la division plafond trap en cas de division par zéro).

Notes
- Les builtins sont des shims minces ; le compilateur les abaisse en mouvements de registres et un `SCALL`.
- Les constructeurs de pointeurs sont purs : la VM garantit que le TLV Norito en INPUT est immuable pendant la durée de l’appel.
 - Les structs avec champs pointer‑ABI (p. ex., `DomainId`, `AccountId`) peuvent être utilisés pour regrouper des arguments syscall de manière ergonomique. Le compilateur mappe `obj.field` vers le registre/valeur correct sans allocations supplémentaires.

## Collections et maps

Type : `Map<K, V>`
- Les maps en mémoire (allouées sur le heap via `Map::new()` ou passées en paramètres) stockent une seule paire clé/valeur ; clés et valeurs doivent être des types de taille mot : `int`, `bool`, `string`, `Blob`, `bytes`, `Json` ou types de pointeur (p. ex., `AccountId`, `Name`).
- Les maps d’état durables (`state Map<...>`) utilisent des clés/valeurs encodées Norito. Clés supportées : `int` ou types de pointeur. Valeurs supportées : `int`, `bool`, `Json`, `Blob`/`bytes` ou types de pointeur.
- `Map::new()` alloue et initialise à zéro l’entrée unique en mémoire (clé/valeur = 0) ; pour les maps non `Map<int,int>`, fournissez une annotation de type explicite ou un type de retour.
- Les maps d’état ne sont pas des valeurs de première classe : vous ne pouvez pas les réassigner (p. ex., `M = Map::new()`), mettez à jour les entrées via l’indexation (`M[key] = value`).
- Opérations :
  - Indexation : `map[key]` obtenir/définir la valeur (set réalisé via syscall hôte ; voir la cartographie API runtime).
  - Existence : `contains(map, key) -> bool` (helper abaissé ; peut être un syscall intrinsèque).
  - Itération : `for (k, v) in map { ... }` avec ordre déterministe et règles de mutation.

Règles d’itération déterministe
- L’ensemble d’itération est l’instantané des clés à l’entrée de la boucle.
- L’ordre est strictement lexicographique ascendant sur les octets des clés encodées Norito.
- Les modifications structurelles (insérer/supprimer/vider) de la map itérée pendant la boucle causent un trap déterministe `E_ITER_MUTATION`.
- Un bornage est requis : soit un max déclaré (`@max_len`) sur la map, un attribut explicite `#[bounded(n)]` ou une borne explicite via `.take(n)`/`.range(..)` ; sinon le compilateur émet `E_UNBOUNDED_ITERATION`.

Helpers de bornes
- `#[bounded(n)]` : attribut optionnel sur l’expression de map, p. ex. `for (k, v) in my_map #[bounded(2)] { ... }`.
- `.take(n)` : itère les `n` premières entrées depuis le début.
- `.range(start, end)` : itère les entrées dans l’intervalle semi-ouvert `[start, end)`. La sémantique équivaut à `start` et `n = end - start`.

Notes sur les bornes dynamiques
- Bornes littérales : `n`, `start` et `end` en tant que littéraux entiers sont entièrement pris en charge et compilent en un nombre fixe d’itérations.
- Bornes non littérales : lorsque la fonctionnalité `kotodama_dynamic_bounds` est activée dans le crate `ivm`, le compilateur accepte des expressions dynamiques `n`, `start` et `end` et insère des assertions d’exécution pour la sûreté (non négatives, `end >= start`). L’abaissement émet jusqu’à K itérations gardées avec `if (i < n)` pour éviter des exécutions supplémentaires du corps (K par défaut = 2). Vous pouvez régler K via `CompilerOptions { dynamic_iter_cap, .. }`.
- Exécutez `koto_lint` pour inspecter les avertissements de lint Kotodama avant compilation ; le compilateur principal procède toujours à l’abaissement après parsing et vérification des types.
- Les codes d’erreur sont documentés dans [Kotodama Compiler Error Codes](./kotodama_error_codes.md) ; utilisez `koto_compile --explain <code>` pour des explications rapides.

## Erreurs et diagnostics

Diagnostics à la compilation (exemples)
- `E_UNBOUNDED_ITERATION` : boucle sur map sans borne.
- `E_MUT_DURING_ITER` : mutation structurelle de la map itérée dans le corps de la boucle.
- `E_STATE_SHADOWED` : les liaisons locales ne peuvent pas masquer des déclarations `state`.
- `E_BREAK_OUTSIDE_LOOP` : `break` utilisé hors d’une boucle.
- `E_CONTINUE_OUTSIDE_LOOP` : `continue` utilisé hors d’une boucle.
- `E0005` : l’initialiseur du for est plus complexe que supporté.
- `E0006` : la clause step du for est plus complexe que supporté.
- `E_BAD_POINTER_USE` : utilisation du résultat d’un constructeur pointer-ABI là où un type de première classe est requis.
- `E_UNRESOLVED_NAME`, `E_TYPE_MISMATCH`, `E_ARITY_MISMATCH`, `E_DUP_SYMBOL`.
- Outils : `koto_compile` exécute le lint avant d’émettre le bytecode ; utilisez `--no-lint` pour ignorer ou `--deny-lint-warnings` pour échouer le build en cas de lint.

Erreurs d’exécution VM (sélection ; liste complète dans ivm.md)
- `E_NORITO_INVALID`, `E_OOB`, `E_UNALIGNED`, `E_SCALL_UNKNOWN`, `E_ASSERT`, `E_ASSERT_EQ`, `E_ITER_MUTATION`.

Messages d’erreur
- Les diagnostics portent des `msg_id` stables qui correspondent aux entrées des tables de traduction `kotoba {}` quand elles sont disponibles.

## Mappage de génération de code vers IVM

Pipeline
1. Lexer/Parser produisent l’AST.
2. L’analyse sémantique résout les noms, vérifie les types et peuple les tables de symboles.
3. Abaissement IR vers une forme simple de type SSA.
4. Allocation de registres vers les GPR IVM (`r10+` pour args/ret par convention) ; spills vers la pile.
5. Émission de bytecode : mélange d’encodages natifs IVM et compatibles RV lorsque pertinent ; en-tête de métadonnées émis avec `abi_version`, features, longueur vectorielle et `max_cycles`.

Points clés du mappage
- L’arithmétique et la logique mappent vers les ops ALU IVM.
- Branches et contrôle mappent vers des branches conditionnelles et des sauts ; le compilateur utilise des formes compressées quand c’est avantageux.
- La mémoire des locales est spillée sur la pile VM ; l’alignement est appliqué.
- Les builtins sont abaissés en mouvements de registres et `SCALL` avec numéro sur 8 bits.
- Les constructeurs de pointeurs placent des TLV Norito dans la région INPUT et produisent leurs adresses.
- Les assertions mappent vers `ASSERT`/`ASSERT_EQ` qui trap en exécution non‑ZK et émettent des contraintes en builds ZK.

Contraintes de déterminisme
- Pas de FP ; pas de syscalls non déterministes.
- L’accélération SIMD/GPU est invisible au bytecode et doit être bit‑identique ; le compilateur n’émet pas d’opérations spécifiques au matériel.

## ABI, en-tête et manifeste

Champs d’en-tête IVM définis par le compilateur
- `version` : version du format de bytecode IVM (major.minor).
- `abi_version` : version de la table des syscalls et du schéma pointer-ABI.
- `feature_bits` : indicateurs de fonctionnalités (p. ex., `ZK`, `VECTOR`).
- `vector_len` : longueur vectorielle logique (0 → non défini).
- `max_cycles` : borne d’admission et indice de padding ZK.

Manifeste (sidecar optionnel)
- `code_hash`, `abi_hash`, métadonnées du bloc `meta {}`, version du compilateur et indices de build pour la reproductibilité.

## Feuille de route

- **KD-231 (Avr 2026) :** ajouter une analyse de plage à la compilation pour les bornes d’itération afin que les boucles exposent des ensembles d’accès bornés au scheduler.
- **KD-235 (Mai 2026) :** introduire un scalaire `bytes` de première classe distinct de `string` pour les constructeurs de pointeurs et la clarté ABI.
- **KD-242 (Jun 2026) :** étendre l’ensemble d’opcodes builtins (hash / vérification de signature) derrière des flags de fonctionnalités avec des fallbacks déterministes.
- **KD-247 (Jun 2026) :** stabiliser les `msg_id` d’erreur et maintenir la correspondance dans les tables `kotoba {}` pour des diagnostics localisés.
### Émission de manifeste

- L’API du compilateur Kotodama peut renvoyer un `ContractManifest` avec le `.to` compilé via `ivm::kotodama::compiler::Compiler::compile_source_with_manifest`.
- Champs :
  - `code_hash` : hash des octets de code (hors en-tête IVM et littéraux) calculé par le compilateur pour lier l’artefact.
  - `abi_hash` : digest stable de la surface de syscalls autorisée pour l’`abi_version` du programme (voir `ivm.md` et `ivm::syscalls::compute_abi_hash`).
- `compiler_fingerprint` et `features_bitmap` optionnels sont réservés pour les toolchains.
- `entrypoints` : liste ordonnée des points d’entrée exportés (publics, `hajimari`, `kaizen`) incluant leurs chaînes `permission(...)` requises et les indices de clés de lecture/écriture au meilleur effort du compilateur afin que l’admission et les schedulers puissent raisonner sur l’accès attendu au WSV.
- Le manifeste est destiné aux vérifications d’admission et aux registres ; voir `docs/source/new_pipeline.md` pour le cycle de vie.
