<!-- Auto-generated stub for Portuguese (pt) translation. Replace this content with the full translation. -->

---
lang: pt
direction: ltr
source: security_best_practices_report.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 6055090fbfab93113392610b3eb78ef0a0e97d6fef4a94292c5609ec5562e925
source_last_modified: "2026-03-26T06:12:11.487656+00:00"
translation_last_reviewed: 2026-04-02
translator: machine-google-reviewed
---

# Relatório de práticas recomendadas de segurança

Data: 25/03/2026

## Resumo Executivo

Atualizei o relatório Torii/Soracloud anterior em relação ao espaço de trabalho atual
código e estendeu a revisão ao servidor de maior risco, SDK e
superfícies de criptografia/serialização. Esta auditoria confirmou inicialmente três
problemas de entrada/autenticação: dois de alta gravidade e um de média gravidade.
Essas três descobertas estão agora fechadas na árvore atual pela correção
descrito abaixo. Transporte de acompanhamento e revisão de despacho interno confirmados
nove problemas adicionais de gravidade média: uma ligação de identidade P2P de saída
lacuna, um padrão de downgrade TLS P2P de saída, dois bugs de limite de confiança Torii em
entrega de webhook e despacho interno MCP, um transporte sensível entre SDK
lacuna nos clientes Swift, Java/Android, Kotlin e JS, um SoraFS
lacuna de política de proxy confiável/IP do cliente, um proxy local SoraFS
lacuna de ligação/autenticação, um fallback de texto simples de pesquisa geográfica de telemetria de pares,
e uma lacuna de bloqueio/codificação de taxa com falha de abertura de IP remoto de autenticação do operador. Aqueles
as descobertas posteriores também são fechadas na árvore atual.As quatro descobertas relatadas anteriormente pela Soracloud sobre chaves privadas brutas em
HTTP, execução de proxy de leitura local somente interna, tempo de execução público ilimitado
fallback e locação de anexo de IP remoto não estão mais ativos no código atual.
Eles estão marcados como fechados/substituídos abaixo com referências de código atualizadas.Esta continuou sendo uma auditoria centrada no código, em vez de um exercício exaustivo da equipe vermelha.
Priorizei a entrada Torii acessível externamente e os caminhos de solicitação de autenticação e, em seguida,
verificado IVM, `iroha_crypto`, `norito`, o Swift/Android/JS SDK
auxiliares de assinatura de solicitação, o caminho geográfico de telemetria de pares e o SoraFS
auxiliares de proxy de estação de trabalho mais a política de IP do cliente pin/gateway SoraFS
superfícies. Nenhum problema confirmado ao vivo dessa entrada/autenticação, política de saída,
geotelemetria peer, padrões de transporte P2P amostrados, despacho MCP, SDK amostrado
transporte, bloqueio de autenticação do operador/chave de taxa, proxy confiável/IP do cliente SoraFS
política ou fatia de proxy local permanece após as correções neste relatório.
O reforço de acompanhamento também expandiu os conjuntos verdadeiros de inicialização com falha fechada para
caminhos de acelerador IVM CUDA/Metal amostrados; esse trabalho não confirmou um novo
problema de falha aberta. O metal amostrado Ed25519
o caminho da assinatura agora é restaurado neste host após corrigir vários desvios de ref10
pontos nas portas Metal/CUDA: tratamento de ponto base positivo na verificação,
a constante `d2`, o caminho exato de redução `fe_sq2`, o final perdido
Etapa de transporte `fe_mul` e a normalização de campo pós-operatória ausente que permitiu ao membro
os limites flutuam na escada escalar. Cobertura de regressão de metal focada agora
mantém o pipeline de assinatura habilitado e verifica `[true, false]` noacelerador em relação ao caminho de referência da CPU. A amostra da verdade da inicialização agora
também teste diretamente o vetor ativo (`vadd64`, `vand`, `vxor`, `vor`) e
kernels em lote AES de rodada única em Metal e CUDA antes desses back-ends
permaneça habilitado. A verificação de dependência posterior adicionou sete descobertas de terceiros ao vivo
para o backlog, mas a árvore atual removeu ambos os `tar` ativos
avisos eliminando a dependência `xtask` Rust `tar` e substituindo o
`iroha_crypto` `libsodium-sys-stable` testes de interoperabilidade com suporte OpenSSL
equivalentes. A árvore atual também substituiu as dependências diretas do PQ
sinalizado nessa varredura, migrando `soranet_pq`, `iroha_crypto` e `ivm` de
`pqcrypto-dilithium` / `pqcrypto-kyber` para
`pqcrypto-mldsa` / `pqcrypto-mlkem` preservando o ML-DSA /
Superfície da API ML-KEM. Uma passagem de dependência posterior no mesmo dia fixou o espaço de trabalho
Versões `reqwest` / `rustls` para versões de patch corrigidas, o que mantém
`rustls-webpki` na linha fixa `0.103.10` na resolução atual. O único
as exceções restantes da política de dependência são os dois transitivos não mantidos
macro caixas (`derivative`, `paste`), que agora são explicitamente aceitas em
`deny.toml` porque não há atualização segura e removê-los exigiria
substituindo ou vendendo várias pilhas upstream. Oo trabalho restante do acelerador é
validação em tempo de execução da correção CUDA espelhada e da verdade CUDA expandida definida em
um host com suporte ao driver CUDA ao vivo, não uma correção confirmada ou falha na abertura
problema na árvore atual.

## Alta Gravidade

### SEC-05: A verificação de solicitação canônica do aplicativo ignorou os limites multisig (fechado em 24/03/2026)

Impacto:

- Qualquer chave de membro único de uma conta controlada por multisig pode autorizar
  solicitações voltadas para aplicativos que deveriam exigir um limite ou ponderação
  quórum.
- Isso afeta todos os endpoints que confiam em `verify_canonical_request`, incluindo
  Soracloud assinou entrada de mutação, acesso a conteúdo e conta assinada ZK
  locação de anexo.

Evidência:

- `verify_canonical_request` expande um controlador multisig para o membro completo
  lista de chaves públicas e aceita a primeira chave que verifica a solicitação
  assinatura, sem avaliação de limite ou peso acumulado:
  `crates/iroha_torii/src/app_auth.rs:198-210`.
- O modelo de política multisig real carrega um `threshold` e um ponderado
  membros e rejeita políticas cujo limite exceda o peso total:
  `crates/iroha_data_model/src/account/controller.rs:92-95`,
  `crates/iroha_data_model/src/account/controller.rs:163-178`,
  `crates/iroha_data_model/src/account/controller.rs:188-196`.
- O auxiliar está no caminho de autorização para entrada de mutação Soracloud em
  `crates/iroha_torii/src/lib.rs:2141-2157`, acesso à conta assinada de conteúdo em
  `crates/iroha_torii/src/content.rs:359-360` e locação de anexo em
  `crates/iroha_torii/src/lib.rs:7962-7968`.

Por que isso é importante:- O assinante da solicitação é tratado como autoridade da conta para admissão HTTP,
  mas a implementação rebaixa silenciosamente as contas multisig para "qualquer
  membro pode agir sozinho."
- Isso transforma uma camada de assinatura HTTP de defesa profunda em uma autorização
  bypass para contas protegidas por multisig.

Recomendação:

- Rejeite contas controladas por multisig na camada de autenticação do aplicativo até que um
  exista um formato de testemunha adequado ou estenda o protocolo para que a solicitação HTTP
  carrega e verifica um conjunto completo de testemunhas multisig que satisfaz o limite e
  peso.
- Adicionar regressões cobrindo middleware de mutação Soracloud, autenticação de conteúdo e ZK
  anexos para assinaturas multisig abaixo do limite.

Status de correção:

- Fechado no código atual por falha no fechamento de contas controladas por multisig em
  `crates/iroha_torii/src/app_auth.rs`.
- O verificador não aceita mais a semântica "qualquer membro pode assinar" para
  autorização HTTP multisig; solicitações multisig são rejeitadas até que um
  existe um formato de testemunha que satisfaz o limite.
- A cobertura de regressão agora inclui um caso de rejeição multisig dedicado em
  `crates/iroha_torii/src/app_auth.rs`.

## Alta Gravidade

### SEC-06: As assinaturas de solicitação canônica do aplicativo podiam ser reproduzidas indefinidamente (fechado em 24/03/2026)

Impacto:- Uma solicitação válida capturada pode ser reproduzida porque a mensagem assinada não possui
  carimbo de data/hora, nonce, expiração ou cache de repetição.
- Isso pode repetir solicitações de mutação Soracloud que alteram o estado e reemitir
  operações de conteúdo/anexo vinculados à conta muito depois do cliente original
  os pretendia.

Evidência:

- Torii define a solicitação canônica do aplicativo como apenas
  `METHOD + path + sorted query + body hash` em
  `crates/iroha_torii/src/app_auth.rs:1-17` e
  `crates/iroha_torii/src/app_auth.rs:74-89`.
- O verificador aceita apenas `X-Iroha-Account` e `X-Iroha-Signature` e não
  não impor atualização ou manter um cache de repetição:
  `crates/iroha_torii/src/app_auth.rs:137-218`.
