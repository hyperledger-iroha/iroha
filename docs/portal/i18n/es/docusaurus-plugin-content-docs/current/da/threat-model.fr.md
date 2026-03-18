---
lang: es
direction: ltr
source: docs/portal/docs/da/threat-model.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::nota Fuente canónica
Reflejo `docs/source/da/threat_model.md`. Gardez les deux versiones sincronizadas
:::

# Modelo de amenazas Disponibilidad de datos Sora Nexus

_Revisión actual: 2026-01-19 -- Planificación de revisión de Prochaine: 2026-04-19_

Cadencia de mantenimiento: Grupo de trabajo sobre disponibilidad de datos (<=90 días). Chaqué
revisión doit apparaitre dans `status.md` avec des gravámenes vers les tickets de
actividades de mitigación y artefactos de simulación.

## Pero y el perímetro

El programa de disponibilidad de datos (DA) mantiene las transmisiones Taikai, los blobs
Nexus carril y artefactos de gobierno recuperables sous des fautes
bizantinos, reseau et operatorur. Ce modele de menaces ancre le travail
ingeniería para DA-1 (arquitectura y modelo de amenazas) y sert de baseline
pour les taches DA siguientes (DA-2 a DA-10).

Componentes en el alcance:
- Extensión de ingesta DA Torii y escritores de metadatos Norito.
- Arbres de stockage de blobs adosses a SoraFS (tiers hot/cold) et politiques de
  replicación.
- Compromisos de bloque Nexus (formatos de cable, pruebas, APIs light-client).
- Ganchos de aplicación PDP/PoTR específicos para cargas útiles DA.
- Operador de flujos de trabajo (fijación, desalojo, corte) y tuberías.
  de observabilidad.
- Aprobaciones de gobierno que admiten o expulsan a los operadores y
  contenido DA.Fuera de alcance para este documento:
- Modelización económica completa (capturada en el flujo de trabajo DA-7).
- Los protocolos base SoraFS dejan cubiertos por el modelo de amenazas SoraFS.
- Ergonomía de los clientes SDK au-dela des consideraciones de superficie de amenaza.

## Vista del conjunto arquitectónico1. **Sumisión:** Los clientes reciben blobs a través de la API de ingesta DA Torii.
   Noeud decoupe los blobs, codifique los manifiestos Norito (tipo de blob, carril,
   época, banderas de códec), y almacene los fragmentos en el nivel caliente SoraFS.
2. **Anuncio:** Intenciones de pin y sugerencias de replicación se propagan a versiones
   proveedores a través del registro (mercado SoraFS) con etiquetas de política qui
   Indicant les objectifs de retención caliente/frío.
3. **Compromiso:** Los secuenciadores Nexus incluyen compromisos de blobs (CID +
   racines KZG optionnelles) dans le bloc canonique. Les clientes ligeros se basan
   sobre el hash de compromiso y los metadatos anunciados para el verificador
   la disponibilidad.
4. **Replicación:** Les noeuds de stockage tirent les acciones/fragmentos asignados,
   Satisfacer los desafíos del PDP/PoTR y promover las donaciones entre niveles calientes.
   et cold selon la politique.
5. **Recuperar:** Los consumidores recuperan las donaciones a través de SoraFS o de las puertas de enlace
   Consciente del DA, verifica las pruebas y emite las demandas de reparación cuando
   des réplicas disparaissent.
6. **Gobernanza:** El Parlamento y el comité de supervisión DA aprueban
   Operadores, horarios de alquiler y escaladas de cumplimiento. Los artefactos de
   La gobernanza son acciones a través del meme voie DA para garantizar la transparencia.

## Activos y propietariosEchelle d'impact: **Crítica** casse la securite/vivacite du ledger; **Once**
bloquear le backfill DA o les clientes; **Modere** degrada la calidad del resto.
recuperable; **Faible** efecto límite.

