---
lang: es
direction: ltr
source: docs/source/compliance/android/eu/gdpr_dpia_summary.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 8ef338a20104dc5d15094e28a1332a604b68bdcfef1ff82fea784d43fdbd10b5
source_last_modified: "2026-01-03T18:07:59.202230+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Resumen de GDPR DPIA: telemetría del SDK de Android (AND7)

| Campo | Valor |
|-------|-------|
| Fecha de evaluación | 2026-02-12 |
| Actividad de procesamiento | Exportación de telemetría del SDK de Android a backends OTLP compartidos |
| Controladores / Procesadores | SORA Nexus Ops (controlador), operadores socios (controladores conjuntos), Hyperledger Iroha Contribuyentes (procesador) |
| Documentos de referencia | `docs/source/sdk/android/telemetry_redaction.md`, `docs/source/android_support_playbook.md`, `docs/source/android_runbook.md` |

## 1. Descripción del procesamiento

- **Propósito:** Proporcionar la telemetría operativa necesaria para admitir la observabilidad de AND7 (latencia, reintentos, estado de la certificación) mientras se refleja el esquema del nodo Rust (§2 de `telemetry_redaction.md`).
- **Sistemas:** Instrumentación SDK de Android -> Exportador OTLP -> Recolector de telemetría compartido administrado por SRE (consulte el Manual de soporte §8).
- **Asuntos interesados:** Personal del operador que utiliza aplicaciones basadas en SDK de Android; puntos finales Torii descendentes (las cadenas de autoridad tienen un hash según la política de telemetría).

## 2. Inventario de datos y mitigaciones

| Canal | Campos | Riesgo de PII | Mitigación | Retención |
|---------|--------|----------|------------|-----------|
| Rastros (`android.torii.http.request`, `android.torii.http.retry`) | Ruta, estado, latencia | Baja (sin PII) | Autoridad procesada con Blake2b + sal rotativa; no se exportan cuerpos de carga útil. | 7 a 30 días (por documento de telemetría). |
| Eventos (`android.keystore.attestation.result`) | Etiqueta de alias, nivel de seguridad, resumen de atestación | Medio (datos operativos) | Alias ​​hash (`alias_label`), `actor_role_masked` registrado para anulaciones con tokens de auditoría Norito. | 90 días para el éxito, 365 días para anulaciones/fallos. |
| Métricas (`android.pending_queue.depth`, `android.telemetry.export.status`) | Recuento de colas, estado del exportador | Bajo | Sólo recuentos agregados. | 90 días. |
| Medidores de perfil del dispositivo (`android.telemetry.device_profile`) | SDK principal, nivel de hardware | Medio | Bucketing (emulador/consumidor/empresa), sin OEM ni números de serie. | 30 días. |
| Eventos de contexto de red (`android.telemetry.network_context`) | Tipo de red, bandera de itinerancia | Medio | El nombre del operador desapareció por completo; admite el requisito de cumplimiento para evitar los datos del suscriptor. | 7 días. |

## 3. Bases legales y derechos

- **Base jurídica:** Interés legítimo (Art. 6(1)(f)): garantizar el funcionamiento confiable de los clientes de libros contables regulados.
- **Prueba de necesidad:** Métricas limitadas al estado operativo (sin contenido de usuario); La autoridad hash garantiza la paridad con los nodos de Rust a través de un mapeo reversible disponible solo para el personal de soporte autorizado (mediante un flujo de trabajo de anulación).
- **Prueba de equilibrio:** La telemetría se centra en dispositivos controlados por el operador, no en datos del usuario final. Las anulaciones requieren artefactos Norito firmados y revisados ​​por Soporte + Cumplimiento (Manual de soporte §3 + §9).
- **Derechos del interesado:** Los operadores se comunican con Ingeniería de soporte (libro de estrategias §2) para solicitar la exportación/eliminación de telemetría. Los registros y anulaciones de redacción (documento de telemetría §Inventario de señales) permiten el cumplimiento en un plazo de 30 días.

## 4. Evaluación de riesgos| Riesgo | Probabilidad | Impacto | Mitigación residual |
|------|------------|--------|---------------------|
| Reidentificación a través de autoridades hash | Bajo | Medio | Rotación de sal registrada a través de `android.telemetry.redaction.salt_version`; sales almacenadas en bóveda segura; anulaciones auditadas trimestralmente. |
| Toma de huellas digitales del dispositivo a través de grupos de perfiles | Bajo | Medio | Solo se exporta el nivel + SDK principal; Support Playbook prohíbe solicitudes de escalamiento de datos OEM/serie. |
| Anular el mal uso y la filtración de PII | Muy bajo | Alto | Norito solicitudes de anulación registradas, caducan en 24 horas, requieren aprobación de SRE (`docs/source/android_runbook.md` §3). |
| Almacenamiento de telemetría fuera de la UE | Medio | Medio | Recolectores desplegados en regiones UE + JP; política de retención aplicada a través de la configuración de backend de OTLP (documentada en el Manual de soporte §8). |

El riesgo residual se considera aceptable dados los controles anteriores y el seguimiento continuo.

## 5. Acciones y seguimientos

1. **Revisión trimestral:** Validar esquemas de telemetría, rotaciones de sal y anular registros; documento en `docs/source/sdk/android/telemetry_redaction_minutes_YYYYMMDD.md`.
2. **Alineación entre SDK:** Coordine con los mantenedores de Swift/JS para mantener reglas de hash/bucketing consistentes (seguidas en la hoja de ruta AND7).
3. **Comunicaciones de socios:** Incluya un resumen de DPIA en los kits de incorporación de socios (Manual de estrategias de soporte §9) y un enlace a este documento desde `status.md`.