---
lang: es
direction: ltr
source: docs/portal/docs/nexus/nexus-spec.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identificación: especificación-nexus
título: Específicacao tecnica da Sora Nexus
descripción: Espelho completo de `docs/source/nexus.md`, cobrindo a arquitetura e as restricoes de design para o ledger Iroha 3 (Sora Nexus).
---

:::nota Fuente canónica
Esta página espelha `docs/source/nexus.md`. Mantenha ambas as copias alinhadas ate que o backlog de traducao chegue ao portal.
:::

#! Iroha 3 - Sora Nexus Ledger: Técnica específica de diseño

Este documento propone la arquitectura de Sora Nexus Ledger para Iroha 3, evolucionando o Iroha 2 para un ledger global único y lógicamente unificado organizado en torno de Data Spaces (DS). Espacios de datos paranecem dominios fortes de privacidade ("espacios de datos privados") e participacao aberta ("espacios de datos públicos"). El diseño preserva la composabilidad a lo largo del libro mayor global en cuanto a garantía de aislamiento estrito y confidencialidad para datos de Private-DS, e introduce una escalada de disponibilidad de datos a través de codificación de apagado en Kura (almacenamiento en bloque) y WSV (World State View).El mismo repositorio compila tanto Iroha 2 (redes autohospedadas) como Iroha 3 (SORA Nexus). La ejecución e impulsión de la máquina virtual Iroha (IVM) comparte la cadena de herramientas Kotodama, de modo que los contratos y artefactos de código de bytes permanecen portátiles entre implementaciones alojadas automáticamente y el libro mayor global de Nexus.

Objetivos
- Un libro mayor lógico global compuesto por muchos validadores cooperantes y espacios de datos.
- Espacios de datos privados para operación autorizada (por ejemplo, CBDC), con datos que nunca se conocen en DS privado.
- Espacios de datos públicos con participación abierta, acceso sin permiso estilo Ethereum.
- Contratos inteligentes composaveis entre Data Spaces, sujetos a permisos explícitos para acesso a ativos de private-DS.
- Aislamiento de desempenho para que atividade publica no degrade las transacciones internas de private-DS.
- Disponibilidade de dados em escala: Kura e WSV com codificacao de apagamento para soportar datos efetivamente ilimitados manteniendo datos de private-DS privados.

Nao objetivos (fase inicial)
- Definir economía de token o incentivos de validadores; políticas de programación y participación son conectables.
- Introducir una nueva versión de ABI; Mudancas visam ABI v1 con extensos explícitos de syscalls y pointer-ABI conforme a la política de IVM.Terminología
- Nexus Libro mayor: El libro mayor lógico global formado con bloques de Data Space (DS) en una historia ordenada única y un compromiso de estado.
- Espacio de datos (DS): Dominio delimitado de ejecución y armazenamento con sus propios validadores, gobernanza, clase de privacidade, política de DA, cuotas y política de impuestos. Existen dos clases: DS pública y DS privada.
- Espacio de datos privado: Validadores permisos y control de acceso; dados de transacao e estado nunca saem do DS. Apenas compromissos/metadados sao ancorados globalmente.
- Espacio Público de Datos: Participacao sem permissao; dados completos e estado sao publicos.
- Manifiesto de espacio de datos (Manifiesto DS): Manifiesto codificado en Norito que declara parámetros DS (validadores/chaves QC, clase de privacidade, política ISI, parámetros DA, retencao, cuotas, política ZK, taxas). O hash do manifest e ancorado na cadeia nexus. Anulación de salvo, certificados de quórum DS usan ML-DSA-87 (clase Dilithium5) como esquema de assinatura post-quantum por padrao.
- Directorio espacial: Contrato de diretorio global on-chain que rastreia manifests DS, versos e eventos degobernanca/rotacao para resolucao e auditorias.
- DSID: Identificador global único para un espacio de datos. Usado para espacios de nombres de todos los objetos y referencias.- Anchor: Compromisso criptografico de um bloco/header DS incluido na cadeia nexus para vincular la historia de DS al ledger global.
- Kura: Armazenamento de blocos Iroha. Estendido aquí com armazenamento de blobs codificados com apagamento e compromissos.
- WSV: Iroha Vista del estado mundial. Estendido aquí com segmentos de estado versionados, com instantáneas, y codificados com apagamento.
- IVM: Iroha Máquina virtual para ejecutar contratos inteligentes (código de bytes Kotodama `.to`).
  - AIRE: Representación Algebraica Intermedia. Visao algebrica de computacao para provas estilo STARK, descrevendo a execucao como tracos basados ​​en campos con restricciones de transicao e fronteira.Modelo de Espacios de Datos
