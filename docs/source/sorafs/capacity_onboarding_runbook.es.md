---
lang: es
direction: ltr
source: docs/source/sorafs/capacity_onboarding_runbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: b5da7fda1820f48591602fc600cde95dd5b42fe897cf05d6801dced1e7fcbd88
source_last_modified: "2025-11-18T09:44:43.519916+00:00"
translation_last_reviewed: "2026-01-30"
---

# Runbook de onboarding y salida de capacidad SoraFS

El item del roadmap **SF-2c** pide que cada onboarding o salida de provider
entregue un paquete repetible de artefactos para que reviewers de governance,
tesoreria y SRE puedan rastrear la decision. Este runbook codifica el workflow
referenciado por `roadmap.md` y el checklist de validacion del marketplace de
capacidad (`docs/source/sorafs/reports/capacity_marketplace_validation.md`).
Sigelo para cada admision, renovacion o retiro de provider.

## Roles y requisitos pre-flight

| Rol | Responsabilidades | Artefactos requeridos |
|-----|-------------------|-----------------------|
| Provider | Redactar spec `CapacityDeclarationV1`, capturar inventario de hardware, suministrar bundles de atestacion si lo exige `docs/source/sorafs/provider_admission_policy.md`. | `specs/<provider>.json`, affidavit de hardware, lista de contactos. |
| Storage WG reviewer | Validar schema, correr regresion CLI, cross-check de stake y metadata de perfil de chunker. | Logs de CLI, outputs `sorafs_manifest_stub`, nota de revision firmada. |
| Governance council | Aprobar declaracion final, registrar ID de voto y rastrear hooks de override/penalty. | ID de ballot del council, link a log de incidentes. |
| Treasury/SRE | Confirmar dashboards y entradas de cuota, archivar hashes de reconciliacion/export, verificar alertas. | Capturas Grafana, dump `/v2/sorafs/capacity/state`, log de silencio Alertmanager si se uso. |

Checklist pre-flight:

1. Provider aparece en la lista verificada de admision SF-2b (si no, rechazar).
2. Las keys requeridas existen en el registry de providers (`iroha_torii` ->
   `/v2/sorafs/providers`).
3. Provider anuncia los perfiles de chunker referenciados en la declaracion.
4. Tesoreria confirma que existe una cuenta de reserva para el tier de staking esperado.

## Workflow de onboarding

### 1. Preparar el bundle de declaracion

Crear un spec legible por humanos que espeje el schema canonico (ver
`docs/examples/sorafs_capacity_declaration.spec.json`). Cada spec debe incluir
`provider_id_hex` determinista, `chunker_capabilities`, stake pointer, caps por
lane y contacto SLA. Generar los payloads canonicos via el helper CLI:

```bash
mkdir -p artifacts/sorafs/providers/acme
sorafs_manifest_stub capacity declaration \
  --spec specs/providers/acme.json \
  --json-out artifacts/sorafs/providers/acme/declaration.json \
  --request-out artifacts/sorafs/providers/acme/request.json \
  --norito-out artifacts/sorafs/providers/acme/declaration.to \
  --base64-out artifacts/sorafs/providers/acme/declaration.b64
```

Este comando valida el schema localmente y emite cada artefacto requerido por
`/v2/sorafs/capacity/declare`. Guardar stdout/stderr en
`artifacts/sorafs/providers/acme/manifest_stub.log`.

### 2. Ejecutar smoke tests de admision

Antes de enviar el payload a Torii, re-ejecutar la regresion CLI para probar
que el validador es determinista:

```bash
cargo test -p sorafs_car --test capacity_cli -- capacity_declaration
```

Adjuntar el `test-stdout.txt` resultante al paquete de onboarding. Reviewers
usan el log para confirmar que la plantilla ejerce los mismos code paths que
Torii invoca.

### 3. Enviar a Torii y capturar respuestas

Enviar el request JSON via la app API de Torii. Puedes llamarlo manualmente:

```bash
TORII="https://torii.example.net"
curl -sS -X POST "$TORII/v2/sorafs/capacity/declare" \
  -H 'Content-Type: application/json' \
  --data-binary @artifacts/sorafs/providers/acme/request.json \
  | tee artifacts/sorafs/providers/acme/declare_response.json
```

...o envolver el request embebiendolo en una transaccion de governance si el
provider debe esperar un voto del council. Capturar el status HTTP, snippet de
log de Torii y el correlation ID de la app API; reviewers de governance esperan
eso en el ticket de release.

