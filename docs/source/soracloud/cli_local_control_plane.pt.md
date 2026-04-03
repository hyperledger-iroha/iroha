<!-- Auto-generated stub for Portuguese (pt) translation. Replace this content with the full translation. -->

---
lang: pt
direction: ltr
source: docs/source/soracloud/cli_local_control_plane.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 567b63e9b61afaecfa5d85aa60f0348c856557e171559885ffaba45168ce61dc
source_last_modified: "2026-03-26T06:12:11.480025+00:00"
translation_last_reviewed: 2026-04-02
translator: machine-google-reviewed
---

# Soracloud CLI e plano de controle

Soracloud v1 é um tempo de execução oficial somente IVM.

- `iroha app soracloud init` é o único comando offline. É um andaime
  `container_manifest.json`, `service_manifest.json` e modelo opcional
  artefatos para serviços Soracloud.
- Todos os outros comandos Soracloud CLI são suportados apenas pela rede e requerem
  `--torii-url`.
- A CLI não mantém nenhum estado ou espelho do plano de controle local do Soracloud
  arquivo.
- Torii atende status Soracloud público e rotas de mutação diretamente de
  estado mundial oficial, além do gerenciador de tempo de execução Soracloud incorporado.

## Escopo do tempo de execução- Soracloud v1 aceita apenas `SoraContainerRuntimeV1::Ivm`.
- `NativeProcess` permanece rejeitado.
- A execução ordenada da caixa de correio executa manipuladores IVM admitidos diretamente.
- A hidratação e a materialização vêm do conteúdo SoraFS/DA comprometido, e não
  do que instantâneos locais sintéticos.
- `SoraContainerManifestV1` agora carrega `required_config_names` e
  `required_secret_names`, mais `config_exports` explícito. Implantar, atualizar,
  e a reversão falha fechada quando o conjunto de material oficial efetivo seria
  não satisfazem essas ligações declaradas ou quando uma exportação de configuração tem como alvo um
  configuração não obrigatória ou um destino de ambiente/arquivo duplicado.
- As entradas de configuração de serviço confirmadas agora são materializadas em
  `services/<service>/<version>/configs/<config_name>` como JSON canônico
  arquivos de carga útil.
- As exportações explícitas de ambiente de configuração são projetadas em
  `services/<service>/<version>/effective_env.json` e as exportações de arquivos são
  materializado sob
  `services/<service>/<version>/config_exports/<relative_path>`. Exportado
  os valores usam o texto canônico da carga JSON da entrada de configuração referenciada.
- Os manipuladores Soracloud IVM agora podem ler essas cargas úteis de configuração autorizadas
  diretamente através da superfície `ReadConfig` do host de tempo de execução, tão comum
  Os manipuladores `query`/`update` não precisam adivinhar os caminhos dos arquivos locais do nó apenas para
  consumir configuração de serviço confirmada.
- Envelopes secretos de serviço comprometido agora são materializados sob
  `services/<service>/<version>/secret_envelopes/<secret_name>` como
  arquivos de envelope oficiais.- Os manipuladores Soracloud IVM comuns agora podem ler os segredos confirmados
  envelopes diretamente através da superfície `ReadSecretEnvelope` do host de tempo de execução.
- A árvore de fallback herdada do tempo de execução privado agora está sincronizada com o commit
  estado de implantação em `secrets/<service>/<version>/<secret_name>` para que o
  caminho de leitura do segredo bruto mais antigo e o ponto do plano de controle autoritativo no
  mesmos bytes.
- O tempo de execução privado `ReadSecret` agora resolve a implantação autoritativa
  `service_secrets` primeiro e somente volta para o nó local legado
  Árvore de arquivos materializados `secrets/<service>/<version>/...` quando não confirmado
  existe uma entrada secreta de serviço para a chave solicitada.
- A ingestão de segredos ainda é intencionalmente mais restrita que a ingestão de configurações:
  `ReadSecretEnvelope` é o contrato de manipulador comum de segurança pública, enquanto
  `ReadSecret` permanece apenas em tempo de execução privado e ainda retorna o commit
  bytes de texto cifrado de envelope em vez de um contrato de montagem de texto simples.
