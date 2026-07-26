---
lang: es
direction: ltr
source: docs/source/crypto/sm_lock_refresh_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 3065571b34a226a5871c4fb68063f9419e48074b20096de215f440bdf54a4e59
source_last_modified: "2026-01-03T18:07:57.085103+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

//! Procedimiento para programar la actualización de Cargo.lock requerida por el pico de SM.

# SM Característica `Cargo.lock` Plan de actualización

El pico de características `sm` para `iroha_crypto` originalmente no pudo completar `cargo check` mientras se aplicaba `--locked`. Esta nota registra los pasos de coordinación para una actualización `Cargo.lock` autorizada y rastrea el estado actual de esa necesidad.

> **Actualización del 12 de febrero de 2026:** La validación reciente muestra que la función opcional `sm` ahora se compila con el archivo de bloqueo existente (`cargo check -p iroha_crypto --features sm --locked` tiene éxito en 7,9 s en frío/0,23 s en caliente). El conjunto de dependencias ya contiene `base64ct`, `ghash`, `opaque-debug`, `pem-rfc7468`, `pkcs8`, `polyval`, `primeorder`, `sm2`, `sm3`, `sm4` e `sm4-gcm`, por lo que no se requiere una actualización de bloqueo inmediata. Mantenga el procedimiento a continuación en espera para futuros aumentos de dependencia o nuevas cajas opcionales.

## Por qué es necesaria la actualización
- Las iteraciones anteriores del pico requerían agregar cajas opcionales que faltaban en el archivo de bloqueo. Las instantáneas de bloqueo actuales ya incluyen la pila RustCrypto (`sm2`, `sm3`, `sm4`, códecs compatibles y ayudantes AES).
- La política del repositorio aún bloquea las ediciones oportunistas de archivos de bloqueo; Si es necesaria una actualización de dependencia futura, el procedimiento siguiente sigue siendo aplicable.
- Conserve este plan para que el equipo pueda ejecutar una actualización controlada cuando se introduzcan nuevas dependencias relacionadas con SM o las existentes necesiten mejoras de versión.

## Pasos de coordinación propuestos
1. **Crear solicitud en Crypto WG + Release Eng sync (propietario: @crypto-wg líder).**
   - Haga referencia a `docs/source/crypto/sm_program.md` y tenga en cuenta la naturaleza opcional de la función.
   - Confirme que no hay ventanas de cambio de archivos de bloqueo simultáneas (por ejemplo, congelaciones de dependencia).
2. **Prepare el parche con lock diff (propietario: @release-eng).**
   - Ejecute `scripts/sm_lock_refresh.sh` (después de la aprobación) para actualizar solo las cajas requeridas.
   - Capturar la salida `cargo tree -p iroha_crypto --features sm` (el script emite `target/sm_dep_tree.txt`).
3. **Revisión de seguridad (propietario: @security-reviews).**
   - Verificar que las nuevas cajas/versiones coincidan con el registro de auditoría y las expectativas de licencia.
   - Registro de hashes en el rastreador de la cadena de suministro.
4. **Ejecución de ventana de combinación.**
   - Enviar PR que contenga solo el delta del archivo de bloqueo, la instantánea del árbol de dependencia (adjunta como artefacto) y notas de auditoría actualizadas.
   - Asegúrese de que CI se ejecute con `cargo check -p iroha_crypto --features sm` antes de la fusión.
5. **Tareas de seguimiento.**
   - Actualizar la lista de verificación de elementos de acción `docs/source/crypto/sm_program.md`.
   - Notificar al equipo del SDK que la función se puede compilar localmente con `--features sm`.## Cronología y propietarios
| Paso | Objetivo | Propietario | Estado |
|------|--------|-------|--------|
| Solicite un espacio en la agenda en la próxima convocatoria de Crypto WG | 2025-01-22 | Líder del Grupo de Trabajo sobre Cripto | ✅ Completado (la revisión concluyó que el pico puede continuar sin actualización) |
| Borrador de comando selectivo `cargo update` + diferencia de cordura | 2025-01-24 | Ingeniería de lanzamiento | ⚪ En espera (reactivar si aparecen nuevas cajas) |
| Revisión de seguridad de cajas nuevas | 2025-01-27 | Reseñas de seguridad | ⚪ En espera (reutilice la lista de verificación de auditoría cuando se reanude la actualización) |
| Fusionar actualización de archivo de bloqueo PR | 2025-01-29 | Ingeniería de lanzamiento | ⚪ En espera |
| Actualizar la lista de verificación de documentos del programa SM | Después de fusionar | Líder del Grupo de Trabajo sobre Cripto | ✅ Abordado vía entrada `docs/source/crypto/sm_program.md` (2026-02-12) |

## Notas
- Mantenga cualquier actualización futura restringida a las cajas relacionadas con SM enumeradas anteriormente (y a los asistentes de soporte como `rfc6979`), evitando `cargo update` en todo el espacio de trabajo.
- Si alguna dependencia transitiva introduce una deriva de MSRV, expóngala antes de fusionarla.
- Una vez fusionado, habilite un trabajo de CI efímero para monitorear los tiempos de compilación de la función `sm`.