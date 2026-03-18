---
lang: es
direction: ltr
source: docs/source/crypto/sm_audit_brief.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 9cda4648f0af7f89022e9d9f4ea243bc22685d9356927bbf1417c77b2057d872
source_last_modified: "2026-01-03T18:07:57.113286+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

% SM2/SM3/SM4 Informe de Auditoría Externa
% Iroha Grupo de trabajo criptográfico
% 2026-01-30

# Descripción general

Este informe presenta el contexto de ingeniería y cumplimiento requerido para una
Revisión independiente de la habilitación SM2/SM3/SM4 de Iroha. Se dirige a los equipos de auditoría.
con experiencia en criptografía Rust y familiaridad con el sistema nacional chino
Estándares de criptografía. El resultado esperado es un informe escrito que cubra
riesgos de implementación, brechas de conformidad y orientación de remediación priorizada
antes del lanzamiento de SM, pasando de la vista previa a la producción.

# Instantánea del programa

- **Alcance de la versión:** Iroha 2/3 base de código compartida, verificación determinista
  rutas a través de nodos y SDK, firma disponible detrás de la guardia de configuración.
- **Fase actual:** SM-P3.2 (integración backend OpenSSL/Tongsuo) con Rust
  implementaciones que ya se envían para verificación y casos de uso simétricos.
- **Fecha prevista de decisión:** 2026-04-30 (los resultados de la auditoría informan si se puede o no
  habilitar el inicio de sesión de SM en compilaciones de validador).
- **Riesgos clave rastreados:** pedigrí de dependencia de terceros, determinista
  comportamiento bajo hardware mixto, preparación para el cumplimiento del operador.

# Código y referencias de accesorios

- `crates/iroha_crypto/src/sm.rs` — Implementaciones de Rust y OpenSSL opcional
  fijaciones (característica `sm-ffi-openssl`).
- `crates/ivm/tests/sm_syscalls.rs` — IVM cobertura de llamada al sistema para hash,
  verificación y modos simétricos.
- `crates/iroha_data_model/tests/sm_norito_roundtrip.rs` — Carga útil Norito
  viajes de ida y vuelta para artefactos SM.
- `docs/source/crypto/sm_program.md`: historial del programa, auditoría de dependencia y
  barandillas desplegables.
- `docs/source/crypto/sm_operator_rollout.md` — habilitación orientada al operador y
  procedimientos de reversión.
- `docs/source/crypto/sm_compliance_brief.md` — resumen regulatorio y exportación
  consideraciones.
- `scripts/sm_openssl_smoke.sh` / `crates/iroha_crypto/tests/sm_openssl_smoke.rs`
  – arnés de humo determinista para flujos respaldados por OpenSSL.
- Corporaciones `fuzz/sm_*`: semillas fuzz basadas en RustCrypto que cubren primitivas SM3/SM4.

# Alcance de auditoría solicitado1. **Conformidad con las especificaciones**
   - Validar verificación de firma SM2, cálculo ZA y canónico
     comportamiento de codificación.
   - Confirmar que las primitivas SM3/SM4 siguen GM/T 0002-2012 y GM/T 0007-2012,
     incluyendo invariantes de modo contador y manejo intravenoso.
2. **Determinismo y garantías de tiempo constante**
   - Revisar las ramificaciones, las búsquedas de tablas y el envío de hardware para la ejecución del nodo.
     sigue siendo determinista entre familias de CPU.
   - Evaluar reclamos en tiempo constante para operaciones de clave privada y confirmar la
     Las rutas OpenSSL/Tongsuo conservan una semántica de tiempo constante.
3. **Análisis de fallas y canales laterales**
   - Inspeccionar los riesgos de sincronización, caché y canal lateral de energía tanto en Rust como
     Rutas de código respaldadas por FFI.
   - Evaluar el manejo de fallas y la propagación de errores para la verificación de firmas y
     Fallos de cifrado autenticados.
4. **Construcción, dependencia y revisión de la cadena de suministro**
   - Confirmar compilaciones reproducibles y procedencia de artefactos OpenSSL/Tongsuo.
   - Revisar la cobertura de licencias y auditorías del árbol de dependencia.
