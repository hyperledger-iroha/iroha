---
lang: es
direction: ltr
source: docs/source/kotodama_grammar.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: f9d64b88546c924258ef054d8071b38230f3f19c8a7d920f9594b0ecb84252ce
source_last_modified: "2025-12-04T09:32:10.286919+00:00"
translation_last_reviewed: 2026-01-05
---

# Gramática y semántica del lenguaje Kotodama

Este documento especifica la sintaxis del lenguaje Kotodama (análisis léxico y gramática), las reglas de tipado, la semántica determinista y cómo los programas se reducen a bytecode de IVM (`.to`) con convenciones de pointer-ABI de Norito. Los archivos fuente de Kotodama usan la extensión `.ko`. El compilador emite bytecode de IVM (`.to`) y opcionalmente puede devolver un manifiesto.

Contenido
- Panorama general y objetivos
- Estructura léxica
- Tipos y literales
- Declaraciones y módulos
- Contenedor de contrato y metadatos
- Funciones y parámetros
- Sentencias
- Expresiones
- Builtins y constructores pointer-ABI
- Colecciones y mapas
- Iteración determinista y límites
- Errores y diagnósticos
- Mapeo de generación de código a IVM
- ABI, encabezado y manifiesto
- Hoja de ruta

## Panorama general y objetivos

- Determinista: los programas deben producir resultados idénticos en todo hardware; no hay punto flotante ni fuentes no deterministas. Todas las interacciones con el host ocurren mediante syscalls con argumentos codificados en Norito.
- Portable: apunta a bytecode de Iroha Virtual Machine (IVM), no a una ISA física. Las codificaciones tipo RISC‑V visibles en el repositorio son detalles de implementación del decodificador de IVM y no deben cambiar el comportamiento observable.
- Auditable: semántica pequeña y explícita; mapeo claro de la sintaxis a opcodes de IVM y a syscalls del host.
- Acotamiento: los bucles sobre datos sin límites deben llevar límites explícitos. La iteración de mapas tiene reglas estrictas para garantizar el determinismo.

## Estructura léxica

Espacios en blanco y comentarios
- El espacio en blanco separa tokens y, por lo demás, es insignificante.
- Los comentarios de línea empiezan con `//` y llegan hasta el fin de línea.
- Los comentarios de bloque `/* ... */` no se anidan.

Identificadores
- Comienzan con `[A-Za-z_]` y continúan con `[A-Za-z0-9_]*`.
- Son sensibles a mayúsculas y minúsculas; `_` es un identificador válido pero desaconsejado.

Palabras clave (reservadas)
- `seiyaku`, `hajimari`, `kotoage`, `kaizen`, `state`, `struct`, `fn`, `let`, `const`, `return`, `if`, `else`, `while`, `for`, `in`, `break`, `continue`, `true`, `false`, `permission`, `kotoba`.

Operadores y signos de puntuación
- Aritméticos: `+ - * / %`
- Bit a bit: `& | ^ ~`, desplazamientos `<< >>`
- Comparación: `== != < <= > >=`
- Lógicos: `&& || !`
- Asignación: `= += -= *= /= %= &= |= ^= <<= >>=`
- Varios: `: , ; . :: ->`
- Paréntesis y llaves: `() [] {}`