- Os auxiliares JS, Swift e Android SDK geram o mesmo cabeçalho propenso a reprodução
  par sem campos nonce/timestamp:
  `javascript/iroha_js/src/canonicalRequest.js:50-82`,
  `IrohaSwift/Sources/IrohaSwift/CanonicalRequest.swift:41-68` e
  `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/CanonicalRequestSigner.java:67-106`.
- O caminho de assinatura do operador do Torii já usa o padrão mais forte que
  falta o caminho voltado para o aplicativo: carimbo de data/hora, nonce e cache de repetição em
  `crates/iroha_torii/src/operator_signatures.rs:1-21` e
  `crates/iroha_torii/src/operator_signatures.rs:266-294`.

Por que isso é importante:

- HTTPS por si só não impede a reprodução por um proxy reverso, registrador de depuração,
  host do cliente comprometido ou qualquer intermediário que possa registrar solicitações válidas.
- Como o mesmo esquema é implementado em todos os principais SDKs de clientes, a reprodução
  a fraqueza é sistêmica e não apenas do servidor.

Recomendação:- Adicione conteúdo de atualização assinado às solicitações de autenticação do aplicativo, com no mínimo um carimbo de data/hora
  e nonce, e rejeitar tuplas obsoletas ou reutilizadas com um cache de repetição limitado.
- Versão do formato de solicitação canônica do aplicativo explicitamente para que Torii e os SDKs possam
  descontinuar o antigo esquema de dois cabeçalhos com segurança.
- Adicionar regressões provando rejeição de repetição para mutações e conteúdo do Soracloud
  acesso e anexo CRUD.

Status de correção:

- Fechado no código atual. Torii agora requer o esquema de quatro cabeçalhos
  (`X-Iroha-Account`, `X-Iroha-Signature`, `X-Iroha-Timestamp-Ms`,
  `X-Iroha-Nonce`) e assina/verifica
  `METHOD + path + sorted query + body hash + timestamp + nonce` em
  `crates/iroha_torii/src/app_auth.rs`.
- A validação de atualização agora impõe uma janela limitada de inclinação do relógio, valida
  formato de nonce e rejeita nonces reutilizados com um cache de repetição na memória cujo
  os botões são revestidos através de `crates/iroha_config/src/parameters/{defaults,actual,user}.rs`.
- Os auxiliares JS, Swift e Android agora emitem o mesmo formato de quatro cabeçalhos em
  `javascript/iroha_js/src/canonicalRequest.js`,
  `IrohaSwift/Sources/IrohaSwift/CanonicalRequest.swift` e
  `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/CanonicalRequestSigner.java`.
- A cobertura de regressão agora inclui verificação de assinatura positiva e repetição,
  carimbo de data/hora obsoleto e casos de rejeição de atualização ausente em
  `crates/iroha_torii/src/app_auth.rs`.

## Gravidade Média

### SEC-07: a aplicação de mTLS confiou em um cabeçalho encaminhado falsificado (fechado em 24/03/2026)

Impacto:- As implantações que dependem de `require_mtls` podem ser ignoradas se Torii for diretamente
  alcançável ou o proxy frontal não remove os dados fornecidos pelo cliente
  `x-forwarded-client-cert`.
- O problema depende da configuração, mas quando acionado torna-se reivindicado
  requisito de certificado do cliente em uma verificação de cabeçalho simples.

Evidência:

- O gate Norito-RPC impõe `require_mtls` chamando
  `norito_rpc_mtls_present`, que apenas verifica se
  `x-forwarded-client-cert` existe e não está vazio:
  `crates/iroha_torii/src/lib.rs:1897-1926`.
- Os fluxos de bootstrap/login de autenticação do operador chamam `check_common`, que rejeita apenas
  quando `mtls_present(headers)` é falso:
  `crates/iroha_torii/src/operator_auth.rs:562-570`.
- `mtls_present` também é apenas um check-in `x-forwarded-client-cert` não vazio
  `crates/iroha_torii/src/operator_auth.rs:1212-1216`.
- Esses manipuladores de autenticação do operador ainda estão expostos como rotas em
  `crates/iroha_torii/src/lib.rs:16658-16672`.

Por que isso é importante:

- Uma convenção de cabeçalho encaminhado só é confiável quando Torii fica atrás de um
  proxy reforçado que remove e reescreve o cabeçalho. O código não verifica
  essa suposição de implantação em si.
- Os controles de segurança que dependem silenciosamente da higiene do proxy reverso são fáceis de implementar
  configuração incorreta durante alterações de roteamento de preparação, canário ou resposta a incidentes.

Recomendação:- Prefira a aplicação direta do estado de transporte sempre que possível. Se um proxy deve ser
  usado, confie em um canal proxy para Torii autenticado e exija uma lista de permissões
  ou atestado assinado desse proxy em vez da presença de cabeçalho bruto.
- Documente que `require_mtls` não é seguro em ouvintes Torii diretamente expostos.
- Adicionar testes negativos para entrada `x-forwarded-client-cert` forjada em Norito-RPC
  e rotas de bootstrap de autenticação do operador.

Status de correção:

- Fechado no código atual vinculando a confiança do cabeçalho encaminhado ao proxy configurado
  CIDRs em vez da presença de cabeçalho bruto apenas.
- `crates/iroha_torii/src/limits.rs` agora fornece o compartilhamento
  Porta `has_trusted_forwarded_header(...)` e ambas Norito-RPC
  (`crates/iroha_torii/src/lib.rs`) e autenticação do operador
  (`crates/iroha_torii/src/operator_auth.rs`) use-o com o par TCP do chamador
  endereço.
- `iroha_config` agora expõe `mtls_trusted_proxy_cidrs` para ambos
  autenticação do operador e Norito-RPC; os padrões são apenas loopback.
- A cobertura de regressão agora rejeita a entrada `x-forwarded-client-cert` forjada de
  um controle remoto não confiável tanto no operador-auth quanto no auxiliar de limites compartilhados.

## Gravidade Média

### SEC-08: As discagens P2P de saída não vincularam a chave autenticada ao ID do peer pretendido (fechado em 25/03/2026)

Impacto:- Uma discagem de saída para o peer `X` pode ser concluída como qualquer outro peer `Y` cuja chave
  assinou com sucesso o handshake da camada de aplicativo, porque o handshake
  autenticou "uma chave nesta conexão", mas nunca verificou se a chave era
  o ID do par que o ator da rede pretendia alcançar.
- Em sobreposições permitidas, as verificações posteriores de topologia/lista de permissões ainda descartam o
  chave errada, então este foi principalmente um bug de substituição/acessibilidade, em vez
  do que um bug de representação de consenso direto. Em sobreposições públicas, poderia deixar um
  endereço comprometido, resposta DNS ou endpoint de retransmissão substituem um diferente
  identidade do observador em uma discagem de saída.

Evidência:

- O estado do peer de saída armazena o `peer_id` pretendido em
  `crates/iroha_p2p/src/peer.rs:5153-5179`, mas o antigo fluxo de handshake
  eliminou esse valor antes da verificação da assinatura.
- `GetKey::read_their_public_key` verificou a carga útil do handshake assinado e
  em seguida, construiu imediatamente um `Peer` a partir da chave pública remota anunciada
  em `crates/iroha_p2p/src/peer.rs:6266-6355`, sem comparação com o
  `peer_id` fornecido originalmente para `connecting(...)`.
- A mesma pilha de transporte desativa explicitamente o certificado TLS/QUIC
  verificação para P2P em `crates/iroha_p2p/src/transport.rs`, vinculando assim o
  chave autenticada na camada de aplicação para o ID do par pretendido é a chave crítica
  verificação de identidade em conexões de saída.

Por que isso é importante:- O design empurra intencionalmente a autenticação de pares acima do transporte
  camada, que faz com que a chave de handshake verifique a única ligação de identidade durável
  em discagens de saída.
- Sem essa verificação, a camada de rede poderia tratar silenciosamente "com sucesso
  autenticou algum par" como equivalente a "alcançou o par que discamos",
  que é uma garantia mais fraca e pode distorcer o estado da topologia/reputação.

Recomendação:

- Transportar o `peer_id` de saída pretendido através dos estágios de handshake assinado e
  falha no fechamento se a chave remota verificada não corresponder a ela.
- Mantenha uma regressão focada que comprove um aperto de mão assinado de forma válida pelo
  a chave errada é rejeitada enquanto um handshake normal assinado ainda é bem-sucedido.

Status de correção:

- Fechado no código atual. `ConnectedTo` e os estados de handshake downstream agora
  transportar o `PeerId` de saída esperado e
  `GetKey::read_their_public_key` rejeita uma chave autenticada incompatível com
  `HandshakePeerMismatch` em `crates/iroha_p2p/src/peer.rs`.
- A cobertura de regressão focada agora inclui
  `outgoing_handshake_rejects_unexpected_peer_identity` e o existente
  caminho `handshake_v1_defaults_to_trust_gossip` positivo em
  `crates/iroha_p2p/src/peer.rs`.

### SEC-09: A entrega de webhook HTTPS/WSS resolveu novamente os nomes de host verificados no momento da conexão (fechado em 25/03/2026)

Impacto:- Respostas DNS de destino validadas com entrega segura de webhook em relação ao webhook
  política de saída, mas depois descartou esses endereços verificados e deixou o cliente
  stack resolve o nome do host novamente durante a conexão HTTPS ou WSS real.
