---
lang: es
direction: ltr
source: docs/source/crypto/sm_operator_rollout.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: dffc2cf6c6e59f54d1fc22136ba93f75466509c699a4361a381bf7e0ce0d1dda
source_last_modified: "2026-01-03T18:07:57.089544+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Lista de verificación de telemetría e implementación de funciones SM

Esta lista de verificación ayuda a los equipos de SRE y operadores a habilitar la función SM (SM2/SM3/SM4)
configurar de forma segura una vez que se hayan superado los controles de auditoría y cumplimiento. Sigue este documento
junto con el resumen de configuración en `docs/source/crypto/sm_program.md` y el
orientación legal/exportación en `docs/source/crypto/sm_compliance_brief.md`.

## 1. Preparación previa al vuelo
- [] Confirme que las notas de la versión del espacio de trabajo muestren `sm` como de solo verificación o firma,
      dependiendo de la etapa de implementación.
- [] Verifique que la flota esté ejecutando binarios creados a partir de una confirmación que incluya el
      Contadores de telemetría SM y botones de configuración. (Lanzamiento objetivo por determinar; seguimiento
      en el ticket de lanzamiento.)
- [] Ejecute `scripts/sm_perf.sh --tolerance 0.25` en un nodo de prueba (por objetivo
      arquitectura) y archivar el resultado resumido. El script ahora se selecciona automáticamente
      la línea de base escalar como objetivo de comparación para los modos de aceleración
      (`--compare-tolerance` tiene el valor predeterminado 5.25 mientras aterriza el SM3 NEON);
      investigar o bloquear el lanzamiento si el principal o el de comparación
      la guardia falla. Al capturar en hardware Linux/aarch64 Neoverse, pase
      `--baseline crates/iroha_crypto/benches/sm_perf_baseline_aarch64_unknown_linux_gnu_<mode>.json --write-baseline`
      para sobrescribir las medianas `m3-pro-native` exportadas con la captura del host
      antes del envío.
- [] Asegúrese de que `status.md` y el ticket de implementación registren las presentaciones de cumplimiento para
      cualquier nodo que opere en jurisdicciones que los requieran (consulte el informe de cumplimiento).
- [] Prepare actualizaciones de KMS/HSM si los validadores almacenarán las claves de firma de SM en
      módulos de hardware.

## 2. Cambios de configuración
1. Ejecute el asistente xtask para generar el inventario de claves SM2 y el fragmento listo para pegar:
   ```bash
   cargo xtask sm-operator-snippet \
     --distid CN12345678901234 \
     --json-out sm2-key.json \
     --snippet-out client-sm2.toml
   ```
   Utilice `--snippet-out -` (y opcionalmente `--json-out -`) para transmitir las salidas a la salida estándar cuando solo necesite inspeccionarlas.
   Si prefiere ejecutar los comandos CLI de nivel inferior manualmente, el flujo equivalente es:
   ```bash
   cargo run -p iroha_cli --features sm -- \
     crypto sm2 keygen \
     --distid CN12345678901234 \
     --output sm2-key.json

   cargo run -p iroha_cli --features sm -- \
     crypto sm2 export \
     --private-key-hex "$(jq -r .private_key_hex sm2-key.json)" \
     --distid CN12345678901234 \
     --snippet-output client-sm2.toml \
     --emit-json --quiet
   ```
   Si `jq` no está disponible, abra `sm2-key.json`, copie el valor `private_key_hex` y páselo directamente al comando de exportación.
2. Agregue el fragmento resultante a la configuración de cada nodo (los valores se muestran para el
   etapa de solo verificación; ajuste por entorno y mantenga las claves ordenadas como se muestra):
