---
lang: es
direction: ltr
source: docs/source/compliance/android/eu/security_target.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 385d17a55579d2b0b365e21090ee081ded79e44655690b2abfbf54068c9b55b0
source_last_modified: "2026-01-03T18:07:59.195967+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Objetivo de seguridad del SDK de Android: alineación ETSI EN 319 401

| Campo | Valor |
|-------|-------|
| Versión del documento | 0.1 (2026-02-12) |
| Alcance | SDK de Android (bibliotecas cliente bajo `java/iroha_android/` más scripts/docs de soporte) |
| Propietario | Cumplimiento y Legal (Sofia Martins) |
| Revisores | Líder de programa de Android, Ingeniería de lanzamiento, Gobernanza de SRE |

## 1. Descripción del DEDO DEL PIE

El objetivo de evaluación (TOE) comprende el código de la biblioteca SDK de Android (`java/iroha_android/src/main/java`), su superficie de configuración (ingestión `ClientConfig` + Norito) y las herramientas operativas a las que se hace referencia en `roadmap.md` para los hitos AND2/AND6/AND7.

Componentes primarios:

1. **Ingestión de configuración**: `ClientConfig` subprocesos puntos finales Torii, políticas TLS, reintentos y enlaces de telemetría del manifiesto `iroha_config` generado y aplica la inmutabilidad posterior a la inicialización (`java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/ClientConfig.java`).
2. **Administración de claves/StrongBox**: la firma respaldada por hardware se implementa a través de `SystemAndroidKeystoreBackend` e `AttestationVerifier`, con políticas documentadas en `docs/source/sdk/android/key_management.md`. La captura/validación de atestación utiliza `scripts/android_keystore_attestation.sh` y el asistente de CI `scripts/android_strongbox_attestation_ci.sh`.
3. **Telemetría y redacción**: la instrumentación canaliza a través del esquema compartido descrito en `docs/source/sdk/android/telemetry_redaction.md`, exportando autoridades hash, perfiles de dispositivos agrupados y anulando ganchos de auditoría aplicados por el Support Playbook.
4. **Runbooks de operaciones**: `docs/source/android_runbook.md` (respuesta del operador) e `docs/source/android_support_playbook.md` (SLA + escalamiento) fortalecen la huella operativa del TOE con anulaciones deterministas, simulacros de caos y captura de evidencia.
5. **Procedencia de la versión**: las compilaciones basadas en Gradle utilizan el complemento CycloneDX más indicadores de compilación reproducibles como se captura en `docs/source/sdk/android/developer_experience_plan.md` y la lista de verificación de cumplimiento AND6. Los artefactos de la versión están firmados y con referencias cruzadas en `docs/source/release/provenance/android/`.

## 2. Activos y supuestos

| Activo | Descripción | Objetivo de seguridad |
|-------|-------------|--------------------|
| Manifiestos de configuración | Instantáneas de `ClientConfig` derivadas de Norito distribuidas con aplicaciones. | Autenticidad, integridad y confidencialidad en reposo. |
| Claves de firma | Claves generadas o importadas a través de proveedores StrongBox/TEE. | Preferencia de StrongBox, registro de atestación, sin exportación de claves. |
| Flujos de telemetría | Seguimientos/registros/métricas de OTLP exportados desde la instrumentación del SDK. | Seudonimización (autoridades hash), PII minimizada, anulación de auditoría. |
| Interacciones del libro mayor | Cargas útiles Norito, metadatos de admisión, tráfico de red Torii. | Autenticación mutua, solicitudes resistentes a la reproducción, reintentos deterministas. |

Supuestos:

- El sistema operativo móvil proporciona sandboxing estándar + SELinux; Los dispositivos StrongBox implementan la interfaz keymaster de Google.
- Los operadores suministran puntos finales Torii con certificados TLS firmados por CA de confianza del consejo.
- La infraestructura de construcción cumple con los requisitos de construcción reproducible antes de publicarla en Maven.

