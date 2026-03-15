---
lang: es
direction: ltr
source: docs/portal/docs/da/threat-model.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::nota Fuente canónica
España `docs/source/da/threat_model.md`. Mantenha como dos versos em
:::

# Modelo de ameacas de Disponibilidad de datos da Sora Nexus

_Última revisión: 2026-01-19 -- Próxima revisión programada: 2026-04-19_

Cadencia de manutencao: Grupo de Trabajo sobre Disponibilidad de Datos (<=90 días). Cada revisao

Debe aparecer en `status.md` con enlaces para entradas de mitigacao ativos e.
artefatos de simulacao.

## Propósito y Escopo

El programa de disponibilidad de datos (DA) mantem transmissoes Taikai, blobs de lane
Nexus e artefatos degobernanca recuperaveis sob falhas bizantinas, de rede e de
operadores. Este modelo de ameacas ancora o trabajo de ingeniería para DA-1
(arquitetura y modelo de ameacas) e sirven como base para tarefas DA
posteriores (DA-2 a DA-10).

Componentes sin alcance:
- Extensao de ingestao DA do Torii y escritores de metadatos Norito.
- Arvores de armazenamento de blobs soportados por SoraFS (tiers hot/cold) e
  políticas de replicacao.
- Compromisos de blocos Nexus (formatos de cable, pruebas, APIs de nivel de cliente).
- Ganchos de cumplimiento PDP/PoTR específicos para cargas útiles DA.
- Flujos de trabajo de operadores (fijación, desalojo, corte) y tuberías de
  observabilidad.
- Aprovacos de gobierno que admiten o eliminan operadores y cuentan con DA.Foros de recogida de este documento:
- Modelagem economica completa (capturada no workstream DA-7).
- Protocolos base SoraFS ja cobertos pelo modelo de ameacas SoraFS.
- Ergonomía de SDK de cliente además de consideraciones de superficie de ameaca.

## Visao arquitectónico1. **Submissao:** Clientes submetem blobs a través de una API de ingesta DA do Torii. oh
   blobs de división de nodos, manifiestos codificados Norito (tipo de blob, carril, época, banderas
   de codec), y los fragmentos de armazena no están calientes en SoraFS.
2. **Anuncio:** Pin intents e tips de replicacao propagam para provedores de
   almacenamiento mediante registro (mercado SoraFS) con etiquetas de política que indicam
   metas de retencao caliente/frío.
3. **Compromiso:** Sequenciadores Nexus incluyen compromisos de blobs (CID + raíces
   KZG opcional) no bloco canonico. Los niveles de clientes dependen del hash de
   compromiso e da metadatos anunciados para verificar disponibilidad.
4. **Replicacao:** Nodos de armazenamento puxam share/chunks atribuidos, atendem
   desafios PDP/PoTR, e promovem dados entre niveles calientes y fríos conforme política.
5. **Obtener:** Los consumidores buscan datos a través de SoraFS o puertas de enlace compatibles con DA,
   verificando pruebas y emitiendo pedidos de reparación cuando las réplicas desaparecen.
6. **Gobernanza:** Parlamento y comité de supervisión DA aprovam operadores,
   horarios de alquiler y escalacoes de cumplimiento. Artefatos de gobernancia sao
   armazenados pela mesma rota DA para garantizar la transparencia del proceso.

## Activos y responsables

Escala de impacto: **Critico** quebra seguranca/vivacidade do ledger; **Alto**bloqueia backfill DA ou clientes; **Moderado** degradación de la calidad pero permanece
recuperación; **Baixo** efecto limitado.

