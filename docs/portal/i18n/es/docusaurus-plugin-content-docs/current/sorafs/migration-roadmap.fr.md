---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/migration-roadmap.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
título: "Hoja de ruta de migración SoraFS"
---

> Adapte de [`docs/source/sorafs/migration_roadmap.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/migration_roadmap.md).

# Hoja de ruta de migración SoraFS (SF-1)

Ce documento pone en práctica las directivas de migración capturadas en
`docs/source/sorafs_architecture_rfc.md`. Il developmente les livrables SF-1 es
Jalons prepara al ejecutor, criterios de paso y listas de verificación de los responsables afin.
que equipan almacenamiento, gobernanza, DevRel y SDK coordinan la transición del

La hoja de ruta es voluntaria y determinista: chaque jalon nomme les
artefactos requeridos, las invocaciones de comandos y las etapas de atestación para
que les pipelines downstream producen salidas idénticas y que la
La gobernanza conserva un rastro auditable.

## Vista del conjunto de jalons

| Jalón | Fentre | Objetos principales | Hazlo libre | Propietarios |
|-------|---------|----------------------|-------------|--------|
| **M1 - Determinación de la aplicación** | Semanas 7-12 | Exiger des fixees signees et preparer les preuves d'alias colgante que les pipelines adoptent les expectation flags. | Verificación nocturna de los accesorios, manifiestos firmados por el consejo, entradas en escena del registro de alias. | Almacenamiento, Gobernanza, SDK |

El estado de los jalones se encuentra en `docs/source/sorafs/migration_ledger.md`. Totales
las modificaciones de esta hoja de ruta DOIVENT mettre a jour le registre afin
que la gobernanza y la ingeniería de lanzamiento se sincronizan.## Pistas de trabajo

### 2. Adopción del determinismo de fijación

| Etapa | Jalón | Descripción | Propietario(s) | Salida |
|-------|-------|-------------|----------|--------|
| Repeticiones de partidos | M0 | Los hebdomadaires realizan ensayos en seco comparando los resúmenes de lugares de fragmentos con `fixtures/sorafs_chunker`. Publier un rapport sous `docs/source/sorafs/reports/`. | Proveedores de almacenamiento | `determinism-<date>.md` con matriz pasa/falla. |
| Exiger les firmas | M1 | `ci/check_sorafs_fixtures.sh` + `.github/workflows/sorafs-fixtures-nightly.yml` se hacen eco de firmas o manifiestos derivados. Las anulaciones de desarrollo exigen un agregado de gobernanza de exención en las relaciones públicas. | Grupo de Trabajo sobre Herramientas | Registro CI, gravamen versus ticket de waiver (si corresponde). |
| Banderas de expectativa | M1 | Les pipelines apelante `sorafs_manifest_stub` con las expectativas explícitas para hacer las salidas: | Documentos CI | Los guiones son un día referente a las banderas de expectativas (voir bloc de commande ci-dessous). |
| Fijar el registro primero | M2 | `sorafs pin propose` e `sorafs pin approve` envuelven las soumisiones de manifiesto; La CLI por defecto utiliza `--require-registry`. | Operaciones de gobernanza | Registro de auditoría del registro CLI, telemetría de propuestas tarifadas. |
| Observabilidad parita | M3 | Los paneles de control Prometheus/Grafana alertan sobre los inventarios de fragmentos divergentes del registro de manifiestos; alertes Branchees sur l'astreinte ops. | Observabilidad | Panel de control de gravámenes, ID de reglas de alerta, resultados de GameDay. |

#### Comando canónico de publicación```bash
cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- docs/book \
  --manifest-out artifacts/docs/book/2025-11-01/docs.manifest \
  --manifest-signatures-out artifacts/docs/book/2025-11-01/docs.manifest_signatures.json \
  --car-out artifacts/docs/book/2025-11-01/docs.car \
  --chunk-fetch-plan-out artifacts/docs/book/2025-11-01/docs.fetch_plan.json \
  --car-digest=13fa919c67e55a2e95a13ff8b0c6b40b2e51d6ef505568990f3bc7754e6cc482 \
  --car-size=429391872 \
  --root-cid=f40101... \
  --dag-codec=0x71
```

Reemplace los valores de digest, taille y CID por las referencias asistentes
recensees dans l'entree du registro de migración para el artefacto.

### 3. Transición de alias y comunicaciones

| Etapa | Jalón | Descripción | Propietario(s) | Salida |
|-------|-------|-------------|----------|--------|
| Preuves d'alias en puesta en escena | M1 | Registrar los reclamos de alias en el Pin Registry staging y adjuntar los preuves Merkle aux manifests (`--alias`). | Gobernanza, Documentos | Bundle de preuves stocke a cote du manifest + commentaire du registre avec le nom d'alias. |
| Aplicación de las normas | M2 | Las puertas de enlace rechazan los manifiestos sin encabezados `Sora-Proof` recientes; CI añada la cinta `sorafs alias verify` para recuperar los antiguos. | Redes | Parche de configuración de puerta de enlace + salida CI capturante de verificación reussie. |

### 4. Comunicación y auditoría

- **Discipline du registre:** cada cambio de estado (derivación de accesorios, registro de salida,
  activación de alias) debe agregar una fecha de nota en
  `docs/source/sorafs/migration_ledger.md`.
- **Actas de gobierno:** las sesiones del consejo aprueban los cambios del registro de PIN o
  les politiques d'alias doivent referencer esta hoja de ruta y le registre.
- **Comunicaciones externas:** DevRel publie des mises a jour a chaque jalon (blog + extracto del registro de cambios)
  mettant en avant les garanties deterministes et les calendriers d'alias.## Dependencias y riesgos

| Dependencia | Impacto | Mitigación |
|------------|--------|------------|
| Disponibilidad del contrato Registro de PIN | Bloquee el pin desplegable M2 primero. | Prepare el contrato anterior a M2 con pruebas de repetición; mantener un sobre de reserva justo para estabilizar. |
| Cles de firma del consejo | Solicitudes para los sobres de manifiesto y el registro de aprobaciones. | Ceremonia de firma documentada en `docs/source/sorafs/signing_ceremony.md`; rotación avec chevauchement et note dans le registre. |
| Cadencia de lanzamiento del SDK | Les clientes deben honrar las preuves d'alias avant M3. | Alinee las ventanas de liberación del SDK en las puertas de los jalones; Agregar listas de verificación de migración y plantillas de lanzamiento. |

Los riesgos residuales y mitigaciones son reprennent dans `docs/source/sorafs_architecture_rfc.md`
et doivent etre recupes lors des ajustements.

## Lista de verificación de criterios de salida

| Jalón | Criterios |
|-------|----------|
| M1 | - Trabajo nocturno de accesorios vert durante septiembre de días consecutivos.  - Preuves d'alias staging verifiées en CI.  - La gobernanza ratifica la política de expectativas. |

## Gestión del cambio1. Proponente de ajustes a través de relaciones públicas mettant a jour ce fichier **et**
   `docs/source/sorafs/migration_ledger.md`.
2. Lea las actas de gobierno y las preuves CI en la descripción de PR.
3. Después de la fusión, notifique la lista de almacenamiento + DevRel con un currículum y acciones
   asistentes de operadores.

Suivre este procedimiento garantiza que el despliegue SoraFS reste deterministe,
auditable y transparente entre los equipos participante au lancement Nexus.