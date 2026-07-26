---
lang: pt
direction: ltr
source: docs/portal/docs/nexus/nexus-spec.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: especificação do nexo
título: Especificação técnica Sora Nexus
description: Polo de operação `docs/source/nexus.md`, projeto de arquitetura e projeto de operação para Iroha 3 (Sora Nexus).
---

:::nota História Canônica
Esta página contém `docs/source/nexus.md`. Selecione uma cópia sincronizada que não seja exibida no portal.
:::

#! Iroha 3 - Sora Nexus Ledger: Especificação técnica

Este documento contém o arquiteto Sora Nexus Ledger para Iroha 3, развивая Iroha 2 em единый Ledger unificado de lógica global, espaço de dados organizacional (DS). Espaços de dados предоставляют сильные домены приватности ("espaços de dados privados") e открытую участие ("espaços de dados públicos"). Дизайн сохраняет composability глобального ledger при строгой изоляции и конфиденциальности данных private-DS e вводит A capacidade de gerenciamento de dados permite que você codifique em Kura (armazenamento em bloco) e WSV (World State View).

Este e este repositório são Iroha 2 (conjunto auto-hospedado) e Iroha 3 (SORA Nexus). Use a máquina virtual Iroha (IVM) e o conjunto de ferramentas Kotodama, usando contratos e артефакты байткода остаются переносимыми между развертываниями и глобальным razão Nexus.

Цели
- Один глобальный логический ledger, составленный из множества кооперативных валидаторов и Espaços de dados.
- Espaços de dados privados para operações autorizadas (por exemplo, CBDC), onde você não pode usar DS privado.
- Espaços de dados públicos são abertos, dados sem permissão no estilo Ethereum.
- Composable смарт-контракты между Data Spaces при явных разрешениях на доступ к private-DS активам.
- Изоляция производительности, чтобы public активность не ухудшала private-DS внутренние транзакции.
- Доступность данных в масштабе: Kura codificado por apagamento e WSV para поддержки практически неограниченных данных при сохранении privado-DS.

Não цели (начальная фаза)
- Validação de tokens ou estímulos válidos; agendamento político e piquetagem должны оставаться подключаемыми.
- Ver nova versão ABI; As opções de ABI v1 são usadas para syscalls e ponteiro-ABI na política IVM.

Terminais
- Nexus Ledger: Глобальный логический ledger, сформированный композицией блоков Data Space (DS) em единую história atualizada e compromisso состояния.
- Espaço de Dados (DS): Ограниченный домен исполнения и хранения со своими валидаторами, governança, classe приватности, DA политикой, квотами e política política. Qual é a sua classe: DS público e DS privado.
- Espaço de dados privado: валидаторы и контроль доступа; A transação e a transação de никогда não serão usadas no DS. Глобально якорятся только compromissos/metadados.
- Espaço de dados públicos: uso sem permissão; полные данные и состояние публичны.
- Manifesto de Espaço de Dados (Manifesto DS): Manifesto codificado em Norito, parâmetros de declaração DS (validadores/chaves QC, classe de privacidade, política ISI, parâmetros DA, retenção, cotas, ZK política, комиссии). Ancoragem do manifesto hash na cadeia do nexo. Se não for necessário, os certificados de quórum DS usam ML-DSA-87 (Dilithium5-класс) como padrão pós-квантовый подпись.
- Diretório de espaço: contrato de diretório on-chain global, manifestos DS de отслеживающий, versão e governança/rotação события для разрешимости и аудитов.
- DSID: Глобально уникальный идентификатор Espaço de Dados. Use o namespace para seus objetos e itens.
- Âncora: Compromisso de criptografia do DS блока/заголовка, включаемый в nexus chain, чтобы связать história DS com глобальным ledger.
- Kura: Bloco de blocos Iroha. Você pode armazenar compromissos e armazenamento de blob codificados por eliminação.
- WSV: Iroha Visão do Estado Mundial. Esta é uma versão com capacidade de captura instantânea e codificação de apagamento.
- IVM: Máquina Virtual Iroha para uso inteligente (bytecode Kotodama `.to`).
  - AIR: Representação Algébrica Intermediária. Алгебраическое представление вычислений para STARK-подобных доказательств, описывающее исполнение как трассы baseado em campo com uma organização excelente e satisfatória.Espaços de dados modelo