Literales
- Entero: decimal (`123`), hex (`0x2A`), binario (`0b1010`). Todos los enteros son de 64 bits con signo en tiempo de ejecución; los literales sin sufijo se tipan por inferencia o como `int` por defecto.
- Cadena: entre comillas dobles con escapes `\`; UTF‑8.
- Booleano: `true`, `false`.

## Tipos y literales

Tipos escalares
- `int`: 64 bits en complemento a dos; la aritmética envuelve módulo 2^64 para suma/resta/multiplicación; la división tiene variantes con y sin signo definidas en IVM; el compilador elige la operación apropiada para la semántica.
- `bool`: valor lógico; se reduce a `0`/`1`.
- `string`: cadena UTF‑8 inmutable; se representa como TLV Norito al pasar a syscalls; en la VM se usan slices de bytes y longitud.
- `bytes`: payload Norito sin procesar; alias del tipo `Blob` del pointer-ABI para entradas de hash/cripto/pruebas y superposiciones durables.

Tipos compuestos
- `struct Name { field: Type, ... }` tipos producto definidos por el usuario. Los constructores usan sintaxis de llamada `Name(a, b, ...)` en expresiones. El acceso a campos `obj.field` está soportado y se reduce internamente a campos posicionales estilo tupla. El ABI de estado durable en cadena está codificado con Norito; el compilador emite overlays que reflejan el orden del struct y pruebas recientes (`crates/iroha_core/tests/kotodama_struct_overlay.rs`) mantienen el layout bloqueado entre versiones.
- `Map<K, V>`: mapa asociativo determinista; la semántica restringe la iteración y las mutaciones durante la iteración (ver abajo).
- `Tuple (T1, T2, ...)`: tipo producto anónimo con campos posicionales; usado para retornos múltiples.

Tipos especiales pointer-ABI (de cara al host)
- `AccountId`, `AssetDefinitionId`, `Name`, `Json`, `NftId`, `Blob` y similares no son tipos de primera clase en tiempo de ejecución. Son constructores que generan punteros tipados e inmutables a la región INPUT (envolturas TLV de Norito) y solo pueden usarse como argumentos de syscalls o moverse entre variables sin mutación.

Inferencia de tipos
- Los `let` locales infieren el tipo desde el inicializador. Los parámetros de función deben estar tipados explícitamente. Los tipos de retorno pueden inferirse salvo que haya ambigüedad.

## Declaraciones y módulos

Elementos de nivel superior
- Contratos: `seiyaku Name { ... }` contienen funciones, estado, structs y metadatos.
- Se permiten múltiples contratos por archivo pero se desaconsejan; un `seiyaku` principal se usa como entrada por defecto en los manifiestos.
- Las declaraciones `struct` definen tipos de usuario dentro de un contrato.

Visibilidad
- `kotoage fn` indica un punto de entrada público; la visibilidad afecta los permisos del dispatcher, no la generación de código.

## Contenedor de contrato y metadatos

Sintaxis
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

Semántica
- `meta { ... }` anula los valores por defecto del compilador para el encabezado IVM emitido: `abi_version`, `vector_length` (0 significa sin establecer), `max_cycles` (0 significa el valor por defecto del compilador), `features` activa bits de características del encabezado (trazado ZK, anuncio de vector). Las características no soportadas se ignoran con una advertencia. Cuando se omite `meta {}`, el compilador emite `abi_version = 1` y usa los valores por defecto de las opciones para los demás campos del encabezado.
- `features: ["zk", "simd"]` (alias: `"vector"`) solicita explícitamente los bits de encabezado correspondientes. Las cadenas de características desconocidas ahora producen un error de parser en lugar de ignorarse.
- `state` declara variables de contrato durables. El compilador baja los accesos a syscalls `STATE_GET/STATE_SET/STATE_DEL` y el host los guarda en un overlay por transaccion (checkpoint/restore para rollback, flush en el commit hacia WSV). Se emiten access hints para rutas literales; las claves dinamicas caen a conflictos a nivel de mapa. Para lecturas/escrituras explicitas del host, usa los helpers `state_get/state_set/state_del` y los helpers de mapa `get_or_insert_default`; pasan por TLVs Norito y mantienen nombres/orden de campos estables.
- Los identificadores `state` están reservados; sombrear un nombre de `state` en parámetros o `let` se rechaza (`E_STATE_SHADOWED`).
- Los valores de mapas de estado no son de primera clase: usa el identificador de estado directamente para operaciones de mapa e iteración. Enlazar o pasar mapas de estado a funciones definidas por el usuario se rechaza (`E_STATE_MAP_ALIAS`).
- Los mapas de estado durables actualmente admiten solo tipos de clave `int` y pointer-ABI; otros tipos de clave se rechazan en compilación.
- Los campos de estado durable deben ser `int`, `bool`, `Json`, `Blob`/`bytes` o tipos pointer-ABI (incluidas structs/tuplas compuestas por estos campos); `string` no se admite para estado durable.

## Declaraciones de disparadores

Las declaraciones de disparadores adjuntan metadatos de programación a los manifiestos de puntos
de entrada y se registran automáticamente cuando se activa una instancia de contrato (se eliminan
al desactivarla). Se analizan dentro de un bloque `seiyaku`.

Sintaxis
```
register_trigger wake {
  call run;
  on time pre_commit;
  repeats 2;
  metadata { tag: "alpha"; count: 1; enabled: true; }
}
```

Notas
- `call` debe referenciar un `kotoage fn` público del mismo contrato; un `namespace::entrypoint`
  opcional se registra en el manifiesto pero los callbacks entre contratos se rechazan por ahora
  (solo callbacks locales).
- Filtros soportados: `time pre_commit` y `time schedule(start_ms, period_ms?)`, más
  `execute trigger <name>` para triggers por llamada, `data any` para eventos de datos y filtros
  de pipeline (`pipeline transaction`, `pipeline block`, `pipeline merge`, `pipeline witness`).
- Los valores de metadata deben ser literales JSON (`string`, `number`, `bool`, `null`) o
  `json!(...)`.
- Claves de metadata inyectadas por el runtime: `contract_namespace`, `contract_id`,
  `contract_entrypoint`, `contract_code_hash`, `contract_trigger_id`.

## Funciones y parámetros

Sintaxis
- Declaración: `fn name(param1: Type, param2: Type, ...) -> Ret { ... }`
- Pública: `kotoage fn name(...) { ... }`
- Inicializador: `hajimari() { ... }` (invocado al desplegar por el runtime, no por la VM en sí).
- Gancho de actualización: `kaizen(args...) permission(Role) { ... }`.

Parámetros y retornos
- Los argumentos se pasan en registros `r10..r22` como valores o punteros INPUT (TLV Norito) según el ABI; argumentos adicionales se derraman a la pila.
- Las funciones devuelven cero o un escalar o tupla. El valor de retorno primario va en `r10` para escalar; las tuplas se materializan en la pila/OUTPUT por convención.

## Sentencias

- Enlaces de variables: `let x = expr;`, `let mut x = expr;` (la mutabilidad es una verificación en compilación; la mutación en tiempo de ejecución se permite solo para locales).
- Asignación: `x = expr;` y formas compuestas `x += 1;` etc. Los destinos deben ser variables o índices de mapa; los campos de tuplas/structs son inmutables.
- Control: `if (cond) { ... } else { ... }`, `while (cond) { ... }`, `for (init; cond; step) { ... }` estilo C.
  - Los inicializadores y pasos de `for` deben ser `let name = expr` simples o sentencias de expresión; el destructuring complejo se rechaza (`E0005`, `E0006`).
  - Alcance de `for`: los enlaces de la cláusula init son visibles en el bucle y después de él; los enlaces creados en el cuerpo o el paso no salen del bucle.
- Igualdad (`==`, `!=`) se admite para `int`, `bool`, `string`, escalares pointer-ABI (p. ej., `AccountId`, `Name`, `Blob`/`bytes`, `Json`); tuplas, structs y mapas no son comparables.
- Bucle de mapa: `for (k, v) in map { ... }` (determinista; ver abajo).
- Flujo: `return expr;`, `break;`, `continue;`.
- Llamada: `name(args...);` o `call name(args...);` (ambas aceptadas; el compilador normaliza a sentencias de llamada).
- Aserciones: `assert(cond);`, `assert_eq(a, b);` mapean a `ASSERT*` de IVM en builds no‑ZK o a restricciones ZK en modo ZK.

## Expresiones

Precedencia (alta → baja)
1. Miembro/índice: `a.b`, `a[b]`
2. Unario: `! ~ -`
3. Multiplicativo: `* / %`
4. Aditivo: `+ -`
5. Desplazamientos: `<< >>`
6. Relacional: `< <= > >=`
7. Igualdad: `== !=`
8. AND/XOR/OR bit a bit: `& ^ |`
9. AND/OR lógico: `&& ||`
10. Ternario: `cond ? a : b`

Llamadas y tuplas
- Las llamadas usan argumentos posicionales: `f(a, b, c)`.
- Literal de tupla: `(a, b, c)` y destructuring: `let (x, y) = pair;`.
- El destructuring de tuplas requiere tipos tupla/struct con aridad coincidente; las descoincidencias se rechazan.

Cadenas y bytes
- Las cadenas son UTF‑8; las funciones que requieren bytes crudos aceptan punteros `Blob` mediante constructores (ver Builtins).

## Builtins y constructores pointer-ABI

Constructores de punteros (emiten TLV Norito en INPUT y devuelven un puntero tipado)
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

Las macros del preludio proporcionan alias más cortos y validación en línea para estos constructores:
- `account!("<i105-account-id>")`, `account_id!("<i105-account-id>")`
- `asset_definition!("62Fk4FPcMuLvW5QjDGNF2a4jAmjM")`, `asset_id!("62Fk4FPcMuLvW5QjDGNF2a4jAmjM")`
- `domain!("wonderland")`, `domain_id!("wonderland")`
- `name!("example")`
- `json!("{\"hello\":\"world\"}")` o literales estructurados como `json!{ hello: "world" }`
- `nft_id!("dragon$demo")`, `blob!("bytes")`, `norito_bytes!("...")`

Las macros se expanden a los constructores anteriores y rechazan literales inválidos en tiempo de compilación.

Estado de implementación
- Implementado: los constructores anteriores aceptan argumentos de cadena literal y se reducen a envolturas TLV de Norito tipadas colocadas en la región INPUT. Devuelven punteros tipados inmutables utilizables como argumentos de syscalls. Las expresiones de cadena no literales se rechazan; usa `Blob`/`bytes` para entradas dinámicas. `blob`/`norito_bytes` también aceptan valores de tipo `bytes` en tiempo de ejecución sin macros.
- Formas extendidas:
  - `json(Blob[NoritoBytes]) -> Json*` vía syscall `JSON_DECODE`.
  - `name(Blob[NoritoBytes]) -> Name*` vía syscall `NAME_DECODE`.
  - Decodificación de punteros desde Blob/NoritoBytes: cualquier constructor de puntero (incluidos tipos AXT) acepta un payload `Blob`/`NoritoBytes` y se reduce a `POINTER_FROM_NORITO` con el id de tipo esperado.
  - Paso directo para formas pointer: `name(Name) -> Name*`, `blob(Blob) -> Blob*`, `norito_bytes(Blob) -> Blob*`.
  - Se admite azúcar de método: `s.name()`, `s.json()`, `b.blob()`, `b.norito_bytes()`.

Builtins de host/syscall (mapean a SCALL; números exactos en ivm.md)
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

Builtins de utilidades
- `info(string|int)`: emite un evento/mensaje estructurado vía OUTPUT.
- `hash(blob) -> Blob*`: devuelve un hash codificado en Norito como Blob.
- `build_submit_ballot_inline(election_id, ciphertext, nullifier32, backend, proof, vk) -> Blob*` y `build_unshield_inline(asset, to, amount, inputs32, backend, proof, vk) -> Blob*`: constructores ISI en línea; todos los argumentos deben ser literales en tiempo de compilación (literales de cadena o constructores de puntero desde literales). `nullifier32` e `inputs32` deben tener exactamente 32 bytes (cadena cruda o hex `0x`), y `amount` debe ser no negativo.
- `schema_info(Name*) -> Json* { "id": "<hex>", "version": N }`
- `pointer_to_norito(ptr) -> NoritoBytes*`: envuelve un TLV pointer-ABI existente como NoritoBytes para almacenamiento o transporte.
- `isqrt(int) -> int`: raíz cuadrada entera (`floor(sqrt(x))`) implementada como opcode de IVM.
- `min(int, int) -> int`, `max(int, int) -> int`, `abs(int) -> int`, `div_ceil(int, int) -> int`, `gcd(int, int) -> int`, `mean(int, int) -> int` — helpers aritméticos fusionados respaldados por opcodes nativos de IVM (la división techo atrapa en división por cero).

Notas
- Los builtins son shims delgados; el compilador los reduce a movimientos de registros y un `SCALL`.
- Los constructores de puntero son puros: la VM garantiza que el TLV de Norito en INPUT es inmutable durante la duración de la llamada.
 - Los structs con campos pointer‑ABI (p. ej., `DomainId`, `AccountId`) pueden usarse para agrupar argumentos de syscall de forma ergonómica. El compilador mapea `obj.field` al registro/valor correcto sin asignaciones extra.

## Colecciones y mapas

Tipo: `Map<K, V>`
- Los mapas en memoria (asignados en heap vía `Map::new()` o pasados como parámetros) almacenan un solo par clave/valor; las claves y valores deben ser tipos de tamaño palabra: `int`, `bool`, `string`, `Blob`, `bytes`, `Json` o tipos de puntero (p. ej., `AccountId`, `Name`).
- Los mapas de estado durables (`state Map<...>`) usan claves/valores codificados con Norito. Claves soportadas: `int` o tipos de puntero. Valores soportados: `int`, `bool`, `Json`, `Blob`/`bytes` o tipos de puntero.
- `Map::new()` asigna e inicializa en cero la entrada única en memoria (clave/valor = 0); para mapas no `Map<int,int>`, proporciona una anotación de tipo explícita o tipo de retorno.
- Los mapas de estado no son valores de primera clase: no puedes reasignarlos (p. ej., `M = Map::new()`); actualiza entradas mediante indexación (`M[key] = value`).
- Operaciones:
  - Indexación: `map[key]` obtiene/establece valor (el set se realiza vía syscall del host; ver mapeo de API de runtime).
  - Existencia: `contains(map, key) -> bool` (helper reducido; puede ser un syscall intrínseco).
  - Iteración: `for (k, v) in map { ... }` con orden determinista y reglas de mutación.

Reglas de iteración determinista
- El conjunto de iteración es la instantánea de claves al entrar en el bucle.
- El orden es estrictamente lexicográfico ascendente por bytes de claves codificadas con Norito.
- Las modificaciones estructurales (insertar/eliminar/limpiar) al mapa iterado durante el bucle causan una trampa determinista `E_ITER_MUTATION`.
- Se requiere acotamiento: ya sea un máximo declarado (`@max_len`) en el mapa, un atributo explícito `#[bounded(n)]` o un límite explícito usando `.take(n)`/`.range(..)`; de lo contrario el compilador emite `E_UNBOUNDED_ITERATION`.

