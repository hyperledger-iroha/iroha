---
lang: es
direction: ltr
source: docs/source/crypto/sm_compliance_brief.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 73f5ca7a7484a26e901102dd6950b7110a18e7fa215a46540c7189c919e0958f
source_last_modified: "2026-01-03T18:07:57.080817+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# SM2/SM3/SM4 Resumen de cumplimiento y exportación

Este resumen complementa las notas de arquitectura en `docs/source/crypto/sm_program.md`.
y proporciona orientación práctica para los equipos de ingeniería, operaciones y legales como
La familia de algoritmos GM/T pasa de una vista previa de solo verificación a una habilitación más amplia.

## Resumen
- **Base regulatoria:** *Ley de Criptografía* de China (2019), *Ley de Ciberseguridad* y
  *La Ley de Seguridad de Datos* clasifica SM2/SM3/SM4 como “criptografía comercial” cuando se implementa
  en tierra. Los operadores deben presentar informes de uso y ciertos sectores requieren acreditación.
  pruebas antes del uso en producción.
- **Controles internacionales:** Fuera de China, los algoritmos se incluyen en la categoría EAR de EE. UU.
  5 Parte 2, UE 2021/821 Anexo 1 (5D002) y regímenes nacionales similares. Código abierto
  La publicación generalmente califica para excepciones de licencia (ENC/TSU), pero los archivos binarios
  Los productos enviados a regiones embargadas siguen siendo exportaciones controladas.
- **Política del proyecto:** Las funciones de SM permanecen deshabilitadas de forma predeterminada. Funcionalidad de firma
  solo se habilitará después del cierre de la auditoría externa, rendimiento/telemetría determinista
  puertas y documentación del operador (este resumen).

## Acciones requeridas por función
| Equipo | Responsabilidades | Artefactos | Propietarios |
|------|------------------|-----------|--------|
| Grupo de Trabajo sobre Cripto | Realice un seguimiento de las actualizaciones de especificaciones de GM/T, coordine auditorías de terceros, mantenga una política determinista (derivación no única, r∥s canónicas). | `sm_program.md`, informes de auditoría, paquetes de accesorios. | Líder del Grupo de Trabajo sobre Cripto |
| Ingeniería de lanzamiento | Funciones Gate SM detrás de la configuración explícita, mantener el valor predeterminado de solo verificación, administrar la lista de verificación de implementación de funciones. | `release_dual_track_runbook.md`, manifiestos de lanzamiento, ticket de lanzamiento. | Lanzamiento TL |
| Operaciones / SRE | Proporcionar lista de verificación de habilitación de SM, paneles de telemetría (uso, tasas de error), plan de respuesta a incidentes. | Runbooks, paneles Grafana, tickets de incorporación. | Operaciones/SRE |
| Enlace Jurídico | Presentar informes de desarrollo/uso de la República Popular China cuando los nodos se ejecuten en China continental; Revisar la postura exportadora de cada paquete. | Plantillas de presentación, declaraciones de exportación. | Contacto jurídico |
| Programa SDK | El algoritmo Surface SM admite de manera consistente, aplica un comportamiento determinista y propaga notas de cumplimiento a los documentos del SDK. | Notas de la versión del SDK, documentos, activación de CI. | Clientes potenciales del SDK |## Requisitos de documentación y presentación (China)
1. **Presentación de producto (开发备案):** Para desarrollo en tierra, envíe la descripción del producto,
   declaración de disponibilidad de fuente, lista de dependencias y pasos de compilación deterministas para
   la administración provincial de criptografía antes de su liberación.
2. **Presentación de ventas/uso (销售/使用备案):** Los operadores que ejecutan nodos habilitados para SM deben
   registrar el alcance de uso, la gestión de claves y la recopilación de telemetría con el mismo
   autoridad. Proporcionar información de contacto y SLA de respuesta a incidentes.
3. **Certificación (检测/认证):** Los operadores de infraestructura crítica pueden requerir
   pruebas acreditadas. Proporcionar scripts de compilación reproducibles, SBOM e informes de prueba.
   para que los integradores posteriores puedan completar la certificación sin alterar el código.
4. **Mantenimiento de registros:** Archive las presentaciones y aprobaciones en el rastreador de cumplimiento.
   Actualice `status.md` cuando nuevas regiones u operadores completen el proceso.

## Lista de verificación de cumplimiento

### Antes de habilitar las funciones SM
- [] Confirmar que el asesor legal revisó las regiones de implementación objetivo.
- [] Capture instrucciones de compilación deterministas, manifiestos de dependencia y SBOM
      exportaciones para su inclusión en las presentaciones.
- [] Alinear `crypto.allowed_signing`, `crypto.default_hash` y admisión
      manifiestos de política con el ticket de implementación.
- [] Producir comunicaciones del operador que describan el alcance de la característica SM,
      requisitos previos de habilitación y planes alternativos para la inhabilitación.
- [] Exportar paneles de telemetría que cubren contadores de firma/verificación SM,
      tasas de error y métricas de rendimiento (`sm3`, `sm4`, sincronización de llamadas al sistema).
- [] Preparar contactos de respuesta a incidentes y rutas de escalada para tierra firme.
      operadores y el Crypto WG.

### Preparación para la presentación y auditoría
- [ ] Seleccione la plantilla de presentación adecuada (producto versus ventas/uso) y complete
      en los metadatos de la versión antes del envío.
- [] Adjunte archivos SBOM, transcripciones de pruebas deterministas y hashes de manifiesto.
- [ ] Asegúrese de que la declaración de control de exportaciones refleje los artefactos exactos que se están
      entregado y cita excepciones de licencia en las que se basa (ENC/TSU).
