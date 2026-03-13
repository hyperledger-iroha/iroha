---
lang: es
direction: ltr
source: docs/examples/finance/repo_governance_packet_template.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 7fe5fb47af37f86d33bfa884dc920efbf66714bbe3535842a786755dd5649f65
source_last_modified: "2025-12-07T08:26:38.035018+00:00"
translation_last_reviewed: 2026-01-01
---

# Plantilla de paquete de governance de repo (Roadmap F1)

Usa esta plantilla al preparar el bundle de artefactos requerido por el item de roadmap
F1 (documentacion y tooling del ciclo de vida de repo). El objetivo es entregar a los revisores un
solo archivo Markdown que liste cada input, hash y bundle de evidencia para que el
consejo de governance pueda reproducir los bytes referenciados en la propuesta.

> Copia la plantilla en tu propio directorio de evidencia (por ejemplo
> `artifacts/finance/repo/2026-03-15/packet.md`), reemplaza los placeholders y
> commitea/sube el archivo junto a los artefactos hasheados referenciados abajo.

## 1. Metadatos

| Campo | Valor |
|-------|-------|
| Identificador de acuerdo/cambio | `<repo-yyMMdd-XX>` |
| Preparado por / fecha | `<desk lead> - 2026-03-15T10:00Z` |
| Revisado por | `<dual-control reviewer(s)>` |
| Tipo de cambio | `Initiation / Haircut update / Substitution matrix change / Margin policy` |
| Custodio(s) | `<custodian id(s)>` |
| Propuesta vinculada / referendum | `<governance ticket id or GAR link>` |
| Directorio de evidencia | ``artifacts/finance/repo/<slug>/`` |

## 2. Payloads de instrucciones

Registra las instrucciones Norito staged que los desks aprobaron via
`iroha app repo ... --output`. Cada entrada debe incluir el hash del archivo emitido
y una descripcion corta de la accion que se enviara una vez que el voto pase.

| Accion | Archivo | SHA-256 | Notas |
|--------|------|---------|-------|
| Initiate | `instructions/initiate.json` | `<sha256>` | Contiene las patas cash/collateral aprobadas por desk + counterparty. |
| Margin call | `instructions/margin_call.json` | `<sha256>` | Captura cadence + participant id que disparo el call. |
| Unwind | `instructions/unwind.json` | `<sha256>` | Prueba de la reverse-leg cuando se cumplan condiciones. |

```bash
# Ejemplo de helper de hash (repetir por archivo de instruccion)
sha256sum artifacts/finance/repo/<slug>/instructions/initiate.json       | tee artifacts/finance/repo/<slug>/hashes/initiate.sha256
```

## 2.1 Acuses de custodio (solo tri-party)

Completa esta seccion cuando un repo use `--custodian`. El paquete de governance
debe incluir un acuse firmado de cada custodio mas el hash del archivo referido en
sec 2.8 de `docs/source/finance/repo_ops.md`.

| Custodio | Archivo | SHA-256 | Notas |
|-----------|------|---------|-------|
| `<i105...>` | `custodian_ack_<custodian>.md` | `<sha256>` | SLA firmado que cubre ventana de custodia, cuenta de ruteo y contacto de drill. |

> Guarda el acuse junto al resto de evidencia (`artifacts/finance/repo/<slug>/`)
> para que `scripts/repo_evidence_manifest.py` registre el archivo en el mismo arbol que
> las instrucciones staged y snippets de config. Ver
> `docs/examples/finance/repo_custodian_ack_template.md` para un template listo que coincide
> con el contrato de evidencia de governance.

## 3. Snippet de configuracion

Pega el bloque TOML `[settlement.repo]` que se aplicara en el cluster (incluyendo
`collateral_substitution_matrix`). Guarda el hash junto al snippet para que los auditores
confirmen la politica runtime activa cuando se aprobo la reserva del repo.

```toml
[settlement.repo]
eligible_collateral = ["bond#wonderland", "note#wonderland"]
default_margin_percent = "0.025"

[settlement.repo.collateral_substitution_matrix]
"bond#wonderland" = ["bill#wonderland"]
```

`SHA-256 (snippet de config): <sha256>`

### 3.1 Snapshots de configuracion post-aprobacion

Despues de que el referendum o voto de governance se complete y el cambio `[settlement.repo]`
se despliegue, captura snapshots de `/v2/configuration` de cada peer para que los
auditores puedan probar que la politica aprobada esta activa en el cluster (ver
`docs/source/finance/repo_ops.md` sec 2.9 para el workflow de evidencia).

```bash
mkdir -p artifacts/finance/repo/<slug>/config/peers
curl -fsSL https://peer01.example/v2/configuration       | jq '.'       > artifacts/finance/repo/<slug>/config/peers/peer01.json
```

| Peer / fuente | Archivo | SHA-256 | Altura de bloque | Notas |
|---------------|------|---------|--------------|-------|
| `peer01` | `config/peers/peer01.json` | `<sha256>` | `<block-height>` | Snapshot capturado justo despues del rollout de config. |
| `peer02` | `config/peers/peer02.json` | `<sha256>` | `<block-height>` | Confirma que `[settlement.repo]` coincide con el TOML staged. |