Helpers de límites
- `#[bounded(n)]`: atributo opcional en la expresión de mapa, p. ej. `for (k, v) in my_map #[bounded(2)] { ... }`.
- `.take(n)`: itera las primeras `n` entradas desde el inicio.
- `.range(start, end)`: itera entradas en el intervalo semiabierto `[start, end)`. La semántica equivale a `start` y `n = end - start`.

Notas sobre límites dinámicos
- Límites literales: `n`, `start` y `end` como literales enteros están totalmente soportados y compilan a un número fijo de iteraciones.
- Límites no literales: cuando la característica `kotodama_dynamic_bounds` está habilitada en el crate `ivm`, el compilador acepta expresiones dinámicas `n`, `start` y `end` e inserta aserciones en tiempo de ejecución para seguridad (no negativas, `end >= start`). La reducción emite hasta K iteraciones guardadas con `if (i < n)` para evitar ejecuciones extra del cuerpo (K por defecto = 2). Puedes ajustar K programáticamente mediante `CompilerOptions { dynamic_iter_cap, .. }`.
- Ejecuta `koto_lint` para inspeccionar advertencias de lint de Kotodama antes de la compilación; el compilador principal siempre procede con la reducción tras parseo y chequeo de tipos.
- Los códigos de error están documentados en [Kotodama Compiler Error Codes](./kotodama_error_codes.md); usa `koto_compile --explain <code>` para explicaciones rápidas.

