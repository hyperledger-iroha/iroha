---
lang: es
direction: ltr
source: docs/source/kotodama_error_codes.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 5e0e4f16000f6a578fe9c9d6e204c01087e987ac3b46d70537a15b072df48a13
source_last_modified: "2026-01-03T18:08:01.373878+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Códigos de error del compilador Kotodama

El compilador Kotodama emite códigos de error estables para que los usuarios de herramientas y CLI puedan
entender rápidamente la causa de una falla. Utilice `koto_compile --explain <code>`
para imprimir la pista correspondiente.

| Código | Descripción | Solución típica |
|-------|-------------|-------------|
| `E0001` | El objetivo de bifurcación está fuera del rango para la codificación de salto IVM. | Divida funciones muy grandes o reduzca la inserción para que las distancias de bloque básicas se mantengan dentro de ±1 MiB. |
| `E0002` | Los sitios de llamadas hacen referencia a una función que nunca se definió. | Compruebe si hay errores tipográficos, modificadores de visibilidad o indicadores de funciones que eliminaron al destinatario. |
| `E0003` | Las llamadas al sistema de estado duradero se emitieron sin ABI v1 habilitado. | Configure `CompilerOptions::abi_version = 1` o agregue `meta { abi_version: 1 }` dentro del contrato `seiyaku`. |
| `E0004` | Las llamadas al sistema relacionadas con activos recibieron punteros no literales. | Utilice `account_id(...)`, `asset_definition(...)`, etc., o pase 0 centinelas para los valores predeterminados del host. |
| `E0005` | El inicializador de bucle `for` es más complejo de lo que se admite actualmente. | Mueva la configuración compleja antes del bucle; Actualmente solo se aceptan inicializadores de expresión/`let` simples. |
| `E0006` | La cláusula de paso del bucle `for` es más compleja de lo que se admite actualmente. | Actualice el contador de bucles con una expresión simple (por ejemplo, `i = i + 1`). |