---
lang: fr
direction: ltr
source: docs/source/kotodama_examples.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 168513edcb6624ab76275b01aaaf6ab9dee310b9d6f5a2960504a9545801c511
source_last_modified: "2026-01-28T15:34:14.183250+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Kotodama Présentation des exemples

Cette page présente des exemples concis de Kotodama et comment ils sont mappés aux appels système IVM et aux arguments du pointeur-ABI. Voir aussi :
- `examples/` pour les sources exécutables
- `docs/source/ivm_syscalls.md` pour l'appel système canonique ABI
- `kotodama_grammar.md` pour la spécification linguistique complète

## Bonjour + Détails du compte

Source : `examples/hello/hello.ko`

```
seiyaku Hello {
  hajimari() { info("Hello from Kotodama"); }

  kotoage fn write_detail() permission(Admin) {
    set_account_detail(
      authority(),
      name!("example"),
      json!{ hello: "world" }
    );
  }
}
```

Cartographie (pointeur‑ABI) :
- `authority()` → `SCALL 0xA4` (l'hôte écrit `&AccountId` dans `r10`)
- `set_account_detail(a, k, v)` → déplacer `r10=&AccountId`, `r11=&Name`, `r12=&Json`, puis `SCALL 0x1A`

## Transfert d'actifs

Source : `examples/transfer/transfer.ko`

```
seiyaku TransferDemo {
  kotoage fn do_transfer() permission(AssetTransferRole) {
    transfer_asset(
      account!("ed0120AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA@wonderland"),
      account!("ed0120BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB@wonderland"),
      asset_definition!("rose#wonderland"),
      10
    );
  }
}
```

Cartographie (pointeur‑ABI) :
- `transfer_asset(from, to, def, amt)` → `r10=&AccountId(from)`, `r11=&AccountId(to)`, `r12=&AssetDefinitionId(def)`, `r13=amount`, puis `SCALL 0x24`

## NFT Créer + Transférer

Source : `examples/nft/nft.ko`

```
seiyaku NftDemo {
  kotoage fn create() permission(NftAuthority) {
    let owner = account!("ed0120AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA@wonderland");
    let nft = nft_id!("dragon$wonderland");
    nft_mint_asset(nft, owner);
  }

  kotoage fn transfer() permission(NftAuthority) {
    let owner = account!("ed0120AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA@wonderland");
    let recipient = account!("ed0120BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB@wonderland");
    let nft = nft_id!("dragon$wonderland");
    nft_transfer_asset(owner, nft, recipient);
  }
}
```

Cartographie (pointeur‑ABI) :
- `nft_mint_asset(id, owner)` → `r10=&NftId`, `r11=&AccountId(owner)`, `SCALL 0x25`
-`nft_transfer_asset(from, id, to)` → `r10=&AccountId(from)`, `r11=&NftId`, `r12=&AccountId(to)`, `SCALL 0x26`

## Pointeur Norito Aides

L'état durable évalué par un pointeur nécessite la conversion des TLV typés vers et depuis le
Enveloppe `NoritoBytes` qui héberge persiste. Kotodama câble désormais ces assistants
directement via le compilateur afin que les constructeurs puissent utiliser les valeurs par défaut du pointeur et la carte
recherches sans colle FFI manuelle :

```
seiyaku PointerDemo {
  state Owners: Map<int, AccountId>;

  fn hajimari() {
    let alice = account_id("ed0120AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA@wonderland");
    let first = get_or_insert_default(Owners, 7, alice);
    assert(first == alice);

    // The second call decodes the stored pointer and re-encodes the input.
    let bob = account_id("ed0120BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB@wonderland");
    let again = get_or_insert_default(Owners, 7, bob);
    assert(again == alice);
  }
}
```

Descente :

- Les valeurs par défaut du pointeur émettent `POINTER_TO_NORITO` après la publication du TLV saisi, donc
  l'hôte reçoit une charge utile canonique `NoritoBytes` pour le stockage.
- Les lectures effectuent l'opération inverse avec `POINTER_FROM_NORITO`, fournissant le
  ID de type de pointeur attendu dans `r11`.
- Les deux chemins publient automatiquement les TLV littéraux dans la région INPUT, permettant
  des contrats pour mélanger les littéraux de chaîne et les pointeurs d’exécution de manière transparente.

Voir `crates/ivm/tests/kotodama_pointer_args.rs` pour une régression d'exécution qui
exerce l'aller-retour contre le `MockWorldStateView`.

## Itération de carte déterministe (conception)

La carte déterministe pour chacun nécessite une limite. L'itération à entrées multiples nécessite une carte d'état ; le compilateur accepte `.take(n)` ou une longueur maximale déclarée.

```
// design example (iteration requires bounds and state storage)
state M: Map<int, int>;

fn sum_first_two() -> int {
  let s = 0;
  for (k, v) in M.take(2) {
    s = s + v;
  }
  return s;
}
```

Sémantique :
- L'ensemble d'itérations est un instantané à l'entrée de la boucle ; l'ordre est lexicographique par Norito octets de la clé.
- Mutations structurelles vers `M` dans le piège à boucle avec `E_ITER_MUTATION`.
- Sans limite le compilateur émet `E_UNBOUNDED_ITERATION`.

## Composants internes du compilateur/hôte (Rust, pas source Kotodama)

Les extraits ci-dessous se trouvent du côté Rust de la chaîne d'outils. Ils illustrent les aides au compilateur et les mécanismes de réduction de VM et ne sont **pas** une source Kotodama `.ko` valide.

## Mises à jour du cadre fragmenté d'opcodes larges

Les assistants d'opcode larges du Kotodama ciblent la disposition des opérandes 8 bits utilisée par le IVM.
encodage large. Les charges et les magasins qui déplacent des valeurs de 128 bits réutilisent le troisième opérande
emplacement pour le registre supérieur, donc le registre de base doit déjà contenir le registre final
adresse. Ajustez la base avec un `ADDI` avant d'émettre le chargement/stockage :

```
use ivm::kotodama::wide::{encode_addi_checked, encode_load128, encode_store128};

fn emit_store_pair(base: u8, lo: u8, hi: u8) -> [u32; 2] {
    let adjust = encode_addi_checked(base, base, 16).expect("16-byte chunk");
    let store = encode_store128(base, lo, hi);
    [adjust, store]
}
```Les mises à jour de trames fragmentées font avancer la base par pas de 16 octets, garantissant ainsi le registre
La paire engagée par `STORE128` atterrit sur la limite d'alignement requise. Le même
le modèle s'applique à `LOAD128` ; délivrer un `ADDI` avec la foulée souhaitée avant
chaque charge maintient le registre de destination haute lié au troisième emplacement d'opérande.
Les adresses mal alignées sont interceptées par `VMError::MisalignedAccess`, correspondant à la VM
comportement exercé dans `crates/ivm/tests/wide_memory128.rs`.

Les programmes qui émettent ces assistants 128 bits doivent annoncer la capacité vectorielle.
Le compilateur Kotodama active automatiquement le bit de mode `VECTOR` à chaque fois
`LOAD128`/`STORE128` apparaissent ; la VM piège avec
`VMError::VectorExtensionDisabled` si un programme tente de les exécuter
sans ce bit défini.

## Abaissement conditionnel large des branches

Lorsque Kotodama abaisse une branche `if`/`else` ou une branche ternaire en bytecode large, il émet un
séquence `BNE cond, zero, +2` fixe suivie d'une paire d'instructions `JAL` :

1. Le court `BNE` maintient la branche conditionnelle dans la voie immédiate de 8 bits
   en sautant par-dessus la solution de repli `JAL`.
2. Le premier `JAL` cible le bloc `else` (exécuté lorsque la condition est
   faux).
3. Le deuxième `JAL` passe au bloc `then` (pris lorsque la condition est
   vrai).

Ce modèle garantit que la vérification des conditions n'a jamais besoin de coder des décalages plus grands.
plus de ± 127 mots tout en prenant en charge des corps arbitrairement grands pour le `then`
et les blocs `else` via le large assistant `JAL`. Voir
`crates/ivm/tests/kotodama.rs::branch_lowering_uses_short_bne_and_dual_jal` pour
le test de régression qui verrouille la séquence.

### Exemple d'abaissement

```
fn branch(b: bool) -> int {
    if b { 1 } else { 2 }
}
```

Compile selon le squelette d'instructions large suivant (numéros de registre et
les décalages absolus dépendent de la fonction englobante) :

```
BNE cond_reg, x0, +2    # skip the fallthrough jump when the condition is true
JAL x0, else_offset     # execute when the condition is false
JAL x0, then_offset     # execute when the condition is true
```

Les instructions suivantes matérialisent les constantes et écrivent la valeur de retour.
Étant donné que `BNE` saute par-dessus le premier `JAL`, le décalage conditionnel est toujours
Mots `+2`, gardant la branche à portée même lorsque les corps de bloc se dilatent.