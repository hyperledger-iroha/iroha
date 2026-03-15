---
lang: pt
direction: ltr
source: docs/portal/docs/da/threat-model.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::nota História Canônica
Esta página contém `docs/source/da/threat_model.md`. Держите обе версии
:::

# Модель угроз Disponibilidade de dados Sora Nexus

_Prova de prova: 19/01/2026 — Prova de prova: 19/04/2026_

Частота обслуживания: Grupo de Trabalho de Disponibilidade de Dados (<=90 dias). Каждая
редакция должна появиться в `status.md` para obter bilhetes ativos
simulação e artefatos de simulação.

## Цель e область

Programa Disponibilidade de Dados (DA) обеспечивает доступность Transmissão Taikai,
Nexus blobs de pista e artefatos de governança de bizantinos, сетевых e операционных
sim. Este modelo é usado para construir o trabalho DA-1 (arquitetura e modelo
угроз) и служит базовым ориентиром para дальнейших задач DA (DA-2 .. DA-10).

Componentes na moldura:
- Расширение Torii DA ingerir e escritores metadados Norito.
- Blobs de configuração padrão em SoraFS (camadas quentes/frias) e replicações políticas.
- Compromissos de bloco Nexus (formatos de transmissão, provas, APIs de cliente leve).
- Ganchos fornecidos PDP/PoTR para cargas úteis DA.
- Processos operacionais (fixação, despejo, corte) e pipelines de observabilidade.
- Aprovações de governança para dopуска/исключения DA операторов и контента.

Qual é a documentação:
- Modelo de política econômica (definido no fluxo de trabalho DA-7).
- Use os protocolos SoraFS e instale o modelo de ameaça SoraFS.
- Ergonomia do SDK do cliente está disponível.

## Arquiteto responsável

1. **Envio:** Клиенты отправляют blobs через Torii DA ingest API. Узел
   criar blobs, codificar manifestos Norito (tipo blob, lane, época, codec de sinalizadores),
   e separar pedaços na camada quente SoraFS.
2. **Anúncio:** Intenções de pinos e dicas de replicação распространяются к
   provedores de armazenamento через registro (mercado SoraFS) com tags de política,
   определяющими цели retenção quente/frio.
3. **Compromisso:** Sequenciadores Nexus включают compromissos de blob (CID + opcional
   Raízes KZG) no bloco canônico. Clientes leves usam hash de compromisso e
   объявленную metadados para verificar a disponibilidade.
4. **Replicação:** Nós de armazenamento подтягивают назначенные compartilhamentos/pedaços, проходят
   Desafios PDP/PoTR e níveis altos/frios de acordo com a política.
5. **Buscar:** Потребители получают данные через SoraFS ou gateways com reconhecimento de DA,
   prove provas e solicite solicitações de reparo em réplicas atualizadas.
6. **Governança:** Parlament и комитет DA утверждают операторов, cronogramas de aluguel
   e aplicação de escalonamentos. Artefatos de governança são produzidos por eles
   processo de processamento.

## Atividades e ações

Шкала влияния: **Crítico** ломает безопасность/живучесть razão; **Alto**
блокирует DA backfill или клиентов; **Moderado** снижает качество, não
восстановимо; **Baixa** avaliação de desempenho.

| Ativo | Descrição | Integridade | Disponibilidade | Confidencialidade | Proprietário |
| --- | --- | --- | --- | --- | --- |
| Blobs DA (pedaços + manifestos) | Taikai, pista, blobs de governança em SoraFS | Crítico | Crítico | Moderado | DA WG / Equipe de Armazenamento |
| Manifestos DA Norito | Tipo de metadados de blobs | Crítico | Alto | Moderado | GT de Protocolo Central |
| Bloquear compromissos | CIDs + raízes KZG em blocos Nexus | Crítico | Alto | Baixo | GT de Protocolo Central |
| Cronogramas PDP/PoTR | Aplicação de cadencias para réplicas DA | Alto | Alto | Baixo | Equipe de armazenamento |
| Cadastro de operadores | Fornecedores de armazenamento e políticas | Alto | Alto | Baixo | Conselho de Governança |
| Registros de aluguel e incentivos | Livro-razão para DA rent e штрафов | Alto | Moderado | Baixo | GT Tesouraria |
| Painéis de observabilidade | SLOs DA, replicações globais, alertas | Moderado | Alto | Baixo | SRE / Observabilidade |
| Intenções de reparação | Propagação de alimentos em pedaços | Moderado | Moderado | Baixo | Equipe de armazenamento |

