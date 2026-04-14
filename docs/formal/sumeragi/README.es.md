<!-- Auto-generated stub for Spanish (es) translation. Replace this content with the full translation. -->

---
lang: es
direction: ltr
source: docs/formal/sumeragi/README.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 56f1412b2db729ba69057ce15ac8bae707310fd5a6d01be2da816fdee18218f7
source_last_modified: "2026-02-23T14:48:46.580877+00:00"
translation_last_reviewed: 2026-04-02
translator: machine-google-reviewed
---

# Sumeragi Modelo formal (TLA+ / Apalache)

Este directorio contiene un modelo formal acotado para la seguridad y vitalidad de la ruta de confirmación Sumeragi.

## Alcance

El modelo captura:
- progresión de fase (`Propose`, `Prepare`, `CommitVote`, `NewView`, `Committed`),
- umbrales de votación y quórum (`CommitQuorum`, `ViewQuorum`),
- quórum de participación ponderado (`StakeQuorum`) para guardias de compromiso estilo NPoS,
- Causalidad de glóbulos rojos (`Init -> Chunk -> Ready -> Deliver`) con evidencia de encabezado/resumen,
- GST y supuestos débiles de equidad sobre acciones de progreso honestas.

Abstrae intencionalmente formatos de cables, firmas y detalles completos de la red.

## Archivos

- `Sumeragi.tla`: modelo de protocolo y propiedades.
- `Sumeragi_fast.cfg`: conjunto de parámetros más pequeño compatible con CI.
- `Sumeragi_deep.cfg`: conjunto de parámetros de tensión mayor.

## Propiedades

Invariantes:
- `TypeInvariant`
- `CommitImpliesQuorum`
- `CommitImpliesStakeQuorum`
- `CommitImpliesDelivered`
- `DeliverImpliesEvidence`

Propiedad temporal:
- `EventuallyCommit` (`[] (gst => <> committed)`), con equidad post-GST codificada
  operativamente en `Next` (protecciones de tiempo de espera/prevención de fallas activadas)
  acciones de avance). Esto mantiene el modelo comprobable con Apalache 0.52.x, que
  no admite operadores de equidad `WF_` dentro de propiedades temporales marcadas.

## Corriendo

Desde la raíz del repositorio:

```bash
bash scripts/formal/sumeragi_apalache.sh fast
bash scripts/formal/sumeragi_apalache.sh deep
```

### Configuración local reproducible (no se requiere Docker)Instale la cadena de herramientas local de Apalache anclada utilizada por este repositorio:

```bash
bash scripts/formal/install_apalache.sh 0.52.2
```

El ejecutor detecta automáticamente esta instalación en:
`target/apalache/toolchains/v0.52.2/bin/apalache-mc`.
Después de la instalación, `ci/check_sumeragi_formal.sh` debería funcionar sin variables de entorno adicionales:

```bash
bash ci/check_sumeragi_formal.sh
```

Si Apalache no está en `PATH`, puedes:

- establezca `APALACHE_BIN` en la ruta ejecutable, o
- utilice el respaldo Docker (habilitado de forma predeterminada cuando `docker` está disponible):
  - imagen: `APALACHE_DOCKER_IMAGE` (predeterminado `ghcr.io/apalache-mc/apalache:latest`)
  - requiere un demonio Docker en ejecución
  - deshabilite el respaldo con `APALACHE_ALLOW_DOCKER=0`.

Ejemplos:

```bash
APALACHE_BIN=/opt/apalache/bin/apalache-mc bash scripts/formal/sumeragi_apalache.sh fast
APALACHE_DOCKER_IMAGE=ghcr.io/apalache-mc/apalache:latest bash scripts/formal/sumeragi_apalache.sh deep
```

## Notas

- Este modelo complementa (no reemplaza) las pruebas ejecutables del modelo Rust en
  `crates/iroha_core/src/sumeragi/main_loop/tests/state_machine_model_tests.rs`
  y
  `crates/iroha_core/src/sumeragi/main_loop/tests/state_machine_fairness_model_tests.rs`.
- Las comprobaciones están limitadas por valores constantes en los archivos `.cfg`.
- PR CI ejecuta estas comprobaciones en `.github/workflows/pr.yml` vía
  `ci/check_sumeragi_formal.sh`.