- Um invasor que possa influenciar o DNS entre a validação e o tempo de conexão poderá
  potencialmente religar um nome de host previamente permitido a um nome de host privado ou bloqueado
  destino somente do operador e ignorar o protetor de webhook baseado em CIDR.

Evidência:

- O protetor de saída resolve e filtra endereços de destino candidatos em
  `crates/iroha_torii/src/webhook.rs:1746-1829` e os caminhos de entrega seguros
  passe essas listas de endereços verificadas para os auxiliares HTTPS/WSS.
- O antigo auxiliar HTTPS construiu um cliente genérico no URL original
  host em `crates/iroha_torii/src/webhook.rs` e não vinculou a conexão
  para o conjunto de endereços verificado, o que significa que a resolução de DNS aconteceu novamente dentro
  o cliente HTTP.
- O antigo auxiliar WSS também chamado `tokio_tungstenite::connect_async(url)`
  contra o nome do host original, que também resolveu novamente o host em vez de
  reutilizando o endereço já aprovado.

Por que isso é importante:

- As listas de permissões de destino só funcionam se o endereço verificado for aquele
  o cliente realmente se conecta.
- A nova resolução após a aprovação da política cria uma lacuna de religação de DNS/TOCTOU em um
  caminho em que os operadores provavelmente confiarão para contenção no estilo SSRF.

Recomendação:- Fixe respostas DNS verificadas no caminho de conexão HTTPS real, preservando
  o nome do host original para validação de SNI/certificado.
- Para WSS, conecte o soquete TCP diretamente a um endereço verificado e execute o TLS
  handshake de websocket sobre esse fluxo em vez de chamar um nome de host baseado
  conector de conveniência.

Status de correção:

- Fechado no código atual. `crates/iroha_torii/src/webhook.rs` agora deriva
  `https_delivery_dns_override(...)` e
  `websocket_pinned_connect_addr(...)` do conjunto de endereços verificado.
- A entrega HTTPS agora usa `reqwest::Client::builder().resolve_to_addrs(...)`
  então o nome do host original permanece visível para TLS enquanto a conexão TCP é
  fixado nos endereços já aprovados.
- A entrega WSS agora abre um `TcpStream` bruto para um endereço verificado e executa
  `tokio_tungstenite::client_async_tls_with_config(...)` nesse fluxo,
  o que evita uma segunda pesquisa de DNS após a validação da política.
- A cobertura de regressão agora inclui
  `https_delivery_dns_override_pins_vetted_domain_addresses`,
  `https_delivery_dns_override_skips_ip_literals` e
  `websocket_pinned_connect_addr_pins_secure_delivery_when_guarded` em
  `crates/iroha_torii/src/webhook.rs`.

### SEC-10: loopback carimbado de despacho de rota interna MCP e privilégio de lista de permissões herdado (fechado em 25/03/2026)

Impacto:- Quando o Torii MCP foi habilitado, o envio da ferramenta interna reescreveu cada solicitação como
  loopback independentemente do chamador real. Rotas que confiam no CIDR do chamador para
  privilégio ou desvio de limitação poderiam, portanto, ver o tráfego MCP como
  `127.0.0.1`.
- O problema foi controlado pela configuração porque o MCP está desabilitado por padrão e o
  as rotas afetadas ainda dependem de uma lista de permissões ou confiança de loopback semelhante
  política, mas transformou o MCP em uma ponte de escalada de privilégios, uma vez que aqueles
  botões foram habilitados juntos.

Evidência:

- `dispatch_route(...)` em `crates/iroha_torii/src/mcp.rs` inserido anteriormente
  `x-iroha-remote-addr: 127.0.0.1` e loopback sintético `ConnectInfo` para
  cada solicitação despachada internamente.
- `iroha.parameters.get` é exposto na superfície do MCP em modo somente leitura e
  `/v1/parameters` ignora a autenticação normal quando o IP do chamador pertence ao
  lista de permissões configurada em `crates/iroha_torii/src/lib.rs:5879-5888`.
- `apply_extra_headers(...)` também aceitou entradas arbitrárias `headers` de
  o chamador MCP, portanto, cabeçalhos de confiança internos reservados, como
  `x-iroha-remote-addr` e `x-forwarded-client-cert` não foram explicitamente
  protegido.

Por que isso é importante:- As camadas de ponte internas devem preservar o limite de confiança original. Substituindo
  o chamador real com loopback trata efetivamente cada chamador MCP como um
  cliente interno assim que a solicitação cruzar a ponte.
- O bug é sutil porque o perfil MCP visível externamente ainda pode parecer
  somente leitura enquanto a rota HTTP interna vê uma origem mais privilegiada.

Recomendação:

- Preservar o IP do chamador que a solicitação externa `/v1/mcp` já recebeu
  do middleware de endereço remoto do Torii e sintetizar `ConnectInfo` a partir de
  esse valor em vez de loopback.
- Trate cabeçalhos de confiança somente de entrada, como `x-iroha-remote-addr` e
  `x-forwarded-client-cert` como cabeçalhos internos reservados para que os chamadores MCP não possam
  contrabandear ou substituí-los por meio do argumento `headers`.

Status de correção:

- Fechado no código atual. `crates/iroha_torii/src/mcp.rs` agora deriva o
  IP remoto despachado internamente do injetado da solicitação externa
  Cabeçalho `x-iroha-remote-addr` e sintetiza `ConnectInfo` daquele real
  IP do chamador em vez de loopback.
- `apply_extra_headers(...)` agora descarta `x-iroha-remote-addr` e
  `x-forwarded-client-cert` como cabeçalhos internos reservados, então chamadores MCP
  não é possível falsificar a confiança de loopback/proxy de entrada por meio de argumentos de ferramenta.
- A cobertura de regressão agora inclui
  `dispatch_route_preserves_inbound_remote_addr_for_internal_allowlist_checks`
  `dispatch_route_blocks_remote_addr_spoofing_from_extra_headers` e
  `apply_extra_headers_blocks_reserved_internal_headers` em
  `crates/iroha_torii/src/mcp.rs`.### SEC-11: clientes SDK permitiram material de solicitação confidencial em transportes inseguros ou entre hosts (fechado em 25/03/2026)

Impacto:

- Os clientes Swift, Java/Android, Kotlin e JS de amostra não
  trate consistentemente todas as formas de solicitação confidenciais como sensíveis ao transporte.
  Dependendo do auxiliar, os chamadores podem enviar cabeçalhos de portador/token de API, brutos
  Campos JSON `private_key*` ou material de assinatura de autenticação canônica do aplicativo sobre
  simples `http` / `ws` ou por meio de substituições de URL absolutas entre hosts.
- Especificamente no cliente JS, os cabeçalhos `canonicalAuth` foram adicionados após
  `_request(...)` concluiu suas verificações de transporte e `private_key` somente corpo
  JSON não contava como transporte confidencial.

Evidência:- Swift agora centraliza a guarda em
  `IrohaSwift/Sources/IrohaSwift/TransportSecurity.swift` e aplica-o a partir de
  `IrohaSwift/Sources/IrohaSwift/NoritoRpcClient.swift`,
  `IrohaSwift/Sources/IrohaSwift/ToriiClient.swift` e
  `IrohaSwift/Sources/IrohaSwift/ConnectClient.swift`; antes desta passagem
  esses ajudantes não partilhavam um único portão de política de transportes.
- Java/Android agora centraliza a mesma política em
  `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/TransportSecurity.java`
  e aplica-o de `NoritoRpcClient.java`, `ToriiRequestBuilder.java`,
  `OfflineToriiClient.java`, `SubscriptionToriiClient.java`,
  `stream/ToriiEventStreamClient.java` e
  `websocket/ToriiWebSocketClient.java`.
- Kotlin agora reflete essa política em
  `kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/client/TransportSecurity.kt`
  e aplica-o a partir do cliente JVM correspondente/request-builder/event-stream /
  superfícies de websocket.
- JS `ToriiClient._request(...)` agora trata `canonicalAuth` mais corpos JSON
  contendo campos `private_key*` como material de transporte sensível em
  `javascript/iroha_js/src/toriiClient.js` e o formato do evento de telemetria em
  `javascript/iroha_js/index.d.ts` agora registra `hasSensitiveBody` /
  `hasCanonicalAuth` quando `allowInsecure` é usado.

Por que isso é importante:

- Ajudantes de desenvolvimento móvel, de navegador e local geralmente apontam para desenvolvimento/preparação mutável
  URLs básicos. Se o cliente não fixar solicitações confidenciais no configurado
  esquema/host, uma substituição de URL absoluta de conveniência ou uma base HTTP simples pode
  transformar o SDK em um caminho de downgrade de exfiltração secreta ou de solicitação assinada.
- O risco é mais amplo do que os tokens ao portador. JSON bruto `private_key` e novo
  assinaturas de autenticação canônica também são sensíveis à segurança na transmissão e
  não deverá ignorar silenciosamente a política de transportes.

Recomendação:- Centralize a validação de transporte em cada SDK e aplique-a antes da E/S da rede
  para todos os formatos de solicitação confidenciais: cabeçalhos de autenticação, assinatura de autenticação canônica do aplicativo,
  e JSON `private_key*` bruto.