- Identidad: `DataSpaceId (DSID)` identifica un DS y faz espacio de nombres de todo. DS puede ser instanciado en dos granularidades:
  - Dominio-DS: `ds::domain::<domain_name>` - execucao e estado delimitados a um dominio.
  - Asset-DS: `ds::asset::<domain_name>::<asset_name>` - execucao e estado delimitados a uma definicao de ativo unica.
  Ambas as formas coexisten; Transacoes podem tocar múltiples DSID de forma atómica.
- Ciclo de vida do manifest: criacao de DS, atualizacoes (rotacao de chaves, mudancas de politica) e aposentadoria sao registradas no Space Directory. Cada artefato DS por ranura referencia o hash se manifiesta más recientemente.
- Clases: DS Pública (participacao aberta, DA publica) y DS Privada (permissionado, DA confidencial). Las políticas hibridas sao possiveis a través de banderas se manifiestan.
- Políticas por DS: permisos ISI, parámetros DA `(k,m)`, criptografía, retencao, cuotas (participacao min/max de tx por bloco), política de prueba ZK/otimista, taxas.
- Gobernanza: membresía DS y rotación de validadores definida por la secao de gobernanza manifiesta (propostas on-chain, multisig o gobernanza externa ancorada por transacoes nexus e atestacoes).Manifiestos de capacidades y UAID
- Cuentas universales: cada participante recibe un determinístico UAID (`UniversalAccountId` en `crates/iroha_data_model/src/nexus/manifest.rs`) que abarca todos los espacios de datos. Manifiestos de capacidades (`AssetPermissionManifest`) vinculan um UAID a un espacio de datos específico, épocas de activación/expiracao y una lista ordenada de registros permitir/denegar `ManifestEntry` que delimitam `dataspace`, `program_id`, `method`, `asset` y roles AMX opcionales. Regras niega semper ganham; El evaluador emite `ManifestVerdict::Denied` con una razao de auditoria o un otorgamiento `Allowed` con metadatos de asignación correspondiente.
- Asignaciones: cada entrada permite cargos deterministas `AllowanceWindow` (`PerSlot`, `PerMinute`, `PerDay`) más un `max_amount` opcional. Los hosts y los SDK contienen la misma carga útil Norito, por lo que la aplicación permanece idéntica entre el hardware y el SDK.
- Telemetria de auditoria: Space Directory transmite `SpaceDirectoryEvent::{ManifestActivated, ManifestExpired, ManifestRevoked}` (`crates/iroha_data_model/src/events/data/space_directory.rs`) siempre que un manifiesto cambio de estado. Una nueva superficie `SpaceDirectoryEventFilter` permite que assinantes Torii/data-event monitorem actualizacoes de manifest UAID, revogacoes y decisoes deny-wins sem plumbing personalizado.Para evidencia operativa de extremo a extremo, notas de migración de SDK y listas de verificación de publicación de manifiesto, espelhe esto seco con una Guía de cuenta universal (`docs/source/universal_accounts_guide.md`). Mantenha ambos os documentos alinhados sempre que a politica ou as ferramentas UAID mudarem.

