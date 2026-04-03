<!-- Auto-generated stub for Portuguese (pt) translation. Replace this content with the full translation. -->

---
lang: pt
direction: ltr
source: docs/source/soracloud/uploaded_private_models.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 97d6a421ce93a0e85be6cc99e828f965c9d8617d0ee27a772a2c9f2f646e77b7
source_last_modified: "2026-03-24T18:59:46.535846+00:00"
translation_last_reviewed: 2026-04-02
translator: machine-google-reviewed
---

# Modelos carregados pelo usuário Soracloud e tempo de execução privado

Esta nota define como o fluxo do modelo enviado pelo usuário de So Ra deve chegar ao
plano modelo Soracloud existente sem inventar um tempo de execução paralelo.

## Objetivo do projeto

Adicione um sistema de modelo carregado somente Soracloud que permite aos clientes:

- fazer upload de seus próprios repositórios de modelos;
- vincular uma versão do modelo fixada a um apartamento de agente ou equipe de arena fechada;
- executar inferência privada com entradas criptografadas e modelo/estado criptografado; e
- receber compromissos públicos, receitas, preços e trilhas de auditoria.

Este não é um recurso `ram_lfe`. `ram_lfe` continua sendo a função oculta genérica
subsistema documentado em `../universal_accounts_guide.md`. Modelo carregado
a inferência privada deveria, em vez disso, estender o registro de modelo existente do Soracloud,
superfícies de artefato, capacidade de apartamento, FHE e política de descriptografia.

## Superfícies Soracloud existentes para reutilizar

A pilha Soracloud atual já possui os objetos base corretos:-`SoraModelRegistryV1`
  - nome oficial do modelo por serviço e estado da versão promovida.
-`SoraModelWeightVersionRecordV1`
  - linhagem de versão, promoção, reversão, proveniência e reprodutibilidade
    hashes.
-`SoraModelArtifactRecordV1`
  - metadados de artefatos determinísticos já vinculados ao pipeline de modelo/peso.
-`SoraCapabilityPolicyV1.allow_model_inference`
  - sinalizador de capacidade de apartamento/serviço que deverá se tornar obrigatório para
    apartamentos vinculados aos modelos carregados.
-`SecretEnvelopeV1` e `CiphertextStateRecordV1`
  - portadoras determinísticas de bytes criptografados e de estado de texto cifrado.
-`FheParamSetV1`, `FheExecutionPolicyV1`, `FheGovernanceBundleV1`,
  `DecryptionAuthorityPolicyV1` e `DecryptionRequestV1`
  - a camada de política/governança para execução criptografada e saída controlada
    lançamento.
- rotas atuais do modelo Torii:
  -`/v1/soracloud/model/weight/{register,promote,rollback,status}`
  -`/v1/soracloud/model/artifact/{register,status}`
- rotas atuais de arrendamento compartilhado HF Torii:
  -`/v1/soracloud/hf/{deploy,status,lease/leave,lease/renew}`

O caminho do modelo carregado deve estender essas superfícies. Não deve sobrecarregar
Locações compartilhadas de HF e não deve reutilizar `ram_lfe` como um tempo de execução de serviço de modelo.

## Contrato de upload canônico

O contrato de modelo carregado privado da Soracloud deve admitir apenas canônicos
Abraçando repositórios de modelos no estilo Face:- arquivos básicos necessários:
  -`config.json`
  - arquivos tokenizadores
  - arquivos de processador/pré-processador quando a família precisar deles
  -`*.safetensors`
- admitiu grupos familiares neste marco:
  - LMs causais somente decodificadores com semântica RoPE/RMSNorm/SwiGLU/GQA
  - Modelos de texto + imagem no estilo LLaVA
  - Modelos de texto + imagem no estilo Qwen2-VL
- rejeitado neste marco:
  - GGUF como o contrato de tempo de execução privado carregado
  -ONNX
  - ativos de tokenizador/processador ausentes
  - arquiteturas/formas não suportadas
  - pacotes multimodais de áudio/vídeo

Por que este contrato:

- corresponde ao já dominante layout do modelo do lado do servidor usado pelo existente
  ecossistema em torno de safetensors e repositórios Hugging Face;
- permite que o plano do modelo compartilhe um caminho de normalização determinístico
  So Ra, Torii e compilação em tempo de execução; e
- evita confundir formatos de importação de tempo de execução local, como GGUF, com o
  Contrato de tempo de execução privado Soracloud.

As locações compartilhadas de HF continuam úteis para fluxos de trabalho de importação de fonte pública/compartilhada, mas
o caminho do modelo carregado privado armazena bytes compilados criptografados na cadeia, em vez
do que alugar bytes de modelo de um pool de origem compartilhado.

## Design de plano modelo em camadas