- Mantenha `allowInsecure` apenas como uma saída de escape local/dev explícita e emita
  telemetria quando os chamadores optam por ela.
- Adicione regressões focadas nos construtores de solicitações compartilhadas, em vez de apenas
  métodos de conveniência de nível superior, para que os futuros ajudantes herdem a mesma guarda.

Status de correção:- Fechado no código atual. A amostra de Swift, Java/Android, Kotlin e JS
  os clientes agora rejeitam transportes inseguros ou entre hosts para os dados confidenciais
  solicitar formas acima, a menos que o chamador opte pelo documento somente para desenvolvedores
  modo inseguro.
- As regressões focadas no Swift agora cobrem cabeçalhos de autenticação Norito-RPC inseguros,
  transporte de websocket Connect inseguro e solicitação raw-`private_key` Torii
  corpos.
- As regressões focadas em Kotlin agora cobrem cabeçalhos de autenticação Norito-RPC inseguros,
  corpos `private_key` off-line/assinatura, cabeçalhos de autenticação SSE e websocket
  cabeçalhos de autenticação.
- Regressões focadas em Java/Android agora cobrem autenticação Norito-RPC insegura
  cabeçalhos, corpos `private_key` off-line/assinatura, cabeçalhos de autenticação SSE e
  cabeçalhos de autenticação do websocket por meio do chicote Gradle compartilhado.
- Regressões focadas em JS agora cobrem hosts inseguros e cruzados
  Solicitações `private_key`-body mais solicitações `canonicalAuth` inseguras em
  `javascript/iroha_js/test/transportSecurity.test.js`, enquanto
  `javascript/iroha_js/test/toriiCanonicalAuth.test.js` agora executa o positivo
  caminho canonical-auth em uma URL base segura.

### SEC-12: O proxy QUIC local SoraFS aceitou ligações sem loopback sem autenticação do cliente (fechado em 25/03/2026)

Impacto:- `LocalQuicProxyConfig.bind_addr` poderia anteriormente ser definido como `0.0.0.0`, um
  IP da LAN, ou qualquer outro endereço sem loopback, que expôs o "local"
  proxy da estação de trabalho como um ouvinte QUIC acessível remotamente.
- Esse ouvinte não autenticou clientes. Qualquer par acessível que possa
  concluir a sessão QUIC/TLS e enviar um handshake de versão correspondente poderia
  em seguida, abra os fluxos `tcp`, `norito`, `car` ou `kaigi`, dependendo de quais
  modos de ponte foram configurados.
- No modo `bridge`, isso transformou a configuração incorreta do operador em um TCP remoto
  superfície de retransmissão e streaming de arquivo local na estação de trabalho do operador.

Evidência:

-`LocalQuicProxyConfig::parsed_bind_addr(...)` em
  `crates/sorafs_orchestrator/src/proxy.rs` anteriormente analisava apenas o soquete
  endereço e não rejeitou interfaces sem loopback.
- `spawn_local_quic_proxy(...)` no mesmo arquivo inicia um servidor QUIC com um
  certificado autoassinado e `.with_no_client_auth()`.
- `handle_connection(...)` aceitou qualquer cliente cujo `ProxyHandshakeV1`
  versão correspondeu à versão do protocolo único suportado e, em seguida, inseriu o
  loop de fluxo do aplicativo.
- `handle_tcp_stream(...)` disca valores `authority` arbitrários via
  `TcpStream::connect(...)`, enquanto `handle_norito_stream(...)`,
  `handle_car_stream(...)` e `handle_kaigi_stream(...)` transmitem arquivos locais
  dos diretórios de spool/cache configurados.

Por que isso é importante:- Um certificado autoassinado protege a identidade do servidor somente se o cliente
  escolhe verificá-lo. Não autentica o cliente. Uma vez que o procurador
  era acessível fora do loopback, o caminho do handshake era apenas de versão
  admissão.
- A API e os documentos descrevem este auxiliar como um proxy de estação de trabalho local para
  integrações de navegador/SDK, permitindo endereços de ligação remotamente acessíveis.
  uma incompatibilidade de limite de confiança, não um modo de serviço remoto pretendido.

Recomendação:

- Falha fechada em qualquer `bind_addr` sem loopback, portanto o auxiliar atual não pode ser
  expostos além da estação de trabalho local.
- Se a exposição remota ao proxy se tornar um requisito do produto, introduza
  autenticação explícita do cliente/admissão de capacidade primeiro em vez de
  relaxando a proteção de ligação.

Status de correção:

- Fechado no código atual. `crates/sorafs_orchestrator/src/proxy.rs` agora
  rejeita endereços de ligação sem loopback com `ProxyError::BindAddressNotLoopback`
  antes que o ouvinte QUIC seja iniciado.
- A documentação do campo de configuração em
  `docs/source/sorafs/developer/orchestrator.md` e
  `docs/portal/docs/sorafs/orchestrator-config.md` agora documenta
  `bind_addr` apenas como loopback.
- A cobertura de regressão agora inclui
  `spawn_local_quic_proxy_rejects_non_loopback_bind_addr` e o existente
  teste de ponte local positivo
  `proxy::tests::tcp_stream_bridge_transfers_payload` em
  `crates/sorafs_orchestrator/src/proxy.rs`.

### SEC-13: TLS-over-TCP P2P de saída rebaixado silenciosamente para texto simples por padrão (fechado em 25/03/2026)

Impacto:- A ativação de `network.tls_enabled=true` não impôs realmente apenas TLS
  transporte de saída, a menos que os operadores também descubram e definam
  `tls_fallback_to_plain=false`.
- Qualquer falha de handshake TLS ou tempo limite no caminho de saída, portanto
  rebaixou a discagem para TCP de texto simples por padrão, o que removeu o transporte
  confidencialidade e integridade contra invasores no caminho ou mau comportamento
  caixas intermediárias.
- O handshake do aplicativo assinado ainda autenticou a identidade do par, portanto
  este foi um rebaixamento da política de transporte, em vez de um desvio de falsificação de pares.

Evidência:

- `tls_fallback_to_plain` padronizado como `true` em
  `crates/iroha_config/src/parameters/user.rs`, então o substituto estava ativo
  a menos que os operadores o substituam explicitamente na configuração.
- `Connecting::connect_tcp(...)` em `crates/iroha_p2p/src/peer.rs` tenta um
  Discagem TLS sempre que `tls_enabled` estiver definido, mas em erros ou tempos limite de TLS ele
  registra um aviso e retorna ao TCP de texto simples sempre que
  `tls_fallback_to_plain` está habilitado.
- A configuração de amostra voltada para o operador em `crates/iroha_kagami/src/wizard.rs` e
  os documentos de transporte público P2P em `docs/source/p2p*.md` também anunciados
  fallback de texto simples como comportamento padrão.

Por que isso é importante:- Uma vez que os operadores ativam o TLS, a expectativa mais segura é falhar no fechamento: se o TLS
  a sessão não pode ser estabelecida, a discagem deve falhar em vez de silenciosamente
  derramamento de proteções de transporte.
- Deixar o downgrade ativado por padrão torna a implantação sensível a
  peculiaridades do caminho da rede, interferência de proxy e interrupção ativa do handshake em um
  maneira que é fácil de perder durante a implementação.

Recomendação:

- Mantenha o fallback de texto simples como um botão de compatibilidade explícito, mas use como padrão
  `false` então `network.tls_enabled=true` significa somente TLS, a menos que a operadora opte
  em comportamento de downgrade.

Status de correção:

- Fechado no código atual. `crates/iroha_config/src/parameters/user.rs` agora
  o padrão é `tls_fallback_to_plain` como `false`.
- O instantâneo do fixture de configuração padrão, configuração de amostra Kagami e padrão
  Os auxiliares de teste P2P/Torii agora espelham esse padrão de tempo de execução reforçado.
- Os documentos `docs/source/p2p*.md` duplicados agora descrevem o substituto de texto simples como
  uma aceitação explícita em vez do padrão enviado.

### SEC-14: A pesquisa geográfica de telemetria de pares voltou silenciosamente para HTTP de terceiros em texto simples (fechado em 25/03/2026)

Impacto:- Habilitar `torii.peer_geo.enabled=true` sem um endpoint explícito causado
  Torii para enviar nomes de host de pares para um texto simples integrado
  Serviço `http://ip-api.com/json/...`.
- Que vazou alvos de telemetria de pares em um HTTP de terceiros não autenticado
  dependência e permitir que qualquer invasor no caminho ou feed de endpoint comprometido seja forjado
  metadados de localização de volta para Torii.
- O recurso foi ativado, mas os documentos de telemetria pública e a configuração de exemplo
  anunciou o padrão integrado, o que tornou o padrão de implantação inseguro
  provavelmente, uma vez que os operadores ativaram pesquisas geográficas de pares.

Evidência:

- `crates/iroha_torii/src/telemetry/peers/monitor.rs` definido anteriormente
  `DEFAULT_GEO_ENDPOINT = "http://ip-api.com/json"` e
  `construct_geo_query(...)` usou esse padrão sempre
  `GeoLookupConfig.endpoint` era `None`.