- IDENTIFICAÇÃO: `DataSpaceId (DSID)` identifica o DS e o namespace de todos. O DS pode ser instalado em duas configurações:
  - Domínio-DS: `ds::domain::<domain_name>` - исполнение и состояние, ограниченные доменом.
  - Asset-DS: `ds::asset::<domain_name>::<asset_name>` - исполнение и состояние, ограниченные одной дефиницией ativo.
  Suas formas estão corretas; A transação pode ser feita automaticamente sem DSID.
- Жизненный цикл manifesto: создание DS, обновления (ротация ключей, изменения политики) e вывод из эксплуатации Vá para o Diretório do Espaço. O artefacto DS por slot contém um hash de manifesto possível.
- Classes: DS público (открытое участие, публичная DA) e DS privado (permitido, конфиденциальная DA). Гибридные политики возможны через флаги manifesto.
- Política no DS: permissões ISI, parâmetros DA `(k,m)`, шифрование, retenção, квоты (min/max доля tx на блок), ZK/política de prova otimista, комиссии.
- Governança: членство DS e ротация валидаторов определяются секцией manifesto de governança (on-chain предложения, multisig ou внешняя governança, заякоренная nexo de transação e atestados).

Manifestos de capacidade e UAID
- Contas universais: каждый участник получает детерминированный UAID (`UniversalAccountId` em `crates/iroha_data_model/src/nexus/manifest.rs`), который охватывает seus espaços de dados. Manifestos de capacidade (`AssetPermissionManifest`) связывают UAID com espaço de dados configurado, época активации/истечения e упорядоченным списком permitir/negar `ManifestEntry` правил, ограничивающих `dataspace`, `program_id`, `method`, `asset` e funções AMX opcionais. Negar правила всегда побеждают; avaliador возвращает `ManifestVerdict::Denied` com причиной аудита или `Allowed` grant соответствующей metadados de permissão.
- Permissões: каждая permitir запись несет детерминированные `AllowanceWindow` baldes (`PerSlot`, `PerMinute`, `PerDay`) и opcional `max_amount`. Hosts e SDK são compatíveis com a carga útil Norito, que permite execução de execução no ambiente de trabalho e SDK.
- Telemetria de auditoria: Space Directory транслирует `SpaceDirectoryEvent::{ManifestActivated, ManifestExpired, ManifestRevoked}` (`crates/iroha_data_model/src/events/data/space_directory.rs`) при изменении состояния manifesto. Новая поверхность `SpaceDirectoryEventFilter` позволяет Torii/data-event подписчикам отслеживать обновления UAID manifests, ревокации e deny-wins решения без кастомного encanamento.

Para a operação de ponta a ponta, notas de migração do SDK e lista de verificação publicam a sincronização do manifesto этот раздел no Guia de conta universal (`docs/source/universal_accounts_guide.md`). Obtenha um documento em conjunto com políticas públicas ou instrumentos da UAID.

Arquiteto de alta qualidade
1) Cadeia de Composição Global (Cadeia Nexus)
- Поддерживает единый canonical порядок 1-секундных Nexus Blocks, которые финализируют атомарные транзакции, затрагивающие один или более Data Spaces (DS). Каждая commited транзакция обновляет единый глобальный estado mundial (vector per-DS raízes).
- Содержит минимальные метаданные плюс агрегированные provas/QCs para composição, finalidade e detecção de fraude (DSIDs, raízes de estado por-DS до/после, compromissos DA, validade por-DS provas e certificado de quorum DS com ML-DSA-87). A privacidade não é válida.
- Консенсус: единый глобальный pipeline BFT comitê размера 22 (3f+1 с f=7), выбранный na época VRF/stake de mais de ~200k кандидатов. Nexus comitê упорядочивает транзакции и финализирует блок за 1s.

