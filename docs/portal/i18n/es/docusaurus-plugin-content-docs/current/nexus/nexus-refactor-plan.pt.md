---
lang: es
direction: ltr
source: docs/portal/docs/nexus/nexus-refactor-plan.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: plan-refactor-nexus
título: Plano de refatoracao do ledger Sora Nexus
descripción: Espelho de `docs/source/nexus_refactor_plan.md`, detalhando o trabajo de limpeza por fases para a base de código do Iroha 3.
---

:::nota Fuente canónica
Esta página refleja `docs/source/nexus_refactor_plan.md`. Mantenha as duas copias alinhadas ate que a edicao multilingue chegue ao portal.
:::

# Plano de refatoracao del libro mayor Sora Nexus

Este documento capturó la hoja de ruta inmediatamente para la refactorización de Sora Nexus Ledger ("Iroha 3"). Ele refleja el diseño actual del repositorio y los ingresos observados en la contabilidad genesis/WSV, consenso Sumeragi, activadores de contratos inteligentes, consultas de instantáneas, enlaces de puntero de host-ABI y códecs Norito. El objetivo e converger para una arquitectura coerente y testavel sem tentar entregar todas las correcciones en un único parche monolítico.## 0. Principios guía
- Preservar el comportamiento determinista en hardware heterogéneo; usar aceleración apenas a través de indicadores de funciones opt-in com fallbacks identicos.
- Norito y una camada de serialización. Cualquier cambio de estado/esquema debe incluir pruebas de codificación/decodificación de ida y vuelta Norito y actualizaciones de accesorios.
- Una configuración fluida según `iroha_config` (usuario -> real -> valores predeterminados). El removedor alterna ad-hoc de ambiente dos caminos de producción.
- A politica ABI permanece V1 e inegociavel. Los hosts deben evitar tipos de punteros/llamadas al sistema desconcertados de forma determinística.
- `cargo test --workspace` y las pruebas doradas (`ivm`, `norito`, `integration_tests`) continúan como base de puerta para cada marco.## 1. Instantánea de la topología del repositorio
- `crates/iroha_core`: controladores Sumeragi, WSV, cargador de génesis, canalizaciones (consulta, superposición, carriles zk), pegamento que aloja los contratos inteligentes.
- `crates/iroha_data_model`: esquema autoritativo para datos y consultas en cadena.
- `crates/iroha`: API de cliente usada por CLI, pruebas, SDK.
- `crates/iroha_cli`: CLI de operadores, actualmente utiliza numerosas API en `iroha`.
- `crates/ivm`: VM de código de bytes Kotodama, puntos de entrada de integración puntero-ABI al host.
- `crates/norito`: códec de serialización con adaptadores JSON y backends AoS/NCB.
- `integration_tests`: aserciones de componentes cruzados cobrindo genesis/bootstrap, Sumeragi, disparadores, paginacao, etc.
- Docs ja delineiam metas do Sora Nexus Ledger (`nexus.md`, `new_pipeline.md`, `ivm.md`), mas a implementacao esta fragmentada y parcialmente defasada em relacao ao codigo.

## 2. Pilares de refatoracao e marcos### Fase A - Fundamentos y observabilidade
1. **Telemetria WSV + Instantáneas**
   - Establecer una API canónica de instantáneas en `state` (rasgo `WorldStateSnapshot`) utilizada para consultas, Sumeragi y CLI.
   - Usar `scripts/iroha_state_dump.sh` para producir instantáneas deterministas a través de `iroha state dump --format norito`.
2. **Determinismo de Génesis/Bootstrap**
   - Refatorar a ingestao de génesis para fluir por un único tubo con Norito (`iroha_core::genesis`).
   - Agregar cobertura de integración/regreso que reprocesa génesis más el primer bloque y afirma raíces WSV idénticas entre arm64/x86_64 (acompañado en `integration_tests/tests/genesis_replay_determinism.rs`).
3. **Pruebas de fijación entre cajas**
   - Expandir `integration_tests/tests/genesis_json.rs` para validar invariantes de WSV, pipeline y ABI en un arnés único.
   - Introducimos un scaffold `cargo xtask check-shape` que causa pánico en la deriva del esquema (acompañado de trabajos pendientes de herramientas DevEx; ver elemento de acción en `scripts/xtask/README.md`).### Fase B - WSV y superficie de consultas
1. **Transacoes de almacenamiento estatal**
   - Colapsar `state/storage_transactions.rs` en un adaptador transacional que imponha ordenacao de commit y detección de conflictos.
   - Las pruebas unitarias ahora verifican que las modificaciones de activos/mundo/triggers hacen rollback en falhas.
2. **Refator del modelo de consultas**
   - Mueva la lógica de página/cursor para componentes reutilizados en `crates/iroha_core/src/query/`. Alinhar representa Norito en `iroha_data_model`.
   - Agregar consultas instantáneas para activadores, activos y roles con ordenación determinista (acompañado a través de `crates/iroha_core/tests/snapshot_iterable.rs` para una cobertura actual).
3. **Consistencia de instantáneas**
   - Garantía de que la CLI `iroha ledger query` utiliza el mismo camino de instantánea que Sumeragi/fetchers.
   - Pruebas de regreso de instantáneas en CLI en vivo en `tests/cli/state_snapshot.rs` (funciones controladas para ejecuciones lentas).### Fase C - Tubería Sumeragi
1. **Topología y gestión de épocas**
   - Extrair `EpochRosterProvider` para un rasgo con implementaciones basadas en instantáneas de participación WSV.
   - `WsvEpochRosterAdapter::from_peer_iter` ofrece un constructor simple y amigable para simulacros en bancos/pruebas.