- O monitor de telemetria peer gera `collect_geo(...)` sempre que
  `geo_config.enabled` é verdadeiro em
  `crates/iroha_torii/src/telemetry/peers/monitor.rs`, então o texto simples
  o substituto era acessível no código de tempo de execução enviado, em vez do código somente de teste.
- A configuração padrão é `crates/iroha_config/src/parameters/defaults.rs` e
  `crates/iroha_config/src/parameters/user.rs` deixe `endpoint` indefinido e o
  documentos de telemetria duplicados em `docs/source/telemetry*.md` plus
  `docs/source/references/peer.template.toml` documentou explicitamente o
  substituto integrado quando os operadores ativaram o recurso.

Por que isso é importante:- A telemetria de peer não deve sair silenciosamente de nomes de host de peer por meio de HTTP de texto simples
  para um serviço de terceiros assim que as operadoras ativarem uma bandeira de conveniência.
- Um padrão oculto e inseguro também prejudica a revisão de alterações: os operadores podem
  ativar a pesquisa geográfica sem perceber que eles introduziram recursos externos
  divulgação de metadados de terceiros e tratamento de respostas não autenticadas.

Recomendação:

- Remova o padrão de endpoint geográfico integrado.
- Exigir um endpoint HTTPS explícito quando as pesquisas geográficas de pares estiverem habilitadas e
  caso contrário, pule as pesquisas.
- Mantenha as regressões focadas provando que endpoints ausentes ou não HTTPS falham no fechamento.

Status de correção:

- Fechado no código atual. `crates/iroha_torii/src/telemetry/peers/monitor.rs`
  agora rejeita endpoints ausentes com `MissingEndpoint`, rejeita não-HTTPS
  endpoints com `InsecureEndpoint` e ignora a pesquisa geográfica de pares em vez de
  voltando silenciosamente para um serviço integrado de texto simples.
- `crates/iroha_config/src/parameters/user.rs` não injeta mais um implícito
  endpoint no momento da análise, portanto, o estado de configuração não definido permanece explícito em todos os
  forma de validação em tempo de execução.
- Os documentos de telemetria duplicados e a configuração de amostra canônica em
  `docs/source/references/peer.template.toml` agora afirma que
  `torii.peer_geo.endpoint` deve ser explicitamente configurado com HTTPS quando o
  recurso está habilitado.
- A cobertura de regressão agora inclui
  `construct_geo_query_requires_explicit_endpoint`,
  `construct_geo_query_rejects_non_https_endpoint`,
  `collect_geo_requires_explicit_endpoint_when_enabled` e
  `collect_geo_rejects_non_https_endpoint` em
  `crates/iroha_torii/src/telemetry/peers/monitor.rs`.### SEC-15: A política de pin e gateway SoraFS não tinha resolução de IP do cliente com reconhecimento de proxy confiável (fechado em 25/03/2026)

Impacto:

- Implantações Torii com proxy reverso poderiam anteriormente colapsar SoraFS distintos
  chamadores no endereço do soquete proxy ao avaliar o CIDR do pino de armazenamento
  listas de permissões, aceleradores de pinos de armazenamento por cliente e cliente de gateway
  impressões digitais.
- Isso enfraqueceu os controles de abuso em `/v1/sorafs/storage/pin` e SoraFS
  o download do gateway surge em topologias comuns de proxy reverso, tornando
  vários clientes compartilham um bucket ou uma identidade de lista de permissões.
- O roteador padrão ainda injeta metadados remotos internos, então isso não foi um problema.
  novo desvio de entrada não autenticado, mas era uma lacuna real no limite de confiança
  para implantações com reconhecimento de proxy e uma confiança excessiva no limite do manipulador do
  cabeçalho IP remoto interno.

Evidência:-`crates/iroha_config/src/parameters/user.rs` e
  `crates/iroha_config/src/parameters/actual.rs` anteriormente não tinha general
  Botão `torii.transport.trusted_proxy_cidrs`, cliente canônico com reconhecimento de proxy
  A resolução IP não era configurável no limite de entrada geral Torii.
-`inject_remote_addr_header(...)` em `crates/iroha_torii/src/lib.rs`
  substituiu anteriormente o cabeçalho `x-iroha-remote-addr` interno de
  Somente `ConnectInfo`, que eliminou metadados IP de cliente encaminhados confiáveis de
  proxies reversos reais.
-`PinSubmissionPolicy::enforce(...)` em
  `crates/iroha_torii/src/sorafs/pin.rs` e
  `gateway_client_fingerprint(...)` em `crates/iroha_torii/src/sorafs/api.rs`
  não compartilhou uma etapa de resolução de IP canônico com reconhecimento de proxy confiável no
  limite do manipulador.
- Aceleração de pinos de armazenamento em `crates/iroha_torii/src/sorafs/pin.rs` também codificada
  apenas no token ao portador sempre que um token estava presente, o que significava vários
  clientes com proxy que compartilhavam um token PIN válido foram forçados à mesma taxa
  bucket mesmo depois que seus IPs de cliente foram diferenciados.

Por que isso é importante:

- Os proxies reversos são um padrão de implantação normal para Torii. Se o tempo de execução
  não consegue distinguir consistentemente um proxy confiável de um chamador não confiável, IP
  listas de permissões e restrições por cliente deixam de significar o que os operadores pensam que eles
  significa.
- Os caminhos do pino e do gateway SoraFS são superfícies explicitamente sensíveis ao abuso, portanto
  colapsando chamadores no IP do proxy ou confiando demais no encaminhamento obsoleto
  metadados são operacionalmente significativos mesmo quando a rota base ainda
  requer outra admissão.

Recomendação:- Adicione uma superfície de configuração geral Torii `trusted_proxy_cidrs` e resolva o problema
  IP canônico do cliente uma vez de `ConnectInfo` mais qualquer encaminhamento pré-existente
  cabeçalho somente quando o peer do soquete estiver nessa lista de permissões.
- Reutilize essa resolução IP canônica dentro dos caminhos do manipulador SoraFS em vez de
  confiando cegamente no cabeçalho interno.
- Escopo dos aceleradores de pinos de armazenamento de token compartilhado por token mais IP canônico do cliente
  quando ambos estão presentes.

Status de correção:

- Fechado no código atual. `crates/iroha_config/src/parameters/defaults.rs`,
  `crates/iroha_config/src/parameters/user.rs` e
  `crates/iroha_config/src/parameters/actual.rs` agora expõe
  `torii.transport.trusted_proxy_cidrs`, padronizando-o para uma lista vazia.
- `crates/iroha_torii/src/lib.rs` agora resolve o IP canônico do cliente com
  `limits::ingress_remote_ip(...)` dentro do middleware de entrada e reescrita
  o cabeçalho `x-iroha-remote-addr` interno somente de proxies confiáveis.
-`crates/iroha_torii/src/sorafs/pin.rs` e
  `crates/iroha_torii/src/sorafs/api.rs` agora resolve IPs de clientes canônicos
  contra `state.trusted_proxy_nets` no limite do manipulador para pino de armazenamento
  impressão digital do cliente de gateway e política, portanto, os caminhos do manipulador direto não podem
  confiar demais em metadados IP encaminhados obsoletos.
- A otimização de pinos de armazenamento agora codifica tokens de portador compartilhados por `token + canônico
  IP do cliente` quando ambos estão presentes, preservando buckets por cliente para compartilhamento
  fichas de pinos.
- A cobertura de regressão agora inclui
  `limits::tests::ingress_remote_ip_preserves_trusted_forwarded_header`,
  `limits::tests::ingress_remote_ip_ignores_forwarded_header_from_untrusted_peer`,
  `sorafs::pin::tests::rate_key_scopes_shared_tokens_by_ip`,
  `storage_pin_rate_limit_uses_forwarded_client_ip_from_trusted_proxy`,
  `car_range_rate_limit_uses_forwarded_client_ip_from_trusted_proxy` e o
  dispositivo de configuração
  `torii_transport_trusted_proxy_cidrs_default_to_empty`.### SEC-16: O bloqueio de autenticação do operador e a codificação de limite de taxa retornaram para um bucket anônimo compartilhado quando o cabeçalho de IP remoto injetado estava faltando (fechado em 25/03/2026)

Impacto:

- A admissão de autenticação do operador já recebeu o IP do soquete aceito, mas o
  a chave de bloqueio/limite de taxa a ignorou e recolheu as solicitações em um arquivo compartilhado
  Bucket `"anon"` sempre que o cabeçalho `x-iroha-remote-addr` interno foi
  ausente.
- Esse não foi um novo desvio de entrada pública no roteador padrão, porque
  O middleware de entrada reescreve o cabeçalho interno antes da execução desses manipuladores.
  Ainda era uma verdadeira lacuna na fronteira de confiança para sistemas internos mais restritos.
  caminhos do manipulador, testes diretos e qualquer rota futura que atinja
  `OperatorAuth` antes do middleware de injeção.
- Nesses casos, um chamador pode consumir o orçamento com limite de taxa ou bloquear outro
  chamadores que deveriam ter sido isolados pelo IP de origem.

Evidência:- `OperatorAuth::check_common(...)` em
  `crates/iroha_torii/src/operator_auth.rs` já recebido
  `remote_ip: Option<IpAddr>`, mas anteriormente chamado de `auth_key(headers)`
  e abandonou totalmente o IP de transporte.