2) Espaço de dados do espaço (público/privado)
- Исполняет per-DS фрагменты глобальных транзакций, обновляет локальный DS WSV e производит artefatos de validade por bloco (provas agregadas por DS e compromissos DA), которые сворачиваются в 1-секундный Bloco Nexus.
- DS privado шифруют данные em repouso e em voo среди авторизованных валидаторов; наружу выходят только compromissos e provas de validade PQ.
- DS público экспортируют полные corpos de dados (via DA) e provas de validade PQ.3) Transação automática entre dados e espaço (AMX)
- Modelo: каждая пользовательская транзакция может касаться нескольких DS (por exemplo, domínio DS e один или несколько ativo DS). Em uma operação automática no bloco Nexus ou bloqueado; частичных эффектов neт.
- Prepare-Commit para 1s: para que o candidato seja transferido para um snapshot do DS (DS root no slot) e você precisa de provas de validade PQ por DS (FASTPQ-ISI) e compromissos DA. Nexus comitê коммитит транзакцию только если все требуемые DS provas проверяются и certificados DA приходят вовремя (цель <=300 ms); иначе транзакция переносится на следующий слот.
- Consistência: leitura-gravação наборы объявляются; O conector detecta o commit das raízes de início do slot. исполнение по DS избегает глобальных stall otimista sem bloqueio; atomicidade обеспечивается правилом nexus commit (tudo ou nada no DS).
- Privacidade: DS privado экспортируют только provas/compromissos, привязанные к pré/pós raízes DS. Никакие сырые private данные не покидают DS.

4) Disponibilidade de dados (DA) com codificação de eliminação
- Kura хранит corpos de bloco e instantâneos WSV como blobs codificados para eliminação. Blobs públicos широко shard-ятся; private blobs são usados ​​como validadores private-DS, com pedaços armazenados.
- Compromissos DA são registrados em artefatos DS, e em blocos Nexus, amostragem de amostragem e garantias de recuperação sem proteção privada.

Estruturar bloco e commit
- Artefato à prova de espaço de dados (no espaço 1s, no DS)
  - Política: dsid, slot, pre_state_root, post_state_root, ds_tx_set_hash, kura_da_commitment, wsv_da_commitment, manifest_hash, ds_qc (ML-DSA-87), ds_validity_proof (FASTPQ-ISI).
  - Artefatos de экспортируют Private-DS em corpos de dados; público DS позволяют получение corpo через DA.

- Bloco Nexus (cadência 1s)
  - Política: block_number, parent_hash, slot_time, tx_list (transferência cross-DS com DSIDs), ds_artifacts[], nexus_qc.
  - Função: finalizar sua transação automática, prover artefatos do DS; обновляет глобальный вектор DS raízes одним шагом.

Consulta e agendamento
- Consenso da cadeia Nexus: единый глобальный BFT com pipeline (classe Sumeragi) com número 22 (3f + 1, f = 7) para blocos 1s e finalidade 1s. Комитет выбирается по épocas через VRF/stake de ~200k кандидатов; A rotação pode ser descentralizada e desativada.
- Consenso de espaço de dados: каждый DS запускает свой BFT para artefatos por slot (provas, compromissos DA, DS QC). Os comitês de retransmissão de pista размером `3f+1` usam o espaço de dados `fault_tolerance` e determinam a configuração da época no espaço de dados validado com semente VRF, fornecido por `(dataspace_id, lane_id)`. DS privado - permitido; DS público - vivacidade aberta com política anti-Sybil. Comitê do Nexo Global неизменен.
- Agendamento de transações: permite realizar transações automáticas com DSIDs e configurações de leitura e gravação. DS é instalado paralelamente no slot; comitê do nexo включает транзакцию в 1s блок, если все artefatos DS provados e certificados DA приходят вовремя (<=300 ms).
- Isolamento de desempenho: o DS usa mempools e uso. As cotas por DS ограничивают число транзакций на блок для данного DS, предотвращая bloqueio head-of-line e zaщищая DS privado de latência.