Arquitectura de alto nivel
1) Camada de composición global (Cadena Nexus)
- Mantenga una orden única canónica de bloques Nexus de 1 segundo que finalice las transacciones atómicas abriendo uno o más espacios de datos (DS). Cada transacción comprometida actualiza el estado mundial unificado (vetor de raíces por DS).
- Contiene metadados mínimos más pruebas/QC agregados para garantizar composabilidad, finalidad y detección de fraude (DSID tocados, raíces de estado por DS antes/depois, compromisos DA, pruebas de validación por DS, y el certificado de quorum DS usando ML-DSA-87). Nenhum dado privado e incluido.
- Consenso: comité BFT global en pipeline de tamaño 22 (3f+1 con f=7), seleccionado de un pool de ate ~200k validadores potenciais por un mecanismo de VRF/stake por época. El comité nexus sequencia transacoes y finaliza el bloque en 1s.2) Camada de Espacio de Datos (Público/Privado)
- Ejecuta fragmentos por DS de transacciones globales, actualiza WSV local do DS y produce artefactos de validación por bloque (provas por DS agregadas y compromisos DA) que se acumulan en bloque Nexus de 1 segundo.
- Criptógrafo DS privado dados en repositorio y en tránsito entre validadores autorizados; apenas compromisos y pruebas de validación PQ saem do DS.
- Public DS exportam corpos completos de dados (vía DA) y pruebas de validación PQ.3) Transacoes atómicas cross-Data-Space (AMX)
- Modelo: cada transacción de usuario puede tocar múltiples DS (ej., dominio DS y um o más activo DS). Ela e comprometida atómicamente en un único bloque Nexus ou aborta; nao ha efeitos parciais.
- Prepare-Commit dentro de 1s: para cada transacción candidata, DS tocados se ejecuta en paralelo contra el mesmo snapshot (roots DS no inicio do slot) y produce pruebas de validación PQ por DS (FASTPQ-ISI) y compromisos DA. O comité nexus commita a transacao apenas se todas as provas DS exigidas verificarem e os certificados DA chegarem a tempo (alvo <=300 ms); caso contrario, a transacao e reprogramada para el próximo slot.
- Consistencia: conjuntos de leitura/escrita sao declarados; Detectao de conflictos ocorre no commit contraroots do inicio do slot. Execucao otimista sin bloqueos por DS evita puestos globales; atomicidade e imposta pela regra de commit nexus (todo o nada entre DS).
- Privacidade: Private DS exportam apenas provas/compromissos vinculados aos root DS pre/post. Nenhum dado privado cru sai do DS.4) Disponibilidade de dados (DA) con codificación de apagado
- Kura armazena corpos de blocos e snapshots WSV como blobs codificados con apagamento. Blobs publicos sao ampliamente fragmentados; blobs privados sao armazenados apenas en validadores privados-DS, con trozos criptografados.
- Compromissos DA sao registrados tanto em artefatos DS quanto em blocos Nexus, possibilitando amostragem y garantias de recuperacao sem revelar conteudo privado.

Estructura de bloque y compromiso.
- Artefato de prueba de Data Space (por slot de 1s, por DS)
  - Campos: dsid, slot, pre_state_root, post_state_root, ds_tx_set_hash, kura_da_commitment, wsv_da_commitment, manifest_hash, ds_qc (ML-DSA-87), ds_validity_proof (FASTPQ-ISI).
  - Private-DS exporta artefatos sem corpos de dados; El DS público permite la recuperación de cuerpos a través de DA.

- Nexus Bloque (cadencia de 1s)
  - Campos: block_number, parent_hash, slot_time, tx_list (transacoes atomicas cross-DS com DSIDs tocados), ds_artifacts[], nexus_qc.
  - Funcao: finaliza todas as transacoes atomicas cujos artefatos DS requeridos verificam; atualiza o vetor de root DS do world state global em um passo.Consenso y programación