- Os planos de serviço de tempo de execução agora expõem a capacidade de ingestão correspondente
  booleanos mais o `config_exports` declarado e efetivo projetado
  ambiente, para que os consumidores de status possam saber se uma revisão materializada
  suporta leituras de configuração de host, leituras de envelope secreto de host, segredo bruto privado
  leituras e injeção de configuração explícita sem inferir do manipulador
  aulas sozinho.

## Comandos CLI-`iroha app soracloud init`
  - apenas andaime offline.
  - suporta modelos `baseline`, `site`, `webapp` e `pii-app`.
-`iroha app soracloud deploy`
  - valida localmente as regras de admissão `SoraDeploymentBundleV1`, assina o
    solicitação e chama `POST /v1/soracloud/deploy`.
  - `--initial-configs <path>` e `--initial-secrets <path>` agora podem anexar
    configuração de serviço inline oficial / mapas secretos atomicamente com o
    primeiro implante para que as ligações necessárias possam ser satisfeitas na primeira admissão.
  - a CLI agora assina a solicitação HTTP canonicamente com
    `X-Iroha-Account`, `X-Iroha-Signature`, `X-Iroha-Timestamp-Ms` e
    `X-Iroha-Nonce` para contas comuns de assinatura única ou
    `X-Iroha-Account` mais `X-Iroha-Witness` quando
    `soracloud.http_witness_file` aponta para uma carga útil JSON de testemunha multisig;
    Torii retorna um conjunto de instruções de transação de rascunho determinístico e o
    A CLI então envia a transação real por meio do cliente Iroha normal
    pista.
  - Torii também impõe limites de admissão de host SCR e capacidade de falha fechada
    verifica antes que a mutação seja aceita.
-`iroha app soracloud upgrade`
  - valida e assina uma nova revisão do pacote e depois chama
    `POST /v1/soracloud/upgrade`.
  - o mesmo fluxo `--initial-configs <path>` / `--initial-secrets <path>` é
    disponível para atualizações de materiais atômicos durante a atualização.
  - As mesmas verificações de admissão do host SCR são executadas no lado do servidor antes da atualização ser
    admitiu.
-`iroha app soracloud status`- consulta o status do serviço oficial de `GET /v1/soracloud/status`.
-`iroha app soracloud config-*`
  - `config-set`, `config-delete` e `config-status` são suportados apenas por Torii.
  - a CLI assina cargas e chamadas de origem de configuração de serviço canônicas
    `POST /v1/soracloud/service/config/set`,
    `POST /v1/soracloud/service/config/delete` e
    `GET /v1/soracloud/service/config/status`.
  - as entradas de configuração persistem no estado de implantação autoritativo e permanecem
    anexado em alterações de revisão de implantação/atualização/reversão.
  - `config-delete` agora falha ao fechar quando a revisão ativa ainda declara
    a configuração nomeada em `container.required_config_names`.
-`iroha app soracloud secret-*`
  - `secret-set`, `secret-delete` e `secret-status` são suportados apenas por Torii.
  - a CLI assina cargas e chamadas de origem secreta de serviço canônico
    `POST /v1/soracloud/service/secret/set`,
    `POST /v1/soracloud/service/secret/delete` e
    `GET /v1/soracloud/service/secret/status`.
  - entradas secretas são persistidas como registros `SecretEnvelopeV1` oficiais
    no estado de implantação e sobreviver às alterações normais de revisão do serviço.
  - `secret-delete` agora falha ao fechar quando a revisão ativa ainda declara
    o segredo nomeado em `container.required_secret_names`.
-`iroha app soracloud rollback`
  - assina metadados de reversão e chama `POST /v1/soracloud/rollback`.
-`iroha app soracloud rollout`
  - assina metadados de implementação e chama `POST /v1/soracloud/rollout`.
-`iroha app soracloud agent-*`
  - todos os comandos de ciclo de vida do apartamento, carteira, caixa de correio e autonomia são
    Apenas com suporte Torii.