| Activo | Descripción | íntegra | Disponibilidad | Confidencialidad | Propietario |
| --- | --- | --- | --- | --- | --- |
| Blobs DA (fragmentos + manifiestos) | Blobs Taikai, carril, acciones de gobernanza en SoraFS | Crítica | Crítica | Modere | DA WG / Equipo de almacenamiento |
| Manifiestos Norito DA | Tipos de metadatos que contienen blobs | Crítica | Eleve | Modere | Grupo de Trabajo sobre el Protocolo Básico |
| Compromisos en bloque | CID + racines KZG dans les blocs Nexus | Crítica | Eleve | Faible | Grupo de Trabajo sobre el Protocolo Básico |
| Horarios PDP/PoTR | Cadence d'enforcement pour les replicas DA | Eleve | Eleve | Faible | Equipo de almacenamiento |
| Operador de registro | Proveedores de almacenamiento aprobado y político | Eleve | Eleve | Faible | Consejo de Gobierno |
| Registros de alquiler e incentivos | Libro mayor de entradas para alquiler DA y sanciones | Eleve | Modere | Faible | Grupo de Trabajo de Tesorería |
| Paneles de observabilidad | SLOs DA, profundo de replicación, alertas | Modere | Eleve | Faible | SRE / Observabilidad |
| Intenciones de reparación | Requetes para rehidratar trozos manquants | Modere | Modere | Faible | Equipo de almacenamiento |

## Adversarios y capacidades| Actor | Capacidades | Motivaciones | Notas |
| --- | --- | --- | --- |
| Cliente malintencionado | Soumettre de blobs malformes, rejouer des manifests obsoletes, tenter DoS sur l'ingest. | Perturber les broadcasts Taikai, injecter des donnees invalides. | Privilegios del pas de cles. |
| Noeud de stockage byzantin | Cuentagotas de réplicas de cesionarios, falsificador de pruebas PDP/PoTR, cómplice. | Reduzca la retención DA, evite el alquiler, retenga los donantes. | Posee credenciales válidas. |
| Compromiso del secuenciador | Omettre des compromisos, equivoker sur les blocs, reordenar los metadatos. | Cacher des presentaciones DA, creer des incoherencias. | Límite par la mayoría de consenso. |
| Operador interno | Abusador del acceso al gobierno, manipulador de las políticas de retención, fuir de credenciales. | Ganancia económica, sabotaje. | Acceso a l'infraestructura frío/calor. |
| Reseau adversario | Particionador de datos, retardador de replicación, inyector de tráfico MITM. | Reduzca la disponibilidad y degrade los SLO. | Ne peut pas casser TLS mais peut dropper/ralentir les gravámenes. |
| Observabilidad attacuante | Manipular paneles/alertas, suprimir incidentes. | Registrar las interrupciones DA. | Es necesario acceder a la telemetría de la tubería. |

## Fronteras de confianza- **Ingreso fronterizo:** Cliente frente a la extensión DA Torii. Requerir par de autenticación
  solicitud, limitación de velocidad y validación de carga útil.
- **Replicación fronteriza:** Noeuds de stockage intercambiables trozos y pruebas. les
  Noeuds sont mutuellement autentifica mais peuvent se porter en byzantin.
- **Libro mayor fronterizo:** Donnees de block commits vs stockage off-chain. le
  consenso garantiza la integridad, pero la disponibilidad requiere una aplicación
  fuera de la cadena.
- **Gobernanza fronteriza:** Decisiones Aprouvant operatorurs del Consejo/Parlamento,
  Presupuestos y recortes. Les bris ici impactent directement le deploiement DA.
- **Observabilidad fronteriza:** Recopilar métricas/registros exportados a paneles y
  herramientas de alerta. La manipulación de la caché provoca interrupciones o ataques.

## Escenarios de amenaza y controles

### Attaques sur le chemin d'ingest

**Escenario:** El cliente malveillant soumet des payloads Norito malformes ou des
Blobs de gran tamaño para descargar recursos o inyectar metadatos
invalidar.**Controles**
- Validación del esquema Norito con negociación estricta de versión; rechazarles
  banderas en desacuerdo.
- Limitación de velocidad y autenticación en el punto final de ingesta Torii.
- Bornes del tamaño del fragmento y las fuerzas determinantes de codificación del fragmento SoraFS.
- El canal de admisión no persiste en los manifiestos después de coincidir con la suma de verificación.
- Replay cache deterministe (`ReplayCache`) adapt les fenetres `(carril, época,
  secuencia)`, persiste las marcas de agua alta en el disco y elimina los duplicados
  et reproduce obsoletos; aprovecha la propiedad y la pelusa cubre las huellas dactilares
  divergentes y presentaciones fuera de orden. [crates/iroha_core/src/da/replay_cache.rs:1]
  [fuzz/da_replay_cache.rs:1] [crates/iroha_torii/src/da/ingest.rs:1]