- Consenso de Nexus Chain: BFT global em pipeline (classe Sumeragi) con comité de 22 nos (3f+1 com f=7) mirando bloques de 1s y propósito de 1s. Los miembros del comité son seleccionados por épocas a través de VRF/stake entre ~200k candidatos; a rotacao mantem descentralizacao e resistencia a censura.
- Consenso de Data Space: cada DS ejecuta su propio BFT entre validadores para producir artefactos por ranura (pruebas, compromisos DA, DS QC). Los comités lane-relay están dimensionados en `3f+1` usando `fault_tolerance` en el espacio de datos y demostrados de forma determinística por época a partir del grupo de validadores del espacio de datos usando una semilla VRF ligada a `(dataspace_id, lane_id)`. DS privado sao autorizados; public DS permitem liveness aberta sujeita a politicas anti-Sybil. El comité global nexus permanece inalterado.
- Programación de transacciones: usuarios submetem transacoes atomicas declarando DSIDs tocados y conjuntos de lectura/escrita. DS ejecuta en paralelo dentro de la ranura; o comite nexus inclui a transacao no bloco de 1s se todos os artefatos DS verificarem e os certificados DA forem pontuais (<=300 ms).
- Aislamiento de desempenho: cada DS tem mempools e ejecucao independentes. Las cuotas de DS limitan la cantidad de transacciones que tocan un DS y pueden comprometerse por bloque para evitar el bloqueo de cabecera y proteger la latencia de DS privado.Modelo de dados y espacio de nombres
- ID calificados por DS: todas las entidades (dominios, contactos, activos, roles) sao calificados por `dsid`. Ejemplo: `ds::<domain>::account`, `ds::<domain>::asset#precision`.
- Referencias globales: una referencia global y una tupla `(dsid, object_id, version_hint)` y pueden colocarse en la cadena en la camada nexus o en los descripciones AMX para uso cross-DS.
- Serialización Norito: todos los mensajes cross-DS (descritores AMX, provas) usan codecs Norito. Sem uso de serde em caminhos de producao.Contratos inteligentes e extensos IVM
- Contexto de ejecución: agregar `dsid` al contexto de ejecución IVM. Los contratos Kotodama siempre se ejecutan dentro de un espacio de datos específico.
- Primitivas atómicas cross-DS:
  - `amx_begin()` / `amx_commit()` delimita una transacción atómica multi-DS sin host IVM.
  - `amx_touch(dsid, key)` declara intención de lectura/escrita para detectar conflictos contra root snapshot do slot.
  - `verify_space_proof(dsid, proof, statement)` -> booleano
  - `use_asset_handle(handle, op, amount)` -> resultado (operacao permitida apenas se a politica permitir e o handle for valido)
- Manejadores de activos y impuestos:
  - Operacoes de ativos sao autorizadas pelas politicas ISI/role do DS; taxas sao pagas no token de gas do DS. Los tokens de capacidad opcionales y políticos más ricos (aprobadores múltiples, límites de tasas, geofencing) pueden ser agregados después de cambiar el modelo atómico.
- Determinismo: todas las nuevas syscalls sao puras y determinísticas dadas como entradas y os conjuntos de leitura/escrita AMX declarados. Sem efectos ocultos de tempo o ambiente.Pruebas de validación post-cuántica (ISI generalizados)
- FASTPQ-ISI (PQ, configuración segura): un argumento basado en hash que generaliza el diseño de transferencia para todas las familias ISI mientras mira prueba sub-segundo para muchos en escala 20k en hardware de clase GPU.
  - Perfil operativo:
    - Nos de producao constroem o prover via `fastpq_prover::Prover::canonical`, que ahora siempre inicializa o backend de producao; o simulado determinista foi removido. [crates/fastpq_prover/src/proof.rs:126]
    - `zk.fastpq.execution_mode` (config) e `irohad --fastpq-execution-mode` permiten que los operadores arreglen la ejecución de CPU/GPU de forma determinística en cuanto al observador gancho registra triples solicitadas/resolvidas/backend para auditorías de fricción. [crates/iroha_config/src/parameters/user.rs:1357] [crates/irohad/src/main.rs:270] [crates/irohad/src/main.rs:2192] [crates/iroha_telemetry/src/metrics.rs:8887]