Modelo de dados e namespace
- IDs qualificados para DS: все сущности (domínios, contas, ativos, funções) квалифицированы `dsid`. Exemplo: `ds::<domain>::account`, `ds::<domain>::asset#precision`.
- Referências Globais: глобальная ссылка - кортеж `(dsid, object_id, version_hint)` e você pode usar descritores on-chain na camada Nexus ou nos descritores AMX para cross-DS.
- Serialização Norito: você usa cross-DS сообщения (descritores AMX, provas) usando codecs Norito. Serde na produção não é preliminar.Contratos inteligentes e segurança IVM
- Contexto de execução: insira `dsid` no contato IVM. Os contratos Kotodama são usados ​​no espaço de dados concreto.
- Primitivos Atômicos Cross-DS:
  - `amx_begin()` / `amx_commit()` ограничивают атомарную multi-DS транзакцию в IVM host.
  - `amx_touch(dsid, key)` объявляет intenção de leitura/gravação para detecção de conflito против snapshot root слота.
  - `verify_space_proof(dsid, proof, statement)` -> bool
  - `use_asset_handle(handle, op, amount)` -> resultado (операция разрешена только при политическом разрешении и валидном handle)
- Tratamento de ativos e taxas:
  - Операции активов авторизуются политиками ISI/função DS; комиссии оплачиваются в gás токене DS. Tokens de capacidade opcionais e políticas mais ricas (aprovadores múltiplos, limites de taxa, delimitação geográfica) podem ser usados ​​para modelos automáticos de implementação.
- Determinismo: todos os novos syscalls são usados ​​e determinados para leitura/gravação AMX. Não há nenhuma tela de efeito durante o período ou a operação.Provas de validade pós-quântica (ISIs generalizadas)
- FASTPQ-ISI (PQ, sem configuração confiável): argumento baseado em hash, transferência de обобщающий дизайн на все ISI, com целью субсекундного provando para lote масштаба 20k na classe GPU железа.
  - Perfil operacional:
    - Provedor de nós de produção строят через `fastpq_prover::Prover::canonical`, который теперь всегда инициализирует back-end de produção; детерминированный mock удален. [crates/fastpq_prover/src/proof.rs:126]
    - `zk.fastpq.execution_mode` (config) e `irohad --fastpq-execution-mode` permitem que a operação determine a configuração da CPU/GPU e a configuração do gancho do observador тройки solicitado/resolvido/backend para auditorias de frota. [crates/iroha_config/src/parameters/user.rs:1357] [crates/irohad/src/main.rs:270] [crates/irohad/src/main.rs:2192] [crates/iroha_telemetry/src/metrics.rs:8887]
- Aritmetização:
  - KV-Update AIR: трактует WSV как типизированную cartão de valor-chave, зафиксированную через Poseidon2-SMT. Каждый ISI разворачивается в небольшой набор leitura-verificação-gravação строк по ключам (contas, ativos, funções, domínios, metadados, fornecimento).
  - Restrições controladas por Opcode: единая AIR таблица com colunas de seletor impostas por ISI (conservação, contadores monotônicos, permissões, verificações de intervalo, atualizações limitadas de metadados).
  - Argumentos de pesquisa: tabelas de hash confirmadas para permissões/funções, precisões de ativos e parâmetros de política que usam a organização bit a bit.
- Compromissos e atualizações do Estado:
  - Prova SMT agregada: все затронутые ключи (pré/pós) доказываются против `old_root`/`new_root` com fronteira de compressão e irmãos desduplicados.
  - Invariantes: глобальные инварианты (por exemplo, oferta total por ativo) impor-ятся через igualdade multiset между linhas de efeito e contadores rastreados.
- Sistema de prova:
  - Compromissos polinomiais estilo FRI (DEEP-FRI) с высокой aridade (8/16) и explosão 8-16; Hashes Poseidon2; Transcrição Fiat-Shamir com SHA-2/3.
  - Recursão opcional: agregação recursiva local DS para armazenar microlotes em uma prova no local de trabalho.
- Escopo e exemplos:
  - Ativos: transferência, cunhagem, gravação, registro/cancelamento de definições de ativos, definição de precisão (limitada), definição de metadados.
  - Contas/Domínios: criar/remover, definir chave/limite, adicionar/remover signatários (apenas estado; проверки подписи аттестуются validadores DS, не доказываются в AIR).
  - Funções/Permissões (ISI): conceder/revogar funções e permissões; tabelas de pesquisa forçadas e verificações de políticas monotônicas.
  - Contratos/AMX: marcadores de início/confirmação AMX, capacidade de cunhar/revogar при включении; доказываются как transições de estado e contadores de políticas.