**Lacunas residuales**
- Torii ingiere para confiar el caché de reproducción a la admisión y persiste los cursos
  de secuencia a través de los reemarrages.
- Los esquemas Norito DA mantienen un arnés de fuzz dedie
  (`fuzz/da_ingest_schema.rs`) para enfatizar las invariantes de codificación/decodificación; les
  Los paneles de cobertura deben alertar si la cible regresa.

### Retención por retención de replicación

**Escenario:** Operadores de almacenamiento bizantinos aceptan los pins y los dropent les
trozos, pasan los desafíos del PDP/PoTR a través de respuestas falsificadas o colusión.**Controles**
- El calendario de desafíos PDP/PoTR incluye cargas útiles auxiliares DA con cobertura par
  época.
- Replicación de múltiples fuentes con solo quorum; l'orchestrateur detecte les
  shards manquants et declenche la reparation.
- La reducción drástica de la gobernanza se encuentra en las pruebas ecos y en las réplicas manquantes.
- Trabajo de conciliación automatizado (`cargo xtask da-commitment-reconcile`) aquí
  comparar los recibos de ingesta con los compromisos DA (SignedBlockWire,
  `.norito` o JSON), agregue un paquete JSON de evidencia para el gobierno y
  Echoue sur tickets manquants/mismatches pour que Alertmanager puisse buscapersonas.

**Lacunas residuales**
- Le arnés de simulación en `integration_tests/src/da/pdp_potr.rs` (cubierto
  par `integration_tests/tests/da/pdp_potr_simulation.rs`) exerce des scenarios
  de colusión y partición, validando que el programa PDP/PoTR detecte archivos
  comportement byzantin de facon deterministe. Continuer a l'etendre avec DA-5
  pour couvrir de nouvelles surfaces de proof.
- La política de desalojo en el nivel frío requiere una firma de seguimiento de auditoría para prevenir
  les gotas furtivas.

### Manipulation des commitments

**Escenario:** El secuenciador compromete la publicación de bloques omittantes o modificadores
compromisos DA, provocando fallos en la búsqueda o incoherencias en el cliente ligero.**Controles**
- El consenso verifica las proposiciones de bloque con los archivos de presentación.
  DA; les peers rejettent les propositions sans engagements requis.
- Los clientes ligeros verifican las pruebas de inclusión antes de exponer las manijas.
  de buscar.
- Pista de auditoría comparativa de los ingresos de ingesta y de los compromisos de bloque.
- Trabajo de conciliación automatizado (`cargo xtask da-commitment-reconcile`) aquí
  comparar los recibos de ingesta con los compromisos DA (SignedBlockWire,
  `.norito` o JSON), agregue un paquete JSON de evidencia para el gobierno y
  Echoue sur tickets manquants or discordancias para que Alertmanager pueda localizar.

**Lacunas residuales**
- Couvert par le job de reconciliation + gancho Alertmanager; los paquetes de
  La gobernanza es parte del mantenimiento del paquete JSON de evidencia por defecto.

### Partición de investigación y censura

**Escenario:** Adversaire particione le reseau de replication, empechant les
Noeuds d'obtener los fragmentos asignados o responder a los desafíos PDP/PoTR.

**Controles**
- Exigencias de proveedores multirregionales garantizados de chemins reseau divers.
- Ventanas de desafío con jitter y respaldo frente a los canales de reparación
  entre banda.
- Paneles de observabilidad que vigilan el profundidad de la replicación, les
  éxito de desafío y la latencia de fetch avec seuils d'alerte.**Lacunas residuales**
- Simulaciones de partición para eventos Taikai live encore manquantes;
  Pruebas de inmersión en remojo.
- Política de reserva de banda pasada de reparación pas encore codificada.

### Abuso interno

**Escenario:** Operador con acceso al registro manipula las políticas de
retención, lista blanca de proveedores malintencionados o supresión de alertas.

