---
lang: es
direction: ltr
source: docs/source/examples/lookup_grand_product.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 6f6421d420a704c5c4af335741e309adf641702ddb8c291dce94ea5581557a66
source_last_modified: "2026-01-03T18:08:00.673232+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Ejemplo de búsqueda de gran producto

Este ejemplo amplía el argumento de búsqueda de permisos FASTPQ mencionado en
`fastpq_plan.md`.  En la etapa 2, el probador evalúa al selector.
(`s_perm`) y columnas testigo (`perm_hash`) en la extensión de bajo grado (LDE)
dominio, actualiza un gran producto en ejecución `Z_i` y finalmente confirma todo el
Secuencia con Poseidón.  El acumulador hash se adjunta a la transcripción.
bajo el dominio `fastpq:v1:lookup:product`, mientras que el `Z_i` final aún coincide
el producto de la tabla de permisos comprometidos `T`.

Consideramos un lote pequeño con los siguientes valores de selector:

| fila | `s_perm` | `perm_hash` |
| --- | -------- | ---------------------------------------------- |
| 0 | 1 | `0x019a...` (función de concesión = auditor, permanente = transferencia_activo) |
| 1 | 0 | `0xabcd...` (sin cambio de permiso) |
| 2 | 1 | `0x42ff...` (revocar rol = auditor, permanente = burn_asset) |

Sea `gamma = 0xdead...` el desafío de búsqueda Fiat-Shamir derivado del
transcripción.  El probador inicializa `Z_0 = 1` y dobla cada fila:

```
Z_0 = 1
Z_1 = Z_0 * (perm_hash_0 + gamma)^(s_perm_0) = 1 * (0x019a... + gamma)
Z_2 = Z_1 * (perm_hash_1 + gamma)^(s_perm_1) = Z_1 (selector is zero)
Z_3 = Z_2 * (perm_hash_2 + gamma)^(s_perm_2)
```

Las filas donde `s_perm = 0` no alteran el acumulador.  Después de procesar el
rastro, el demostrador Poseidón codifica la secuencia `[Z_1, Z_2, ...]` para la transcripción
pero también publica `Z_final = Z_3` (el producto final en ejecución) para que coincida con la tabla
condición de frontera.

En el lado de la tabla, el árbol Merkle de permisos comprometidos codifica el determinista
conjunto de permisos activos para la ranura.  El verificador (o el probador durante
generación de testigos) calcula

```
T = product over entries: (entry.hash + gamma)
```

El protocolo aplica la restricción de límite `Z_final / T = 1`.  si el rastro
introdujo un permiso que no está presente en la tabla (u omitió uno que
es), la relación del gran producto diverge de 1 y el verificador la rechaza.  porque
ambos lados se multiplican por `(value + gamma)` dentro del campo Ricitos de Oro, la relación
permanece estable en todos los backends de CPU/GPU.

Para serializar el ejemplo como Norito JSON para aparatos, registre la tupla de
`perm_hash`, selector y acumulador después de cada fila, por ejemplo:

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

Los marcadores de posición hexadecimales (`0x...`) se pueden reemplazar con Ricitos de Oro concretos.
elementos de campo al generar pruebas automatizadas.  Accesorios de la etapa 2 además
registrar el hash Poseidon del acumulador en ejecución pero mantener la misma forma JSON,
por lo que el ejemplo puede servir como plantilla para futuros vectores de prueba.