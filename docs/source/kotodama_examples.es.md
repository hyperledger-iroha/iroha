---
lang: es
direction: ltr
source: docs/source/kotodama_examples.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 168513edcb6624ab76275b01aaaf6ab9dee310b9d6f5a2960504a9545801c511
source_last_modified: "2026-01-28T15:34:14.183250+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Kotodama Descripción general de ejemplos

Esta página muestra ejemplos concisos de Kotodama y cómo se asignan a las llamadas al sistema IVM y a los argumentos de puntero-ABI. Ver también:
- `examples/` para fuentes ejecutables
- `docs/source/ivm_syscalls.md` para la llamada al sistema canónica ABI
- `kotodama_grammar.md` para la especificación de idioma completa

## Hola + Detalle de cuenta

Fuente: `examples/hello/hello.ko`

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

Mapeo (puntero-ABI):
- `authority()` → `SCALL 0xA4` (el host escribe `&AccountId` en `r10`)
- `set_account_detail(a, k, v)` → mover `r10=&AccountId`, `r11=&Name`, `r12=&Json`, luego `SCALL 0x1A`

## Transferencia de activos

Fuente: `examples/transfer/transfer.ko`

```
seiyaku TransferDemo {
  kotoage fn do_transfer() permission(AssetTransferRole) {
    transfer_asset(
      account!("soraゴヂアヌャェボヰセキュホュヨモチゥカッパダォレジゴシホセギツキゴヒョヲヌタシャッヱロゥテニョヒシホイヌヘ"),
      account!("soraゴヂアニラショリャヒャャサピテヶベチュヲボヹヂギタクアニョロホドチャヘヱヤジヶハシャウンベニョャルフハケネキカ"),
      asset_definition!("62Fk4FPcMuLvW5QjDGNF2a4jAmjM"),
      10
    );
  }
}
```

Mapeo (puntero-ABI):
- `transfer_asset(from, to, def, amt)` → `r10=&AccountId(from)`, `r11=&AccountId(to)`, `r12=&AssetDefinitionId(def)`, `r13=amount`, luego `SCALL 0x24`

## NFT Crear + Transferir

Fuente: `examples/nft/nft.ko`

```
seiyaku NftDemo {
  kotoage fn create() permission(NftAuthority) {
    let owner = account!("soraゴヂアヌャェボヰセキュホュヨモチゥカッパダォレジゴシホセギツキゴヒョヲヌタシャッヱロゥテニョヒシホイヌヘ");
    let nft = nft_id!("dragon$wonderland");
    nft_mint_asset(nft, owner);
  }

  kotoage fn transfer() permission(NftAuthority) {
    let owner = account!("soraゴヂアヌャェボヰセキュホュヨモチゥカッパダォレジゴシホセギツキゴヒョヲヌタシャッヱロゥテニョヒシホイヌヘ");
    let recipient = account!("soraゴヂアニラショリャヒャャサピテヶベチュヲボヹヂギタクアニョロホドチャヘヱヤジヶハシャウンベニョャルフハケネキカ");
    let nft = nft_id!("dragon$wonderland");
    nft_transfer_asset(owner, nft, recipient);
  }
}
```

Mapeo (puntero-ABI):
- `nft_mint_asset(id, owner)` → `r10=&NftId`, `r11=&AccountId(owner)`, `SCALL 0x25`
- `nft_transfer_asset(from, id, to)` → `r10=&AccountId(from)`, `r11=&NftId`, `r12=&AccountId(to)`, `SCALL 0x26`

## Puntero Norito Ayudantes

El estado duradero valorado por puntero requiere convertir los TLV escritos hacia y desde el
El sobre `NoritoBytes` que alberga los hosts persiste. Kotodama ahora conecta estos ayudantes
directamente a través del compilador para que los constructores puedan usar valores predeterminados de puntero y mapear
Búsquedas sin pegamento FFI manual:

```
seiyaku PointerDemo {
  state Owners: Map<int, AccountId>;

  fn hajimari() {
    let alice = account_id("soraゴヂアヌャェボヰセキュホュヨモチゥカッパダォレジゴシホセギツキゴヒョヲヌタシャッヱロゥテニョヒシホイヌヘ");
    let first = get_or_insert_default(Owners, 7, alice);
    assert(first == alice);

    // The second call decodes the stored pointer and re-encodes the input.
    let bob = account_id("soraゴヂアニラショリャヒャャサピテヶベチュヲボヹヂギタクアニョロホドチャヘヱヤジヶハシャウンベニョャルフハケネキカ");
    let again = get_or_insert_default(Owners, 7, bob);
    assert(again == alice);
  }
}
```

Bajando:

- Los valores predeterminados del puntero emiten `POINTER_TO_NORITO` después de publicar el TLV escrito, por lo que
  el host recibe una carga útil canónica `NoritoBytes` para almacenamiento.