## 3. Amenazas y controles| Amenaza | Controlar | Evidencia |
|--------|---------|----------|
| Manifiestos de configuración manipulados | `ClientConfig` valida los manifiestos (hash + esquema) antes de aplicarlos y registra las recargas denegadas a través de `android.telemetry.config.reload`. | `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/ClientConfig.java`; `docs/source/android_runbook.md` §1–2. |
| Compromiso de firma de claves | Las políticas requeridas por StrongBox, los arneses de certificación y las auditorías de matriz de dispositivos identifican la desviación; anulaciones documentadas por incidente. | `docs/source/sdk/android/key_management.md`; `docs/source/sdk/android/readiness/android_strongbox_device_matrix.md`; `scripts/android_strongbox_attestation_ci.sh`. |
| Fuga de PII en telemetría | Autoridades con hash Blake2b, perfiles de dispositivos agrupados, omisión del operador, anulación de registros. | `docs/source/sdk/android/telemetry_redaction.md`; Manual de estrategias de apoyo §8. |
| Reproducir o degradar en Torii RPC | El generador de solicitudes `/v2/pipeline` aplica la fijación de TLS, la política de canal de ruido y los presupuestos de reintento con contexto de autoridad hash. | `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/ToriiRequestBuilder.java`; `docs/source/sdk/android/networking.md` (planificado). |
| Autorizaciones sin firmar o no reproducibles | Atestaciones de CycloneDX SBOM + Sigstore controladas por la lista de verificación AND6; Los RFC de publicación requieren evidencia en `docs/source/release/provenance/android/`. | `docs/source/sdk/android/developer_experience_plan.md`; `docs/source/compliance/android/eu/sbom_attestation.md`. |
| Manejo incompleto de incidentes | Runbook + playbook definen anulaciones, simulacros de caos y árbol de escalada; las anulaciones de telemetría requieren solicitudes Norito firmadas. | `docs/source/android_runbook.md`; `docs/source/android_support_playbook.md`. |

## 4. Actividades de evaluación

1. **Revisión de diseño**: Cumplimiento + SRE verifican que la configuración, la administración de claves, la telemetría y los controles de liberación se correspondan con los objetivos de seguridad de ETSI.
2. **Comprobaciones de implementación** — Pruebas automatizadas:
   - `scripts/android_strongbox_attestation_ci.sh` verifica los paquetes capturados para cada dispositivo StrongBox enumerado en la matriz.
   - `scripts/check_android_samples.sh` y Managed Device CI garantizan que las aplicaciones de muestra respeten los contratos de telemetría/`ClientConfig`.
3. **Validación operativa**: simulacros de caos trimestrales según `docs/source/sdk/android/telemetry_chaos_checklist.md` (ejercicios de redacción + anulación).
4. **Retención de evidencia**: artefactos almacenados en `docs/source/compliance/android/` (esta carpeta) y referenciados desde `status.md`.

## 5. Mapeo ETSI EN 319 401| Cláusula EN 319 401 | Control SDK |
|-------------------|-------------|
| 7.1 Política de seguridad | Documentado en este objetivo de seguridad + Manual de estrategias de soporte. |
| 7.2 Seguridad organizacional | RACI + propiedad de guardia en el Libro de estrategias de soporte §2. |
| 7.3 Gestión de activos | Objetivos de configuración, clave y activos de telemetría definidos en el §2 anterior. |
| 7.4 Control de acceso | Políticas de StrongBox + anulación del flujo de trabajo que requiere artefactos Norito firmados. |
| 7.5 Controles criptográficos | Requisitos de generación, almacenamiento y certificación de claves de AND2 (guía de administración de claves). |
| 7.6 Seguridad de las operaciones | Hash de telemetría, ensayos de caos, respuesta a incidentes y liberación de pruebas. |
| 7.7 Seguridad de las comunicaciones | `/v2/pipeline` Política TLS + autoridades hash (documento de redacción de telemetría). |
| 7.8 Adquisición/desarrollo del sistema | Construcciones de Gradle reproducibles, SBOM y puertas de procedencia en planos AND5/AND6. |
| 7.9 Relaciones con proveedores | Atestaciones de Buildkite + Sigstore registradas junto con SBOM de dependencia de terceros. |
| 7.10 Gestión de incidentes | Escalado de Runbook/Playbook, anulación de registros, contadores de errores de telemetría. |

## 6. Mantenimiento

- Actualice este documento cada vez que el SDK introduzca nuevos algoritmos criptográficos, categorías de telemetría o cambios en la automatización de versiones.
- Vincular copias firmadas en `docs/source/compliance/android/evidence_log.csv` con resúmenes SHA-256 y aprobaciones de revisores.