5. **Crítica del arnés de prueba y verificación**
   - Evaluar pruebas de humo deterministas, arneses de fuzz y accesorios Norito.
   - Recomendar cobertura adicional (por ejemplo, pruebas diferenciales, pruebas basadas en la propiedad)
     pruebas) si persisten lagunas.
6. **Validación de cumplimiento y guía del operador**
   - Verificar la documentación enviada con los requisitos legales y esperados.
     controles del operador.

# Entregables y Logística

- **Inicio:** 2026-02-24 (virtual, 90 minutos).
- **Entrevistas:** Crypto WG, mantenedores de IVM, operaciones de plataforma (según sea necesario).
- **Acceso a artefactos:** espejo del repositorio de solo lectura, registros de canalización de CI, accesorio
  salidas y SBOM de dependencia (CycloneDX).
- **Actualizaciones provisionales:** estado escrito semanal + avisos de riesgo.
- **Entregables finales (fecha prevista para el 15 de abril de 2026):**
  - Resumen ejecutivo con calificación de riesgo.
  - Hallazgos detallados (por problema: impacto, probabilidad, referencias de código,
    orientación de remediación).
  - Plan de reprueba/verificación.
  - Declaración sobre determinismo, postura de tiempo constante y alineación de cumplimiento.

## Estado de compromiso

| Proveedor | Estado | Inicio | Ventana de campo | Notas |
|--------|--------|----------|--------------|-------|
| Trail of Bits (práctica CN) | Declaración de obra ejecutada 2026-02-21 | 2026-02-24 | 2026-02-24–2026-03-22 | Entrega prevista para el 15 de abril de 2026; Hui Zhang lidera el compromiso con Alexey M. como contraparte de ingeniería. Llamada de estado semanal los miércoles a las 09:00 UTC. |
| Grupo NCC (APAC) | Espacio de contingencia reservado | N/A (en espera) | Provisional 2026-05-06–2026-05-31 | Activación sólo si los hallazgos de alto riesgo requieren un segundo pase; preparación confirmada por Priya N. (Seguridad) y la mesa de participación del Grupo NCC 2026-02-22. |

# Archivos adjuntos incluidos en el paquete de extensión- `docs/source/crypto/sm_program.md`
- `docs/source/crypto/sm_operator_rollout.md`
-`docs/source/crypto/sm_compliance_brief.md`
- `docs/source/crypto/sm_lock_refresh_plan.md`
- `docs/source/crypto/sm_rust_vector_check.md`
- `docs/source/crypto/attachments/sm_iroha_crypto_tree.txt` — Instantánea `cargo tree -p iroha_crypto --no-default-features --features "sm sm-ffi-openssl"`.
- `docs/source/crypto/attachments/sm_iroha_crypto_metadata.json` — Exportación `cargo metadata` para la caja `iroha_crypto` (gráfico de dependencia bloqueado).
- `docs/source/crypto/attachments/sm_openssl_smoke.log`: última ejecución de `scripts/sm_openssl_smoke.sh` (omite las rutas SM2/SM4 cuando falta soporte del proveedor).
- `docs/source/crypto/attachments/sm_openssl_provenance.md`: procedencia del kit de herramientas local (notas de la versión pkg-config/OpenSSL).
- Manifiesto de corpus difuso (`fuzz/sm_corpus_manifest.json`).

> **Advertencia del entorno:** La instantánea de desarrollo actual utiliza la cadena de herramientas OpenSSL 3.x suministrada (función `openssl` caja `vendored`), pero macOS carece de elementos intrínsecos de CPU SM3/SM4 y el proveedor predeterminado no expone SM4-GCM, por lo que el arnés de humo de OpenSSL aún omite la cobertura de SM4 y el análisis del ejemplo de SM2 del anexo. Un ciclo de dependencia del espacio de trabajo (`sorafs_manifest ↔ sorafs_car`) también obliga al script auxiliar a omitir la ejecución después de emitir el error `cargo check`. Vuelva a ejecutar el paquete dentro del entorno de compilación de la versión de Linux (OpenSSL/Tongsuo con SM4 habilitado y sin el ciclo) para capturar la paridad total antes de la auditoría externa.