### 1. Proveniência e camada de registro

Estenda o design de registro do modelo atual em vez de substituí-lo:- adicione `SoraModelProvenanceKindV1::UserUpload`
  - os tipos atuais (`TrainingJob`, `HfImport`) não são suficientes para distinguir um
    modelo carregado e normalizado diretamente por um cliente como So Ra.
- mantenha `SoraModelRegistryV1` como o índice da versão promovida.
- mantenha `SoraModelWeightVersionRecordV1` como registro de linhagem/promoção/reversão.
- estender `SoraModelArtifactRecordV1` com tempo de execução privado carregado opcional
  referências:
  -`private_bundle_root`
  -`chunk_manifest_root`
  -`compile_profile_hash`
  -`privacy_mode`

O registro do artefato continua sendo a âncora determinística que liga a proveniência,
metadados de reprodutibilidade e agrupar a identidade. O recorde de peso
continua sendo o objeto de linhagem da versão promovida.

### 2. Camada de armazenamento de pacote/pedaço

Adicione registros Soracloud de primeira classe para material de modelo carregado criptografado:

-`SoraUploadedModelBundleV1`
  -`model_id`
  -`weight_version`
  -`family`
  -`modalities`
  -`runtime_format`
  -`bundle_root`
  -`chunk_count`
  -`plaintext_bytes`
  -`ciphertext_bytes`
  -`compile_profile_hash`
  -`pricing_policy`
  -`decryption_policy_ref`
-`SoraUploadedModelChunkV1`
  -`model_id`
  -`bundle_root`
  -`ordinal`
  -`offset_bytes`
  -`plaintext_len`
  -`ciphertext_len`
  -`ciphertext_hash`
  - carga útil criptografada (`SecretEnvelopeV1`)

Regras determinísticas:- os bytes de texto simples são fragmentados em pedaços fixos de 4 MiB antes da criptografia;
- a ordem dos pedaços é estrita e orientada por ordinais;
- resumos de pedaços/raiz são estáveis ​​durante a reprodução; e
- cada fragmento criptografado deve ficar abaixo do `SecretEnvelopeV1` atual
  teto de texto cifrado de bytes `33,554,432`.

Este marco armazena bytes criptografados literais em estado de cadeia por meio de pedaços
registros. Ele não descarrega bytes privados do modelo carregado para SoraFS.

Como a cadeia é pública, a confidencialidade do upload tem que vir de um verdadeiro
Chave do destinatário mantida pelo Soracloud, não de chaves determinísticas derivadas de público
metadados. A área de trabalho deve buscar um destinatário de criptografia de upload anunciado,
criptografar pedaços sob uma chave de pacote aleatória por upload e publicar apenas o
metadados do destinatário mais envelope de chave de pacote empacotado junto com o texto cifrado.

### 3. Camada de compilação/tempo de execução

Adicione uma camada de tempo de execução/compilador de transformador privado dedicado no Soracloud:- padronizar a inferência compilada determinística de baixa precisão apoiada por BFV para
  agora, porque o CKKS existe na discussão do esquema, mas não na implementação
  tempo de execução local;
- compilar modelos admitidos em um IR privado determinístico Soracloud que cobre
  embeddings, camadas lineares/de projetor, matmuls de atenção, RoPE, RMSNorm /
  Aproximações LayerNorm, blocos MLP, projeção de visão e
  caminhos do projetor de imagem para decodificador;
- usar inferência determinística de ponto fixo com:
  - pesos int8
  - ativações int16
  - acumulação int32
  - aproximações polinomiais aprovadas para não linearidades

Este compilador/tempo de execução é separado de `ram_lfe`. Pode reutilizar primitivas BFV
e objetos de governança Soracloud FHE, mas não é o mesmo mecanismo de execução
ou encaminhar família.

### 4. Camada de inferência/sessão

Adicione registros de sessões e pontos de verificação para execuções privadas:-`SoraPrivateCompileProfileV1`
  -`family`
  -`quantization`
  -`opset_version`
  -`max_context`
  -`max_images`
  -`vision_patch_policy`
  -`fhe_param_set`
  -`execution_policy`
-`SoraPrivateInferenceSessionV1`
  -`session_id`
  -`apartment`
  -`model_id`
  -`weight_version`
  -`bundle_root`
  -`input_commitments`
  -`token_budget`
  -`image_budget`
  -`status`
  -`receipt_root`
  -`xor_cost_nanos`
-`SoraPrivateInferenceCheckpointV1`
  -`session_id`
  -`step`
  -`ciphertext_state_root`
  -`receipt_hash`
  -`decrypt_request_id`
  -`released_token`
  -`compute_units`
  -`updated_at_ms`

Execução privada significa:

