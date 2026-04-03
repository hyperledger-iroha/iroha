<!-- Auto-generated stub for Portuguese (pt) translation. Replace this content with the full translation. -->

---
lang: pt
direction: ltr
source: security_audit_report.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 4343a8f72823a2539a2e79e80a1c7162d455e03b450e2fd613f01ac5c61aec63
source_last_modified: "2026-03-26T13:01:48.980087+00:00"
translation_last_reviewed: 2026-04-02
translator: machine-google-reviewed
---

# Relatório de auditoria de segurança

Data: 26/03/2026

## Resumo Executivo

Esta auditoria se concentrou nas superfícies de maior risco na árvore atual: fluxos HTTP/API/auth Torii, transporte P2P, APIs de manipulação de segredos, proteções de transporte SDK e o caminho do sanitizador de anexos.

Encontrei 6 problemas acionáveis:

- 2 achados de alta gravidade
- 4 achados de gravidade média

Os problemas mais importantes são:

1. Torii atualmente registra cabeçalhos de solicitação de entrada para cada solicitação HTTP, que pode expor tokens de portador, tokens de API, tokens de sessão do operador/bootstrap e marcadores mTLS encaminhados para logs.
2. Várias rotas e SDKs Torii públicos ainda suportam o envio de valores `private_key` brutos para o servidor para que Torii possa assinar em nome do chamador.
3. Vários caminhos "secretos" são tratados como órgãos de solicitação comuns, incluindo derivação de sementes confidenciais e autenticação de solicitação canônica em alguns SDKs.

## Método

- Revisão estática de caminhos de manipulação de segredos Torii, P2P, criptografia/VM e SDK
- Comandos de validação direcionados:
  - `cargo check -p iroha_torii --lib --message-format short` -> passar
  - `cargo check -p iroha_p2p --message-format short` -> passar
  - `cargo test -p iroha_torii --lib confidential_derive_keyset_endpoint_roundtrip -- --nocapture` -> passar
  - `cargo deny check advisories bans sources --hide-inclusion-graph` -> passar, apenas avisos de versão duplicada
- Não concluído neste passe:
  - construção/teste/clippy de espaço de trabalho completo
  - Conjuntos de testes Swift/Gradle
  - Validação de tempo de execução CUDA/Metal

## Descobertas

### SA-001 Alto: Torii registra cabeçalhos de solicitação confidenciais globalmenteImpacto: qualquer implantação que envie rastreamento de solicitação pode vazar tokens de portador/API/operador e material de autenticação relacionado nos logs do aplicativo.

Evidência:

- `crates/iroha_torii/src/lib.rs:20752` habilita `TraceLayer::new_for_http()`
- `crates/iroha_torii/src/lib.rs:20753` habilita `DefaultMakeSpan::default().include_headers(true)`
- Nomes de cabeçalho confidenciais são usados ativamente em outras partes do mesmo serviço:
  -`crates/iroha_torii/src/operator_auth.rs:40`
  -`crates/iroha_torii/src/operator_auth.rs:41`
  -`crates/iroha_torii/src/operator_auth.rs:42`
  -`crates/iroha_torii/src/operator_auth.rs:43`

Por que isso é importante:

- `include_headers(true)` registra valores completos do cabeçalho de entrada em intervalos de rastreamento.
- Torii aceita material de autenticação em cabeçalhos como `Authorization`, `x-api-token`, `x-iroha-operator-session`, `x-iroha-operator-token` e `x-forwarded-client-cert`.
- Um comprometimento do coletor de log, uma coleta de log de depuração ou um pacote de suporte pode, portanto, tornar-se um evento de divulgação de credenciais.

Correção recomendada:

- Pare de incluir cabeçalhos de solicitação completos em períodos de produção.
- Adicione redação explícita para cabeçalhos sensíveis à segurança se o registro de cabeçalho ainda for necessário para depuração.
- Trate o registro de solicitação/resposta como secreto por padrão, a menos que os dados estejam positivamente na lista de permissões.

