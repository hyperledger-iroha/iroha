---
lang: es
direction: ltr
source: docs/examples/taikai_anchor_lineage_packet.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: a2037fed472e37a06559e7cd871c1b916b514b9804f309413fc369d5ded662b6
source_last_modified: "2025-11-21T18:09:53.463728+00:00"
translation_last_reviewed: 2026-01-01
---

# Plantilla de paquete de linaje de anclaje Taikai (SN13-C)

El item del roadmap **SN13-C - Manifests & SoraNS anchors** requiere que cada rotacion de alias
entregue un paquete de evidencia deterministico. Copia esta plantilla en tu directorio de
artefactos de rollout (por ejemplo
`artifacts/taikai/anchor/<event>/<alias>/<timestamp>/packet.md`) y reemplaza los placeholders
antes de enviar el paquete a governance.

## 1. Metadatos

| Campo | Valor |
|-------|-------|
| ID de evento | `<taikai.event.launch-2026-07-10>` |
| Stream / rendition | `<main-stage>` |
| Namespace / nombre de alias | `<sora / docs>` |
| Directorio de evidencia | `artifacts/taikai/anchor/<event>/<alias>/2026-07-10T18-00Z/` |
| Contacto del operador | `<name + email>` |
| Ticket GAR / RPT | `<governance ticket or GAR digest>` |

## Helper de bundle (opcional)

Copia los artefactos del spool y emite un resumen JSON (opcionalmente firmado) antes de
completar las secciones restantes:

```bash
cargo xtask taikai-anchor-bundle \
  --spool config/da_manifests/taikai \
  --copy-dir artifacts/taikai/anchor/<event>/<alias>/<timestamp>/spool \
  --out artifacts/taikai/anchor/<event>/<alias>/<timestamp>/anchor_bundle.json \
  --signing-key <hex-ed25519-optional>
```

El helper extrae `taikai-anchor-request-*`, `taikai-trm-state-*`, `taikai-lineage-*`,
envelopes y sentinels del directorio spool Taikai
(`config.da_ingest.manifest_store_dir/taikai`) para que la carpeta de evidencia ya
contenga los archivos exactos referenciados abajo.

## 2. Ledger de linaje y hint

Adjunta tanto el ledger de linaje en disco como el JSON de hint que Torii escribio para
esta ventana. Estos vienen directamente de
`config.da_ingest.manifest_store_dir/taikai/taikai-trm-state-<alias>.json` y
`taikai-lineage-<lane>-<epoch>-<sequence>-<storage_ticket>-<fingerprint>.json`.

| Artefacto | Archivo | SHA-256 | Notas |
|----------|------|---------|-------|
| Ledger de linaje | `taikai-trm-state-docs.json` | `<sha256>` | Prueba el digest/ventana del manifiesto previo. |
| Hint de linaje | `taikai-lineage-l1-140-6a-b2b.json` | `<sha256>` | Capturado antes de subir al anclaje SoraNS. |

```bash
sha256sum artifacts/taikai/anchor/<event>/<alias>/<ts>/taikai-trm-state-*.json \
  | tee artifacts/taikai/anchor/<event>/<alias>/<ts>/hashes/lineage.sha256
```

## 3. Captura del payload de anclaje

Registra el payload POST que Torii envio al servicio de anclaje. El payload incluye
`envelope_base64`, `ssm_base64`, `trm_base64` y el objeto inline `lineage_hint`; las
auditorias dependen de esta captura para probar el hint enviado a SoraNS. Torii ahora
escribe este JSON automaticamente como
`taikai-anchor-request-<lane>-<epoch>-<sequence>-<ticket>-<fingerprint>.json`
dentro del directorio spool Taikai (`config.da_ingest.manifest_store_dir/taikai/`), por lo
que los operadores pueden copiarlo directamente en lugar de extraer logs HTTP.

| Artefacto | Archivo | SHA-256 | Notas |
|----------|------|---------|-------|
| POST del anchor | `requests/2026-07-10T18-00Z.json` | `<sha256>` | Solicitud cruda copiada de `taikai-anchor-request-*.json` (Taikai spool). |

## 4. Acuse de digest del manifiesto

| Campo | Valor |
|-------|-------|
| Digest del manifiesto nuevo | `<hex digest>` |
| Digest del manifiesto previo (del hint) | `<hex digest>` |
| Ventana inicio / fin | `<start seq> / <end seq>` |
| Timestamp de aceptacion | `<ISO8601>` |

Referencia los hashes de ledger/hint registrados arriba para que los reviewers puedan
verificar la ventana que fue reemplazada.

## 5. Metricas / `taikai_alias_rotations`

- `taikai_trm_alias_rotations_total` snapshot: `<Prometheus query + export path>`
- `/status taikai_alias_rotations` dump (por alias): `<file path + hash>`

Proporciona el export de Prometheus/Grafana o la salida de `curl` que muestre el incremento
del contador y el arreglo `/status` para este alias.

## 6. Manifiesto para el directorio de evidencia

Genera un manifiesto deterministico del directorio de evidencia (archivos spool,
captura de payload, snapshots de metricas) para que governance pueda verificar cada hash sin
desempacar el archivo.

```bash
python3 scripts/repo_evidence_manifest.py \
  --root artifacts/taikai/anchor/<event>/<alias>/<ts> \
  --agreement-id <event/alias/window> \
  --output artifacts/taikai/anchor/<event>/<alias>/<ts>/manifest.json
```

| Artefacto | Archivo | SHA-256 | Notas |
|----------|------|---------|-------|
| Manifiesto de evidencia | `manifest.json` | `<sha256>` | Adjunta esto al paquete de governance / GAR. |

## 7. Checklist

- [ ] Ledger de linaje copiado + hasheado.
- [ ] Hint de linaje copiado + hasheado.
- [ ] Payload POST del anchor capturado y hasheado.
- [ ] Tabla de digest del manifiesto completada.
- [ ] Snapshots de metricas exportados (`taikai_trm_alias_rotations_total`, `/status`).
- [ ] Manifiesto generado con `scripts/repo_evidence_manifest.py`.
- [ ] Paquete subido a governance con hashes + info de contacto.

Mantener esta plantilla para cada rotacion de alias mantiene el bundle de governance de
SoraNS reproducible y vincula los hints de linaje directamente con la evidencia GAR/RPT.