## Errores y diagnósticos

Diagnósticos en compilación (ejemplos)
- `E_UNBOUNDED_ITERATION`: un bucle sobre mapa carece de límite.
- `E_MUT_DURING_ITER`: mutación estructural del mapa iterado en el cuerpo del bucle.
- `E_STATE_SHADOWED`: los enlaces locales no pueden sombrear declaraciones `state`.
- `E_BREAK_OUTSIDE_LOOP`: `break` usado fuera de un bucle.
- `E_CONTINUE_OUTSIDE_LOOP`: `continue` usado fuera de un bucle.
- `E0005`: el inicializador del for es más complejo de lo soportado.
- `E0006`: la cláusula step del for es más compleja de lo soportado.
- `E_BAD_POINTER_USE`: uso del resultado de un constructor pointer-ABI donde se requiere un tipo de primera clase.
- `E_UNRESOLVED_NAME`, `E_TYPE_MISMATCH`, `E_ARITY_MISMATCH`, `E_DUP_SYMBOL`.
- Herramientas: `koto_compile` ejecuta el lint antes de emitir bytecode; usa `--no-lint` para omitirlo o `--deny-lint-warnings` para fallar el build si hay salida de lint.

Errores de VM en tiempo de ejecución (seleccionados; lista completa en ivm.md)
- `E_NORITO_INVALID`, `E_OOB`, `E_UNALIGNED`, `E_SCALL_UNKNOWN`, `E_ASSERT`, `E_ASSERT_EQ`, `E_ITER_MUTATION`.

