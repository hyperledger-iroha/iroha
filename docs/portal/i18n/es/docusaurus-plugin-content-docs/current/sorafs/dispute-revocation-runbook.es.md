---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/dispute-revocation-runbook.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: disputa-revocación-runbook
título: Runbook de disputas y revocaciones de SoraFS
sidebar_label: Runbook de disputas y revocaciones
descripción: Flujo de gobernanza para presentar disputas de capacidad de SoraFS, coordinar revocaciones y evacuar datos de forma determinista.
---

:::nota Fuente canónica
Esta página refleja `docs/source/sorafs/dispute_revocation_runbook.md`. Mantén ambas copias sincronizadas hasta que se retire la documentación heredada de Sphinx.
:::

##Propósito

Este runbook guía a los operadores de gobernanza para presentar disputas de capacidad de SoraFS, coordinar revocaciones y garantizar que la evacuación de datos se complete de forma determinista.

## 1. Evaluar el incidente

- **Condiciones de activación:** detección de incumplimiento de SLA (tiempo de actividad/fallo de PoR), déficit de replicación o desacuerdo de facturación.
- **Confirmar telemetría:** captura instantáneas de `/v1/sorafs/capacity/state` y `/v1/sorafs/capacity/telemetry` para el proveedor.
- **Notificar a las partes interesadas:** Storage Team (operaciones del proveedor), Governance Council (órgano decisorio), Observability (actualizaciones de tableros).

## 2. Preparar el paquete de evidencias1. Recopila artefactos en bruto (telemetría JSON, logs de CLI, notas de auditoría).
2. Normaliza en un archivo determinista (por ejemplo, un tarball); registrar:
   - digerir BLAKE3-256 (`evidence_digest`)
   - tipo de medio (`application/zip`, `application/jsonl`, etc.)
   - URI de alojamiento (almacenamiento de objetos, pin de SoraFS o endpoint accesible por Torii)
3. Guarde el paquete en el cubo de recolección de evidencias de gobernanza con acceso de escritura única.

## 3. Presentar la disputa

1. Cree una especificación JSON para `sorafs_manifest_stub capacity dispute`:

   ```json
   {
     "provider_id_hex": "<hex>",
     "complainant_id_hex": "<hex>",
     "replication_order_id_hex": "<hex or omit>",
     "kind": "replication_shortfall",
     "submitted_epoch": 1700100000,
     "description": "Provider failed to ingest order within SLA.",
     "requested_remedy": "Slash 10% stake and suspend adverts",
     "evidence": {
       "digest_hex": "<blake3-256>",
       "media_type": "application/zip",
       "uri": "https://evidence.sora.net/bundles/<id>.zip",
       "size_bytes": 1024
     }
   }
   ```

2. Ejecuta la CLI:

   ```bash
   sorafs_manifest_stub capacity dispute \
     --spec=dispute.json \
     --norito-out=dispute.to \
     --base64-out=dispute.b64 \
     --json-out=dispute_summary.json \
     --request-out=dispute_request.json \
     --authority=ih58... \
     --private-key=ed25519:<key>
   ```

3. Revisa `dispute_summary.json` (confirma tipo, resumen de evidencias y marcas de tiempo).
4. Envía el JSON de la solicitud a Torii `/v1/sorafs/capacity/dispute` a través de la cola de transacciones de gobernanza. Captura el valor de respuesta `dispute_id_hex`; ancla las acciones de revocación posteriores y los informes de auditoría.

## 4. Evacuación y revocación1. **Ventana de gracia:** notifica al proveedor sobre la revocación inminente; permite la evacuación de datos fijados cuando la política lo permite.
2. **Género `ProviderAdmissionRevocationV1`:**
   - Usa `sorafs_manifest_stub provider-admission revoke` con la razón aprobada.
   - Verifica firmas y el resumen de revocación.
3. **Publica la revocación:**
   - Envía la solicitud de revocación a Torii.
   - Asegúrese de que los anuncios del proveedor estén bloqueados (se espera que `torii_sorafs_admission_total{result="rejected",reason="admission_missing"}` aumente).
4. **Actualiza paneles:** marca al proveedor como revocado, referencia el ID de disputa y enlaza el paquete de evidencias.

## 5. Post-mortem y seguimiento

- Registra la línea de tiempo, la causa raíz y las acciones de remediación en el rastreador de incidentes de gobernanza.
- Determina la restitución (recorte de participación, clawbacks de comisiones, reembolsos a clientes).
- Documenta aprendizajes; Actualiza umbrales de SLA o alertas de monitoreo si es necesario.

## 6. Materiales de referencia

- `sorafs_manifest_stub capacity dispute --help`
- `docs/source/sorafs/storage_capacity_marketplace.md` (sección de disputas)
- `docs/source/sorafs/provider_admission_policy.md` (flujo de revocación)
- Panel de observabilidad: `SoraFS / Capacity Providers`

## Lista de verificación- [ ] Paquete de evidencias capturadas y hasheado.
- [ ] Carga útil de disputa validada localmente.
- [ ] Transacción de disputa en Torii aceptada.
- [ ] Revocación ejecutada (si fue aprobada).
- [ ] Paneles/runbooks actualizados.
- [ ] Post-mortem presentado ante el consejo de gobernanza.