### SA-002 Alto: APIs públicas Torii ainda aceitam chaves privadas brutas para assinatura do lado do servidor

Impacto: os clientes são incentivados a transmitir chaves privadas brutas pela rede para que o servidor possa assinar em seu nome, criando um canal desnecessário de exposição de segredos nas camadas API, SDK, proxy e memória do servidor.

Evidência:- A documentação da rota de governança anuncia explicitamente a assinatura do lado do servidor:
  -`crates/iroha_torii/src/gov.rs:495`
- A implementação da rota analisa a chave privada fornecida e assina no lado do servidor:
  -`crates/iroha_torii/src/gov.rs:1088`
  -`crates/iroha_torii/src/gov.rs:1091`
  -`crates/iroha_torii/src/gov.rs:1123`
  -`crates/iroha_torii/src/gov.rs:1125`
- Os SDKs serializam ativamente `private_key` em corpos JSON:
  -`kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/client/OfflineToriiClient.kt:47`
  -`kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/client/OfflineToriiClient.kt:50`
  -`kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/client/OfflineToriiClient.kt:260`
  -`kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/client/OfflineToriiClient.kt:261`

Notas:

- Este padrão não está isolado de uma família de rotas. A árvore atual contém o mesmo modelo de conveniência em governança, dinheiro offline, assinaturas e outros DTOs voltados para aplicativos.
- As verificações de transporte somente HTTPS reduzem o transporte acidental de texto simples, mas não resolvem o tratamento de segredos do lado do servidor ou o risco de exposição de registro/memória.

Correção recomendada:

- Descontinuar todos os DTOs de solicitação que transportam dados `private_key` brutos.
- Exigir que os clientes assinem localmente e enviem assinaturas ou transações/envelopes totalmente assinados.
- Remova exemplos `private_key` de OpenAPI/SDKs após uma janela de compatibilidade.

### SA-003 Medium: A derivação de chave confidencial envia material inicial secreto para Torii e o ecoa de volta

Impacto: a API de derivação de chave confidencial transforma o material inicial em dados normais de carga útil de solicitação/resposta, aumentando a chance de divulgação inicial por meio de proxies, middleware, logs, rastreamentos, relatórios de falhas ou uso indevido do cliente.

Evidência:- A solicitação aceita sementes diretamente:
  -`crates/iroha_torii/src/routing.rs:2736`
  -`crates/iroha_torii/src/routing.rs:2738`
  -`crates/iroha_torii/src/routing.rs:2740`
- O esquema de resposta ecoa a semente em hexadecimal e base64:
  -`crates/iroha_torii/src/routing.rs:2745`
  -`crates/iroha_torii/src/routing.rs:2746`
  -`crates/iroha_torii/src/routing.rs:2747`
- O manipulador recodifica explicitamente e retorna a semente:
  -`crates/iroha_torii/src/routing.rs:2797`
  -`crates/iroha_torii/src/routing.rs:2801`
  -`crates/iroha_torii/src/routing.rs:2802`
  -`crates/iroha_torii/src/routing.rs:2804`
- O Swift SDK expõe isso como um método de rede regular e persiste a semente ecoada no modelo de resposta:
  -`IrohaSwift/Sources/IrohaSwift/ToriiClient.swift:4716`
  -`IrohaSwift/Sources/IrohaSwift/ToriiClient.swift:4717`
  -`IrohaSwift/Sources/IrohaSwift/ToriiClient.swift:4718`
  -`IrohaSwift/Sources/IrohaSwift/ToriiClient.swift:11912`
  -`IrohaSwift/Sources/IrohaSwift/ToriiClient.swift:11926`

Correção recomendada:

- Prefira a derivação de chave local no código CLI/SDK e remova totalmente a rota de derivação remota.
- Caso a rota deva permanecer, nunca devolver a semente na resposta e marcar os corpos portadores de sementes como sensíveis em todas as guardas de transporte e caminhos de telemetria/registro.

### SA-004 Médio: a detecção de sensibilidade de transporte do SDK tem pontos cegos para material secreto não `private_key`