## Proteja-se e возможности| Ator | Desenvolvimento | Motivações | Nomeação |
| --- | --- | --- | --- |
| Cliente malicioso | Eliminar blobs malformados, reproduzir manifestos obsoletos, ingestão de DoS. | Срыв Transmissão Taikai, инъекция невалидных данных. | Não use uma chave privada. |
| Nó de armazenamento bizantino | Abandone réplicas, forje provas PDP/PoTR, conspire. | Срезать DA retenção, избежать aluguel, удерживать данные. | Eu tenho credenciais de operador válidas. |
| Sequenciador comprometido | Оmitir compromissos, equivocar bloqueios, reordenar metadados. | Escreva submissões do DA, sem necessidade. | Ограничен консенсусом большинства. |
| Operador interno | Злоупотребление governança доступом, подмена política de retenção, утечка credenciais. | Экономическая выгода, саботаж. | Instale uma infraestrutura quente/fria. |
| Adversário da rede | Partição узлов, replicação задержка, tráfego MITM. | Disponibilidade de informações, SLOs de degradação. | Não use TLS, não pode ser transferido/transferido. |
| Atacante de observabilidade | Você pode gerenciar painéis/alertas, fornecer informações. | Скрыть interrupções no DA. | Требует доступа к pipeline de telemetria. |

## Graniцы доверия

- **Limite de entrada:** Cliente -> Extensão Torii DA. Nenhuma autenticação em
  запрос, limitação de taxa e validação de carga útil.
- **Limite de replicação:** Nós de armazenamento armazenam pedaços e provas. Узлы
  взаимно аутентифицированы, но могут вести себя Bizantino.
- **Limite do razão:** Dados de bloco confirmados versus armazenamento fora da cadeia. Consciência
  Embora seja seguro, não há disponibilidade para aplicação fora da cadeia.
- **Limite de governação:** Решения Conselho/Parlamento по операторам, бюджетам и
  cortando. A configuração foi realizada na implantação do DA.
- **Limite de observabilidade:** Coletar métricas/logs e exportar para painéis/alertas
  ferramentas. A adulteração causa interrupções ou ataques.

## Сценарии угроз и контрмеры

### Атаки на ingerir путь

**Сенарий:** Cliente malicioso отправляет cargas úteis Norito malformadas ou
blobs grandes para metadados de recuperação de histórico ou de metadados.

**Contadores**
- Validação do esquema Norito na versão atual; rejeitar bandeiras desconhecidas.
- Limitação de taxa e autenticação no endpoint de ingestão Torii.
- Organize o tamanho do bloco e determine a codificação no chunker SoraFS.
- Pipeline de admissão сохраняет manifestos только после совпадения checksum.
- Cache de repetição determinístico (`ReplayCache`) отслеживает окна `(lane, epoch, sequence)`,
  obter marcas d'água altas no disco e excluir duplicatas/replays obsoletos; propriedade
  e chicotes de fuzz geram impressões digitais divergentes e envios fora de ordem.
  [crates/iroha_core/src/da/replay_cache.rs:1]
  [fuzz/da_replay_cache.rs:1] [crates/iroha_torii/src/da/ingest.rs:1]

**Problemas de teste**
- Torii ingerir должен встроить cache de repetição na admissão e sequência сохранять
  cursores между рестартами.
- Esquemas Norito DA имеют выделенный fuzz chicote (`fuzz/da_ingest_schema.rs`) para
  проверки codificar/decodificar инвариантов; painéis de cobertura должны сигнализировать
  por registro.

### Удержание репликации

**Сценарий:** Operadores de armazenamento bizantinos determinam atribuições de pinos, sem queda
pedaços e fornecer PDP/PoTR são respostas forjadas ou conluio.

**Contadores**
- Cronograma de desafio PDP/PoTR definido em cargas úteis DA com suporte na época.
- Replicação multifonte com limites de quorum; buscar orquestrador выявляет
  fragmentos ausentes e reparação de инициирует.
- Redução da governança de provas com falha e réplicas ausentes.
- Trabalho de reconciliação automatizado (`cargo xtask da-commitment-reconcile`) сравнивает
  ingerir recibos com compromissos DA (SignedBlockWire/`.norito`/JSON), formulário
  Pacote de evidências JSON para governança e падает при tickets ausentes/incompatíveis,
  O Alertmanager pode ser protegido por omissão/adulteração.

**Problemas de teste**
- Chicote de simulação em `integration_tests/src/da/pdp_potr.rs` (testes:
  `integration_tests/tests/da/pdp_potr_simulation.rs`) теперь покрывает conluio
  e partição, проверяя детерминированное выявление Bizantino. Produto
  расширение вместе с DA-5.