- [] Verificar que los informes de auditoría, el seguimiento de las soluciones y los runbooks del operador
      están vinculados desde el paquete de presentación.
- [ ] Almacenar presentaciones, aprobaciones y correspondencia firmadas en el
      rastreador con referencias versionadas.

### Operaciones posteriores a la aprobación
- [] Actualización `status.md` y el ticket de implementación una vez aceptada la presentación.
- [] Vuelva a ejecutar la validación de telemetría para confirmar coincidencias de cobertura de observabilidad
      los insumos de presentación.
- [ ] Programar revisiones periódicas (al menos anualmente) de presentaciones, informes de auditoría,
      y exportar declaraciones para capturar actualizaciones de especificaciones/regulaciones.
- [] Activar la presentación de apéndices siempre que se realice la configuración, el alcance de las funciones o el alojamiento
      la huella cambia materialmente.## Guía de exportación y distribución
- Incluir una breve declaración de exportación en las notas de la versión/manifiestos que hagan referencia a la confianza.
  sobre ENC/TSU. Ejemplo:
  > "Esta versión contiene implementaciones SM2/SM3/SM4. La distribución sigue ENC
  > (15 CFR Parte 742) / UE 2021/821 Anexo 1 5D002. Los operadores deben garantizar el cumplimiento
  > con las leyes locales de exportación/importación.”
- Para compilaciones alojadas dentro de China, coordine con Operaciones para publicar artefactos de
  infraestructura terrestre; Evite la transferencia transfronteriza de binarios habilitados para SM a menos que
  las licencias apropiadas están en vigor.
- Al realizar la duplicación en repositorios de paquetes, registre qué artefactos incluyen características de SM
  para simplificar los informes de cumplimiento.

## Lista de verificación del operador
- [] Confirmar perfil de versión (`scripts/select_release_profile.py`) + indicador de función SM.
- [ ] Revisión `sm_program.md` y este escrito; asegurarse de que se registren las presentaciones legales.
- [] Habilite las funciones de SM compilando con `sm`, actualizando `crypto.allowed_signing` para incluir `sm2` y cambiando `crypto.default_hash` a `sm3-256` solo después de que las salvaguardas de determinismo estén implementadas y el estado de auditoría sea verde.
- [] Actualizar paneles/alertas de telemetría para incluir contadores SM (fallos de verificación,
      solicitudes de firma, métricas de rendimiento).
- [ ] Mantenga los manifiestos, las pruebas de hash/firma y las confirmaciones de presentación adjuntas a
      el boleto de lanzamiento.

## Plantillas de archivo de muestra

Las plantillas se encuentran bajo `docs/source/crypto/attachments/` para facilitar su inclusión en
archivar paquetes. Copie la plantilla de Markdown relevante en el cambio del operador
regístrelo o expórtelo a PDF según lo requieran las autoridades locales.

- [`sm_product_filing_template.md`](attachments/sm_product_filing_template.md) —
  Formulario de presentación de productos provinciales (开发备案) que captura metadatos de lanzamiento, algoritmos,
  Referencias de SBOM y contactos de soporte.
- [`sm_sales_usage_filing_template.md`](attachments/sm_sales_usage_filing_template.md) —
  presentación de ventas/uso del operador (销售/使用备案) que describe la huella de implementación,
  procedimientos clave de gestión, telemetría y respuesta a incidentes.
- [`sm_export_statement_template.md`](attachments/sm_export_statement_template.md) —
  declaración de control de exportaciones adecuada para notas de lanzamiento, manifiestos o documentos legales.
  correspondencia basada en excepciones de licencia ENC/TSU.## Estándares y citas
- **GM/T 0002-2012 / GB/T 32907-2016** — Cifrado en bloque SM4 y parámetros AEAD (ECB/GCM/CCM). Coincide con los vectores capturados en `docs/source/crypto/sm_vectors.md`.
- **GM/T 0003-2012 / GB/T 32918.x-2016**: criptografía de clave pública SM2, parámetros de curva, proceso de firma/verificación y pruebas de respuesta conocida del Anexo D.
- **GM/T 0004-2012 / GB/T 32905-2016** — Especificación de la función hash SM3 y vectores de conformidad.
- **RFC 8998** — Intercambio de claves SM2 y uso de firmas en TLS; citar al documentar la interoperabilidad con OpenSSL/Tongsuo.
- **Ley de Criptografía de la República Popular China (2019)**, **Ley de Ciberseguridad (2017)**, **Ley de Seguridad de Datos (2021)**: base legal para el flujo de trabajo de presentación mencionado anteriormente.
- **US EAR Categoría 5 Parte 2** y **Reglamento UE 2021/821 Anexo 1 (5D002)**: regímenes de control de exportaciones que rigen los archivos binarios habilitados para SM.
- **Artefactos Iroha:** `scripts/sm_interop_matrix.sh` e `scripts/sm_openssl_smoke.sh` proporcionan transcripciones de interoperabilidad deterministas que los auditores pueden reproducir antes de firmar informes de cumplimiento.

## Referencias
- `docs/source/crypto/sm_program.md` — arquitectura técnica y política.
- `docs/source/release_dual_track_runbook.md`: proceso de liberación y activación.
- `docs/source/sora_nexus_operator_onboarding.md`: flujo de incorporación de operadores de muestra.
-GM/T 0002-2012, GM/T 0003-2012, GM/T 0004-2012, serie GB/T 32918, RFC 8998.

¿Preguntas? Comuníquese con Crypto WG o el enlace legal a través del rastreador de implementación de SM.