Impacto: alguns SDKs imporão HTTPS para solicitações `private_key` brutas, mas ainda permitirão que outros materiais de solicitação sensíveis à segurança viajem por HTTP inseguro ou para hosts incompatíveis.

Evidência:- Swift trata os cabeçalhos de autenticação de solicitação canônica como confidenciais:
  -`IrohaSwift/Sources/IrohaSwift/TransportSecurity.swift:4`
  -`IrohaSwift/Sources/IrohaSwift/TransportSecurity.swift:7`
  -`IrohaSwift/Sources/IrohaSwift/TransportSecurity.swift:8`
  -`IrohaSwift/Sources/IrohaSwift/TransportSecurity.swift:9`
  -`IrohaSwift/Sources/IrohaSwift/TransportSecurity.swift:10`
- Mas Swift ainda só combina corpo em `"private_key"`:
  -`IrohaSwift/Sources/IrohaSwift/TransportSecurity.swift:18`
  -`IrohaSwift/Sources/IrohaSwift/TransportSecurity.swift:66`
  -`IrohaSwift/Sources/IrohaSwift/TransportSecurity.swift:69`
- Kotlin reconhece apenas os cabeçalhos `authorization` e `x-api-token` e, em seguida, recorre à mesma heurística de corpo `"private_key"`:
  -`kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/client/TransportSecurity.kt:50`
  -`kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/client/TransportSecurity.kt:53`
  -`kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/client/TransportSecurity.kt:58`
  -`kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/client/TransportSecurity.kt:61`
- Java/Android tem a mesma limitação:
  -`java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/TransportSecurity.java:24`
  -`java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/TransportSecurity.java:25`
  -`java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/TransportSecurity.java:100`
  -`java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/TransportSecurity.java:104`
- Os signatários de solicitações canônicas Kotlin/Java geram cabeçalhos de autenticação adicionais que não são classificados como confidenciais por seus próprios protetores de transporte:
  -`kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/client/CanonicalRequestSigner.kt:17`
  -`kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/client/CanonicalRequestSigner.kt:18`
  -`kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/client/CanonicalRequestSigner.kt:51`
  -`java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/CanonicalRequestSigner.java:25`
  -`java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/CanonicalRequestSigner.java:26`
  -`java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/CanonicalRequestSigner.java:27`
  -`java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/CanonicalRequestSigner.java:28`

Correção recomendada:

- Substituir a verificação heurística do corpo pela classificação explícita de solicitações.
- Trate cabeçalhos de autenticação canônicos, campos de semente/frase secreta, cabeçalhos de mutação assinados e quaisquer campos futuros contendo segredos como confidenciais por contrato, não por correspondência de substring.
- Mantenha as regras de confidencialidade alinhadas em Swift, Kotlin e Java.

### SA-005 Medium: O anexo "sandbox" é apenas um subprocesso mais `setrlimit`Impacto: o desinfetante de anexos é descrito e relatado como "em área restrita", mas a implementação é apenas uma bifurcação/exec do binário atual com limites de recursos. Uma exploração de analisador ou arquivo ainda seria executada com o mesmo usuário, visualização do sistema de arquivos e privilégios de rede/processo de ambiente que Torii.

Evidência:

- O caminho externo marca o resultado como sandbox após gerar um filho:
  -`crates/iroha_torii/src/zk_attachments.rs:756`
  -`crates/iroha_torii/src/zk_attachments.rs:760`
  -`crates/iroha_torii/src/zk_attachments.rs:776`
  -`crates/iroha_torii/src/zk_attachments.rs:782`
- O filho assume como padrão o executável atual:
  -`crates/iroha_torii/src/zk_attachments.rs:913`
  -`crates/iroha_torii/src/zk_attachments.rs:919`
- O subprocesso volta explicitamente para `AttachmentSanitizerMode::InProcess`:
  -`crates/iroha_torii/src/zk_attachments.rs:1794`
  -`crates/iroha_torii/src/zk_attachments.rs:1803`