Mensajes de error
- Los diagnósticos llevan `msg_id`s estables que mapean a entradas en tablas de traducción `kotoba {}` cuando están disponibles.

## Mapeo de generación de código a IVM

Pipeline
1. Lexer/Parser producen el AST.
2. El análisis semántico resuelve nombres, comprueba tipos y rellena tablas de símbolos.
3. Reducción de IR a una forma simple tipo SSA.
4. Asignación de registros a GPRs de IVM (`r10+` para args/ret por convención); derrames a la pila.
5. Emisión de bytecode: mezcla de codificaciones nativas de IVM y compatibles con RV según convenga; encabezado de metadatos emitido con `abi_version`, features, longitud de vector y `max_cycles`.

Aspectos destacados del mapeo
- Aritmética y lógica mapean a ops ALU de IVM.
- Ramas y control mapean a saltos condicionales; el compilador usa formas comprimidas cuando conviene.
- La memoria para locales se derrama a la pila de la VM; la alineación se impone.
- Los builtins se reducen a movimientos de registros y `SCALL` con número de 8 bits.
- Los constructores de punteros colocan TLVs Norito en la región INPUT y producen sus direcciones.
- Las aserciones mapean a `ASSERT`/`ASSERT_EQ`, que atrapan en ejecución no‑ZK y emiten restricciones en builds ZK.