# Socios de auditoría candidatos y alcance

| Empresa | Experiencia relevante | Alcance típico y entregables | Notas |
|------|---------------------|------------------------------|-------|
| Trail of Bits (práctica de criptografía CN) | Revisiones de código Rust (`ring`, zkVMs), evaluaciones previas de GM/T para pilas de pagos móviles. | Diferencia de conformidad de especificaciones (GM/T 0002/3/4), revisión en tiempo constante de las rutas Rust + OpenSSL, fuzzing diferencial, revisión de la cadena de suministro, hoja de ruta de remediación. | Ya comprometido; La tabla se conserva para que esté completa al planificar futuros ciclos de actualización. |
| Grupo NCC APAC | Equipos rojos de hardware/SOC + criptografía Rust, revisiones publicadas de primitivas RustCrypto y puentes HSM de pago. | Evaluación holística de los enlaces de Rust + JNI/FFI, validación de políticas deterministas, revisión de la puerta de rendimiento/telemetría, guía del manual del operador. | Reservado como contingencia; También puede proporcionar informes bilingües para los reguladores chinos. |
| Kudelski Security (equipo de Blockchain y criptografía) | Auditorías de Halo2, Mina, zkSync, esquemas de firma personalizados implementados en Rust. | Concéntrese en la corrección de la curva elíptica, la integridad de la transcripción, el modelado de amenazas para la aceleración de hardware y la evidencia de implementación/CI. | Útil para segundas opiniones sobre aceleración de hardware (SM-5a) e interacciones FASTPQ a SM. |
| Mínima autoridad | Auditorías de protocolos criptográficos para blockchains basadas en Rust (Filecoin, Polkadot), consultoría de compilaciones reproducibles. | Verificación de compilación determinista, verificación del códec Norito, verificación cruzada de evidencia de cumplimiento, revisión de comunicación del operador. | Muy adecuado para entregables de informes de auditoría/transparencia cuando los reguladores solicitan una verificación independiente más allá de la revisión del código. |

Todos los compromisos solicitan el mismo paquete de artefactos enumerado anteriormente más los siguientes complementos opcionales según la empresa:- **Conformidad de especificaciones y comportamiento determinista:** Verificación línea por línea de la derivación SM2 ZA, el relleno SM3, las funciones redondas SM4 y la puerta de despacho de tiempo de ejecución `sm_accel` para garantizar que la aceleración nunca altere la semántica.
- **Revisión de canal lateral y FFI:** Inspección de reclamos de tiempo constante, bloques de código inseguro y capas puente OpenSSL/Tongsuo, incluidas pruebas de diferencias con la ruta de Rust.
- **Validación de CI/cadena de suministro:** Reproducción de los arneses `sm_interop_matrix`, `sm_openssl_smoke` e `sm_perf` junto con las certificaciones SBOM/SLSA para que los hallazgos de la auditoría se puedan vincular directamente a la evidencia de publicación.
- **Garantía de cara al operador:** Verificación cruzada de `sm_operator_rollout.md`, plantillas de presentación de cumplimiento y paneles de telemetría para confirmar que las mitigaciones prometidas en la documentación son técnicamente ejecutables.

Al evaluar el alcance de auditorías futuras, reutilice esta tabla para alinear las fortalezas del proveedor con el hito específico de la hoja de ruta (por ejemplo, favorezca a Kudelski para versiones pesadas de hardware/rendimiento, Trail of Bits para la corrección del lenguaje/tiempo de ejecución y Least Authority para garantías de compilación reproducibles).

# Puntos de contacto

- **Propietario técnico:** Líder de Crypto WG (Alexey M., `alexey@iroha.tech`)
- **Gerente de programa:** Coordinadora de operaciones de plataforma (Sarah K.,
  `sarah@iroha.tech`)
- **Enlace de seguridad:** Ingeniería de seguridad (Priya N., `security@iroha.tech`)
- **Enlace de documentación:** Líder de Docs/DevRel (Jamila R.,
  `docs@iroha.tech`)