- O único endurecimento aplicado é CPU/espaço de endereço `setrlimit`:
  -`crates/iroha_torii/src/zk_attachments.rs:1845`
  -`crates/iroha_torii/src/zk_attachments.rs:1850`
  -`crates/iroha_torii/src/zk_attachments.rs:1851`
  -`crates/iroha_torii/src/zk_attachments.rs:1872`

Correção recomendada:

- Implemente uma sandbox de sistema operacional real (por exemplo, namespaces/seccomp/landlock/jail-style isolamento, eliminação de privilégios, sem rede, sistema de arquivos restrito) ou pare de rotular o resultado como `sandboxed`.
- Trate o design atual como "isolamento de subprocesso" em vez de "sandboxing" em APIs, telemetria e documentos até que exista um verdadeiro isolamento.

### SA-006 Médio: Transportes P2P TLS/QUIC opcionais desativam a verificação de certificadoImpacto: Quando `quic` ou `p2p_tls` está ativado, o canal fornece criptografia, mas não autentica o terminal remoto. Um invasor ativo no caminho ainda pode retransmitir ou encerrar o canal, anulando as expectativas normais de segurança que os operadores associam ao TLS/QUIC.

Evidência:

- QUIC documenta explicitamente a verificação permissiva do certificado:
  -`crates/iroha_p2p/src/transport.rs:12`
  -`crates/iroha_p2p/src/transport.rs:13`
  -`crates/iroha_p2p/src/transport.rs:14`
  -`crates/iroha_p2p/src/transport.rs:15`
- O verificador QUIC aceita incondicionalmente o certificado do servidor:
  -`crates/iroha_p2p/src/transport.rs:33`
  -`crates/iroha_p2p/src/transport.rs:35`
  -`crates/iroha_p2p/src/transport.rs:44`
  -`crates/iroha_p2p/src/transport.rs:112`
  -`crates/iroha_p2p/src/transport.rs:114`
  -`crates/iroha_p2p/src/transport.rs:115`
- O transporte TLS sobre TCP faz o mesmo:
  -`crates/iroha_p2p/src/transport.rs:229`
  -`crates/iroha_p2p/src/transport.rs:232`
  -`crates/iroha_p2p/src/transport.rs:241`
  -`crates/iroha_p2p/src/transport.rs:279`
  -`crates/iroha_p2p/src/transport.rs:281`
  -`crates/iroha_p2p/src/transport.rs:282`

Correção recomendada:

- Verifique os certificados de peer ou adicione uma ligação de canal explícita entre o handshake assinado de camada superior e a sessão de transporte.
- Se o comportamento atual for intencional, renomeie/documente o recurso como transporte criptografado não autenticado para que os operadores não o confundam com autenticação TLS completa de pares.

## Pedido de correção recomendado1. Corrija o SA-001 imediatamente, redigindo ou desativando o registro do cabeçalho.
2. Projetar e enviar um plano de migração para SA-002 para que as chaves privadas brutas parem de cruzar os limites da API.
3. Remover ou restringir a rota remota de derivação de chave confidencial e classificar os corpos produtores de sementes como sensíveis.
4. Alinhe as regras de sensibilidade de transporte do SDK em Swift/Kotlin/Java.
5. Decida se o saneamento de anexos precisa de uma caixa de areia real ou de uma renomeação/redefinição de escopo honesta.
6. Esclarecer e fortalecer o modelo de ameaça P2P TLS/QUIC antes que os operadores habilitem esses transportes que esperam TLS autenticado.

## Notas de validação

- `cargo check -p iroha_torii --lib --message-format short` aprovado.
- `cargo check -p iroha_p2p --message-format short` aprovado.
- `cargo deny check advisories bans sources --hide-inclusion-graph` aprovado após correr fora da sandbox; emitiu avisos de versão duplicada, mas relatou `advisories ok, bans ok, sources ok`.
- Um teste Torii focado para a rota confidencial de derivação de conjunto de chaves foi iniciado durante esta auditoria, mas não foi concluído antes da redação do relatório; a conclusão é apoiada pela inspeção direta na fonte, independentemente.