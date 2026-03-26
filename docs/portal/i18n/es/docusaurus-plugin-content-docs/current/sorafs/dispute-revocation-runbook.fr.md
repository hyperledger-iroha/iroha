---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/dispute-revocation-runbook.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: disputa-revocación-runbook
título: Runbook des litiges et révocations SoraFS
sidebar_label: Litigios y revocaciones de Runbook
descripción: Flujo de gobierno para depositar litigios de capacidad SoraFS, coordinar las revocaciones y evacuar las donaciones de manera determinada.
---

:::nota Fuente canónica
Esta página refleja `docs/source/sorafs/dispute_revocation_runbook.md`. Guarde las dos copias sincronizadas justo con la documentación heredada de Sphinx ya retirada.
:::

## Objetivo

Este runbook guía a los operadores de gobierno en la creación de litigios de capacidad SoraFS, la coordinación de revocaciones y la garantía de una evacuación determinada de los donantes.

## 1. Evaluar el incidente

- **Condiciones de cancelación:** detección de una violación del SLA (disponibilité/échec PoR), déficit de réplica o incumplimiento de facturación.
- **Confirme la télémétrie:** capture las instantáneas `/v1/sorafs/capacity/state` y `/v1/sorafs/capacity/telemetry` para el proveedor.
- **Notificador de las partes embarazadas:** Equipo de almacenamiento (operaciones del proveedor), Consejo de gobernanza (organe décisionnel), Observabilidad (mises à jour des Dashboards).

## 2. Preparar el paquete de preuves1. Recopilador de artefactos brutos (telemetría JSON, registros CLI, notas de auditoría).
2. Normalizar en un archivo determinado (por ejemplo, un tarball); consignador:
   - digerir BLAKE3-256 (`evidence_digest`)
   - tipo de medio (`application/zip`, `application/jsonl`, etc.)
   - URI de alojamiento (almacenamiento de objetos, pin SoraFS o punto final accesible a través de Torii)
3. Stocker le bundle dans le bucket de Collecte des preuves de gouvernance avec un accès write-once.

## 3. Presentar el litigio

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

2. Lanza la CLI :

   ```bash
   sorafs_manifest_stub capacity dispute \
     --spec=dispute.json \
     --norito-out=dispute.to \
     --base64-out=dispute.b64 \
     --json-out=dispute_summary.json \
     --request-out=dispute_request.json \
     --authority=<katakana-i105-account-id> \
     --private-key=ed25519:<key>
   ```

3. Verifique `dispute_summary.json` (confirme el tipo, el resumen de las etiquetas anteriores y las marcas de tiempo).
4. Soumettez le JSON de requête à Torii `/v1/sorafs/capacity/dispute` a través del archivo de transacciones de gobierno. Capture el valor de respuesta `dispute_id_hex`; elle ancre les actiones de révocation suivantes et les rapports d’audit.

## 4. Evacuación y revocación1. **Fenêtre de grâce :** avertissez le fournisseur de la révocation imminente ; autorisez l’évacuation des données épinglées lorsque la politique le permet.
2. **Générez `ProviderAdmissionRevocationV1` :**
   - Utilice `sorafs_manifest_stub provider-admission revoke` con la razón aprobada.
   - Verifique las firmas y el resumen de revocación.
3. **Publiez the révocation :**
   - Soumettez la solicitud de revocación a Torii.
   - Asegúrese de que los anuncios del proveedor estén bloqueados (atienda una casa de `torii_sorafs_admission_total{result="rejected",reason="admission_missing"}`).
4. **Mettez à jour les Dashboards:** señalez le fournisseur comme révoqué, référencez l’ID du litige et liez le bundle de preuves.

## 5. Post-mortem y siguientes

- Registrez la cronología, la causa racine y las acciones de remediación en el rastreador de incidentes de gobierno.
- Determinación de la restitución (recorte de la participación, reembolsos de gastos, reembolsos a los clientes).
- Documentez les leçons; Configure cada día las alertas SLA o las alertas de monitoreo si es necesario.

## 6. Documentos de referencia

- `sorafs_manifest_stub capacity dispute --help`
- `docs/source/sorafs/storage_capacity_marketplace.md` (sección litigios)
- `docs/source/sorafs/provider_admission_policy.md` (flujo de trabajo de revocación)
- Panel de observación: `SoraFS / Capacity Providers`

## Lista de verificación- [ ] Bundle de preuves capturé et haché.
- [ ] Carga útil de litigio válido localmente.
- [ ] Transacción de litigio Torii aceptada.
- [ ] Révocación ejecutada (si aprobada).
- [] Paneles/runbooks actualizados.
- [ ] Declaración post-mortem auprès du conseil de gouvernance.