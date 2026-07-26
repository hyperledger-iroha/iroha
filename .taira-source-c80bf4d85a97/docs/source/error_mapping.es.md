---
lang: es
direction: ltr
source: docs/source/error_mapping.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: cba8780bcec4ebf562dc9c5725f328b0ea2d9009517efa5b5a504e2fb6be81fe
source_last_modified: "2026-01-18T05:31:56.950113+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Guía de mapeo de errores

Última actualización: 2025-08-21

Esta guía asigna modos de falla comunes en Iroha a categorías de error estables detectadas por el modelo de datos. Úselo para diseñar pruebas y hacer que el manejo de errores del cliente sea predecible.

Principios
- Las rutas de instrucción y consulta emiten enumeraciones estructuradas. Evite los pánicos; reportar una categoría específica siempre que sea posible.
- Las categorías son estables, los mensajes pueden evolucionar. Los clientes deben coincidir en categorías, no en cadenas de formato libre.

Categorías
- InstrucciónExecutionError::Buscar: Falta entidad (activo, cuenta, dominio, NFT, función, activador, permiso, clave pública, bloque, transacción). Ejemplo: eliminar una clave de metadatos inexistente produce Find(MetadataKey).
- InstrucciónExecutionError::Repetición: Registro duplicado o ID en conflicto. Contiene el tipo de instrucción y el IdBox repetido.
- InstrucciónExecutionError::Mintability: Invariante de mintability violado (`Once` agotado dos veces, `Limited(n)` sobregirado o intentos de desactivar `Infinitely`). Ejemplos: acuñar dos veces un activo definido como `Once` produce `Mintability(MintUnmintable)`; la configuración de `Limited(0)` produce `Mintability(InvalidMintabilityTokens)`.
- InstrucciónExecutionError::Math: errores de dominio numérico (desbordamiento, división por cero, valor negativo, cantidad insuficiente). Ejemplo: quemar más cantidad de la disponible produce Math(NotEnoughQuantity).
- InstrucciónExecutionError::InvalidParameter: parámetro o configuración de instrucción no válidos (por ejemplo, activación de tiempo en el pasado). Úselo para cargas útiles de contratos con formato incorrecto.
- InstrucciónExecutionError::Evaluar: DSL/especificaciones no coinciden para la forma o los tipos de instrucción. Ejemplo: la especificación numérica incorrecta para el valor de un activo produce Evaluate(Type(AssetNumericSpec(..))).
- InstrucciónExecutionError::InvariantViolation: Violación de una invariante del sistema que no se puede expresar en otras categorías. Ejemplo: intentar eliminar al último firmante.
- InstrucciónExecutionError::Query: ajuste de QueryExecutionFail cuando una consulta falla durante la ejecución de la instrucción.

Error de ejecución de consulta
- Buscar: entidad faltante en el contexto de la consulta.
- Conversión: tipo incorrecto esperado por una consulta.
- NotFound: falta el cursor de consulta en vivo.
- CursorMismatch / CursorDone: Errores de protocolo del cursor.
- FetchSizeTooBig: se superó el límite impuesto por el servidor.
- GasBudgetExceeded: la ejecución de la consulta superó el presupuesto de gas/materialización.
- InvalidSingularParameters: parámetros no admitidos para consultas singulares.
- CapacityLimit: se alcanzó la capacidad del almacén de consultas en vivo.

Consejos de prueba
- Preferir pruebas unitarias cercanas al origen de un error. Por ejemplo, se puede generar una discrepancia en las especificaciones numéricas de los activos en las pruebas del modelo de datos.
- Las pruebas de integración deben cubrir el mapeo de extremo a extremo para casos representativos (por ejemplo, registro duplicado, clave faltante al eliminar, transferencia sin propiedad).
- Mantenga las aserciones resistentes haciendo coincidir variantes de enumeración en lugar de subcadenas de mensajes.