2. **Simplificación del flujo de consenso**
   - Reorganizar `crates/iroha_core/src/sumeragi/*` en módulos: `pacemaker`, `aggregation`, `availability`, `witness` con tipos compartidos sob `consensus`.
   - Sustituir el paso de mensajes ad-hoc por sobres Norito tipados e introducir pruebas de propiedad de cambio de vista (acompanhado no backlog de mensageria Sumeragi).
3. **Carril Integracao/prueba**
   - Alinhar lane pruebas con compromisos de DA y garantía de que la entrada de RBC seja uniforme.
   - La prueba de integración de extremo a extremo `integration_tests/tests/extra_functional/seven_peer_consistency.rs` ahora verifica el camino con RBC habilitado.### Fase D - Contratos inteligentes y puntero de hosts-ABI
1. **Auditoria da fronteira do host**
   - Consolidar verificacoes de pointer-type (`ivm::pointer_abi`) y adaptadores de host (`iroha_core::smartcontracts::ivm::host`).
   - Como expectativas de la tabla de punteros y los enlaces del manifiesto del host están cubiertos por `crates/iroha_core/tests/ivm_pointer_abi_tlv_types.rs` e `ivm_host_mapping.rs`, que ejercitan las asignaciones TLV golden.
2. **Sandbox de ejecución de activadores**
   - Reforzar los disparadores para rodar a través de un `TriggerExecutor` junto con el control de gas, la validación de punteros y el registro de eventos.
   - Agregar pruebas de regreso para triggers de call/time cobrindo paths de falha (acompañado vía `crates/iroha_core/tests/trigger_failure.rs`).
3. **Alinhamento de CLI y cliente**
   - Garantía de que las operaciones de CLI (`audit`, `gov`, `sumeragi`, `ivm`) utilizan funciones compartidas del cliente `iroha` para evitar derivas.
   - Pruebas de instantáneas JSON en CLI viven en `tests/cli/json_snapshot.rs`; mantenha-os atualizados para que a sayda dos comandos continúen alinhada a referencia JSON canonica.### Fase E - Endurecimiento del códec Norito
1. **Registro de esquemas**
   - Cree un registro de esquema Norito en `crates/norito/src/schema/` para fornecer codificaciones canónicas para tipos core.
   - Adicionados doc tests verificando a codificacao de payloads de exemplo (`norito::schema::SamplePayload`).
2. **Actualizar los accesorios dorados**
   - Actualizar los accesorios dorados en `crates/norito/tests/*` para coincidir con el nuevo esquema WSV cuando se refactoriza.
   - `scripts/norito_regen.sh` regenera el JSON dorado Norito de forma determinística a través del asistente `norito_regen_goldens`.
3. **Integracao IVM/Norito**
   - Validar la serialización de manifiestos Kotodama de extremo a extremo a través de Norito, garantizando que un puntero de metadatos ABI sea consistente.
   - `crates/ivm/tests/manifest_roundtrip.rs` mantiene la paridade Norito codifica/decodifica para manifiestos.## 3. Preocupaciones transversales
- **Estrategia de pruebas**: Cada fase promueve pruebas unitarias -> pruebas de caja -> pruebas de integración. Pruebas que falham capturam regresan atuais; Las nuevas pruebas impiden que regresen.
- **Documentacao**: Después de cada fase, actualice `status.md` y levar itens em aberto para `roadmap.md` mientras elimina tarefas concluidas.
- **Benchmarks de rendimiento**: Bancos Manter existentes en `iroha_core`, `ivm` e `norito`; Agregar médicos base pos-refactor para validar que nao ha regresado.
- **Indicadores de funciones**: Manter alterna el nivel de caja solo para backends que exigen cadenas de herramientas externas (`cuda`, `zk-verify-batch`). Paths SIMD de CPU siempre construidos y seleccionados en tiempo de ejecución; fornecer fallbacks escalares deterministas para hardware nao suportado.## 4. Aces inmediatas
- Scaffolding da Fase A (rasgo instantáneo + cableado de telemetría) - ver tarefas acionaveis nas atualizacoes do roadmap.
- A auditoria recientes de defeitos para `sumeragi`, `state` e `ivm` revelan los siguientes destaques:
  - `sumeragi`: asignaciones de protección de código muerto o transmisión de pruebas de cambio de vista, estado de reproducción VRF y exportación de telemetría EMA. Eles permanecem gated ate que a simplificacao do fluxo de consenso da Fase C e os entregaveis de integracao lane/proof sejam entregues.
  - `state`: a limpeza de `Cell` y o roteamento de telemetria entram na trilha de telemetria WSV da Fase A, mientras que notas de SoA/parallel-apply entram no backlog de otimizacao de pipeline da Fase C.
  - `ivm`: exposición de alternar CUDA, validación de sobres y cobertura Halo2/Metal mapeiam para el trabajo de host-boundary de la Fase D más el tema transversal de aceleración de GPU; Los kernels permanecen sin atrasos dedicados a la GPU y estarán prontos.
- Preparar un equipo cruzado de RFC resumindo este plano para firmar antes de aplicar mudancas invasivas de código.## 5. Questoes em aberto
- ¿RBC debe permanecer opcional además de P1, o debe tener los carriles del libro mayor Nexus? Solicitar decisión a las partes interesadas.
- Impomos grupos de composabilidade DS em P1 ou os mantemos desativados ate que as laneproofs amadurecam?
- ¿Cuál es el canónico local para los parámetros ML-DSA-87? Candidato: novo crate `crates/fastpq_isi` (criacao pendente).

---

_Última actualización: 2025-09-12_