-`iroha app soracloud training-*`
  - todos os comandos de trabalho de treinamento são suportados apenas por Torii.-`iroha app soracloud model-*`
  - todos os comandos de artefato de modelo, peso, modelo carregado e tempo de execução privado
    são suportados apenas por Torii.
  - a superfície uploaded-model/private-runtime agora reside na mesma família:
    `model-upload-encryption-recipient`, `model-upload-init`,
    `model-upload-chunk`, `model-upload-finalize`, `model-upload-status`,
    `model-compile`, `model-compile-status`, `model-allow`,
    `model-run-private`, `model-run-status`, `model-decrypt-output` e
    `model-publish-private`.
  - `model-run-private` agora oculta o handshake de tempo de execução de rascunho e finalização em
    a CLI e retorna o status oficial da sessão pós-finalização.
  - `model-publish-private` agora suporta tanto um preparado
    agrupar/pedaço/finalizar/compilar/permitir plano de publicação e um plano de publicação de nível superior
    projecto de documento. O rascunho agora traz `source: PrivateModelSourceV1`,
    que aceita `LocalDir { path }` ou
    `HuggingFaceSnapshot { repo, revision }`.
  - quando chamado com `--draft-file`, a CLI normaliza a fonte declarada
    em uma árvore temporária determinística, valida o contrato de tensores de segurança HF v1,
    serializa e criptografa deterministicamente o pacote em relação ao ativo
    Destinatário de upload Torii, fragmenta-o em pedaços criptografados de tamanho fixo,
    opcionalmente, grava o plano preparado via `--emit-plan-file` e, em seguida,
    executa a sequência de upload/finalização/compilação/permissão.
  - As revisões `HuggingFaceSnapshot` são obrigatórias e devem ser fixadas no commit
    SHAs; referências semelhantes a ramificações são rejeitadas e fechadas com falha.- o layout da fonte admitido é intencionalmente estreito na v1: `config.json`,
    ativos tokenizer, um ou mais fragmentos `*.safetensors` e opcional
    metadados de processador/pré-processador para modelos com capacidade de imagem. GGUF, ONNX,
    outros pesos não-safetensores e layouts personalizados aninhados arbitrários são
    rejeitado.
  - quando chamado com `--plan-file`, o CLI ainda consome um já
    documento de plano de publicação preparado e falha no fechamento quando o upload do plano
    o destinatário não corresponde mais ao destinatário oficial Torii.
  - veja `uploaded_private_models.md` para o design que coloca essas rotas em camadas
    no registro de modelo existente e nos registros de artefato/peso.
- Rotas do plano de controle `model-host`
  - Torii agora expõe autoridade
    `POST /v1/soracloud/model-host/advertise`,
    `POST /v1/soracloud/model-host/heartbeat`,
    `POST /v1/soracloud/model-host/withdraw` e
    `GET /v1/soracloud/model-host/status`.
  - essas rotas persistem anúncios de recurso de host validador opt-in em
    estado mundial oficial e permitir que os operadores inspecionem quais validadores são
    atualmente anunciando capacidade de modelo-host.
  -`iroha app soracloud model-host-advertise`,
    `model-host-heartbeat`, `model-host-withdraw` e
    `model-host-status` agora assina as mesmas cargas úteis de proveniência canônica que o
    API bruta e chame as rotas Torii correspondentes diretamente.