| activo | Descripción | Integración | Disponibilidad | Confidencialidad | Responsavel |
| --- | --- | --- | --- | --- | --- |
| Blobs DA (fragmentos + manifiestos) | Blobs Taikai, carril y gobierno armados en SoraFS | Crítico | Crítico | Moderado | DA WG / Equipo de almacenamiento |
| Manifiestos Norito DA | Metadatos tipada descrevendo blobs | Crítico | Alto | Moderado | Grupo de Trabajo sobre el Protocolo Básico |
| Compromisos de bloque | CIDs + raíces KZG dentro de blocos Nexus | Crítico | Alto | Bajo | Grupo de Trabajo sobre el Protocolo Básico |
| Horarios PDP/PoTR | Cadencia de cumplimiento para réplicas DA | Alto | Alto | Bajo | Equipo de almacenamiento |
| Registro de operadores | Proveedores de almacenamiento aprobados y políticos | Alto | Alto | Bajo | Consejo de Gobierno |
| Registros de alquiler e incentivos | Libro mayor de entradas para alquiler DA y penalidades | Alto | Moderado | Bajo | Grupo de Trabajo de Tesorería |
| Paneles de observabilidad | SLOs DA, profundidad de replicación, alertas | Moderado | Alto | Bajo | SRE / Observabilidad |
| Intentos de reparación | Pedidos para reidratar trozos ausentes | Moderado | Moderado | Bajo | Equipo de almacenamiento |

## Adversarios y capacidades| Ator | Capacidades | Motivaciones | Notas |
| --- | --- | --- | --- |
| Cliente malicioso | Blobs submétricos malformados, repetición de manifiestos antiguos, tentar DoS sin ingesta. | Interromper transmite Taikai, inyectar datos invalidos. | Sem chaves privilegiadas. |
| Nodo de armazenamento bizantino | Drop de réplicas atribuidas, forjar pruebas PDP/PoTR, coludir. | Reduzir retencao DA, evitar rent, reter dados. | Possui credenciais validas de operador. |
| Secuenciador comprometido | Omitir compromisos, equivocar blocos, reordenar metadatos de blobs. | Ocultar submissao DA, criar inconsistencia. | Limitado pela mayoria de consenso. |
| Operador interno | Abusar acesso degobernanza, manipular politicas de retencao, vazar credenciais. | Ganho económico, sabotaje. | Acesso a infraestrutura frío/calor. |
| Adversario de red | Particionar nodos, atrasar replicacao, inyectar trafego MITM. | Reducir la disponibilidad, degradar los SLO. | No quebra TLS más pode soltar/atrasar enlaces. |
| Atacante de observabilidade | Manipular cuadros de mando/alertas, suprimir incidentes. | Ocultar cortes DA. | Solicite acceso a la tubería de telemetría. |

## Fronteras de confianza- **Fronteira de ingress:** Cliente para extensa DA do Torii. Solicitar autorización por
  solicitud, limitación de velocidad y validación de carga útil.
- **Fronteira de replicacao:** Nodos de almacenamiento de trozos de trocam y pruebas. nodos os
  se autenticam mutuamente mas podem se comportar de forma bizantina.
- **Fronteira do ledger:** Dados de bloco comprometidos vs almacenamiento fuera de la cadena.
  Consenso garante integridade, mas disponibilidad requiere aplicación fuera de la cadena.
- **Fronteira de Gobernanza:** Decisiones Consejo/Parlamento aprovando operadores,
  orcamentos y roza. Falhas aqui impactam directamente o desplegar de DA.
- **Fronteira de observabilidade:** Coleta de metrics/logs exportada para
  paneles/herramientas de alerta. La manipulación esconde cortes o ataques.

## Cenarios de ameaca e controles

### Ataques no camino de ingestao

**Escenario:** Cliente malicioso submete payloads Norito malformados o blobs
superdimensionados para explorar recursos o insertar metadatos no válidos.**Controles**
- Validacao de esquema Norito com negociacao estrita de versao; banderas de rejeitar
  desconhecidos.
- Limitación de la tasa y autenticacao no endpoint de ingestao Torii.
- Limites de tamaño de fragmento y codificación determinística fuerza para el fragmentador SoraFS.
- Pipeline de admissao so persiste manifiesta apos checksum de integridade
  coincidir.