- Verificações Out-of-AIR para latência de segurança:
  - Подписи и тяжелая криптография (por exemplo, assinaturas de usuário ML-DSA) testar validadores DS e аттестуются в DS QC; prova de validade покрывает только estado de consistência e conformidade com a política. Это оставляет provas PQ и быстрыми.
- Metas de desempenho (exemplo, CPU de 32 núcleos + GPU adicional):
  - 20k ISIs mistos sem toque de tecla (<= 8 teclas/ISI): ~0,4-0,9 s de prova, ~150-450 KB de prova, ~5-15 ms de verificação.
  - Melhor ISI: microlote (por exemplo, 10x2k) + recursão, чтобы держать <1 s на слот.
- Configuração do Manifesto DS:
  -`zk.policy = "fastpq_isi"`
  -`zk.hash = "poseidon2"`, `zk.fri = { blowup: 8|16, arity: 8|16 }`
  -`state.commitment = "smt_poseidon2"`
  -`zk.recursion = { none | local }`
  - `attestation.signatures_in_proof = false` (provavelmente DS QC)
  - `attestation.qc_signature = "ml_dsa_87"` (por умолчанию; альтернативы должны быть явно объявлены)
- Subsídios:
  - Сложные/кастомные ISI могут использовать общий STARK (`zk.policy = "stark_fri_general"`) com prova diferida e 1s finalidade через QC атттестацию + slashing на неверных доказательствах.
  - Variantes não-PQ (por exemplo, Plonk com KZG) usam configuração confiável e não podem ser encontradas no servidor padrão.AIR Primer (para Nexus)
- Rastreamento de execução: матрица с шириной (registro de colunas) e длиной (etapas). Каждая строка - логический шаг ISI; Você tem opções pré/pós, seletor e sinalizadores.
- Restrições:
  - Restrições de transição: impor отношения между строками (por exemplo, post_balance = pre_balance - amount для debit строки при `sel_transfer = 1`).
  - Restrições de limite: привязывают E/S pública (old_root/new_root, contadores) em первой/последней строкам.
  - Pesquisas/permutações: garantem associação e igualdade multiset através de tabelas comprometidas (permissões, parâmetros de ativos) no mesmo esquema.
- Compromisso e verificação:
  - Prover коммитит трассы кодировками и строит polinômios de baixo grau, валидные только при соблюдении ограничений.
  - Verificador проверяет через FRI de baixo grau (baseado em hash, pós-quântico) с несколькими Merkle открытиями; стоимость логарифмична по passos.
- Exemplo (Transferência): регистры включают pre_balance, amount, post_balance, nonce e seletores. Ограничения impor неотрицательность/диапазон, сохранение и монотонность nonce, um agregado SMT multi-prova связывает pré/pós folhas com raízes antigas/novas.

Desenvolvimento de ABI e syscalls (ABI v1)
- Syscalls em uso (imagem):
  -`SYS_AMX_BEGIN`, `SYS_AMX_TOUCH`, `SYS_AMX_COMMIT`, `SYS_VERIFY_SPACE_PROOF`, `SYS_USE_ASSET_HANDLE`.
- Tipos de ponteiro-ABI para uso:
  -`PointerType::DataSpaceId`, `PointerType::AmxDescriptor`, `PointerType::AssetHandle`, `PointerType::ProofBlob`.
- Otimização do trabalho:
  - Insira em `ivm::syscalls::abi_syscall_list()` (por exemplo), portão na política.
  - Mapeie um número novo em `VMError::UnknownSyscall` em hosts.
  - Testes de teste: lista de syscall golden, hash ABI, ID de tipo de ponteiro goldens e testes de política.
  - Documentos: `crates/ivm/docs/syscalls.md`, `status.md`, `roadmap.md`.

Modelo privado
- Contenção de dados privados: тела транзакций, диффы состояния e instantâneos WSV para DS privado никогда не покидают subconjunto de validador privado.
- Exposição pública: cabeçalhos de dados, compromissos DA e provas de validade PQ.
- Provas ZK opcionais: Provas ZK privadas de DS podem ser usadas (например, достаточный баланс, соблюдение политики), позволяя cross-DS действия без раскрытия внутреннего состояния.
- Controle de acesso: política imposta pela autoridade ISI/função внутри DS. Tokens de capacidade são opcionais e podem ser usados.