- Политика de despejo de nível frio требует trilha de auditoria assinada, чтобы исключить gotas secretas.

### Compromissos de Подмена

**Сценарий:** Sequenciador comprometido публикует блоки с пропуском/изменением DA
compromissos, você pode buscar falhas e inconsistências de clientes leves.**Contadores**
- Консенсус проверяет bloquear propostas против filas de submissão DA; pares
  отвергают предложения без обязательных compromissos.
- Clientes leves fornecem provas de inclusão antes de usar identificadores de busca.
- A trilha de auditoria exibe recibos de envio e bloqueia compromissos.
- Trabalho de reconciliação automatizado (`cargo xtask da-commitment-reconcile`) сравнивает
  ingerir recibos com compromissos DA (SignedBlockWire/`.norito`/JSON), formato
  Pacote de evidências JSON e adicionado a tickets ausentes/incompatíveis para Alertmanager.

**Problemas de teste**
- Trabalho de reconciliação Закрыто + gancho Alertmanager; pacotes de governança
  para usar o pacote de evidências JSON ingerido.

### Partição de rede e segurança

**Сценарий:** Adversário разделяет rede de replicação, мешая узлам получать
назначенные pedaços ou отвечать на PDP/PoTR desafios.

**Contadores**
- Provedor multirregional требования обеспечивают разнообразные caminhos de rede.
- Desafio do Windows com jitter e fallback para reparo fora de banda.
- Painéis de observabilidade monitoram a profundidade da replicação, desafiam o sucesso e
  buscar latência com limites de alerta.

**Problemas de teste**
- Simulações de partição para eventos ao vivo do Taikai; нужны testes de imersão.
- Política de reserva de largura de banda de reparo sem formalização.

### Liberação gratuita

**Сценарий:** Operador с доступом к registro манипулирует retenção политиками,
lista de permissões - provedores maliciosos ou alertas.

**Contadores**
- As ações de governança incluem assinaturas multipartidárias e registros autenticados Norito.
- Mudanças de política são publicadas em monitoramento e arquivamento de logs.
- O pipeline de observabilidade impõe registros Norito somente anexados com encadeamento de hash.
- Automação de revisão de acesso trimestral (`cargo xtask da-privilege-audit`) fornecida
  diretório de manifesto/replay (que é usado para operar), отмечает ausente/não-diretório/
  entradas graváveis mundialmente e usar pacote JSON assinado para painéis de governança.

**Problemas de teste**
- Instantâneos assinados com evidências de violação do painel.

## Реестр остаточных рисков

| Risco | Probabilidade | Impacto | Proprietário | Plano de Mitigação |
| --- | --- | --- | --- | --- |
| Repetir manifestos DA para usar o cache de sequência DA-2 | Possível | Moderado | GT de Protocolo Central | Realizar cache de sequência + validação de nonce em DA-2; testes de regressão добавить. |
| Conluio PDP/PoTR при компрометации >f узлов | Improvável | Alto | Equipe de armazenamento | Вывести новый cronograma de desafio com amostragem entre provedores; arnês de simulação валидировать через. |
| Lacuna na auditoria de despejo nas camadas frias | Possível | Alto | Equipe SRE/Armazenamento | Registrar registros de auditoria assinados e recibos na cadeia para despejos; monitorar painéis. |
| Latência обнаружения sequenciador de omissão | Possível | Alto | GT de Protocolo Central | O `cargo xtask da-commitment-reconcile` compara recibos versus compromissos (SignedBlockWire/`.norito`/JSON) e verifica a governança de tickets ausentes/incompatíveis. |
| Resiliência de partição para transmissões ao vivo de Taikai | Possível | Crítico | Rede TL | Провести exercícios de partição; зарезервировать reparar largura de banda; Failover SOP de documentação. |
| Desvio de privilégio de governança | Improvável | Alto | Conselho de Governança | Trimestralmente `cargo xtask da-privilege-audit` (diretórios de manifesto/repetição + caminhos extras) com JSON assinado + portão do painel; artefatos de auditoria âncora na cadeia. |

## Acompanhamentos necessários

1. Abra esquemas Norito para ingestão de DA e vetores de exemplo (instalados em DA-2).
2. Configure o cache de repetição para Torii DA ingest e сохранять cursores de sequência
   primeiro reiniciá-lo.
3. **Concluído (2026-02-05):** Chicote de simulação PDP/PoTR теперь моделирует
   conluio + partição с modelagem de backlog de QoS; sim. `integration_tests/src/da/pdp_potr.rs`
   (testes: `integration_tests/tests/da/pdp_potr_simulation.rs`) с детерминированными сводками.