```toml
[crypto]
default_hash = "sm3-256"
allowed_signing = ["ed25519", "sm2"]   # remove "sm2" to stay in verify-only mode
sm2_distid_default = "1234567812345678"
# enable_sm_openssl_preview = true  # optional: only when deploying the OpenSSL/Tongsuo path
```
3. Reinicie el nodo y confirme la superficie `crypto.sm_helpers_available` y (si habilitó el backend de vista previa) `crypto.sm_openssl_preview_enabled` como se esperaba en:
   - `/status` JSON (`"crypto":{"sm_helpers_available":true,"sm_openssl_preview_enabled":true,...}`).
   - El `config.toml` renderizado para cada nodo.
4. Actualice las entradas de manifiestos/génesis para agregar algoritmos SM a la lista de permitidos si
   la firma se habilita más adelante en la implementación. Cuando se utiliza `--genesis-manifest-json`
   sin un bloque de génesis prefirmado, `irohad` ahora genera la criptografía en tiempo de ejecución
   instantánea directamente desde el bloque `crypto` del manifiesto; asegúrese de que el manifiesto esté
   verificó su plan de cambios antes de continuar.## 3. Telemetría y monitoreo
- Elimine los puntos finales Prometheus y asegúrese de que aparezcan los siguientes contadores/medidores:
  - `iroha_sm_syscall_total{kind="verify"}`
  - `iroha_sm_syscall_total{kind="hash"}`
  - `iroha_sm_syscall_total{kind="seal|open",mode="gcm|ccm"}`
  - `iroha_sm_openssl_preview` (medidor 0/1 que informa el estado de alternancia de vista previa)
  - `iroha_sm_syscall_failures_total{kind="verify|hash|seal|open",reason="..."}`
- Ruta de firma de enlace una vez habilitada la firma SM2; agregar contadores para
  `iroha_sm_sign_total` y `iroha_sm_sign_failures_total`.
- Crear paneles/alertas Grafana para:
  - Picos en contadores de fallos (ventana 5m).
  - Caídas repentinas en el rendimiento de las llamadas al sistema SM.
  - Diferencias entre nodos (por ejemplo, habilitación no coincidente).

## 4. Pasos de implementación
| Fase | Acciones | Notas |
|-------|---------|-------|
| Sólo verificación | Actualice `crypto.default_hash` a `sm3-256`, deje `allowed_signing` sin `sm2`, monitoree los contadores de verificación. | Objetivo: ejercitar rutas de verificación SM sin correr el riesgo de divergencia en el consenso. |
| Piloto de Fichaje Mixto | Permitir firma SM limitada (subconjunto de validadores); monitorear los contadores de firmas y la latencia. | Asegúrese de que el respaldo a Ed25519 siga estando disponible; detenerse si la telemetría muestra discrepancias. |
| Firma GA | Amplíe `allowed_signing` para incluir `sm2`, actualice manifiestos/SDK y publique el runbook final. | Requiere resultados de auditoría cerrados, presentaciones de cumplimiento actualizadas y telemetría estable. |

### Revisiones de preparación
- **Verificar solo la preparación (SM-RR1).** Convocar a Release Eng, Crypto WG, Ops y Legal. Requerir:
  - `status.md` observa el estado de presentación de cumplimiento + procedencia de OpenSSL.
  - `docs/source/crypto/sm_program.md` / `sm_compliance_brief.md` / esta lista de verificación se actualizó en la última ventana de lanzamiento.
  - `defaults/genesis` o el manifiesto específico del entorno muestra `crypto.allowed_signing = ["ed25519","sm2"]` e `crypto.default_hash = "sm3-256"` (o la variante de solo verificación sin `sm2` si todavía está en la etapa uno).
  - Registros `scripts/sm_openssl_smoke.sh` + `scripts/sm_interop_matrix.sh` adjuntos al ticket de implementación.
  - Panel de telemetría (`iroha_sm_*`) revisado para comprobar su comportamiento en estado estable.