- Replay cache determinista (`ReplayCache`) rastreia janelas `(carril, época,
  secuencia)`, persiste marcas altas en disco, y rejeita duplicados/repeticiones
  obsoletos; arneses de propiedade e fuzz cobrem huellas dactilares divergentes e
  envios fora de ordem. [crates/iroha_core/src/da/replay_cache.rs:1]
  [fuzz/da_replay_cache.rs:1] [crates/iroha_torii/src/da/ingest.rs:1]

**Lacunas residuales**
- Torii debe ingerir o reproducir caché para admitir y persistir cursores de
  secuencia durante reinicios.
- Schemas Norito DA ahora existe un arnés fuzz dedicado
  (`fuzz/da_ingest_schema.rs`) para enfatizar invariantes de codificación/decodificación; sistema operativo
  Los paneles de cobertura deben alertar sobre el objetivo de regredir.

### Retencao por retención de replicación

**Cenario:** Operadores de almacenamiento bizantinos aceitam pins mas dropam chunks,
passando desafios PDP/PoTR via respostas forjadas ou colusao.**Controles**
- El cronograma de desafíos PDP/PoTR se estende a payloads DA com cobertura por
  época.
- Replicación de umbrales de quórum con múltiples fuentes; o orquestador detecta fragmentos
  faltantes y disparantes reparados.
- Slashing degobernanca vinculado a pruebas falhas e réplicas faltantes.
- Trabajo de reconciliación automatizado (`cargo xtask da-commitment-reconcile`) que
  comparar recibos de ingestao com compromisos DA (SignedBlockWire, `.norito` ou
  JSON), emite paquete JSON de evidencia para gobierno, y falha em tickets
  Faltantes o divergentes para que Alertmanager pagine por omisión/manipulación.

**Lacunas residuales**
- O arnés de simulación en `integration_tests/src/da/pdp_potr.rs` (coberto por
  `integration_tests/tests/da/pdp_potr_simulation.rs`) ejercicio colusao e
  particao, validando que o Schedule PDP/PoTR detecta comportamiento bizantino
  de forma determinística. Continuar escuchando junto con DA-5 para cobrir novas
  superficies de prueba.
- Una política de desalojo de nivel frío solicita una pista de auditoría assinada para evitar caídas
  encobertos.

### Manipulación de compromisos

**Cenario:** Secuenciador comprometido publica blocos omitindo ou alterando
compromisos DA, provocando falhas de fetch ou inconsistencias em clientes leves.**Controles**
- Consenso cruza propuestas de bloque con filas de presentación DA; compañeros rejeitam
  propostas sem compromisos requeridos.
- Clientes leves verificam inclusión de pruebas antes de exportar manijas de recuperación.
- Pista de auditoría comparando recibos de presentación con compromisos de bloque.
- Trabajo de reconciliación automatizado (`cargo xtask da-commitment-reconcile`) que
  comparar recibos de ingestao com compromisos DA (SignedBlockWire, `.norito` ou
  JSON), emite paquete JSON de evidencia para gobierno, y falha em tickets
  Faltantes o divergentes para que Alertmanager pagine por omisión/manipulación.

**Lacunas residuales**
- Coberto pelo job de reconciliacao + gancho Alertmanager; pacotes de gobernancia
  Ahora no se incluye el paquete JSON de evidencia por defecto.

### Particao de rede y censura

**Cenario:** Adversario particiona a rede de replicacao, impidiendo nodos de
Obtenga trozos atribuidos o responda a desafíos PDP/PoTR.

**Controles**
- Requisitos de proveedores de garantía multirregional caminhos de rede diversos.
- Janelas de desafio incluyen jitter y fallback para canais de reparo fora de
  banda.
- Paneles de observabilidad monitoram profundidad de replicación, éxito de
  desafíos y latencia de fetch com umbrales de alerta.**Lacunas residuales**
- Simulacros de participación para eventos Taikai live ainda faltam; cosas necesarias
  pruebas de remojo.
- Política de reserva de banda de reparación ainda nao esta codificada.

### Abuso interno

**Cenario:** Operador com acesso ao registra manipula politicas de retencao,
Lista blanca de proveedores maliciosos o alertas superiores.

