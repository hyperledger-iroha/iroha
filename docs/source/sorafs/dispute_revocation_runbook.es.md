---
lang: es
direction: ltr
source: docs/source/sorafs/dispute_revocation_runbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 8a7de0f1c8264a61e18e812826bbda8ec913a4fc4403391fb415f510cb1a03c4
source_last_modified: "2025-11-02T04:40:40.157303+00:00"
translation_last_reviewed: "2026-01-30"
---

# Runbook de disputa y revocacion SoraFS

Este runbook guia a operadores de governance a traves del filing de disputas de
capacidad SoraFS, coordinacion de revocaciones y asegurar que la evacuacion de
datos ocurra de forma determinista.

## 1. Evaluar el incidente

- **Condiciones de disparo:** deteccion de breach de SLA (uptime/falla PoR),
  shortfall de replicacion o desacuerdo de facturacion.
- **Confirmar telemetria:** capturar snapshots de `/v1/sorafs/capacity/state` y
  `/v1/sorafs/capacity/telemetry` para el provider.
- **Notificar stakeholders:** Storage Team (operaciones de provider), Governance
  Council (cuerpo decisor), Observability (updates de dashboards).

## 2. Preparar bundle de evidencia

1. Recolectar artefactos crudos (telemetria JSON, logs de CLI, notas de auditor).
2. Normalizar en un archivo determinista (p. ej., tarball); registrar:
   - Digest BLAKE3-256 (`evidence_digest`)
   - Tipo de media (`application/zip`, `application/jsonl`, etc.)
   - URI de hosting (object storage, pin SoraFS o endpoint accesible via Torii)
3. Guardar el bundle en el bucket de evidencia de governance con acceso write-once.

## 3. Presentar la disputa

1. Crear un JSON spec para `sorafs_manifest_stub capacity dispute`:

   ```json
   {
     "provider_id_hex": "<hex>",
     "complainant_id_hex": "<hex>",
     "replication_order_id_hex": "<hex or omit>",
     "kind": "replication_shortfall",
     "submitted_epoch": 1700100000,
     "description": "El proveedor no ingesto la orden dentro del SLA.",
     "requested_remedy": "Slash 10% stake and suspend adverts",
     "evidence": {
       "digest_hex": "<blake3-256>",
       "media_type": "application/zip",
       "uri": "https://evidence.sora.net/bundles/<id>.zip",
       "size_bytes": 1024
     }
   }
   ```

2. Ejecutar el CLI:

   ```
   sorafs_manifest_stub capacity dispute \
     --spec=dispute.json \
     --norito-out=dispute.to \
     --base64-out=dispute.b64 \
     --json-out=dispute_summary.json \
     --request-out=dispute_request.json \
     --authority=soraカタカナ... \
     --private-key=ed25519:<key>
   ```

3. Revisar `dispute_summary.json` (confirmar kind, digest de evidencia, timestamps).
4. Enviar el request JSON a Torii `/v1/sorafs/capacity/dispute` via la cola de
   transacciones de governance. Capturar el `dispute_id_hex` de respuesta; ancla
   acciones de revocacion y reportes de auditoria.

## 4. Evacuacion y revocacion

1. **Ventana de gracia:** notificar al provider de la revocacion inminente; permitir
   evacuacion de datos pinneados cuando la politica lo permita.
2. **Generar `ProviderAdmissionRevocationV1`:**
   - Usar `sorafs_manifest_stub provider-admission revoke` con la razon aprobada.
   - Verificar firmas y digest de revocacion.
3. **Publicar revocacion:**
   - Enviar el request de revocacion a Torii.
   - Asegurar que los provider adverts quedan bloqueados (esperar que
     `torii_sorafs_admission_total{result="rejected",reason="admission_missing"}` suba).
4. **Actualizar dashboards:** marcar al provider como revocado, referenciar el ID
   de disputa y enlazar el bundle de evidencia.

## 5. Post-mortem y seguimiento

- Registrar timeline, causa raiz y acciones de remediacion en el tracker de
  incidentes de governance.
- Determinar restitucion (slash de stake, clawbacks de fees, refunds a clientes).
- Documentar aprendizajes; actualizar thresholds de SLA o alertas si aplica.

## 6. Material de referencia

- `sorafs_manifest_stub capacity dispute --help`
- `docs/source/sorafs/storage_capacity_marketplace.md` (seccion de disputas)
- `docs/source/sorafs/provider_admission_policy.md` (workflow de revocacion)
- Dashboard de observabilidad: `SoraFS / Capacity Providers`

## Checklist

- [ ] Bundle de evidencia capturado y hasheado.
- [ ] Payload de disputa validado localmente.
- [ ] Transaccion de disputa Torii aceptada.
- [ ] Revocacion ejecutada (si se aprueba).
- [ ] Dashboards/runbooks actualizados.
- [ ] Post-mortem archivado con el council de governance.