- Aritmetizacao:
  - KV-Update AIR: trata WSV como un mapa clave-valor tipado comprometido a través de Poseidon2-SMT. Cada ISI se expande para un pequeño conjunto de líneas de lectura-verificación-escritura sobre cuentas (contas, activos, roles, dominios, metadatos, suministro).
  - Restricciones con puertas de código de operación: una tabla única AIR con columnas selectoras impuestas por ISI (conservación, contadores monotónicos, permisos, comprobaciones de rango, actualizaciones de metadatos limitados).- Argumentos de búsqueda: tablas transparentes comprometidas por hash para permisos/roles, precisao de ativos y parámetros de política evitam restricciones bit a bit pesadas.
- Compromisos y actualizaciones de estado:
  - Prova SMT agregada: todas las chaves tocadas (pre/post) sao provadas contra `old_root`/`new_root` usando um frontier comprimido com siblings deduplicados.
  - Invariantes: invariantes global (ej., suministro total por activo) sao impostas via igualdade de multiconjuntos entre líneas de efectivo y contadores rastreados.
- Sistema de prueba:
  - Compromissos polinomiais estilo FRI (DEEP-FRI) con alta aridade (8/16) e Blow-up 8-16; hashes Poseidon2; transcripción Fiat-Shamir con SHA-2/3.
  - Recursao opcional: agregacao recursiva local DS para comprimir microlotes en una prueba por ranura si es necesario.
- Escopo y ejemplos cubiertos:
  - Objetivos: transferir, acuñar, grabar, registrar/anular el registro de definiciones de activos, establecer precisión (limitado), establecer metadatos.
  - Contas/Dominios: crear/eliminar, establecer clave/umbral, agregar/eliminar firmantes (apenas estado; verificacoes de assinatura sao atestadas por validadores DS, nao provadas dentro de AIR).
  - Roles/Permisos (ISI): otorgar/revocar roles y permisos; impostas por tablas de búsqueda y cheques de política monótona.- Contratos/AMX: marcadores comenzar/commitir AMX, capacidad mint/revoke se habilita; provados como transicos de estado e contadores de politica.
- Comprobaciones fora do AIR para preservar la latencia:
  - Assinaturas e criptografia pesada (ej., assinaturas ML-DSA de usuario) sao verificadas por validadores DS e atestadas no DS QC; a prueba de validación de cobre apenas consistencia de estado e conformidade de política. Isso mantem provas PQ e rapidas.
- Metas de desarrollo (ilustrativas, CPU 32 núcleos + una GPU moderna):
  - 20k ISI mistos con toque de tecla pequeño (<=8 chaves/ISI): ~0.4-0.9 s de prueba, ~150-450 KB de prueba, ~5-15 ms de verificación.
  - ISIs mais pesadas (mais chaves/contrastes ricos): micro-lote (ej., 10x2k) + recursao para manter <1 s por slot.
- Configuración del manifiesto DS:
  - `zk.policy = "fastpq_isi"`
  - `zk.hash = "poseidon2"`, `zk.fri = { blowup: 8|16, arity: 8|16 }`
  - `state.commitment = "smt_poseidon2"`
  - `zk.recursion = { none | local }`
  - `attestation.signatures_in_proof = false` (asinaturas verificadas por DS QC)
  - `attestation.qc_signature = "ml_dsa_87"` (padrao; alternativas deben ser declaradas explícitamente)