- **Firma de preparación del piloto (SM-RR2).** Puertas adicionales:
  - Informe de auditoría de stack de RustCrypto SM cerrado o RFC para controles de compensación firmados por Seguridad.
  - Runbooks del operador (específicos de la instalación) actualizados con pasos de respaldo/reversión de firma.
  - Los manifiestos de Génesis para la cohorte piloto incluyen `allowed_signing = ["ed25519","sm2"]` y la lista de permitidos se refleja en la configuración de cada nodo.
  - Plan de salida/reversión documentado (cambiar `allowed_signing` nuevamente a Ed25519, restaurar manifiestos, restablecer paneles).
- **Preparación GA (SM-RR3).** Requiere un informe piloto positivo, presentaciones de cumplimiento actualizadas para todas las jurisdicciones de validadores, líneas base de telemetría firmadas y aprobación del ticket de lanzamiento por parte de la tríada Release Eng + Crypto WG + Ops/Legal.## 5. Lista de verificación de embalaje y cumplimiento
- **Agrupe artefactos de OpenSSL/Tongsuo.** Envíe bibliotecas compartidas OpenSSL/Tongsuo 3.0+ (`libcrypto`/`libssl`) con cada paquete de validación o documente la dependencia exacta del sistema. Registre la versión, los indicadores de compilación y las sumas de verificación SHA256 en el manifiesto de lanzamiento para que los auditores puedan rastrear la compilación del proveedor.
- **Verificar durante CI.** Agregue un paso de CI que ejecute `scripts/sm_openssl_smoke.sh` en los artefactos empaquetados en cada plataforma de destino. El trabajo debe fallar si el indicador de vista previa está habilitado pero el proveedor no se puede inicializar (faltan encabezados, algoritmo no compatible, etc.).
- **Publicar notas de cumplimiento.** Actualice las notas de la versión / `status.md` con la versión del proveedor incluida, referencias de control de exportaciones (GM/T, GB/T) y cualquier presentación específica de jurisdicción requerida para los algoritmos SM.
- **Actualizaciones del runbook del operador.** Documente el flujo de actualización: prepare los nuevos objetos compartidos, reinicie los pares con `crypto.enable_sm_openssl_preview = true`, confirme el campo `/status` y el indicador `iroha_sm_openssl_preview` cambie a `true` y mantenga un plan de reversión (cambie la bandera de configuración o revierta el paquete) si la telemetría de vista previa se desvía en toda la flota.
- **Retención de evidencia.** Archive los registros de compilación y las certificaciones de firma para los paquetes OpenSSL/Tongsuo junto con los artefactos de lanzamiento del validador para que futuras auditorías puedan reproducir la cadena de procedencia.

## 6. Respuesta a incidentes
- **Picos de falla de verificación:** Revertir a una compilación sin soporte SM o eliminar `sm2`
  desde `allowed_signing` (revirtiendo `default_hash` según sea necesario) y conmutando por error al anterior
  liberación mientras se investiga. Capture cargas útiles fallidas, hashes comparativos y registros de nodos.
- **Regresiones de rendimiento:** Compare las métricas de SM con las líneas de base de Ed25519/SHA2.
  Si la ruta intrínseca de ARM causa divergencia, configure `crypto.sm_intrinsics = "force-disable"`
  (alternancia de función pendiente de implementación) e informar los resultados.
- **Brechas de telemetría:** Si faltan contadores o no se actualizan, presente un problema
  contra Release Engineering; no continúe con un despliegue más amplio hasta que el espacio
  está resuelto.

## 7. Plantilla de lista de verificación
- [] Configuración preparada y reiniciada por el par.
- [ ] Contadores de telemetría visibles y paneles de control configurados.
- [ ] Cumplimiento/medidas legales registradas.
- [] Fase de implementación aprobada por Crypto WG / Release TL.
- [ ] Revisión posterior a la implementación completada y hallazgos documentados.

Mantenga esta lista de verificación en el ticket de implementación y actualice `status.md` cuando
transiciones de flota entre fases.