- `auth_key(...)` em `crates/iroha_torii/src/operator_auth.rs` anteriormente
  analisou apenas `limits::REMOTE_ADDR_HEADER` e retornou `"anon"`.
- O auxiliar geral Torii em `crates/iroha_torii/src/limits.rs` já tinha
  a regra correta de resolução de falha fechada em ` Effective_remote_ip (headers,
  remoto)`, preferindo o cabeçalho canônico injetado, mas voltando para o
  IP de soquete aceito quando invocações diretas do manipulador ignoram o middleware.

Por que isso é importante:

- O estado de bloqueio e limite de taxa deve ter a mesma identidade efetiva do chamador
  que o restante do Torii usa para decisões políticas. Voltando a um ambiente compartilhado
  bucket anônimo transforma um salto de metadados internos ausentes em cliente cruzado
  interferência em vez de localizar o efeito para o verdadeiro chamador.
- A autenticação do operador é um limite sensível ao abuso, portanto, mesmo um limite de gravidade média
  vale a pena encerrar explicitamente o problema de colisão de balde.

Recomendação:

- Derive a chave de autenticação do operador de `limits::efficient_remote_ip(headers,
  remote_ip)` então o cabeçalho injetado ainda vence quando presente, mas direto
  as invocações do manipulador retornam ao endereço de transporte em vez de `"anon"`.
- Mantenha `"anon"` apenas como substituto final quando o cabeçalho interno e
  o IP de transporte não está disponível.

Status de correção:- Fechado no código atual. `crates/iroha_torii/src/operator_auth.rs` agora liga
  `auth_key(headers, remote_ip)` de `check_common(...)` e `auth_key(...)`
  agora deriva a chave de bloqueio/limite de taxa de
  `limits::effective_remote_ip(headers, remote_ip)`.
- A cobertura de regressão agora inclui
  `operator_auth_key_uses_remote_ip_when_internal_header_missing` e
  `operator_auth_key_prefers_injected_header_over_transport_remote_ip` em
  `crates/iroha_torii/src/operator_auth.rs`.

## Descobertas encerradas ou substituídas do relatório anterior

- Descoberta anterior do Soracloud de chave privada bruta: fechada. Entrada de mutação atual
  rejeita campos inline `authority` / `private_key` em
  `crates/iroha_torii/src/soracloud.rs:5305-5308`, vincula o signatário HTTP ao
  proveniência da mutação em `crates/iroha_torii/src/soracloud.rs:5310-5315`, e
  retorna instruções de transação em rascunho em vez de enviar ao servidor um documento assinado
  transação em `crates/iroha_torii/src/soracloud.rs:5556-5565`.
- Descoberta anterior de execução de proxy de leitura local somente interna: fechada. Público
  a resolução de rota agora ignora manipuladores não públicos e de atualização/atualização privada em
  `crates/iroha_torii/src/soracloud.rs:8445-8463` e o tempo de execução rejeita
  rotas de leitura local não públicas em
  `crates/irohad/src/soracloud_runtime.rs:5906-5923`.
- Descoberta anterior de fallback ilimitado em tempo de execução pública: fechada conforme escrito. Público
  a entrada em tempo de execução agora impõe limites de taxa e limites de voo em
  `crates/iroha_torii/src/lib.rs:8837-8852` antes de resolver uma rota pública em
  `crates/iroha_torii/src/lib.rs:8858-8860`.
- Descoberta anterior de locação de anexo de IP remoto: fechada. Locação de anexo agora
  requer uma conta assinada verificada em
  `crates/iroha_torii/src/lib.rs:7962-7968`.
  Locação de anexo herdada anteriormente SEC-05 e SEC-06; essa herança
  é fechado pelas correções atuais de autenticação de aplicativo acima.## Descobertas de Dependência- `cargo deny check advisories bans sources --hide-inclusion-graph` agora é executado
  diretamente contra o `deny.toml` rastreado e agora relata três ao vivo
  descobertas de dependência de um arquivo de bloqueio de espaço de trabalho gerado.
- Os avisos `tar` não estão mais presentes no gráfico de dependência ativa:
  `xtask/src/mochi.rs` agora usa `Command::new("tar")` com um argumento fixo
  vetor e `iroha_crypto` não extrai mais `libsodium-sys-stable` para
  Testes de interoperabilidade Ed25519 após trocar essas verificações para OpenSSL.
- Resultados atuais:
  - `RUSTSEC-2024-0388`: `derivative` não tem manutenção.
  - `RUSTSEC-2024-0436`: `paste` não tem manutenção.
- Triagem de impacto:
  - Os avisos `tar` relatados anteriormente estão fechados para o ativo
    gráfico de dependência. `cargo tree -p xtask -e normal -i tar`,
    `cargo tree -p iroha_crypto -e all -i tar` e
    `cargo tree -p iroha_crypto -e all -i libsodium-sys-stable` agora todos falham
    com “especificação do ID do pacote... não corresponde a nenhum pacote” e
    `cargo deny` não informa mais `RUSTSEC-2026-0067` ou
    `RUSTSEC-2026-0068`.
  - Os avisos de substituição direta de PQ relatados anteriormente estão agora encerrados em
    a árvore atual. `crates/soranet_pq/Cargo.toml`,
    `crates/iroha_crypto/Cargo.toml` e `crates/ivm/Cargo.toml` agora dependem
    em `pqcrypto-mldsa` / `pqcrypto-mlkem`, e o ML-DSA / ML-KEM tocado
    os testes de tempo de execução ainda passam após a migração.
  - O comunicado `rustls-webpki` relatado anteriormente não está mais ativo em
    a resolução atual. O espaço de trabalho agora fixa `reqwest`/`rustls` aoversões de patch corrigidas que mantêm `rustls-webpki` em `0.103.10`, que é
    fora da faixa de aconselhamento.
  - `derivative` e `paste` não são usados ​​diretamente na origem do espaço de trabalho.
    Eles entram transitivamente através da pilha BLS/arkworks em
    `w3f-bls` e várias outras caixas a montante, portanto, removê-las requer
    alterações no upstream ou na pilha de dependências em vez de uma limpeza de macro local.
    A árvore atual agora aceita esses dois avisos explicitamente em
    `deny.toml` com motivos registrados.

## Notas de Cobertura- Servidor/tempo de execução/configuração/rede: SEC-05, SEC-06, SEC-07, SEC-08, SEC-09,
  SEC-10, SEC-12, SEC-13, SEC-14, SEC-15 e SEC-16 foram confirmados durante
  a auditoria e agora estão fechados na árvore atual. Endurecimento adicional em
  a árvore atual agora também faz com que a admissão de websocket/sessão do Connect falhe
  fechado quando o cabeçalho IP remoto injetado interno está faltando, em vez de
  padronizando essa condição para loopback.
- IVM/crypto/serialization: nenhuma descoberta adicional confirmada desta auditoria
  fatia. A evidência positiva inclui a zerização de material chave confidencial em
  `crates/iroha_crypto/src/confidential.rs:53-60` e Soranet PoW com reconhecimento de reprodução
  validação de ticket assinado em `crates/iroha_crypto/src/soranet/pow.rs:823-879`.
  O endurecimento de acompanhamento agora também rejeita a saída malformada do acelerador em dois
  caminhos Norito amostrados: `crates/norito/src/lib.rs` valida JSON acelerado
  Fitas de estágio 1 anteriores a `TapeWalker` desreferenciam deslocamentos e agora também exigem
  ajudantes Metal/CUDA Stage-1 carregados dinamicamente para provar a paridade com o
  construtor de índice estrutural escalar antes da ativação, e
  `crates/norito/src/core/gpu_zstd.rs` valida comprimentos de saída relatados pela GPU
  antes de truncar buffers de codificação/decodificação. `crates/norito/src/core/simd_crc64.rs`
  agora também faz autotestes de ajudantes GPU CRC64 carregados dinamicamente em relação ao
  fallback canônico antes de `hardware_crc64` confiar neles, então malformado
  bibliotecas auxiliares falham ao fechar em vez de alterar silenciosamente a soma de verificação Noritocomportamento. Resultados inválidos do auxiliar agora retrocedem em vez de entrar em pânico
  compila ou deriva a paridade da soma de verificação. No lado IVM, acelerador amostrado
  os portões de inicialização agora também cobrem o CUDA Ed25519 `signature_kernel`, CUDA BN254
  add/sub/mul kernels, CUDA `sha256_leaves` / `sha256_pairs_reduce`, o live
  Kernels de lote vetor CUDA/AES (`vadd64`, `vand`, `vxor`, `vor`,
  `aesenc_batch`, `aesdec_batch`) e o metal correspondente
  Kernels em lote `sha256_leaves`/vector/AES antes que esses caminhos sejam confiáveis. O
  amostra do caminho de assinatura do Metal Ed25519 agora também está de volta
  dentro do acelerador ativo definido neste host: a falha de paridade anterior foi
  corrigido restaurando a normalização ref10 ligada ao membro através da escada escalar,
  e a regressão Metal focada agora verifica `[s]B`, `[h](-A)`, o
  escada de ponto base de potência de dois e verificação de lote `[true, false]` completa
  no Metal em relação ao caminho de referência da CPU. As alterações de origem CUDA espelhadas
  compilar sob `--features cuda --tests`, e a verdade de inicialização CUDA definida agora
  falha no fechamento se os kernels folha/par Merkle vivos se desviarem da CPU
  caminho de referência. A validação CUDA em tempo de execução permanece limitada pelo host neste
  ambiente.