### 4. Verificar estado del registry y publicar evidencia

Despues de que Torii acepte la declaracion, exportar un snapshot de estado y
el JSON amigable para governance llamando:

```bash
curl -sS "$TORII/v2/sorafs/capacity/state" \
  | jq '.providers[] | select(.provider_id_hex=="0xACME...")' \
  > artifacts/sorafs/providers/acme/state_record.json
```

Confirmar que `capacity_gib`, `chunker_profiles[*].handle` y `stake_pointer`
coinciden con el spec original. Cross-check de la entrada del provider en el
board Grafana `sorafs_capacity_health` (`dashboards/grafana/sorafs_capacity_health.json`).
Tomar una captura con timestamp visible y archivarla junto al dump JSON para
que SRE pueda probar cobertura de telemetria.

### 5. Hand-off de governance y tesoreria

1. Governance crea un ballot referenciando el hash del payload y registra el
   resultado del voto en `docs/examples/sorafs_capacity_marketplace_validation/<date>_onboarding_signoff.md`.
2. Tesoreria anota el output de reconciliacion nightly producido por
   `scripts/telemetry/capacity_reconcile.py --snapshot … --ledger …` con el ID
   del provider para que reviewers de payouts rastreen el primer accrual de rent
   y el gating de Alertmanager.
3. Support/SRE anotan el onboarding en `docs/source/sorafs/ops_log.md`,
   enlazando al path del paquete y la captura de Grafana.

## Workflow de salida / retiro

1. **Drenar asignaciones.** Ejecutar `iroha app sorafs replication list --status active \
   --provider-id <hex>` hasta que no queden referencias de manifiesto. Encolar
   ballots de reasignacion para stragglers antes de aprobar el retiro.
2. **Publicar telemetria final.** Usar `sorafs_manifest_stub capacity telemetry` para
   generar el snapshot final y enviarlo via `POST /v2/sorafs/capacity/telemetry`.
   Registrar la respuesta y asegurar que la fila del provider en
   `/v2/sorafs/capacity/state` pase a `status="retiring"` o `"inactive"`.
3. **Revocar acceso.** Remover las credenciales del provider de la distribucion
   de adverts, revocar tokens OAuth/API y rotar cualquier material de secreto
   compartido.
4. **Archivar artefactos.** Guardar el request/response de telemetria, snapshot
   de estado final, captura Grafana y los IDs de strike/approval dentro de
   `docs/examples/sorafs_capacity_marketplace_validation/<date>_exit_signoff.md`.

Ejecutar el checklist de salida una vez por release candidate satisface el
requisito de smoke test en `roadmap.md` y los criterios de aceptacion GA en
`docs/source/sorafs/storage_capacity_marketplace.md`.

## Evidencia y layout de storage

Mantener la siguiente estructura (hashes capturados via `b2sum`) para cada
paquete de provider:

```
artifacts/sorafs/providers/<provider>/
  declaration.json
  declaration.to
  declaration.b64
  request.json
  declare_response.json
  state_record.json
  telemetry_final.json              # solo salida
  grafana_capacity_health_<ts>.png
  manifest_stub.log
  tests/capacity_cli.log
docs/examples/sorafs_capacity_marketplace_validation/
  YYYY-MM-DD_<provider>_{onboarding,exit}_signoff.md
```

Los archivos de signoff incluyen hashes para cada artefacto mas links a
entradas de silencio Alertmanager o numeros de incidentes. Tesoreria y
governance dependen de estos hashes al aprobar payouts o solicitudes de
slashing.

## Matriz de revision por roles

| Fase | Storage WG | Governance | Treasury/SRE |
|------|------------|------------|--------------|
| Validacion de spec | Verificar schema, correr test CLI, confirmar perfiles de chunker | — | — |
| Submission Torii | Monitorear `/v2/sorafs/capacity/state`, actualizar ops log | Registrar ID de ballot, adjuntar artefacto de voto | Vigilar Grafana y reglas de alerta, capturar logs de reconciliacion |
| Salida | Verificar reasignacion + telemetria, compilar paquete | Aprobar retiro y enlazar log de disputa | Confirmar que dashboards bajan al provider, archivar auditoria de payouts |

Mantener esta matriz junto al release checklist; auditores esperan ver nombres
de owners y timestamps llenos para cada ola de onboarding o salida.

_Actualizado por ultima vez: 2026-02-11_