- entradas de prompt/imagem criptografadas;
- pesos e ativações de modelos criptografados;
- liberação explícita de política de descriptografia para saída;
- receitas públicas em tempo de execução e contabilidade de custos.

Isso não significa execução oculta, sem compromissos ou auditabilidade.

## Responsabilidades do cliente

Portanto, Ra ou outro cliente deve realizar o pré-processamento local determinístico antes
o upload chega ao Soracloud:

- aplicação tokenizadora;
- pré-processamento de imagens em tensores de patch para famílias de texto+imagem admitidas;
- normalização determinística do pacote;
- criptografia do lado do cliente de ids de token e tensores de patch de imagem.

Torii deve receber entradas criptografadas mais compromissos públicos, não prompt bruto
texto ou imagens brutas, para o caminho privado.

## Plano API e ISIMantenha as rotas de registro do modelo existente como a camada de registro canônico e adicione
novas rotas de upload/tempo de execução no topo:

-`POST /v1/soracloud/model/upload/init`
-`POST /v1/soracloud/model/upload/chunk`
-`POST /v1/soracloud/model/upload/finalize`
-`GET /v1/soracloud/model/upload/encryption-recipient`
-`POST /v1/soracloud/model/compile`
-`POST /v1/soracloud/model/allow`
-`POST /v1/soracloud/model/run-private`
-`GET /v1/soracloud/model/run-status`
-`POST /v1/soracloud/model/decrypt-output`

Apoie-os com ISIs Soracloud correspondentes:

- registro de pacote
- pedaço anexar/finalizar
- compilar admissão
- início de execução privada
- registro de ponto de verificação
- liberação de saída

O fluxo deve ser:

1. upload/init estabelece a sessão do pacote determinístico e a raiz esperada;
2. upload/chunk anexa fragmentos criptografados em ordem ordinal;
3. upload/finalize sela a raiz e o manifesto do pacote;
4. compile produz um perfil de compilação privado determinístico vinculado ao
   pacote admitido;
5. registros de registro de modelo/artefato + modelo/peso fazem referência ao pacote carregado
   em vez de apenas um trabalho de formação;
6. permitir-modelo vincula o modelo carregado a um apartamento que já admite
   `allow_model_inference`;
7. run-private registra uma sessão e emite pontos de verificação/recibos;
8. Liberações de saída descriptografadas controlam o material de saída.

## Política de preços e plano de controle

Estenda o comportamento atual do plano de carregamento/controle do Soracloud:- `allow_model_inference` é necessário para apartamentos com modelos carregados;
- armazenamento de preços, compilação, etapas de tempo de execução e liberação de descriptografia em XOR;
- manter a propagação narrativa desativada para execuções de modelos carregados neste marco;
- manter os modelos carregados dentro da arena fechada de So Ra e dos fluxos controlados para exportação.

## Autorização e semântica de ligação

Carregar, compilar e executar são recursos separados e devem permanecer separados em
o plano modelo.

- o upload de um pacote de modelo não deve autorizar implicitamente um apartamento a executá-lo;
- o sucesso da compilação não deve promover implicitamente uma versão do modelo para atual;
- a ligação de apartamento deve ser explícita por meio de uma mutação no estilo `allow-model`
  que registra:
  - apartamento,
  - identificação do modelo,
  - versão de peso,
  - raiz do pacote,
  - modo de privacidade,
  - sequência de signatário/auditoria;
- apartamentos vinculados a modelos carregados já devem admitir
  `allow_model_inference`;
- as rotas de mutação devem continuar a exigir a mesma solicitação assinada do Soracloud
  disciplina usada pelas rotas de modelo/artefato/treinamento existentes e deve ser
  protegido por `CanManageSoracloud` ou uma autoridade delegada igualmente explícita
  modelo.

Isso evita "Eu carreguei, portanto, todos os apartamentos privados podem executá-lo"
deriva e mantém a política de execução do apartamento explícita.

## Status e modelo de auditoria

Os novos registros precisam de superfícies autorizadas de leitura e auditoria, não apenas de mutação
rotas.

Adições recomendadas:- status de upload
  - consulta por `service_name + model_name + weight_version` ou por
    `model_id + bundle_root`;
- status de compilação
  - consulta por `model_id + bundle_root + compile_profile_hash`;
- status de execução privada
  - consulta por `session_id`, com contexto apartamento/modelo/versão incluído no
    resposta;
- status de saída de descriptografia
  - consulta por `decrypt_request_id`.

A auditoria deve permanecer na sequência global existente do Soracloud, em vez de
criando um segundo contador por recurso. Adicione eventos de auditoria de primeira classe para:

- carregar init/finalizar
- pedaço anexar / selar
- compilação admitida / compilação rejeitada
- modelo de apartamento permitir/revogar
- início/ponto de verificação/conclusão/falha de execução privada
- liberação/negação de saída