**Controles**
- Acoes degobernanzarequerem assinaturas multipartidistas e registros Norito
  notarizados.
- Mudancas de política emitem eventos para monitoreo y registros de archivo.
- Canalización de observabilidad de registros de aplicaciones Norito encadenamiento de hash com de solo anexo.
- A automacao de revisao trimestral (`cargo xtask da-privilege-audit`) percorre
  diretorios de manifest/replay (más caminos fornecidos por operadores), marca
  entradas faltantes/nao diretorio/world-writable, y emite paquete JSON assinado
  para paneles de control de gobierno.

**Lacunas residuales**
- Evidencia de manipulación en paneles de control que requieren instantáneas asesinadas.

## Registro de riesgos residuales| Risco | Probabilidades | Impacto | Propietario | Plano de mitigacao |
| --- | --- | --- | --- | --- |
| Reproducción de manifiestos DA antes del caché de secuencia DA-2 | Posible | Moderado | Grupo de Trabajo sobre el Protocolo Básico | Implementar secuencia caché + validación de nonce en DA-2; Agregar testes de regressao. |
| Colusao PDP/PoTR cuando >f nodos sao comprometidos | Mejorar | Alto | Equipo de almacenamiento | Derivar novo cronograma de desafíos con muestreo entre proveedores; validar mediante arnés de simulación. |
| Gap de auditoria de desalojo de nivel frío | Posible | Alto | SRE / Equipo de Almacenamiento | Anexar registra assinados y recibos on-chain para desalojos; monitorizar a través de paneles de control. |
| Latencia de detección de omisión de secuenciador | Posible | Alto | Grupo de Trabajo sobre el Protocolo Básico | `cargo xtask da-commitment-reconcile` ahora compara recibos vs compromisos (SignedBlockWire/`.norito`/JSON) y la página gobierna em tickets faltantes o divergentes. |
| Resiliencia a particao para streams Taikai live | Posible | Crítico | Redes TL | Ejecutar ejercicios de participación; reservar banda de reparación; SOP documental de conmutación por error. |
| Deriva de privilegios de gobernancia | Mejorar | Alto | Consejo de Gobierno | `cargo xtask da-privilege-audit` trimestral (dirs manifest/replay + paths extras) con JSON assinado + puerta de tablero; ancorar artefatos de auditoria on-chain. |

## Seguimientos requeridos1. Publicar esquemas Norito de ingestao DA e vetores de exemplo (carregado em
   DA-2).
2. Encadear o reproducir caché na ingestao DA do Torii y persistir cursores de
   secuencia durante reinicios de nodos.
3. **Concluido (2026-02-05):** O arnés de simulacao PDP/PoTR agora exercita
   colusao + particao com modelagem de backlog QoS; veja
   `integration_tests/src/da/pdp_potr.rs` (com pruebas em
   `integration_tests/tests/da/pdp_potr_simulation.rs`) para implementar y
   resumos deterministas capturados abaixo.
4. **Concluido (2026-05-29):** `cargo xtask da-commitment-reconcile` compara
   recibos de ingestao com compromisos DA (SignedBlockWire/`.norito`/JSON),
   emite `artifacts/da/commitment_reconciliation.json`, y esta ligado a
   Alertmanager/pacotes de gobierno para alertas de omisión/manipulación
   (`xtask/src/da.rs`).
5. **Concluido (2026-05-29):** `cargo xtask da-privilege-audit` percorre o spool
   de manifest/replay (más rutas fornecidos por operadores), marca entradas
   faltantes/nao diretorio/world-writable, y produce el paquete JSON asociado para
   paneles/revisiones de gobierno (`artifacts/da/privilege_audit.json`),
   fechando a laguna de automacao de acesso.

**Onde olhar a seguir:**- La memoria caché de reproducción y la persistencia de cursores aterrissaram em DA-2. veja a
  Implementación en `crates/iroha_core/src/da/replay_cache.rs` (lógica del caché)
  e a integracao Torii em `crates/iroha_torii/src/da/ingest.rs`, que encadeia cheques de
  huella digital a través de `/v2/da/ingest`.