Proibição isolada e QoS
- Consenso, mempools e armazenamento definidos no DS.
- Nexus cotas de agendamento por DS ограничивают время включения âncoras e предотвращают bloqueio head-of-line.
- Orçamentos de recursos de contrato por DS (computação/memória/IO) impostos pelo host IVM. A contenção de DS público não pode ser substituída por uma disputa de DS privado.
- Асинхронные cross-DS вызовы избегают длинных синхронных ожиданий внутри private-DS исполнения.

Disponibilidade de dados e design de armazenamento
1) Codificação de apagamento
- Use Reed-Solomon sistemático (por exemplo, GF (2 ^ 16)) para codificação de eliminação em nível de blob, blocos Kura e instantâneos WSV: parâmetros `(k, m)` e fragmentos `n = k + m`.
- Parâmetros padrão (DS público): `k=32, m=16` (n=48), disponível para 16 fragmentos de expansão por expansão de ~1,5x. Para DS privado: `k=16, m=8` (n=24) com permissão de acesso. Você pode configurar o Manifesto do DS.
- Blobs públicos: shards usados ​​em muitos nós/validadores DA com verificações de disponibilidade baseadas em amostragem. Compromissos DA em cabeçalhos fornecem clientes leves.
- Blobs privados: shards зашифрованы и распределены только внутри validadores DS privados (ou custodiantes назначенных). A configuração global não requer compromissos DA (não locais de shard ou ключей).

2) Compromissos e amostragem
- Para o blob: insira a raiz Merkle em fragmentos e acesse `*_da_commitment`. Сохранять PQ, избегая compromissos de curva elíptica.
- Atestadores DA: Atestadores regionais amostrados por VRF (por exemplo, 64 por região) usam a certificação ML-DSA-87 de fragmentos de amostragem usuais. Latência de atestado DA alta <=300 ms. Comitê Nexus валидирует сертификаты вместо вытягивания shards.

3) Integração Kura
- Bloqueia corpos de transação como blobs codificados por apagamento e compromissos Merkle.
- Cabeçalhos sem compromissos de blob; corpos извлекаются через DA сеть para DS público e через canais privados para DS privado.4) Integração WSV
- Snapshotting WSV: ponto de verificação de períodos DS состояния em snapshots fragmentados e codificados para eliminação e compromissos em cabeçalhos. Muitos snapshots podem conter logs de alterações. Instantâneos públicos широко shard-ятся; snapshots privados são armazenados em validadores privados.
- Acesso de transporte de prova: контракты могут предоставлять (ou запрашивать) provas de estado (Merkle/Verkle), compromissos de instantâneo ancorados. O DS privado pode usar atestados de conhecimento zero em provas brutas.

5) Retenção e poda
- Nenhuma poda para DS público: хранить все Kura bodys e WSV snapshots через DA (горизонтальное масштабирование). O DS privado pode oferecer retenção de valor, sem compromissos de exportação sem compromissos. A camada Nexus é configurada em vez de Nexus Blocos e compromissos de artefato DS.

Сеть e роли узлов
- Validadores globais: use o consenso do Nexus, valide blocos Nexus e artefatos DS, verifique verificações DA para DS público.
- Validadores de espaço de dados: запускают DS consenso, исполняют контракты, управляют локальным Kura/WSV, обрабатывают DA para seu DS.
- Nós DA (опционально): criar/gerar blobs públicos, coletar amostragem. Para nós privados DS DA co-localizados com validadores ou custodiantes.