- Las lecturas realizan la operación inversa con `POINTER_FROM_NORITO`, suministrando el
  ID de tipo de puntero esperado en `r11`.
- Ambas rutas publican automáticamente TLV literales en la región INPUT, lo que permite
  contratos para mezclar literales de cadena y punteros de tiempo de ejecución de forma transparente.

Consulte `crates/ivm/tests/kotodama_pointer_args.rs` para ver una regresión en tiempo de ejecución que
ejerce el ida y vuelta contra el `MockWorldStateView`.

## Iteración de mapa determinista (diseño)

El mapa determinista para cada uno requiere un límite. La iteración de entradas múltiples requiere un mapa de estado; el compilador acepta `.take(n)` o una longitud máxima declarada.

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

Semántica:
- El conjunto de iteraciones es una instantánea de la entrada del bucle; El orden es lexicográfico por Norito bytes de la clave.
- Mutaciones estructurales a `M` en la trampa de bucle con `E_ITER_MUTATION`.
- Sin límite, el compilador emite `E_UNBOUNDED_ITERATION`.

## Componentes internos del compilador/host (Rust, no fuente Kotodama)

Los fragmentos a continuación se encuentran en el lado Rust de la cadena de herramientas. Ilustran los ayudantes del compilador y la mecánica de reducción de VM y **no** son una fuente Kotodama `.ko` válida.

## Actualizaciones de marcos fragmentados de código de operación amplio

Los amplios asistentes de código de operación del Kotodama se dirigen al diseño de operandos de 8 bits utilizado por el IVM.
codificación amplia. Las cargas y almacenes que mueven valores de 128 bits reutilizan el tercer operando
ranura para el registro alto, por lo que el registro base ya debe contener el registro final
dirección. Ajuste la base con un `ADDI` antes de emitir la carga/almacenamiento:

```
use ivm::kotodama::wide::{encode_addi_checked, encode_load128, encode_store128};

fn emit_store_pair(base: u8, lo: u8, hi: u8) -> [u32; 2] {
    let adjust = encode_addi_checked(base, base, 16).expect("16-byte chunk");
    let store = encode_store128(base, lo, hi);
    [adjust, store]
}
```Las actualizaciones de tramas fragmentadas hacen avanzar la base en pasos de 16 bytes, lo que garantiza que el registro
El par comprometido por `STORE128` aterriza en el límite de alineación requerido. lo mismo
el patrón se aplica a `LOAD128`; emitiendo un `ADDI` con la zancada deseada antes
cada carga mantiene el registro de destino alto vinculado a la tercera ranura de operando.
Trampa de direcciones desalineadas con `VMError::MisalignedAccess`, que coincide con la VM
comportamiento ejercido en `crates/ivm/tests/wide_memory128.rs`.

Los programas que emiten estos ayudantes de 128 bits deben anunciar capacidad vectorial.
El compilador Kotodama habilita el bit de modo `VECTOR` automáticamente siempre que
Aparece `LOAD128`/`STORE128`; las trampas VM con
`VMError::VectorExtensionDisabled` si un programa intenta ejecutarlos
sin ese bit configurado.

## Bajada de rama condicional amplia

Cuando Kotodama reduce una `if`/`else` o una rama ternaria a un código de bytes amplio, emite un
secuencia fija `BNE cond, zero, +2` seguida de un par de instrucciones `JAL`:

1. El corto `BNE` mantiene la rama condicional dentro del carril inmediato de 8 bits.
   saltando el fallo `JAL`.
2. El primer `JAL` apunta al bloque `else` (se ejecuta cuando la condición es
   falso).
3. El segundo `JAL` salta al bloque `then` (tomado cuando la condición es
   cierto).

Este patrón garantiza que la verificación de condición nunca necesite codificar compensaciones mayores
de ±127 palabras y al mismo tiempo admite cuerpos arbitrariamente grandes para el `then`
y bloques `else` a través del amplio asistente `JAL`. Ver
`crates/ivm/tests/kotodama.rs::branch_lowering_uses_short_bne_and_dual_jal` para
la prueba de regresión que bloquea la secuencia.

### Ejemplo de descenso

```
fn branch(b: bool) -> int {
    if b { 1 } else { 2 }
}
```

Se compila en el siguiente esqueleto de instrucciones amplio (números de registro y
Las compensaciones absolutas dependen de la función envolvente):

```
BNE cond_reg, x0, +2    # skip the fallthrough jump when the condition is true
JAL x0, else_offset     # execute when the condition is false
JAL x0, then_offset     # execute when the condition is true
```

Las instrucciones posteriores materializan las constantes y escriben el valor de retorno.
Debido a que `BNE` salta sobre el primer `JAL`, el desplazamiento condicional siempre es
`+2` palabras, manteniendo la rama dentro del alcance incluso cuando los cuerpos del bloque se expanden.