**Controles**
- Las acciones de gobierno requieren firmas multipartidistas y registros.
  certifica ante notario Norito.
- Los cambios políticos realizados en eventos relacionados con el monitoreo y los registros
  de archivo.
- La canalización de observabilidad impone los registros Norito de solo agregar con hash
  encadenamiento.
- La automatización de la auditoría trimestral (`cargo xtask da-privilege-audit`) parcourt
  les repertoires manifest/replay (más rutas proporcionadas por operadores), señale les
  entradas manquantes/non-directory/world-writable, y emet un paquete con firma JSON
  para paneles de gobierno.

**Lacunas residuales**
- La prevención de manipulación del tablero requiere instantáneas firmadas.

## Registro de riesgos residuales| Riesgoso | Probabilidad | Impacto | Propietario | Plan de mitigación |
| --- | --- | --- | --- | --- |
| Reproducción de manifiestos DA antes de la llegada del caché de secuencia DA-2 | Posible | Modere | Grupo de Trabajo sobre el Protocolo Básico | Caché de secuencia del implementador + validación de nonce en DA-2; Agregar pruebas de regresión. |
| Colusión PDP/PoTR cuando los conflictos son compromisos | Peu probable | Eleve | Equipo de almacenamiento | Derivar un nuevo cronograma de desafíos con muestreo entre proveedores; validador mediante arnés de simulación. |
| Brecha de auditoría de desalojo en el nivel frío | Posible | Eleve | SRE / Equipo de Almacenamiento | Adjunto de registros, firmas y recibos en cadena para desalojos; monitor a través de paneles de control. |
| Latencia de detección de omisión de secuenciador | Posible | Eleve | Grupo de Trabajo sobre el Protocolo Básico | `cargo xtask da-commitment-reconcile` nocturno compara recibos versus compromisos (SignedBlockWire/`.norito`/JSON) y pagina la gobernanza sobre tickets manquants o desajustes. |
| Resiliencia partición pour streams Taikai live | Posible | Crítica | Redes TL | Ejecutor de ejercicios de partición; reserver la bande passante de reparation; documentador SOP de conmutación por error. |
| Deriva del privilegio de gobierno | Peu probable | Eleve | Consejo de Gobierno | `cargo xtask da-privilege-audit` trimestriel (directorios manifiesto/repetición + rutas adicionales) con firma JSON + panel de puerta; Ancrer les artefactos de auditoría en cadena. |

## Requisitos de seguimiento1. Publier les esquemas Norito d'ingest DA et des vecteurs d'exemple (porte dans
   DA-2).
2. Ramifique la caché de reproducción en la ingesta DA Torii y persista los cursores de
   secuencia a través de los reemarrages.
3. **Termine (2026-02-05):** El arnés de simulación PDP/PoTR mantiene el ejercicio
   colusión + partición con modelización de la QoS pendiente; ver
   `integration_tests/src/da/pdp_potr.rs` (pruebas bajo
   `integration_tests/tests/da/pdp_potr_simulation.rs`) para la implementación y
   les resumes deterministes captures ci-dessous.
4. **Terminar (2026-05-29):** `cargo xtask da-commitment-reconcile` comparar archivos
   recibos de ingesta de compromisos auxiliares DA (SignedBlockWire/`.norito`/JSON), emet
   `artifacts/da/commitment_reconciliation.json`, et est rama a
   Alertmanager/paquetes de gobierno para alertas de omisión/manipulación
   (`xtask/src/da.rs`).
5. **Termine (2026-05-29):** `cargo xtask da-privilege-audit` parcourt le spool
   manifiesto/repetición (más rutas proporcionadas por operadores), señal de entradas
   manquantes/non-directory/world-writable y produce un paquete con firma JSON para
   paneles/revistas de gobierno (`artifacts/da/privilege_audit.json`),
   fermant la laguna de automatización de acceso.

**Nuestro baño privado:**- El caché de reproducción y la persistencia de los cursores se activan en DA-2. Ver
  La implementación en `crates/iroha_core/src/da/replay_cache.rs` (lógica de
  caché) y la integración Torii en `crates/iroha_torii/src/da/ingest.rs`, aquí hilo
  les checks de huellas dactilares a través de `/v1/da/ingest`.