- Opciones alternativas:
  - ISIs complexas/personalizadas podem usar um STARK general (`zk.policy = "stark_fri_general"`) com prova adiada e finalidade de 1 s via atestacao QC + slashing em provas invalidas.
  - Las opciones nao PQ (por ejemplo, Plonk con KZG) exigen una configuración confiable y son más compatibles sin padrao de compilación.Introducción de AIR (para Nexus)
- Traco de ejecución: matriz con larga (colunas de registros) y comprimento (passos). Cada línea y un paso lógico del proceso ISI; columnas armazenam valores pre/post, selectores e flags.
- Restricciones:
  - Restricoes de transicao: impoem relacoes de linha a linha (ej., post_balance = pre_balance - importe para uma línea de débito quando `sel_transfer = 1`).
  - Restricos de fronteira: ligam I/O publico (old_root/new_root, contadores) as primeiras/ultimas linhas.
  - Búsquedas/permutaciones: membresía de garantía e igualdade de multiconjuntos contra tablas comprometidas (permisos, parámetros de activos) sem circuitos pesados ​​de bits.
- Compromiso y verificación:
  - O prover compromete traces via codificacoes hash e constroi polinomios de baixo grau que sao validos se as restricoes forem satisfeitas.
  - O verificador checa baixo grau vía FRI (basado en hash, post-cuántico) con pocas aberturas Merkle; o custo e logaritmico nos passos.
- Ejemplo (Transferencia): registros que incluyen pre_balance, importe, post_balance, nonce y selectores. Restricoes impem nao negatividade/intervalo, conservacao e monotonicidade de nonce, enquanto uma multiprova SMT agregada liga folhas pre/post a root old/new.Evolución de ABI y syscalls (ABI v1)
- Syscalls para agregar (nombres ilustrativos):
  - `SYS_AMX_BEGIN`, `SYS_AMX_TOUCH`, `SYS_AMX_COMMIT`, `SYS_VERIFY_SPACE_PROOF`, `SYS_USE_ASSET_HANDLE`.
- Tipos de puntero-ABI para agregar:
  - `PointerType::DataSpaceId`, `PointerType::AmxDescriptor`, `PointerType::AssetHandle`, `PointerType::ProofBlob`.
- Actualizaciones necesarias:
  - Agregar a `ivm::syscalls::abi_syscall_list()` (manter ordenacao), gate por politica.
  - Mapear numeros desconocidos para `VMError::UnknownSyscall` nos hosts.
  - Actualizar pruebas: lista de llamadas al sistema golden, hash ABI, ID de tipo de puntero goldens y pruebas de políticas.
  - Documentos: `crates/ivm/docs/syscalls.md`, `status.md`, `roadmap.md`.

Modelo de privacidad
- Contencao de dados privados: corpos de transacao, diffs de estado e snapshots WSV de private DS nunca deixam o subconjunto privado de validadores.
- Exposicao publica: apenas encabezados, compromisos DA y pruebas de validación PQ sao exportados.
- Provas ZK opcionais: DS privado podem produzir provas ZK (ej., saldo suficiente, politica satisfeita) habilitando acoes cross-DS sem revelar estado interno.
- Control de acceso: autorizacao e imposta por politicas ISI/role dentro de DS. Los tokens de capacidad son opcionales y pueden introducirse después.Aislamiento de desempleo y QoS
- Consenso, mempools y armazenamento separados por DS.
- Cuotas de programación de nexus por DS para limitar el tiempo de inclusión de anclajes y evitar el bloqueo de cabeceras.
- Orcamentos de recursos de contrato por DS (compute/memory/IO), impostos pelo host IVM. Contencao em public DS nao pode consumir orcamentos de DS privado.
- Chamadas cross-DS assincronas evitam esperas sincronas longas dentro de la ejecución privada-DS.