Registra los digests junto a los peer ids en `hashes.txt` (o el resumen equivalente)
para que los revisores puedan rastrear que nodos ingirieron el cambio. Los snapshots
viven en `config/peers/` junto al snippet TOML y seran recogidos automaticamente por
`scripts/repo_evidence_manifest.py`.

## 4. Artefactos de tests deterministas

Adjunta los ultimos outputs de:

- `cargo test -p iroha_core -- repo_deterministic_lifecycle_proof_matches_fixture`
- `cargo test --package integration_tests --test repo`

Registra rutas de archivos + hashes para los bundles de logs o JUnit XML que produzca tu CI.

| Artefacto | Archivo | SHA-256 | Notas |
|----------|------|---------|-------|
| Log de lifecycle proof | `tests/repo_lifecycle.log` | `<sha256>` | Capturado con `--nocapture`. |
| Log de integration test | `tests/repo_integration.log` | `<sha256>` | Incluye cobertura de substitution + cadence de margen. |

## 5. Snapshot de lifecycle proof

Cada paquete debe incluir el snapshot determinista exportado desde
`repo_deterministic_lifecycle_proof_matches_fixture`. Ejecuta el harness con los
knobs de export habilitados para que los revisores puedan comparar el frame JSON y el
digest contra la fixture en `crates/iroha_core/tests/fixtures/` (ver
`docs/source/finance/repo_ops.md` sec 2.7).

```bash
REPO_PROOF_SNAPSHOT_OUT=artifacts/finance/repo/<slug>/repo_proof_snapshot.json     REPO_PROOF_DIGEST_OUT=artifacts/finance/repo/<slug>/repo_proof_digest.txt     cargo test -p iroha_core       -- --exact smartcontracts::isi::repo::tests::repo_deterministic_lifecycle_proof_matches_fixture
```

O usa el helper fijado para regenerar las fixtures y copiarlas a tu bundle de evidencia en un solo paso:

```bash
scripts/regen_repo_proof_fixture.sh --toolchain <toolchain>       --bundle-dir artifacts/finance/repo/<slug>
```

| Artefacto | Archivo | SHA-256 | Notas |
|----------|------|---------|-------|
| Snapshot JSON | `repo_proof_snapshot.json` | `<sha256>` | Frame de lifecycle canonico emitido por el proof harness. |
| Digest file | `repo_proof_digest.txt` | `<sha256>` | Digest hex en mayusculas reflejado desde `crates/iroha_core/tests/fixtures/repo_lifecycle_proof.digest`; adjunta incluso si no cambia. |

## 6. Manifiesto de evidencia

Genera el manifiesto para todo el directorio de evidencia para que los auditores puedan verificar
hashes sin desempaquetar el archivo. El helper refleja el workflow descrito en
`docs/source/finance/repo_ops.md` sec 3.2.

```bash
python3 scripts/repo_evidence_manifest.py       --root artifacts/finance/repo/<slug>       --agreement-id <repo-identifier>       --output artifacts/finance/repo/<slug>/manifest.json
```

| Artefacto | Archivo | SHA-256 | Notas |
|----------|------|---------|-------|
| Manifiesto de evidencia | `manifest.json` | `<sha256>` | Incluye el checksum en el ticket de governance / notas del referendum. |

## 7. Snapshot de telemetria y eventos

Exporta las entradas relevantes de `AccountEvent::Repo(*)` y cualquier dashboard o export CSV
referenciado en `docs/source/finance/repo_ops.md`. Registra archivos + hashes aqui para que los
revisores vayan directo a la evidencia.

| Export | Archivo | SHA-256 | Notas |
|--------|------|---------|-------|
| Repo events JSON | `evidence/repo_events.ndjson` | `<sha256>` | Stream Torii en bruto filtrado a las cuentas de desk. |
| Telemetry CSV | `evidence/repo_margin_dashboard.csv` | `<sha256>` | Exportado desde Grafana usando el panel Repo Margin. |

## 8. Aprobaciones y firmas

- **Firmantes de doble control:** `<names + timestamps>`
- **GAR / minutes digest:** `<sha256>` del PDF GAR firmado o del upload de minutes.
- **Ubicacion de storage:** `governance://finance/repo/<slug>/packet/`

## 9. Checklist

Marca cada item cuando este completo.

- [ ] Payloads de instrucciones staged, hasheados y adjuntos.
- [ ] Hash del snippet de configuracion registrado.
- [ ] Logs de tests deterministas capturados + hasheados.
- [ ] Snapshot de lifecycle + digest exportado.
- [ ] Manifiesto de evidencia generado y hash registrado.
- [ ] Exports de eventos/telemetria capturados + hasheados.
- [ ] Acuses de doble control archivados.
- [ ] GAR/minutes cargados; digest registrado arriba.

Mantener esta plantilla junto a cada paquete mantiene el DAG de governance determinista y
brinda a los auditores un manifiesto portable para decisiones del ciclo de vida de repo.