Sistema de gerenciamento e solução
- Desacoplamento de sequenciamento/mempool: usar DAG mempool (por exemplo, Narwhal-стиль), usar BFT em pipeline na camada nexus, aumentar a latência e aumentar a taxa de transferência modelo de lógica lógica.
- Cotas DS e justiça: cotas por DS por bloco e limites de peso substituem o bloqueio inicial e обеспечивают предсказуемую latência para DS privado.
- Atestado DS (PQ): Certificados de quórum DS используют ML-DSA-87 (Dilithium5-класс) по умолчанию. Este pós-atendimento e mais, quando solicitado pela EC, não é recomendado para 1 QC por slot. DS pode ser usado para ML-DSA-65/44 (меньше) ou EC подписи, если объявлено в DS Manifest; público DS настоятельно рекомендуется сохранять ML-DSA-87.
- Atestadores DA: para DS público использовать atestadores regionais amostrados por VRF, certificados DA выдающих. Comitê Nexus валидирует сертификаты вместо amostragem de fragmentos brutos; DS privado держат atestados DA внутренними.
- Recursão e provas de época: опционально агрегировать несколько micro-lotes no DS na própria prova recursiva no слот/эпоху para стабильного размера provas и времени verifique при высокой нагрузке.
- Escala de pista (если нужно): если единый глобальный comitê становится узким местом, ввести K параллельных pistas de sequenciamento с mesclagem детерминированным. Este é um conjunto de soluções globais e mastigáveis.
- Aceleração determinística: usar kernels SIMD/CUDA com sinalizadores de recurso para hashing/FFT com fallback de CPU com bit exato, чтобы сохранить детерминизм на разном железе.
- Limiares de ativação de faixa (proposta): включать 2-4 faixas, если (a) p95 finalidade превышает 1,2 s ou 3 minutos подряд, ou (b) ocupação por bloco превышает 85% mais de 5 minutos, ou (c) sua taxa de transferência será menor que 1,2x a capacidade do bloco usada. As faixas determinam a transação do bucket no hash DSID e a mesclagem no bloco Nexus.

Taxas e economia (начальные дефолты)
- Unidade de gás: token de gás por DS medido com computação/IO; taxas оплачиваются в нативном ativo de gás DS. Конверсия между DS - ответственность приложения.
- Prioridade de inclusão: round-robin para DS com cotas por DS para justiça e 1s SLO; внутри DS taxa licitação pode ser разруливать.
- Futuro: mercado de taxas globalizado ou política minimizadora de MEV, atomicidade ou prova de PQ.

Fluxo de trabalho entre espaços de dados (exemplo)
1) Пользователь отправляет AMX транзакцию, затрагивающую DS P público e DS S privado: переместить ativo X из S к beneficiário B, чей аккаунт в P.
2) Внутри слота P e S usa seus fragmentos no snapshot. S prove autorização e disponibilidade, обновляет внутреннее состояние e формирует Prova de validade PQ e compromisso DA (não утечки dados privados). P готовит соответствующее обновление состояния (por exemplo, hortelã/queimadura/travamento em P по политике) e sua prova.
3) Comitê Nexus fornece provas DS e certificados DA; если обе валидны в слоте, транзакция commit-ится атомарно в 1s Nexus Block, обновляя оба DS Roots no глобальном estado mundial vetor.
4) Qualquer prova de certificado ou certificado DA emitido/negativo, transação abortada (sem efeitos), e o cliente pode ser solicitado no slot certo. Никакие private данные S não покидают на любом шаге.- Considerações de segurança
- Execução Determinística: syscalls IVM остаются детерминированными; cross-DS oferece resultados de commit e finalidade do AMX, sem relógio de parede ou com configurações definidas.
- Controle de acesso: permissões ISI em DS privado, o que pode permitir transações e transações operacionais. Os tokens de capacidade codificam um valor refinado para cross-DS.
- Confidencialidade: шифрование end-to-end para private-DS данных, fragmentos codificados para eliminação хранятся только у авторизованных участников, опциональные provas ZK para atestados внешних.
- Resistência DoS: isolamento no mempool/consensus/storage evita o congestionamento público no progresso do DS privado.

Configuração do componente Iroha
- iroha_data_model: добавить `DataSpaceId`, IDs qualificados para DS, descritores AMX (conjuntos de leitura/gravação), tipos de compromissos de prova/DA. Número de série Norito.
- ivm: cria syscalls e tipos de ponteiro-ABI para AMX (`amx_begin`, `amx_commit`, `amx_touch`) e provas DA; обновить Testes/documentos ABI na política v1.