Disponibilidad de dados y diseño de armazenamento.
1) Codificación de apagado
- Usar Reed-Solomon sistematico (ej., GF(2^16)) para codificación de apagado en nivel de blob de blocos Kura e snapshots WSV: parámetros `(k, m)` con `n = k + m` shards.
- Parámetros padrao (propostos, public DS): `k=32, m=16` (n=48), permitindo recuperacao de ate 16 shards perdidos com ~1.5x de expansao. Para DS privado: `k=16, m=8` (n=24) dentro del conjunto autorizado. Ambos configurados por DS Manifest.
- Blobs públicos: fragmentos distribuidos por muchos datos/validadores con comprobaciones de disponibilidad por amostragem. Compromissos DA nos headers permite la verificación por clientes ligeros.
- Blobs privados: fragmentos criptografados y distribuidos apenas entre validadores privados-DS (o custodios designados). A cadeia global carrega apenas compromissos DA (sem localizacoes de shards ou chaves).2) Compromisos y amostragem
- Para cada blob: calcular una raíz Merkle sobre fragmentos e incluirla en `*_da_commitment`. Manter PQ evitando compromisos de curva elíptica.
- DA Attesters: los attesters regionales demostrados por VRF (ej., 64 por región) emiten un certificado ML-DSA-87 atestando amostragem bem sucedida de shards. Meta de latencia de atestacao DA <=300 ms. El comité nexus valida certificados em vez de buscar fragmentos.

3) Integraçao com Kura
- Blocos armazenam corpos de transacao como blobs codificados com apagamento e compromissos Merkle.
- Los encabezados cargan compromisos de blob; corpos sao recuperaveis via rede DA para public DS y via canais privados para private DS.

4) Integracao con WSV
- Instantáneas WSV: controla periódicamente el estado de DS en instantáneas fragmentadas y codificados con apagados con compromisos registrados en encabezados. Entre instantáneas, registros de cambios de mantem. Instantáneas públicas sao ampliamente fragmentadas; Las instantáneas privadas permanecen dentro de validadores privados.
- Acesso com prova: contratos podem fornecer (ou solicitar) provas de estado (Merkle/Verkle) ancoradas por compromissos de snapshot. Private DS podem fornecer atestacoes zero-knowledge em vez de pruebas cruas.5) Retencao y poda
- Sem poda para DS público: reter todos los cuerpos Kura e instantáneas WSV a través de DA (escalabilidad horizontal). Private DS puede definir retencao interna, pero los compromisos exportados permanecen imutaveis. A camada nexus retem todos os blocos Nexus e compromissos de artefatos DS.

Rede e papies de nos
- Validadores globales: participam do consenso nexus, validam blocos Nexus e artefatos DS, realizam checagens DA para public DS.
- Validadores de Data Space: ejecutan consenso DS, ejecutan contratos, gerenciam Kura/WSV local, lidam com DA para su DS.
- Nos DA (opcional): armazenam/publicam blobs publicos, facilitam amostragem. Para DS privado, nos DA sao co-localizados com validadores ou custodios confiaveis.Melhorias e consideraciones de sistema
- Desacoplamento sequencing/mempool: agregar un mempool DAG (ej., estilo Narwhal) alimentando un BFT en una tubería en la camada nexus para reducir la latencia y mejorar el rendimiento sin cambiar el modelo lógico.
- Cuotas DS e fairness: cuotas por DS por bloco e caps de peso para evitar el bloqueo de cabecera y garantizar latencia previsivel para DS privado.
- Atestacao DS (PQ): certificados de quórum DS usados ​​ML-DSA-87 (clase Dilithium5) por padrao. Y post-quantum y mayor que assinaturas EC, mas aceitavel com um QC por slot. DS podem optar explícitamente por ML-DSA-65/44 (menores) ou assinaturas EC se declara no DS Manifest; public DS sao fortemente encorajados a manter ML-DSA-87.
- DA attesters: para DS público, usar attesters regionales demostrados por VRF que emiten certificados DA. El comité nexus valida certificados em vez de amostragem bruta de shards; privado DS mantem atestacoes DA internas.
- Recursao e provas por epoca: opcionalmente agregue varios microlotes dentro de un DS en una prueba recursiva por slot/epoca para manter tamanho de prova e tempo de verificacao estaveis sob alta carga.
- Escalado de carriles (se necesita): se um unico comite global virar gargalo, introduzir K carriles de secuenciamento paralelos con fusión determinista. Isso preserva uma unica ordem global enquanto escala horizontalmente.- Aceleración determinística: fornecer kernels SIMD/CUDA con indicadores de características para hash/FFT con respaldo de CPU bit-exato para preservar el determinismo entre hardware.
- Limiares de ativacao de lanes (proposta): habilitar 2-4 lanes se (a) a finalidade p95 exceder 1.2 s por >3 minutos consecutivos, ou (b) a ocupacao por bloco exceder 85% por >5 minutos, ou (c) a taxa de entrada de tx exigiria >1.2x a capacidade do bloco em niveis sustentados. Los carriles forman cubos de transacciones de forma determinística por hash de DSID y se combinan sin bloque nexus.

