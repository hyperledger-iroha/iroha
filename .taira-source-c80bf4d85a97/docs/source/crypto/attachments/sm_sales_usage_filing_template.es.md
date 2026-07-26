---
lang: es
direction: ltr
source: docs/source/crypto/attachments/sm_sales_usage_filing_template.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 14f32b40ff71fa4eef698eac80d8d7dd27104b46b84523d735d054dedea1c47a
source_last_modified: "2026-01-03T18:07:57.068055+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

Plantilla de archivo de uso y ventas % SM2/SM3/SM4 (销售/使用备案)
% Hyperledger Iroha Grupo de trabajo de cumplimiento
% 2026-05-06

# Instrucciones

Utilice esta plantilla cuando presente el uso de implementación ante una oficina de SCA para operaciones en tierra
operadores. Proporcione un envío por clúster de implementación o espacio de datos. Actualizar
los marcadores de posición con detalles específicos del operador y adjunte la evidencia enumerada
en la lista de verificación.

# 1. Resumen del operador y la implementación

| Campo | Valor |
|-------|-------|
| Nombre del operador | {{ OPERADOR_NOMBRE }} |
| Identificación de registro comercial | {{ REG_ID }} |
| Dirección registrada | {{ DIRECCIÓN }} |
| Contacto principal (nombre/cargo/correo electrónico/teléfono) | {{ CONTACTO }} |
| Identificador de implementación | {{ DEPLOYMENT_ID }} |
| Ubicación(es) de implementación | {{ UBICACIONES }} |
| Tipo de presentación | Ventas / Uso (销售/使用备案) |
| Fecha de presentación | {{ AAAA-MM-DD }} |

# 2. Detalles de implementación

- ID/hash de compilación del software: `{{ BUILD_HASH }}`
- Fuente de compilación: {{ BUILD_SOURCE }} (por ejemplo, creada por el operador a partir de la fuente, binario proporcionado por el proveedor).
- Fecha de activación: {{ ACTIVATION_DATE }}
- Ventanas de mantenimiento planificadas: {{ MAINTENANCE_CADENCE }}
- Roles de los nodos que participan en la firma SM:
  | Nodo | Rol | Funciones SM habilitadas | Ubicación de la bóveda de claves |
  |------|------|---------------------|--------------------|
  | {{ NODE_ID }} | {{ PAPEL }} | {{ CARACTERÍSTICAS }} | {{ BÓVEDA }} |

# 3. Controles criptográficos

- Algoritmos permitidos: {{ ALGORITHMS }} (asegúrese de que el conjunto SM coincida con la configuración).
- Resumen del ciclo de vida clave:
  | Etapa | Descripción |
  |-------|-------------|
  | Generación | {{ KEY_GENERATION }} |
  | Almacenamiento | {{ KEY_STORAGE }} |
  | Rotación | {{ KEY_ROTATION }} |
  | Revocación | {{ KEY_REVOCATION }} |
- Política de identidad distinta (`distid`): {{ DISTID_POLICY }}
- Extracto de configuración (sección `crypto`): proporcione una instantánea Norito/JSON con hashes.

# 4. Telemetría y seguimientos de auditoría

- Monitoreo de puntos finales: {{ METRICS_ENDPOINTS }} (`/metrics`, paneles).
- Métricas registradas: `crypto.sm.verification_total`, `crypto.sm.sign_total`,
  histogramas de latencia, contadores de errores.
- Política de retención de registros: {{ LOG_RETENTION }} (se recomienda ≥ tres años).
- Ubicación de almacenamiento del registro de auditoría: {{ AUDIT_STORAGE }}

# 5. Respuesta a incidentes y contactos

| Rol | Nombre | Teléfono | Correo electrónico | Acuerdo de Nivel de Servicio |
|------|------|-------|-------|-----|
| Líder de operaciones de seguridad | {{ NOMBRE }} | {{ TELÉFONO }} | {{ CORREO ELECTRÓNICO }} | {{ Acuerdo de Nivel de Servicio }} |
| Cripto de guardia | {{ NOMBRE }} | {{ TELÉFONO }} | {{ CORREO ELECTRÓNICO }} | {{ Acuerdo de Nivel de Servicio }} |
| Legal/cumplimiento | {{ NOMBRE }} | {{ TELÉFONO }} | {{ CORREO ELECTRÓNICO }} | {{ Acuerdo de Nivel de Servicio }} |
| Soporte del proveedor (si corresponde) | {{ NOMBRE }} | {{ TELÉFONO }} | {{ CORREO ELECTRÓNICO }} | {{ Acuerdo de Nivel de Servicio }} |

# 6. Lista de verificación de archivos adjuntos- [] Instantánea de configuración (Norito + JSON) con hashes.
- [] Prueba de compilación determinista (hashes, SBOM, notas de reproducibilidad).
- [] Exportaciones de paneles de telemetría y definiciones de alertas.
- [ ] Plan de respuesta a incidentes y documento de rotación de guardias.
- [ ] Acuse de recibo de capacitación del operador o recibo del runbook.
- [ ] Declaración de control de exportaciones que refleja los artefactos entregados.
- [ ] Copias de acuerdos contractuales relevantes o exenciones de póliza.

# 7. Declaración del operador

> Confirmamos que la implementación mencionada anteriormente cumple con las normas comerciales de la República Popular China.
> regulaciones de criptografía, que los servicios habilitados para SM sigan las normas documentadas
> políticas de respuesta a incidentes y telemetría, y que los artefactos de auditoría serán
> retenido durante al menos tres años.

- Firmante autorizado: ________________________
- Fecha: ________________________