-`iroha app soracloud hf-*`
  - `hf-deploy`, `hf-status`, `hf-lease-leave` e `hf-lease-renew` são
    Somente com suporte Torii.- `hf-deploy` e `hf-lease-renew` agora também admitem automaticamente o determinístico
    serviço de inferência HF gerado para o `service_name` solicitado, e
    admitir automaticamente o apartamento HF gerado determinístico para
    `apartment_name` quando solicitado, antes da mutação de arrendamento compartilhado
    é submetido.
  - a reutilização é fechada com falha: se o serviço/apartamento nomeado já existe, mas é
    não a implantação HF gerada esperada para essa fonte canônica, o HF
    a mutação é rejeitada em vez de vincular silenciosamente o arrendamento a não relacionados
    Objetos Soracloud.
  - quando o gerenciador de tempo de execução incorporado estiver conectado, `hf-status` agora também
    retorna uma projeção de tempo de execução para a fonte canônica, incluindo limite
    serviços/apartamentos, visibilidade na próxima janela na fila e pacote local/
    falhas no cache de artefatos; `importer_pending` segue essa projeção de tempo de execução
    em vez de confiar apenas na enumeração de origem oficial.
  - quando `hf-deploy` ou `hf-lease-renew` admite o serviço HF gerado em
    a mesma transação que a mutação de arrendamento compartilhado, o HF autoritativo
    fonte agora muda para `Ready` imediatamente e `importer_pending` permanece
    `false` na resposta.
  - O status de concessão de HF e as respostas de mutação agora também expõem qualquer autoridade
    instantâneo de veiculação já anexado à janela de locação ativa, incluindohosts atribuídos, contagem de hosts elegíveis, contagem de hosts quentes e separado
    campos de taxa de armazenamento versus computação.
  - `hf-deploy` e `hf-lease-renew` agora derivam o recurso HF canônico
    perfil dos metadados resolvidos do repositório Hugging Face antes de enviarem o
    mutação:
    - Torii inspeciona o repositório `siblings`, prefere `.gguf` a
      `.safetensors` em layouts de peso PyTorch, HEADs os arquivos selecionados para
      deriva `required_model_bytes` e mapeia isso para um primeiro lançamento
      back-end/formato mais pisos de RAM/disco;
    - a admissão do arrendamento falha quando nenhum anúncio do host do validador ao vivo pode
      satisfazer esse perfil; e
    - quando um conjunto de hosts está disponível, a janela ativa agora registra um
      posicionamento determinístico ponderado por participação e uma reserva de computação separada
      taxa junto com a contabilidade de arrendamento de armazenamento existente.
  - membros posteriores que aderirem a uma janela HF ativa agora pagam armazenamento rateado e
    computar compartilhamentos apenas para a janela restante, enquanto os membros anteriores
    receber o mesmo reembolso de armazenamento determinístico e contabilidade de reembolso de computação
    daquela adesão tardia.
  - o gerenciador de tempo de execução incorporado pode sintetizar o pacote stub HF gerado
    localmente, para que os serviços gerados possam se materializar sem esperar por um
    carga útil SoraFS comprometida apenas para o pacote de inferência de espaço reservado.- o gerenciador de tempo de execução incorporado agora também importa o repositório Hugging Face permitido
    arquivos em `soracloud_runtime.state_dir/hf_sources/<source_id>/files/` e
    persiste um `import_manifest.json` local com o commit resolvido, importado
    arquivos, arquivos ignorados e qualquer erro do importador.
  - leituras locais HF `metadata` geradas agora retornam esse manifesto de importação local,
    incluindo o inventário de arquivos importados e se a execução local e
    o fallback da ponte está habilitado para o nó.
  - leituras locais HF `infer` geradas agora preferem a execução no nó em relação ao
    bytes compartilhados importados:
    - `irohad` materializa um script de adaptador Python incorporado no local
      Diretório de estado de tempo de execução Soracloud e o invoca através
      `soracloud_runtime.hf.local_runner_program`;
    - o executor incorporado primeiro verifica uma estrofe de fixture determinística em
      `config.json` (usado por testes), caso contrário carrega a fonte importada
      diretório através de `transformers.pipeline(..., local_files_only=True)` então
      o modelo é executado na importação local compartilhada em vez de extrair
      bytes de hub novos; e
    - se `soracloud_runtime.hf.allow_inference_bridge_fallback = true` e
      `soracloud_runtime.hf.inference_token` está configurado, o tempo de execução cai
      retornar ao URL base de inferência HF configurado somente quando a execução local
      está indisponível ou falha e o chamador aceita explicitamente
      `x-soracloud-hf-allow-bridge-fallback: 1`, `true` ou `yes`.
  - a projeção do tempo de execução agora mantém uma fonte HF em `PendingImport` atéexiste um manifesto de importação local bem-sucedido e as falhas do importador surgem como
    tempo de execução `Failed` mais `last_error` em vez de relatar silenciosamente `Ready`.
  - os apartamentos HF gerados agora consomem autonomia aprovada através do
    caminho de tempo de execução local do nó:
    - `agent-autonomy-run` agora segue um fluxo de duas etapas: a primeira assinada
      mutação registra a aprovação oficial e retorna um valor determinístico
      rascunho da transação, então uma segunda solicitação de finalização assinada solicita ao
      gerenciador de tempo de execução incorporado para executar aquela execução aprovada no limite
      serviço HF `/infer` gerado e retorna qualquer acompanhamento oficial
      instruções como outro rascunho determinístico;
    - o registro de execução aprovado agora também persiste como canônico
      `request_commitment`, para que o recibo de serviço gerado posteriormente possa ser
      vinculado à aprovação exata da autonomia oficial;
    - as aprovações agora podem persistir em um `workflow_input_json` canônico opcional
      corpo; quando presente, o tempo de execução incorporado encaminha a carga JSON exata
      para o manipulador HF `/infer` gerado e, quando ausente, ele volta para
      o envelope `run_label`-as-`inputs` mais antigo com a autoridade
      `artifact_hash` / `provenance_hash` / `budget_units` / `run_id` transportado
      como parâmetros estruturados;
    - `workflow_input_json` agora também pode optar por sequencial determinísticaexecução em várias etapas com
      `{ "workflow_version": 1, "steps": [...] }`, onde cada etapa executa um
      solicitação HF `/infer` gerada e etapas posteriores podem fazer referência a saídas anteriores
      através de `${run.*}`, `${previous.text|json|result_commitment}` e
      Espaços reservados `${steps.<step_id>.text|json|result_commitment}`; e
    - tanto a resposta à mutação quanto o `agent-autonomy-status` agora emergem
      resumo de execução local do nó, quando disponível, incluindo sucesso/falha,
      a revisão do serviço vinculado, compromissos de resultados determinísticos, ponto de verificação
      / hashes de artefato de diário, o serviço gerado `AuditReceipt` e o
      corpo de resposta JSON analisado.
    - quando o recibo de serviço gerado estiver presente, Torii o registra em
      oficial `soracloud_runtime_receipts` e expõe o resultado
      recebimento oficial de tempo de execução sobre o status de execução recente junto com o
      resumo de execução local do nó.
    - o caminho de autonomia de HF gerado agora também registra uma autoridade dedicada
      evento de auditoria do apartamento `AutonomyRunExecuted` e status de execução recente
      retorna essa auditoria de execução junto com o recebimento oficial do tempo de execução.
  - `hf-lease-renew` agora possui dois modos:
    - se a janela atual expirou ou foi esgotada, ela abre imediatamente uma nova
      janela;
    - se a janela atual ainda estiver ativa, ela coloca o chamador na fila como o
      patrocinador da próxima janela, cobra o armazenamento e computação completos da próxima janelataxas de reserva antecipadas, persiste a próxima janela determinística
      plano de colocação e expõe o patrocínio na fila por meio de `hf-status`
      até que uma mutação posterior avance o conjunto.
  - o ingresso público HF `/infer` gerado agora resolve o problema autoritativo
    colocação e, quando o nó receptor não é o primário quente, faz proxy do
    solicitar mensagens de controle Soracloud P2P para o host primário atribuído;
    o tempo de execução incorporado ainda falha ao fechar na réplica direta/local não atribuído
    execução e recibos de tempo de execução HF gerados carregam `placement_id`,
    validador e atribuição de pares do registro de colocação oficial.
  - quando o caminho do proxy para o primário expira, fecha antes de uma resposta ou
    volta com uma falha de tempo de execução não cliente da autoridade
    primário, o nó de entrada agora reporta `AssignedHeartbeatMiss` para esse
    primário e enfileira `ReconcileSoracloudModelHosts` através do mesmo
    pista de mutação interna.
  - reconciliação oficial de host expirado agora os registros persistiram
    evidência de violação do host do modelo, reutiliza o caminho da barra do validador de via pública,
    e aplica a política padrão de penalidade de arrendamento compartilhado HF:
    `warmup_no_show_slash_bps=500`,
    `assigned_heartbeat_miss_slash_bps=250`,
    `assigned_heartbeat_miss_strike_threshold=3` e
    `advert_contradiction_slash_bps=1000`.
  - A integridade do tempo de execução HF atribuída localmente agora também alimenta o mesmo caminho de evidência:falhas de importação/aquecimento em tempo de reconciliação em uma emissão de host `Warming` local
    `WarmupNoShow` e falhas de trabalhadores residentes no primário quente local
    emitir relatórios `AssignedHeartbeatMiss` limitados por meio do normal
    fila de transações.
  - reconciliar agora também os pré-arranques e sondar os trabalhadores residentes em HF localmente
    hosts atribuídos de aquecimento/aquecimento, incluindo réplicas, para que uma réplica possa falhar
    fechado no caminho oficial `AssignedHeartbeatMiss` antes de qualquer
    a solicitação pública `/infer` sempre chega ao primário.
  - quando a investigação local é bem-sucedida, o tempo de execução agora também emite um
    mutação `model-host-heartbeat` oficial para o validador local quando
    o host atribuído ainda é `Warming` ou o anúncio do host ativo precisa de um TTL
    atualização, para que a prontidão local bem-sucedida promova a mesma autoridade
    o posicionamento/anúncio informa que as pulsações manuais seriam atualizadas.
  - quando o tempo de execução emite um `WarmupNoShow` local ou
    `AssignedHeartbeatMiss`, agora também enfileira
    `ReconcileSoracloudModelHosts` através da mesma via de mutação interna, então
    o failover/preenchimento autoritativo começa imediatamente em vez de esperar
    uma varredura periódica posterior de expiração do host.
  - quando o ingresso de HF gerado pelo público falha ainda mais cedo porque o compromisso
    o posicionamento não tem primário quente para proxy, Torii agora pergunta ao tempo de execução
    identificador para enfileirar esse mesmo autoridadeInstrução `ReconcileSoracloudModelHosts` imediatamente em vez de esperar
    para uma expiração posterior ou sinal de falha do trabalhador.
  - quando o ingresso de HF gerado pelo público recebe uma resposta de sucesso por proxy,
    Torii agora verifica se o recibo de tempo de execução incluído ainda prova a execução por
    o primário quente comprometido para a colocação ativa; ausente ou incompatível
    a atribuição de veiculação agora falha e sugere que a mesma autoridade
    Caminho `ReconcileSoracloudModelHosts` em vez de retornar um
    resposta não oficial. Torii agora também rejeita sucesso de proxy
    respostas quando os compromissos de recebimento de tempo de execução ou a política de certificação não
    não corresponde à resposta que está prestes a retornar, e o mesmo recebimento incorreto
    path também alimenta o relatório `AssignedHeartbeatMiss` primário remoto
    gancho.
  - falhas de execução de HF geradas por proxy agora solicitam o mesmo
    caminho `ReconcileSoracloudModelHosts` oficial após relatar o
    falha remota de saúde primária, em vez de esperar por uma expiração posterior
    varrer.
  - Torii agora vincula cada solicitação de proxy HF gerada pendente ao
    peer primário autoritativo direcionado. Uma resposta proxy do errado
    peer agora é ignorado em vez de envenenar a solicitação pendente, portanto, apenas
    o primário autoritativo pode concluir ou falhar na solicitação. Um procurador
    resposta do par esperado com um esquema de resposta de proxy não suportadoversão ainda falha ao ser fechada em vez de ser aceita apenas porque é
    `request_id` correspondeu a uma solicitação pendente. Se o colega errado que respondeu
    ele próprio ainda é um host HF gerado atribuído para esse posicionamento, o
    o tempo de execução agora relata esse host por meio do existente
    Caminho de evidência `WarmupNoShow` / `AssignedHeartbeatMiss` com base em seu
    status de atribuição oficial e também dicas de autoridade
    `ReconcileSoracloudModelHosts`, desvio de autoridade primário/réplica obsoleto
    alimenta o loop de controle em vez de ser ignorado apenas no ingresso.
  - a execução do proxy Soracloud de entrada agora também está restrita ao pretendido
    caso de consulta HF gerado `infer` no primário quente confirmado. Não-HF
    rotas públicas de leitura local e solicitações HF geradas entregues a um nó
    esse não é mais o primário quente oficial, agora falha no fechamento
    de execução no caminho do proxy P2P. O primário oficial também agora
    recalcula o compromisso de solicitação HF canônico gerado antes da execução,
    portanto, envelopes de proxy forjados ou incompatíveis falham no fechamento.
  - quando uma réplica atribuída ou antigo primário obsoleto rejeita aquela entrada
    execução do proxy HF gerado porque não é mais o autoritativo
    primário quente, o tempo de execução do lado do receptor agora também sugere
    `ReconcileSoracloudModelHosts` em vez de depender apenas do chamador
    visualização de roteamento.- quando a mesma falha de autoridade de proxy HF gerada de entrada ocorre em
    o próprio primário autoritativo local, o tempo de execução agora o trata como um
    sinal de saúde do host de primeira classe: auto-relato de primárias quentes
    `AssignedHeartbeatMiss`, auto-relato de primárias de aquecimento `WarmupNoShow`,
    e ambos os caminhos reutilizam imediatamente a mesma autoridade
    Malha de controle `ReconcileSoracloudModelHosts`.
  - quando esse mesmo receptor não primário ainda é um dos destinatários autorizados
    hosts atribuídos e pode resolver o primário quente do estado da cadeia confirmada,
    agora ele faz o proxy da solicitação HF gerada para esse primário
    de falhar imediatamente. Validadores não atribuídos falham ao fechar em vez de agir
    como saltos de proxy HF intermediários genéricos, e o nó de entrada original ainda
    valida o recibo de tempo de execução retornado em relação ao posicionamento oficial
    estado. Se o salto da réplica atribuída para o primário falhar após o
    solicitação é realmente despachada, o tempo de execução do lado do receptor relata o
    falha de saúde primária remota e dicas confiáveis
    `ReconcileSoracloudModelHosts`; se a réplica local atribuída não puder sequer
    tente esse salto adiante porque seu próprio transporte/tempo de execução do proxy está faltando,
    a falha agora é tratada como uma falha de host atribuído local em vez de
    culpando o primário.
  - reconciliar agora também emite `AdvertContradiction` automaticamente quando oo ID do par de tempo de execução configurado do validador local discorda do
    ID de peer oficial `model-host-advertise` para esse validador.
  - mutações válidas de re-anunciação de modelo-hospedeiro agora também sincronizam autoridades
    metadados `peer_id` / `host_class` do host atribuído e recalcular a corrente
    taxas de reserva de colocação quando a classe anfitriã muda.
  - mutações contraditórias de re-anunciação de modelo-hospedeiro agora emitem
    Evidência `AdvertContradiction`, aplique a barra/despejo do validador existente
    caminho e atualizar os canais afetados em vez de apenas falhar na validação.
  - o trabalho restante de hospedagem HF é agora:
    - sinais de integridade mais amplos entre nós/cluster de tempo de execução além do local
      observações diretas do trabalhador/aquecimento do validador mais host atribuído
      falhas de autoridade do lado do receptor quando a saúde interna do ponto remoto deveria
      também alimente o caminho oficial de reequilíbrio/barra.
  - a execução local HF gerada agora mantém um trabalhador Python residente por origem
    vivo em `irohad`, reutiliza o modelo carregado em `/infer` repetido
    chama e reinicia esse trabalhador de forma determinística se a importação local
    manifestar alterações ou o processo será encerrado.
  - essas rotas não são o caminho do modelo carregado privado. Locações compartilhadas de HF ficam
    focado na associação de origem/importação compartilhada, em vez de criptografia on-chain
    bytes do modelo privado.- as aprovações de autonomia de HF geradas agora suportam sequencial determinístico
    envelopes de solicitação de várias etapas, mas não lineares/utilizadores de ferramentas mais amplos
    orquestração e execução de gráfico de artefato ainda permanecem como trabalho de acompanhamento
    além das etapas `/infer` encadeadas.