Restricciones de determinismo
- Sin FP; sin syscalls no deterministas.
- La aceleración SIMD/GPU es invisible al bytecode y debe ser bit‑idéntica; el compilador no emite operaciones específicas de hardware.

## ABI, encabezado y manifiesto

Campos del encabezado IVM establecidos por el compilador
- `version`: versión del formato de bytecode IVM (major.minor).
- `abi_version`: versión de la tabla de syscalls y del esquema pointer-ABI.
- `feature_bits`: flags de características (p. ej., `ZK`, `VECTOR`).
- `vector_len`: longitud lógica del vector (0 → sin establecer).
- `max_cycles`: límite de admisión y pista de relleno ZK.

Manifiesto (sidecar opcional)
- `code_hash`, `abi_hash`, metadatos del bloque `meta {}`, versión del compilador y pistas de build para reproducibilidad.

## Hoja de ruta

- **KD-231 (Abr 2026):** añadir análisis de rango en compilación para límites de iteración para que los bucles expongan conjuntos de acceso acotados al planificador.
- **KD-235 (May 2026):** introducir un escalar `bytes` de primera clase distinto de `string` para constructores de punteros y claridad de ABI.
- **KD-242 (Jun 2026):** ampliar el conjunto de opcodes builtins (hash / verificación de firma) detrás de flags de características con fallbacks deterministas.
- **KD-247 (Jun 2026):** estabilizar `msg_id`s de error y mantener el mapeo en tablas `kotoba {}` para diagnósticos localizados.
### Emisión de manifiesto

- La API del compilador de Kotodama puede devolver un `ContractManifest` junto al `.to` compilado mediante `ivm::kotodama::compiler::Compiler::compile_source_with_manifest`.
- Campos:
  - `code_hash`: hash de los bytes de código (excluyendo el encabezado IVM y literales) calculado por el compilador para vincular el artefacto.
  - `abi_hash`: digest estable de la superficie de syscalls permitida para el `abi_version` del programa (ver `ivm.md` y `ivm::syscalls::compute_abi_hash`).
- `compiler_fingerprint` y `features_bitmap` opcionales están reservados para toolchains.
- `entrypoints`: lista ordenada de entrypoints exportados (públicos, `hajimari`, `kaizen`) incluyendo sus cadenas `permission(...)` requeridas y los mejores esfuerzos del compilador en pistas de claves de lectura/escritura para que la admisión y los schedulers razonen sobre el acceso esperado al WSV.
- El manifiesto está pensado para chequeos de admisión y para registros; ver `docs/source/new_pipeline.md` para el ciclo de vida.
