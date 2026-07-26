---
lang: es
direction: ltr
source: docs/source/examples/smt_update.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 788902cfafc6c7db6d52d4237b46ffe78193efd57852bc3427a16d7f3cda2f9c
source_last_modified: "2026-01-03T18:08:00.438859+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Ejemplo de actualización dispersa de Merkle

Este ejemplo resuelto ilustra cómo la traza FASTPQ Etapa 2 codifica un
testigo de no membresía utilizando la columna `neighbour_leaf`. El escaso árbol Merkle
es binario sobre elementos de campo Poseidon2. Las claves se convierten a canónicas.
Cadenas little-endian de 32 bytes, codificadas en un elemento de campo, y la mayoría
Los bits significativos seleccionan la rama en cada nivel.

## Escenario

- Licencias previas al estado
  - `asset::alice::rose` -> clave hash `0x12b7...` con valor `0x0000_0000_0000_0005`.
  - `asset::bob::rose` -> clave hash `0x1321...` con valor `0x0000_0000_0000_0003`.
- Solicitud de actualización: insertar `asset::carol::rose` con valor 2.
- El hash de clave canónica de Carol se expande al prefijo de 5 bits `0b01011`. el
  los vecinos existentes tienen los prefijos `0b01010` (Alice) y `0b01101` (Bob).

Debido a que no existe ninguna hoja cuyo prefijo coincida con `0b01011`, el probador debe proporcionar
evidencia adicional de que el intervalo `(alice, bob)` está vacío. La etapa 2 se completa
la fila de seguimiento a través de las columnas `path_bit_{level}`, `sibling_{level}`,
`node_in_{level}` e `node_out_{level}` (con `level` en `[0, 31]`). Todos los valores
son elementos de campo Poseidon2 codificados en forma little-endian:

| nivel | `path_bit_level` | `sibling_level` | `node_in_level` | `node_out_level` | Notas |
| ----- | ---------------- | --------------------------- | ------------------------------------ | ------------------------------------ | ----- |
| 0 | 1 | `0x241f...` (hachís de hoja de Alicia) | `0x0000...` | `0x4b12...` (`value_2 = 2`) | Insertar: comenzar desde cero, almacenar un nuevo valor. |
| 1 | 1 | `0x7d45...` (derecha vacía) | Poseidón2(`node_out_0`, `sibling_0`) | Poseidón2(`sibling_1`, `node_out_1`) | Siga el bit de prefijo 1. |
| 2 | 0 | `0x03ae...` (rama Bob) | Poseidón2(`node_out_1`, `sibling_1`) | Poseidón2(`node_in_2`, `sibling_2`) | La rama se invierte porque bit = 0. |
| 3 | 1 | `0x9bc4...` | Poseidón2(`node_out_2`, `sibling_2`) | Poseidón2(`sibling_3`, `node_out_3`) | Los niveles más altos continúan subiendo. |
| 4 | 0 | `0xe112...` | Poseidón2(`node_out_3`, `sibling_3`) | Poseidón2(`node_in_4`, `sibling_4`) | Nivel de raíz; El resultado es la raíz post-estado. |

La columna `neighbour_leaf` para esta fila está completa con la hoja de Bob.
(`key = 0x1321...`, `value = 3`, `hash = Poseidon2(key, value) = 0x03ae...`). cuando
verificando, el AIR comprueba que:

1. El vecino suministrado corresponde al hermano utilizado en el nivel 2.
2. La clave vecina es lexicográficamente mayor que la clave insertada y la
   el hermano izquierdo (Alice) es lexicográficamente más pequeño.
3. Reemplazar la hoja insertada con la vecina reproduce la raíz anterior al estado.En conjunto, estas comprobaciones demuestran que no existía ninguna hoja para el intervalo `(0b01010,
0b01101)` antes de la actualización. Las implementaciones que generan seguimientos FASTPQ pueden utilizar
este diseño palabra por palabra; Las constantes numéricas anteriores son ilustrativas. por un completo
Testigo JSON, emite las columnas exactamente como aparecen en la tabla anterior (con
sufijos numéricos por nivel), utilizando cadenas de bytes little-endian serializadas con
Norito Ayudantes JSON.