## Semântica de status

`/v1/soracloud/status` e os endpoints de status de agente/treinamento/modelo relacionados agora
reflete o estado de tempo de execução oficial:

- admitiu revisões de serviço do estado mundial comprometido;
- estado de hidratação/materialização do tempo de execução do gerenciador de tempo de execução incorporado;
- recibos reais de execução de caixas de correio e estado de falha;
- artefatos de periódicos/pontos de verificação publicados;
- integridade do cache e do tempo de execução em vez de correções de status de espaço reservado.

Se o material de tempo de execução oficial estiver obsoleto ou indisponível, as leituras falharão no fechamento
em vez de recorrer aos espelhos estaduais locais.

`/v1/soracloud/status` é o único endpoint de status Soracloud documentado na v1.
Não há rota `/v1/soracloud/registry` separada.

## Andaime local removido

Esses conceitos antigos de simulação local não existem mais na v1:

- Arquivos de registro/estado locais CLI ou opções de caminho de registro
- Espelhos de plano de controle com suporte de arquivo local Torii

## Exemplo

```bash
iroha app soracloud deploy \
  --container container_manifest.json \
  --service service_manifest.json \
  --torii-url http://127.0.0.1:8080 \
  --api-token <token-if-required> \
  --timeout-secs 10
```

## Notas- A validação local ainda é executada antes que as solicitações sejam assinadas e enviadas.
- Os endpoints de mutação Soracloud padrão não aceitam mais `authority` bruto/
  Campos JSON `private_key` para implantação, atualização, reversão, distribuição, agente
  caminhos de ciclo de vida, treinamento, host de modelo e peso de modelo; Torii autentica
  essas solicitações dos cabeçalhos de assinatura HTTP canônicos.