4. **Concluído (29/05/2026):** `cargo xtask da-commitment-reconcile` сравнивает
   ingerir recibos com compromissos DA (SignedBlockWire/`.norito`/JSON), эмитирует
   `artifacts/da/commitment_reconciliation.json` e подключен к Alertmanager/ governança
   pacotes para alertas de omissão/adulteração (`xtask/src/da.rs`).
5. **Concluído (2026-05-29):** `cargo xtask da-privilege-audit` проходит manifesto/replay
   spool (e caminhos de operadores), отмечает ausente/não-diretório/gravável mundialmente e
   Gerar pacote JSON assinado para análises de painel/governança
   (`artifacts/da/privilege_audit.json`), abrindo lacuna para automação de revisão de acesso.

**Isso é mais importante:**- Cache de repetição e cursores de persistência pousados ​​no DA-2. Realização em
  `crates/iroha_core/src/da/replay_cache.rs` (lógica de cache) e integração Torii em
  `crates/iroha_torii/src/da/ingest.rs`, as verificações de impressão digital são processadas pelo `/v1/da/ingest`.
- Simulações de streaming PDP / PoTR упражняются через arnês de fluxo de prova em
  `crates/sorafs_car/tests/sorafs_cli.rs`, fluxos de solicitação PoR/PDP/PoTR e
  cenários de falha do modelo do jogo.
- Capacidade e reparação de imersão результаты в
  `docs/source/sorafs/reports/sf2c_capacity_soak.md`, e Sumeragi matriz de imersão em
  `docs/source/sumeragi_soak_matrix.md` (variantes locais). Эти
  artefatos фиксируют долгие brocas из реестра рисков.
- Reconciliação + automação de auditoria de privilégios
  `docs/automation/da/README.md` e novo comando
  `cargo xtask da-commitment-reconcile`/`cargo xtask da-privilege-audit`; usar
  saídas по умолчанию в `artifacts/da/` при прикреплении evidências к pacotes de governança.

## Evidência de simulação e modelagem de QoS (2026-02)

Чтобы закрыть DA-1 follow-up #3, мы реализовали детерминированный PDP/PoTR
chicote de simulação в `integration_tests/src/da/pdp_potr.rs` (testes:
`integration_tests/tests/da/pdp_potr_simulation.rs`). Arnês распределяет
узлы по 3 região, вводит partições/conluio согласно вероятностям roteiro,
отслеживает atraso PoTR e питает modelo de backlog de reparo, отражающую hot-tier
orçamento de reparação. Cenário padrão do Запуск (12 épocas, 18 desafios PDP + 2 PoTR
janelas por época) na métrica da sequência:

<!-- BEGIN_DA_SIM_TABLE -->
<!-- AUTO-GENERATED by scripts/docs/render_da_threat_model_tables.py; do not edit manually. -->
| Métrica | Valor | Notas |
| --- | --- | --- |
| Falhas de PDP detectadas | 48/49 (98,0%) | As partições devem ser detectadas; единственный недетектированный сбой связан с честным jitter. |
| PDP significa latência de detecção | 0,0 épocas | Сбои фиксируются в исходном época. |
| Falhas PoTR detectadas | 28/77 (36,4%) | Detecte срабатывает при пропуске >=2 janelas PoTR, оставляя большинство событий no registro de risco residual. |
| PoTR significa latência de detecção | Épocas 2.0 | Limite de atraso de 2 épocas definido no escalonamento de arquivamento. |
| Pico na fila de reparos | 38 manifestos | Backlog растет, когда partições накапливаются быстрее 4 reparos/época. |
| Latência de resposta p95 | 30.068ms | Abra a janela de desafio de 30 s com jitter de +/-75 ms para amostragem de QoS. |
<!-- END_DA_SIM_TABLE -->

Esses resultados permitem que você crie painéis DA e crie critérios
приемки "arnês de simulação + modelagem QoS" do roteiro.

Instalação automática para
`cargo xtask da-threat-model-report [--out <path|->] [--seed <u64|0xhex>] [--config <path>]`,
você precisa obter um chicote e inserir Norito JSON em
`artifacts/da/threat_model_report.json` para uso. Novo uso
este é um valor para gerar matriz em documentos e alertas sobre desvios nas taxas de detecção,
filas de reparo ou amostras de QoS.

Para obter tabelas você usa `make docs-da-threat-model`, o que você precisa
`cargo xtask da-threat-model-report`, verificado
`docs/source/da/_generated/threat_model_report.json`, e verifique a verificação de segurança
`scripts/docs/render_da_threat_model_tables.py`. Porta `docs/portal`
(`docs/portal/docs/da/threat-model.md`) обновляется em seu processo de sincronização.