- SDKs/exemplos: SEC-11 foi confirmado durante a amostra focada em transporte
  passar pelos clientes Swift, Java/Android, Kotlin e JS, e issoa descoberta agora está fechada na árvore atual. O JS, Swift e Android
  auxiliares de solicitação canônica também foram atualizados para o novo
  esquema de quatro cabeçalhos com reconhecimento de frescor.
  A análise de transporte de streaming QUIC amostrada também não produziu uma transmissão ao vivo
  localização em tempo de execução na árvore atual: `StreamingClient::connect(...)`,
  `StreamingServer::bind(...)` e os auxiliares de negociação de capacidade são
  atualmente exercido apenas a partir do código de teste em `crates/iroha_p2p` e
  `crates/iroha_core`, portanto, o verificador autoassinado permissivo nesse auxiliar
  path atualmente é apenas de teste/auxiliar, em vez de uma superfície de entrada enviada.
- Exemplos e exemplos de aplicativos móveis foram revisados apenas em nível de verificação pontual e
  não devem ser tratados como auditados exaustivamente.

## Lacunas de validação e cobertura- `cargo deny check advisories bans sources --hide-inclusion-graph` agora é executado
  diretamente com o `deny.toml` rastreado. Sob essa execução do esquema atual,
  `bans` e `sources` estão limpos, enquanto `advisories` falha com os cinco
  descobertas de dependência listadas acima.
- Validação de limpeza do gráfico de dependência para as descobertas `tar` fechadas aprovadas:
  `cargo tree -p xtask -e normal -i tar`,
  `cargo tree -p iroha_crypto -e all -i tar` e
  `cargo tree -p iroha_crypto -e all -i libsodium-sys-stable` agora todos reportam
  “especificação do ID do pacote... não corresponde a nenhum pacote”, enquanto
  `NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 cargo test -p xtask create_archive_packages_bundle_directory -- --nocapture`,
  `NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 cargo check -p xtask`,
  `NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 cargo test -p iroha_crypto ed25519_verify -- --nocapture`,
  e
  `NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 cargo test -p iroha_crypto ed25519_sign -- --nocapture`
  tudo passou.
- `bash scripts/fuzz_smoke.sh` agora executa sessões reais do libFuzzer via
  `cargo +nightly fuzz`, mas a metade IVM do script não terminou dentro
  esta passagem porque a primeira compilação noturna para `tlv_validate` ainda estava em
  progresso na transferência. Desde então, essa compilação foi concluída o suficiente para executar o
  gerou o binário libFuzzer diretamente:
  `cd crates/ivm/fuzz && ./target/aarch64-apple-darwin/release/tlv_validate -runs=200 -rss_limit_mb=3072 -max_total_time=15`
  agora atinge o loop de execução do libFuzzer e sai corretamente após 200 execuções de um
  corpus vazio. A metade Norito foi concluída com sucesso após consertar o
  desvio de chicote / manifesto e a compilação de alvo fuzz `json_from_json_equiv`
  quebrar.
- A validação de remediação Torii agora inclui:
  -`CARGO_TARGET_DIR=/tmp/iroha-codex-target cargo check -p iroha_torii --lib`
  -`CARGO_TARGET_DIR=/tmp/iroha-codex-target cargo test -p iroha_torii --lib --no-run`
  -`CARGO_TARGET_DIR=/tmp/iroha-codex-target cargo test -p iroha_torii --lib verify_accepts_valid_signature -- --nocapture`
  -`CARGO_TARGET_DIR=/tmp/iroha-codex-target cargo test -p iroha_torii --lib verify_rejects_replayed_nonce -- --nocapture`
  -`CARGO_TARGET_DIR=/tmp/iroha-codex-target cargo test -p iroha_torii --lib operator_auth_rejects_forwarded_mtls_from_untrusted_proxy -- --nocapture`
  -`NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 CARGO_HOME=/tmp/iroha-cargo-home-operator-auth-key CARGO_TARGET_DIR=/tmp/iroha-codex-target-operator-auth-key cargo test -p iroha_torii --lib operator_auth_key_ -- --nocapture`
  -`NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 CARGO_HOME=/tmp/iroha-cargo-home-operator-auth-key CARGO_TARGET_DIR=/tmp/iroha-codex-target-operator-auth-key cargo test -p iroha_torii --lib operator_auth_rejects_forwarded_mtls_from_untrusted_proxy -- --nocapture`
  -`CARGO_TARGET_DIR=/tmp/iroha-codex-target cargo test -p iroha_torii --lib trusted_forwarded_header_requires_proxy_membership -- --nocapture`
  -`NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 CARGO_TARGET_DIR=/tmp/iroha-codex-target-webhook-https cargo check -p iroha_torii --lib --features app_api_https`
  -`NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 CARGO_TARGET_DIR=/tmp/iroha-codex-target-webhook-https cargo test -p iroha_torii --lib https_delivery_dns_override_ --features app_api_https -- --nocapture`
  -`NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 CARGO_TARGET_DIR=/tmp/iroha-codex-target-webhook2 cargo test -p iroha_torii --lib websocket_pinned_connect_addr_pins_secure_delivery_when_guarded -- --nocapture`-`NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 CARGO_TARGET_DIR=/tmp/iroha-codex-target-mcp cargo test -p iroha_torii --lib dispatch_route_preserves_inbound_remote_addr_for_internal_allowlist_checks -- --nocapture`
  -`NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 CARGO_TARGET_DIR=/tmp/iroha-codex-target-mcp cargo test -p iroha_torii --lib dispatch_route_blocks_remote_addr_spoofing_from_extra_headers -- --nocapture`
  -`NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 CARGO_TARGET_DIR=/tmp/iroha-codex-target-mcp cargo test -p iroha_torii --lib apply_extra_headers_blocks_reserved_internal_headers -- --nocapture`
  - o mais estreito `--no-default-features --features app_api,app_api_https` Torii
    matriz de teste ainda tem falhas de compilação existentes não relacionadas no DA/
    Código de teste lib controlado por Soracloud, então esta passagem validou o enviado
    caminho MCP do recurso padrão e o caminho do webhook `app_api_https` em vez de
    reivindicando cobertura completa de recursos mínimos.
- A validação de remediação Trusted-proxy/SoraFS agora inclui:
  -`CARGO_HOME=/tmp/iroha-cargo-home-trusted-proxy CARGO_TARGET_DIR=/tmp/iroha-codex-target-trusted-proxy cargo test -p iroha_torii --lib limits::tests:: -- --nocapture`
  -`CARGO_HOME=/tmp/iroha-cargo-home-trusted-proxy CARGO_TARGET_DIR=/tmp/iroha-codex-target-trusted-proxy cargo test -p iroha_torii --lib sorafs::pin::tests:: -- --nocapture`
  -`CARGO_HOME=/tmp/iroha-cargo-home-trusted-proxy CARGO_TARGET_DIR=/tmp/iroha-codex-target-trusted-proxy cargo test -p iroha_torii --lib storage_pin_requires_token_and_respects_allowlist_and_rate_limit -- --nocapture`
  -`CARGO_HOME=/tmp/iroha-cargo-home-trusted-proxy CARGO_TARGET_DIR=/tmp/iroha-codex-target-trusted-proxy cargo test -p iroha_torii --lib storage_pin_rate_limit_uses_forwarded_client_ip_from_trusted_proxy -- --nocapture`
  -`CARGO_HOME=/tmp/iroha-cargo-home-trusted-proxy CARGO_TARGET_DIR=/tmp/iroha-codex-target-trusted-proxy cargo test -p iroha_torii --lib car_range_rate_limits_repeated_clients -- --nocapture`
  -`CARGO_HOME=/tmp/iroha-cargo-home-trusted-proxy CARGO_TARGET_DIR=/tmp/iroha-codex-target-trusted-proxy cargo test -p iroha_torii --lib car_range_rate_limit_uses_forwarded_client_ip_from_trusted_proxy -- --nocapture`
  -`CARGO_HOME=/tmp/iroha-cargo-home-trusted-proxy CARGO_TARGET_DIR=/tmp/iroha-codex-target-trusted-proxy cargo test -p iroha_config --test fixtures torii_transport_trusted_proxy_cidrs_default_to_empty -- --nocapture`
- A validação de remediação P2P agora inclui:
  -`NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 CARGO_TARGET_DIR=/tmp/iroha-codex-target-p2p cargo test -p iroha_p2p --lib --no-run`
  -`NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 CARGO_TARGET_DIR=/tmp/iroha-codex-target-p2p cargo test -p iroha_p2p --lib outgoing_handshake_rejects_unexpected_peer_identity -- --nocapture`
  -`NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 CARGO_TARGET_DIR=/tmp/iroha-codex-target-p2p cargo test -p iroha_p2p --lib handshake_v1_defaults_to_trust_gossip -- --nocapture`