- Proprietários de Soracloud controlados por Multisig agora usam `X-Iroha-Witness`; ponto
  `soracloud.http_witness_file` no JSON de testemunha exato que você deseja que a CLI
  repetição para a próxima solicitação de mutação e Torii falhará ao fechar se o
  a conta do sujeito da testemunha ou o hash da solicitação canônica não correspondem.
- `hf-deploy` e `hf-lease-renew` agora incluem auxiliar assinado pelo cliente
  proveniência dos artefatos determinísticos de serviços/apartamentos HF gerados,
  então Torii não precisa mais de chaves privadas do chamador para admitir esses acompanhamentos
  objetos.
- `agent-autonomy-run` e `model/run-private` agora usam um rascunho e finalização
  fluxo: a primeira mutação assinada registra a aprovação/início oficial,
  e uma segunda solicitação de finalização assinada executa o caminho do tempo de execução e retorna
  quaisquer instruções de acompanhamento oficiais como rascunho determinístico
  transações.
- `model/decrypt-output` agora retorna a inferência privada autoritativa
  ponto de verificação como um rascunho de transação determinístico, assinado apenas pelo exteriortransação em vez de uma chave privada incorporada mantida por Torii.
- O anexo ZK CRUD agora remove a locação da conta Iroha assinada e ainda
  trata os tokens de API como uma porta de acesso extra quando habilitado.
- A entrada de leitura local pública do Soracloud agora aplica taxa explícita por IP e
  limites de simultaneidade e verifica novamente a visibilidade da rota pública antes de local ou
  execução por procuração.
- A aplicação da capacidade de tempo de execução privado acontece dentro da ABI do host Soracloud,
  não dentro do andaime local CLI ou Torii.
- `ram_lfe` continua sendo um subsistema separado de funções ocultas. Privado enviado pelo usuário
  a execução do transformador deve reutilizar a governança Soracloud FHE/descriptografia e
  registros de modelo, não o caminho de solicitação `ram_lfe`.
- Saúde, hidratação e execução em tempo de execução são provenientes de
  Configuração `[soracloud_runtime]` e estado confirmado, não ambiente
  alterna.