Isso mantém a atividade do modelo carregado visível no mesmo replay oficial e
história de operações como o serviço atual, treinamento, peso do modelo, artefato do modelo,
Fluxos de auditoria de arrendamento compartilhado e apartamentos em HF.

## Cotas de admissão e limites de crescimento estadual

Bytes de modelo criptografados literais na cadeia são viáveis apenas se a admissão for limitada
agressivamente.

A implementação deverá definir limites determinísticos para pelo menos:

- máximo de bytes de texto simples por pacote carregado;
- máximo de bytes criptografados por pacote;
- contagem máxima de pedaços por pacote;
- máximo de sessões simultâneas de upload em voo por autoridade/serviço;
- máximo de jobs de compilação por janela de serviço/apartamento;
- contagem máxima de pontos de verificação retidos por sessão privada;
- máximo de solicitações de liberação de saída por sessão.Torii e core devem rejeitar uploads que excedam os limites declarados antes
ocorre amplificação do estado. Os limites devem ser orientados pela configuração onde
apropriado, mas os resultados da validação devem permanecer determinísticos entre pares.

## Replay e determinismo do compilador

O caminho do compilador/tempo de execução privado tem uma carga de determinismo maior do que um caminho normal
implantação do serviço.

Invariantes necessários:

- a detecção e normalização da família devem produzir um pacote canônico estável
  antes que qualquer hash de compilação seja emitido;
- compilar hashes de perfil deve vincular:
  - raiz do pacote normalizada,
  - família,
  - receita de quantização,
  - versão opset,
  - Conjunto de parâmetros FHE,
  - política de execução;
- o tempo de execução deve evitar kernels não determinísticos, desvio de ponto flutuante e
  reduções específicas de hardware que podem alterar as saídas ou receitas em todo
  pares.

Antes de dimensionar o conjunto familiar admitido, instale um pequeno dispositivo determinístico para
cada classe de família e bloqueia saídas de compilação mais recibos de tempo de execução com ouro
testes.

## Lacunas de design restantes antes do código

As maiores questões de implementação não resolvidas estão agora reduzidas a questões concretas
decisões de back-end:- formatos DTO exatos de upload/pedaço/solicitação e esquemas Norito;
- chaves de indexação de estado mundial para pesquisas de pacote/pedaço/sessão/ponto de verificação;
- colocação de cota/configuração padrão em `iroha_config`;
- se o status do modelo/artefato deve se tornar orientado à versão em vez de
  orientado para o trabalho de treinamento quando `UserUpload` está presente;
- o comportamento preciso de revogação quando um apartamento perde
  `allow_model_inference` ou uma versão de modelo fixada é revertida.

Esses são os próximos itens da ponte de design para código. A colocação arquitetônica de
o recurso agora deve estar estável.

## Matriz de teste- validação de upload:
  - aceitar repositórios canônicos de tensores de HF
  - rejeitar GGUF, ONNX, ativos de tokenizador/processador ausentes, sem suporte
    arquiteturas e pacotes multimodais de áudio/vídeo
- fragmentação:
  - raízes determinísticas do pacote
  - ordenação estável de pedaços
  - reconstrução exata
  - aplicação do teto do envelope
- consistência do registro:
  - correção da promoção de pacote/pedaço/artefato/peso no replay
- compilador:
  - um pequeno acessório para cada decodificador, estilo LLaVA e estilo Qwen2-VL
  - rejeição de operações e formas não suportadas
- tempo de execução privado:
  - teste de fumaça criptografado de ponta a ponta com recibos estáveis ​​e
    liberação de saída de limite
- preços:
  - Cobranças XOR para upload, compilação, etapas de tempo de execução e descriptografia
- Integração So Ra:
  - fazer upload, compilar, publicar, vincular-se à equipe, executar arena fechada, inspecionar recibos,
    salvar projeto, reabrir, executar novamente deterministicamente
- segurança:
  - sem desvio do portão de exportação
  - sem autopropagação narrativa
  - a ligação do apartamento falha sem `allow_model_inference`

## Fatias de implementação1. Adicione os campos do modelo de dados ausentes e os novos tipos de registro.
2. Adicione os novos tipos de solicitação/resposta Torii e manipuladores de rota.
3. Adicione ISIs Soracloud correspondentes e armazenamento de estado mundial.
4. Adicionar validação determinística de pacote/bloco e byte criptografado em cadeia
   armazenamento.
5. Adicione um pequeno caminho de tempo de execução/fixação de transformador privado apoiado por BFV.
6. Estenda os comandos do modelo CLI para cobrir fluxos de upload/compilação/execução privada.
7. Faça a integração do So Ra assim que o caminho de back-end for oficial.