Taxas y economía (padroes iniciais)
- Unidad de gas: token de gas por DS com Compute/IO medido; taxas sao pagas no ativo de gas nativo do DS. Conversao entre DS y responsabilidad de la aplicación.
- Prioridad de inclusión: round-robin entre DS con cuotas por DS para preservar la equidad y SLO de 1s; Dentro de un DS, la tarifa de licitación se puede desempatar.
- Futuro: mercado global de impuestos o políticas que minimizan MEV podrían ser explorados sin mudar a atomicidade o diseño de pruebas PQ.Fluxo cross-Data-Space (ejemplo)
1) Um usuario envia uma transacao AMX tocando public DS P e private DS S: mover o activo X de S para o beneficiario B cuja conta esta em P.
2) Dentro de la ranura, P y S ejecutan su fragmento contra la instantánea de la ranura. S verifica autorizacao e disponibilidade, actualiza su estado interno y produce una prueba de validación PQ y compromiso DA (nenhum dado privado vaza). P prepara a atualizacao de estado correspondiente (ej., mint/burn/locking em P conforme politica) e sua prova.
3) O comité nexus verifica ambas as provas DS e certificados DA; se ambas verificarem dentro de do slot, a transacao e commited atomicamente no bloco Nexus de 1s, atualizando ambas raíces DS no vetor de world state global.
4) Se qualquer prova ou certificado DA estiver faltando/invalido, a transacao aborta (sem efeitos) y o cliente pode reenviar para o proximo slot. Nenhum dado privado sai de S em nenhum passo.- Consideraciones de seguridad
- Ejecución determinística: syscalls IVM permanecem determinística; Los resultados de Cross-DS están guiados por commit AMX y finalizados, no por reloj ni por sincronización de red.
- Control de acceso: permisos ISI en DS privados restringidos que pueden submeter transacces y quais operacoes sao permitidas. Los tokens de capacidad codifican directamente finos para uso cross-DS.
- Confidencialidade: criptografía de extremo a extremo para datos private-DS, shards codificados com apagamento armazenados apenas entre miembros autorizados, provas ZK opcionales para atestacoes externos.
- Resistencia a DoS: aislamiento en mempool/consenso/armazenamento impide que la congestión pública impacte o progreso de DS privado.

Mudancas nos componentes Iroha
- iroha_data_model: introduzir `DataSpaceId`, ID calificados por DS, descritos AMX (conjuntos leitura/escrita), tipos de prueba/compromissos DA. Serializacao momentáneamente Norito.
- ivm: agregar syscalls y tipos pointer-ABI para AMX (`amx_begin`, `amx_commit`, `amx_touch`) y pruebas DA; actualizar testes/docs ABI conforme politica v1.