- Como simulacros de streaming PDP/PoTR también ejercitados a través del arnésproof-stream
  en `crates/sorafs_car/tests/sorafs_cli.rs`, cobrindo flujos de requisicao
  PoR/PDP/PoTR e escenarios de falha animados no modelo de ameacas.
- Resultados de capacidad y reparación remojo vivem em
  `docs/source/sorafs/reports/sf2c_capacity_soak.md`, mientras que la matriz de remojo
  Sumeragi más amplia y acompañada en `docs/source/sumeragi_soak_matrix.md`
  (variantes localizadas incluidas). Esses artefatos capturam os drills de longa
  duracao referenciados no registro de riesgos residuales.
- A automacao de reconciliacao + privilegio-auditoría vive em
  `docs/automation/da/README.md` y nuestros comandos
  `cargo xtask da-commitment-reconcile` / `cargo xtask da-privilege-audit`; usar
  as sayas padrao sollozo `artifacts/da/` ao anexar evidencia a pacotes de
  gobernancia.

## Evidencia de simulación y modelado QoS (2026-02)Para fechar el seguimiento DA-1 #3, codificamos un arnés de simulación PDP/PoTR
sollozo determinista `integration_tests/src/da/pdp_potr.rs` (coberto por
`integration_tests/tests/da/pdp_potr_simulation.rs`). O aprovechar los nodos aloca em
tres regioes, injeta particoes/colusao conforme as probabilidades do roadmap,
acompanha lateness PoTR, e alimenta um modelo de backlog de reparo que refleja o
orcamento de reparación do tier hot. Rodar o escenario default (12 épocas, 18 desafíos
PDP + 2 janelas PoTR por época) producidos como siguientes métricas:

<!-- BEGIN_DA_SIM_TABLE -->
<!-- AUTO-GENERATED by scripts/docs/render_da_threat_model_tables.py; do not edit manually. -->
| Métrica | Valor | Notas |
| --- | --- | --- |
| Falhas PDP detectadas | 48 / 49 (98,0%) | Particoes ainda disparam detectado; uma falha nao detectada vem de jitter honesto. |
| Latencia media de detección PDP | 0,0 épocas | Falhas surgem dentro de la época de origen. |
| Falhas PoTR detectadas | 28/77 (36,4%) | Se detecta dispar cuando un nodo perde >=2 janelas PoTR, deixando a maioria dos eventos no registro de riesgos residuales. |
| Latencia media de detección PoTR | 2.0 épocas | Corresponde ao limiar de atraso de dos épocas embutido na escalacao de arquivo. |
| Pico de fila de reparación | 38 manifiestos | El backlog crece cuando las piezas empilham mais rapido que cuatro reparaciones disponibles por época. |
| Latencia de respuesta p95 | 30.068 ms | Refleje una janela de desafío de 30 s con jitter de +/-75 ms aplicado sin QoS de muestreo. |
<!-- END_DA_SIM_TABLE -->Estas salidas ahora alimentan los prototipos de tablero DA y satisfacen los
Criterios de aceitacao de "arnés de simulación + modelado QoS" referenciados
sin hoja de ruta.

A automacao agora vive por tras de
`cargo xtask da-threat-model-report [--out <path|->] [--seed <u64|0xhex>] [--config <path>]`,
que chama o arnés compartido y emite Norito JSON para
`artifacts/da/threat_model_report.json` por defecto. Jobs noturnos consomem este
archivo para actualizar como matrices este documento y alertar sobre deriva em

taxas de detección, filas de reparación o muestras QoS.

Para actualizar una tabla acima para docs, ejecute `make docs-da-threat-model`,
que invoca `cargo xtask da-threat-model-report`, regenera
`docs/source/da/_generated/threat_model_report.json`, y reescreve esta secao vía
`scripts/docs/render_da_threat_model_tables.py`. El espelho `docs/portal`
(`docs/portal/docs/da/threat-model.md`) e actualizado no mesmo passo para que
como dos copias fiquem em sync.