- A validação de correção de proxy local SoraFS agora inclui:
  -`NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 CARGO_TARGET_DIR=/tmp/iroha-codex-target-proxy cargo test -p sorafs_orchestrator spawn_local_quic_proxy_rejects_non_loopback_bind_addr -- --nocapture`
  -`/tmp/iroha-codex-target-proxy/debug/deps/sorafs_orchestrator-b3be10a343598c7b --exact proxy::tests::tcp_stream_bridge_transfers_payload --nocapture`
- A validação de correção padrão TLS P2P agora inclui:
  -`CARGO_HOME=/tmp/iroha-cargo-home-sec13 CARGO_TARGET_DIR=/tmp/iroha-codex-target-config-tls2 cargo test -p iroha_config tls_fallback_defaults_to_tls_only -- --nocapture`
  -`CARGO_HOME=/tmp/iroha-cargo-home-sec13 CARGO_TARGET_DIR=/tmp/iroha-codex-target-p2p-sec13 cargo test -p iroha_p2p --lib start_rejects_tls_without_feature_when_tls_only_outbound -- --nocapture`
  -`CARGO_HOME=/tmp/iroha-cargo-home-sec13 CARGO_TARGET_DIR=/tmp/iroha-codex-target-p2p-sec13 cargo test -p iroha_p2p --lib tls_only_dial_requires_p2p_tls_feature_when_no_fallback -- --nocapture`
  -`NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 CARGO_HOME=/tmp/iroha-cargo-home-sec13 CARGO_TARGET_DIR=/tmp/iroha-codex-target-torii-connect-gating cargo test -p iroha_torii --test connect_gating --no-run`
- A validação de correção do lado do SDK agora inclui:
  -`node --test javascript/iroha_js/test/canonicalRequest.test.js javascript/iroha_js/test/toriiCanonicalAuth.test.js`
  -`cd IrohaSwift && swift test --filter CanonicalRequestTests`
  -`cd IrohaSwift && swift test --filter 'NoritoRpcClientTests/testCallRejectsInsecureAuthorizationHeader'`
  -`cd IrohaSwift && swift test --filter 'ConnectClientTests/testBuildsConnectWebSocketRequestRejectsInsecureTransport'`
  -`cd IrohaSwift && swift test --filter 'ToriiClientTests/testCreateSubscriptionPlanRejectsInsecureTransportForPrivateKeyBody'`
  -`cd kotlin && ./gradlew :core-jvm:test --tests org.hyperledger.iroha.sdk.client.TransportSecurityClientTest --console=plain`
  -`cd java/iroha_android && JAVA_HOME=$(/usr/libexec/java_home -v 21) ANDROID_HOME=~/Library/Android/sdk ANDROID_SDK_ROOT=~/Library/Android/sdk ./gradlew android:compileDebugUnitTestJavaWithJavac --console=plain`
  -`cd java/iroha_android && JAVA_HOME=$(/usr/libexec/java_home -v 21) ANDROID_HOME=~/Library/Android/sdk ANDROID_SDK_ROOT=~/Library/Android/sdk ANDROID_HARNESS_MAINS=org.hyperledger.iroha.android.client.NoritoRpcClientTests,org.hyperledger.iroha.android.client.OfflineToriiClientTests,org.hyperledger.iroha.android.client.SubscriptionToriiClientTests,org.hyperledger.iroha.android.client.stream.ToriiEventStreamClientTests,org.hyperledger.iroha.android.client.websocket.ToriiWebSocketClientTests ./gradlew :core:test --tests org.hyperledger.iroha.android.GradleHarnessTests --console=plain`
  -`node --test javascript/iroha_js/test/transportSecurity.test.js javascript/iroha_js/test/toriiCanonicalAuth.test.js`
  -`node --test javascript/iroha_js/test/toriiSubscriptions.test.js`
- A validação de acompanhamento Norito agora inclui:
  -`python3 scripts/check_norito_bindings_sync.py`
  -`CARGO_TARGET_DIR=/tmp/iroha-codex-target cargo test -p norito validate_accel_rejects_out_of_bounds_offsets -- --nocapture`
  -`CARGO_TARGET_DIR=/tmp/iroha-codex-target cargo test -p norito validate_accel_rejects_non_structural_offsets -- --nocapture`
  -`CARGO_TARGET_DIR=/tmp/iroha-codex-target cargo test -p norito try_gpu_encode_rejects_invalid_success_length -- --nocapture`
  -`CARGO_TARGET_DIR=/tmp/iroha-codex-target cargo test -p norito try_gpu_decode_rejects_invalid_success_length -- --nocapture`
  - `bash scripts/fuzz_smoke.sh` (Norito tem como alvo `json_parse_string`,
    `json_parse_string_ref`, `json_skip_value` e `json_from_json_equiv`aprovado após correções de chicote/alvo)
- A validação de acompanhamento de fuzz IVM agora inclui:
  -`cd crates/ivm/fuzz && ./target/aarch64-apple-darwin/release/tlv_validate -runs=200 -rss_limit_mb=3072 -max_total_time=15`
- A validação de acompanhamento do acelerador IVM agora inclui:
  -`xcrun -sdk macosx metal -c crates/ivm/src/metal_ed25519.metal -o /tmp/metal_ed25519.air`
  -`CARGO_TARGET_DIR=/tmp/iroha-codex-target-ivm-cuda2 cargo check -p ivm --features cuda --tests`
  -`NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 CARGO_TARGET_DIR=/tmp/iroha-codex-target-ivm-cuda-check cargo check -p ivm --features cuda --tests`
  -`CARGO_TARGET_DIR=/tmp/iroha-codex-target-ivm-metal cargo check -p ivm --features metal --tests`
  -`NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 CARGO_TARGET_DIR=/tmp/iroha-codex-target-ivm-metal cargo test -p ivm --features metal --lib metal_bitwise_single_vector_matches_scalar -- --nocapture`
  -`NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 CARGO_TARGET_DIR=/tmp/iroha-codex-target-ivm-metal cargo test -p ivm --features metal --lib metal_aes_batch_matches_scalar -- --nocapture`
  -`NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 CARGO_TARGET_DIR=/tmp/iroha-codex-target-ivm-metal cargo test -p ivm --features metal --lib metal_ed25519_batch_matches_cpu -- --nocapture`
  -`CARGO_TARGET_DIR=/tmp/iroha-codex-target-ivm-metal cargo test -p ivm --features metal --lib metal_sha256_leaves_matches_cpu -- --nocapture`
- A execução do teste lib CUDA focado permanece limitada pelo ambiente neste host:
  `CARGO_TARGET_DIR=/tmp/iroha-codex-target-ivm-cuda2 cargo test -p ivm --features cuda --lib selftest_covers_ -- --nocapture`
  ainda não consegue vincular porque os símbolos do driver CUDA (`cu*`) não estão disponíveis.
- A validação do tempo de execução do Focused Metal agora é executada totalmente no acelerador neste
  host: o pipeline de assinatura Ed25519 amostrado permanece habilitado durante a inicialização
  autotestes e `metal_ed25519_batch_matches_cpu` verifica `[true, false]`
  diretamente no Metal em relação ao caminho de referência da CPU.
- Não executei novamente uma varredura completa do teste de ferrugem do espaço de trabalho, `npm test` completo ou o
  suítes completas do Swift/Android durante esta passagem de correção.

## Pendências de remediação priorizadas

### Próxima Tranche- Monitorar substituições upstream para o transitivo explicitamente aceito
  Dívida macro `derivative` / `paste` e remover as exceções `deny.toml` quando
  atualizações seguras ficam disponíveis sem desestabilizar o BLS / Halo2 / PQ /
  Pilhas de dependências da UI.
- Execute novamente o script fuzz-smoke IVM noturno completo em um cache quente para
  `tlv_validate` / `kotodama_lower` têm resultados registrados estáveis próximos ao
  alvos Norito agora verdes. Uma execução binária direta `tlv_validate` agora é concluída,
  mas a fumaça noturna programada completa ainda é excelente.
- Execute novamente a fatia de autoteste CUDA lib-test em um host com driver CUDA
  bibliotecas instaladas, para que o conjunto de verdade de inicialização expandido do CUDA seja validado
  além de `cargo check` e da correção de normalização Ed25519 espelhada mais o
  novos testes de inicialização de vetor/AES são exercidos em tempo de execução.
- Execute novamente suítes JS/Swift/Android/Kotlin mais amplas quando o nível de suíte não relacionado
  bloqueadores neste branch são limpos, então a nova solicitação canônica e
  os guardas de segurança de transporte são cobertos além dos testes de ajudantes focados acima.
- Decidir se a história multisig de autenticação de aplicativo de longo prazo deve permanecer
  fechar com falha ou desenvolver um formato de testemunha multisig HTTP de primeira classe.

### Monitorar- Continuar a revisão focada de aceleração de hardware/caminhos inseguros `ivm`
  e os limites restantes de streaming/criptografia `norito`. O JSON Estágio-1
  e as transferências auxiliares do GPU zstd agora foram reforçadas para falhar no fechamento
  versões de lançamento e os conjuntos de verdade de inicialização do acelerador IVM amostrados agora são
  mais amplo, mas a revisão mais ampla do inseguro/determinismo ainda está em aberto.