- Las simulaciones de streaming PDP/PoTR se realizan a través del arnésproof-stream
  En `crates/sorafs_car/tests/sorafs_cli.rs`, fluye el flujo de solicitud.
  PoR/PDP/PoTR y los escenarios de fracaso animes en el modelo de amenazas.
- Los resultados de capacidad y reparación empapan vivent sous
  `docs/source/sorafs/reports/sf2c_capacity_soak.md`, tandis que la matriz de
  remojar Sumeragi más grande est suivie dans `docs/source/sumeragi_soak_matrix.md`
  (las variantes localizan inclus). Estos artefactos capturan los taladros a largo plazo.
  referencias en el registro de riesgos residuales.
- La reconciliación de automatización + auditoría de privilegios vit dans
  `docs/automation/da/README.md` y los nuevos comandos
  `cargo xtask da-commitment-reconcile` / `cargo xtask da-privilege-audit`; utilisé
  Las salidas por defecto bajo `artifacts/da/` lors de l'attachement d'evidence aux
  paquetes de gobierno.

## Evidencia de simulación y modelización QoS (2026-02)Para cerrar el seguimiento DA-1 #3, codificamos un arnés de simulación
PDP/PoTR determinista bajo `integration_tests/src/da/pdp_potr.rs` (cubierta par
`integration_tests/tests/da/pdp_potr_simulation.rs`). Le arnés alloue des
noeuds sur trois regiones, inyectar particiones/colusión según las probabilidades del
hoja de ruta, adaptarse a la tardanza PoTR, y alimentar un modelo de acumulación de reparación
qui refleja el presupuesto de reparación del nivel caliente. La ejecución del escenario par
defaut (12 épocas, 18 desafíos PDP + 2 fenetres PoTR por época) a produit les
métricas siguientes:<!-- BEGIN_DA_SIM_TABLE -->
<!-- AUTO-GENERATED by scripts/docs/render_da_threat_model_tables.py; do not edit manually. -->
| Métrica | valor | Notas |
| --- | --- | --- |
| Fallos del PDP detectados | 48 / 49 (98,0%) | Las particiones declencent siguen siendo detectadas; un seul echec non detecte vient d'un jitter honnete. |
| Latencia media de detección de PDP | 0,0 épocas | Les echecs sont signales dans l'epoch d'origine. |
| Fallos PoTR detectados | 28/77 (36,4%) | La detección se reduce cuando un noeud rate >=2 fenetres PoTR, libera la mayor parte de los eventos en el registro de riesgos residuales. |
| Latencia media de detección de PoTR | 2.0 épocas | Correspond au seuil de lateness a deux epochs integre dans l'escalation d'archivage. |
| Pico de cola de reparación | 38 manifiestos | El backlog monte quand les particiones s'empilent plus vite que les quatre reparaciones disponibles por época. |
| Latencia de respuesta p95 | 30.068 ms | Refleje la ventana de desafío durante 30 s con jitter de +/-75 ms aplicado al muestreo QoS. |
<!-- END_DA_SIM_TABLE -->

Estas salidas alimentan el mantenimiento de los prototipos de tablero DA y satisfacen
Los criterios de aceptación de "arnés de simulación + modelado de QoS" hacen referencia a ellos.
la hoja de ruta.La automatización del trasero
`cargo xtask da-threat-model-report [--out <path|->] [--seed <u64|0xhex>] [--config <path>]`,
qui appelle le Harness partage et emet Norito versión JSON
`artifacts/da/threat_model_report.json` por defecto. Los trabajos nocturnos
consomment ce fichier pour rafraichir les matrices dans ce document et alerter
sur la deriva de tareas de detección, colas de reparación o muestras de QoS.

Pour rafraichir la table ci-dessus pour les docs, ejecutante `make docs-da-threat-model`,
qui invoque `cargo xtask da-threat-model-report`, regenere
`docs/source/da/_generated/threat_model_report.json`, y reescriba esta sección
vía `scripts/docs/render_da_threat_model_tables.py`. El espejo `docs/portal`
(`docs/portal/docs/da/threat-model.md`) est mis a jour dans le meme pasaje para
que las dos copias permanecen sincronizadas.