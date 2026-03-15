---
lang: pt
direction: ltr
source: docs/source/connect_architecture_strawman.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: a1a6bcc6bca3d7f70b82e35734b71d706ac46d8dc9c728351fabbd8a61dd3f31
source_last_modified: "2026-01-04T10:50:53.610255+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Conectar arquitetura de sessão Strawman (Swift/Android/JS)

Esta proposta de espantalho descreve o design compartilhado para fluxos de trabalho do Nexus Connect
nos SDKs Swift, Android e JavaScript. Pretende-se apoiar a
Workshop entre SDKs de fevereiro de 2026 e captura de perguntas abertas antes da implementação.

> Última atualização: 29/01/2026  
> Autores: Líder Swift SDK, Android Networking TL, Líder JS  
> Status: Rascunho para revisão do conselho (modelo de ameaça + alinhamento de retenção de dados adicionado em 12/03/2026)

## Metas

1. Alinhar carteira ↔ ciclo de vida da sessão dApp, incluindo inicialização de conexão,
   aprovações, solicitações de assinatura e desmontagem.
2. Defina o esquema de envelope Norito (abrir/aprovar/assinar/controlar) compartilhado por todos
   SDKs e garantir paridade com `connect_norito_bridge`.
3. Dividir responsabilidades entre transporte (WebSocket/WebRTC), criptografia
   (Norito Connect frames + troca de chaves) e camadas de aplicação (fachadas SDK).
4. Garanta um comportamento determinístico em plataformas desktop/móveis, incluindo
   buffer off-line e reconexão.

## Ciclo de vida da sessão (alto nível)

```
┌────────────┐      ┌─────────────┐      ┌────────────┐
│  dApp SDK  │←────→│  Connect WS │←────→│ Wallet SDK │
└────────────┘      └─────────────┘      └────────────┘
      │                    │                    │
      │ 1. open (app→wallet) frame (metadata, permissions, chain_id)
      │────────────────────────────────────────>│
      │                    │                    │
      │                    │ 2. route frame     │
      │                    │────────────────────│
      │                    │                    │
      │                    │     3. approve frame (wallet pk, account,
      │                    │        permissions, proof/attest)
      │<────────────────────────────────────────│
      │                    │                    │
      │ 4. sign request    │                    │
      │────────────────────────────────────────>│
      │                    │                    │
      │                    │ 5. sign result     │
      │                    │────────────────────│
      │                    │                    │
      │ 6. control frames for reject/close, error propagation, heartbeats.
```

## Envelope/Esquema Norito

Todos os SDKs DEVEM usar o esquema canônico Norito definido em `connect_norito_bridge`:

- `EnvelopeV1` (abrir/aprovar/assinar/controlar)
- `ConnectFrameV1` (quadros de texto cifrado com carga útil AEAD)
- Códigos de controle:
  - `open_ext` (metadados, permissões)
  - `approve_ext` (conta, permissões, provas, assinatura)
  -`reject`, `close`, `ping/pong`, `error`

A Swift já enviava codificadores JSON de espaço reservado (`ConnectCodec.swift`). Em abril de 2026, o SDK
sempre usa a ponte Norito e falha ao fechar quando o XCFramework está faltando, mas esse espantalho
ainda captura o mandato que levou à integração da ponte:

| Função | Descrição | Estado |
|----------|-------------|--------|
| `connect_norito_encode_control_open_ext` | quadro aberto dApp | Implementado em ponte |
| `connect_norito_encode_control_approve_ext` | Aprovação da carteira | Implementado |
| `connect_norito_encode_envelope_sign_request_tx/raw` | Assinar solicitações | Implementado |
| `connect_norito_encode_envelope_sign_result_ok/err` | Assinar resultados | Implementado |
| `connect_norito_decode_*` | Análise de carteiras/dApps | Implementado |

### Trabalho Necessário

- Swift: Substitua os auxiliares JSON do espaço reservado `ConnectCodec` por chamadas de ponte e superfície
  wrappers digitados (`ConnectFrame`, `ConnectEnvelope`) usando os tipos Norito compartilhados. ✅ (abril de 2026)
- Android/JS: Certifique-se de que existam os mesmos wrappers; alinhar códigos de erro e chaves de metadados.
- Compartilhado: criptografia de documentos (troca de chaves X25519, AEAD) com derivação de chave consistente
  de acordo com a especificação Norito e fornece exemplos de testes de integração usando a ponte Rust.

## Contrato de Transporte- Transporte primário: WebSocket (`/v1/connect/ws?sid=<session_id>`).
- Futuro opcional: WebRTC (TBD) – fora do escopo do espantalho inicial.
- Estratégia de reconexão: back-off exponencial com jitter total (base 5s, máximo 60s); constantes compartilhadas em Swift, Android e JS para que as novas tentativas permaneçam previsíveis.
- Cadência de ping/pong: batimentos cardíacos de 30s com tolerância para três pongs perdidos antes de reconectar; JS limita o intervalo mínimo em 15s para satisfazer as regras de limitação do navegador.
- Ganchos de push: o SDK da carteira Android expõe a integração opcional do FCM para ativações, enquanto o JS permanece baseado em pesquisas (limitações documentadas para permissões de push do navegador).
- Responsabilidades do SDK:
  - Mantenha os batimentos cardíacos do pingue-pongue (evite descarregar as baterias do celular).
  - Buffer de quadros de saída quando offline (fila limitada, persistida para dApp).
- Fornece API de fluxo de eventos (Swift Combine `AsyncStream`, Android Flow, iter assíncrono JS).
- Ganchos de reconexão de superfície e permitem a nova assinatura manual.
- Redação de telemetria: emite apenas contadores de nível de sessão (hash `sid`, direção,
  janela de sequência, profundidade da fila) com sais documentados na telemetria do Connect
  guia; cabeçalhos/chaves nunca devem aparecer em logs ou strings de depuração.

## Criptografia e gerenciamento de chaves

### Identificadores e sais de sessão

- `sid` é um identificador de 32 bytes derivado de `BLAKE2b-256("iroha-connect|sid|" || chain_id || app_ephemeral_pk || nonce16)`.  
  DApps calculam antes de chamar `/v1/connect/session`; as carteiras ecoam em quadros `approve` para que ambos os lados possam digitar diários e telemetria de forma consistente.
- O mesmo salt alimenta todas as etapas de derivação de chave para que os SDKs nunca dependam da entropia coletada da plataforma host.

### Manipulação de chaves efêmeras

- Cada sessão usa material chave X25519 novo.  
  Swift o armazena no Keychain/Secure Enclave via `ConnectCrypto`, as carteiras Android são padronizadas como StrongBox (retrocedendo para keystores apoiados por TEE) e JS requer uma instância WebCrypto de contexto seguro ou o plug-in nativo `iroha_js_host`.
- Os quadros abertos incluem a chave pública efêmera do dApp, além de um pacote de atestado opcional. As aprovações da carteira retornam a chave pública da carteira e qualquer atestado de hardware necessário para fluxos de conformidade.
- As cargas de atestado seguem o esquema aceito:  
  `attestation { platform, evidence_b64, statement_hash }`.  
  Os navegadores podem omitir o bloqueio; carteiras nativas o incluem sempre que chaves apoiadas por hardware estão em uso.

### Teclas direcionais e AEAD

- Os segredos compartilhados são expandidos com HKDF-SHA256 (por meio dos auxiliares da ponte Rust) e strings de informações separadas por domínio:
  - `iroha-connect|k_app` → tráfego do aplicativo→carteira.
  - `iroha-connect|k_wallet` → carteira→tráfego de aplicativos.
- AEAD é ChaCha20-Poly1305 para o envelope v1 (`connect_norito_bridge` expõe auxiliares em todas as plataformas).  
  Os dados associados são iguais a `("connect:v1", sid, dir, seq_le, kind=ciphertext)`, portanto, é detectada violação nos cabeçalhos.
- Nonces são derivados do contador de sequência de 64 bits (`nonce[0..4]=0`, `nonce[4..12]=seq_le`). Os testes auxiliares compartilhados garantem que as conversões BigInt/UInt se comportem de forma idêntica em todos os SDKs.

### Aperto de mão de rotação e recuperação- A rotação permanece opcional, mas o protocolo é definido: os dApps emitem um quadro `Control::RotateKeys` quando os contadores de sequência se aproximam do wrap guard, as carteiras respondem com a nova chave pública mais uma confirmação assinada e ambos os lados derivam imediatamente novas chaves direcionais sem fechar a sessão.
- A perda de chave do lado da carteira aciona o mesmo handshake seguido por um controle `resume` para que os dApps saibam como liberar o texto cifrado em cache que tinha como alvo a chave retirada.

Para substitutos históricos do CryptoKit, consulte `docs/connect_swift_ios.md`; Kotlin e JS têm referências correspondentes em `docs/connect_kotlin_ws*.md`.

## Permissões e provas

- Os manifestos de permissão devem percorrer a estrutura Norito compartilhada exportada pela ponte.  
  Campos:
  - `methods` — verbos (`sign_transaction`, `sign_raw`, `submit_proof`,…).  
  - `events` — assinaturas às quais o dApp pode se anexar.  
  - `resources` — filtros opcionais de contas/ativos para que as carteiras possam acessar o escopo.  
  - `constraints` — ID de cadeia, TTL ou botões de política personalizados que a carteira aplica antes de assinar.
- Os metadados de conformidade acompanham as permissões:
  - `attachments[]` opcional contém referências de anexo Norito (pacotes KYC, recibos de reguladores).  
  - `compliance_manifest_id` vincula a solicitação a um manifesto previamente aprovado para que os operadores possam auditar a procedência.
- As respostas da carteira usam os códigos acordados:
  -`user_declined`, `permissions_mismatch`, `compliance_failed`, `internal_error`.  
  Cada um pode carregar um `localized_message` para dicas de UI, além de um `reason_code` legível por máquina.
- Os quadros de aprovação incluem a conta/controlador selecionado, eco de permissão, pacote de provas (prova ou atestado ZK) e quaisquer alternâncias de política (por exemplo, `offline_queue_enabled`).  
  As rejeições espelham o mesmo esquema com `proof` vazio, mas ainda registram o `sid` para auditabilidade.

## Fachadas SDK

| SDK | API proposta | Notas |
|-----|-------------|-------|
| Rápido | `ConnectClient`, `ConnectSession`, `ConnectRequest`, `ConnectApproval` | Substitua os espaços reservados por wrappers digitados + fluxos assíncronos. |
| Android | Corrotinas Kotlin + classes seladas para frames | Alinhe-se com a estrutura Swift para portabilidade. |
| JS | Iteradores assíncronos + enumerações TypeScript para tipos de quadros | Fornece SDK compatível com bundler (navegador/nó). |

### Comportamentos comuns- `ConnectSession` orquestra o ciclo de vida:
  1. Estabeleça o WebSocket e execute o handshake.
  2. Troque frames abertos/aprovados.
  3. Lidar com solicitações/respostas de sinalização.
  4. Emita eventos para a camada de aplicação.
- Fornecer ajudantes de alto nível:
  -`requestSignature(tx, metadata)`
  -`approveSession(account, permissions)`
  -`reject(reason)`
  - `cancelRequest(hash)` – emite um quadro de controle reconhecido pela carteira.
- Tratamento de erros: mapeie códigos de erro Norito para erros específicos do SDK; incluir
  códigos específicos de domínio para UI usando a taxonomia compartilhada (`Transport`, `Codec`, `Authorization`, `Timeout`, `QueueOverflow`, `Internal`). A implementação básica do Swift + guia de telemetria reside em [`connect_error_taxonomy.md`](connect_error_taxonomy.md) e é a referência para paridade Android/JS.
- Emite ganchos de telemetria para profundidade da fila, contagens de reconexão e latência de solicitação (`connect.queue_depth`, `connect.reconnects_total`, `connect.latency_ms`).
 
## Números de sequência e controle de fluxo

- Cada direção mantém um contador `sequence` dedicado de 64 bits que começa em zero quando a sessão é aberta. Os tipos auxiliares compartilhados fixam os incrementos e acionam um handshake `ConnectError.sequenceOverflow` + rotação de teclas bem antes do contador terminar.
- Nonces e dados associados fazem referência ao número de sequência, para que as duplicatas possam ser rejeitadas sem analisar cargas úteis. Os SDKs devem armazenar `{sid, dir, seq, payload_hash}` em seus diários para tornar a desduplicação determinística nas reconexões.
- As carteiras anunciam a contrapressão através de uma janela lógica (quadros de controle `FlowControl`). Os DApps são retirados da fila apenas quando um token de janela está disponível; as carteiras emitem novos tokens após processar o texto cifrado para manter os pipelines limitados.
- A negociação de retomada é explícita: ambos os lados emitem `Control::Resume { seq_app_max, seq_wallet_max, queue_depths }` após se reconectarem para que os observadores possam verificar quantos dados foram reenviados e se os periódicos contêm lacunas.
- Conflitos (por exemplo, duas cargas úteis com o mesmo `(sid, dir, seq)`, mas hashes diferentes) aumentam para `ConnectError.Internal` e forçam um novo `sid` para evitar divergência silenciosa.

## Modelo de ameaças e alinhamento de retenção de dados- **Superfícies consideradas:** Transporte WebSocket, codificação/decodificação de ponte Norito,
  persistência de diário, exportadores de telemetria e retornos de chamada voltados para aplicativos.
- **Objetivos principais:** proteger segredos de sessão (chaves X25519, chaves AEAD derivadas,
  contadores nonce/sequência) contra vazamentos em logs/telemetria, evita a reprodução e
  ataques de downgrade e retenção limitada de diários e relatórios de anomalias.
- **Mitigações codificadas:**
  - Os periódicos carregam apenas texto cifrado; os metadados armazenados são limitados a hashes, comprimento
    campos, carimbos de data/hora e números de sequência.
  - As cargas úteis de telemetria edita qualquer conteúdo de cabeçalho/carga útil e inclui apenas
    hashes salgados de `sid` mais contadores agregados; lista de verificação de redação compartilhada
    entre SDKs para paridade de auditoria.
  - Os logs de sessão são alternados e expiram após 7 dias por padrão. Exposição de carteiras
    um botão `connectLogRetentionDays` (SDK padrão 7) e documente o comportamento
    portanto, implantações regulamentadas podem fixar janelas mais rígidas.
  - Uso indevido da API Bridge (ligações ausentes, texto cifrado corrompido, sequência inválida)
    retorna erros digitados sem ecoar cargas ou chaves brutas.

As perguntas pendentes da revisão são rastreadas em `docs/source/sdk/swift/connect_workshop.md`
e será deliberado em ata do conselho; uma vez fechado, o espantalho será
promovido de rascunho para aceito.

## Buffer off-line e reconexões

### Contrato de registro no diário

Cada SDK mantém um diário somente de acréscimos por sessão para que o dApp e a carteira
pode enfileirar quadros enquanto estiver off-line, retomar sem perda de dados e fornecer evidências
para telemetria. O contrato espelha os tipos de ponte Norito para que o mesmo byte
a representação sobrevive nas pilhas móveis/JS.- Os diários vivem sob um identificador de sessão com hash (`sha256(sid)`), produzindo dois
  arquivos por sessão: `app_to_wallet.queue` e `wallet_to_app.queue`. Swift usa
  um wrapper de arquivo em sandbox, o Android armazena os arquivos via `Room`/`FileChannel`,
  e JS grava em IndexedDB; todos os formatos são binários e estáveis ​​​​endian.
- Cada registro é serializado como `ConnectJournalRecordV1`:
  -`direction: u8` (`0 = app→wallet`, `1 = wallet→app`)
  -`sequence: u64`
  - `payload_hash: [u8; 32]` (Blake3 de texto cifrado + cabeçalhos)
  -`ciphertext_len: u32`
  -`received_at_ms: u64`
  -`expires_at_ms: u64`
  - `ciphertext: [u8; ciphertext_len]` (quadro Norito exato já embalado em AEAD)
- Os diários armazenam texto cifrado literalmente. Nunca criptografamos novamente a carga útil; AEAD
  cabeçalhos já autenticam chaves de direção, então a persistência se reduz a
  sincronizando o registro anexado.
- Uma estrutura `ConnectQueueState` na memória espelha os metadados do arquivo (profundidade,
  bytes usados, sequência mais antiga/mais recente). Alimenta os exportadores de telemetria e os
  Auxiliar `FlowControl`.
- Limite de diários em 32 frames/1MiB por padrão; bater na tampa expulsa o
  entradas mais antigas (`reason=overflow`). `ConnectFeatureConfig.max_queue_len`
  substitui esses padrões por implantação.
- Os periódicos retêm os dados por 24h (`expires_at_ms`). O GC em segundo plano remove obsoleto
  segmentos avidamente para que o espaço ocupado no disco permaneça limitado.
- Segurança contra falhas: anexe, fsync e atualize o espelho de memória _antes_ de notificar
  o chamador. Na inicialização, os SDKs verificam o diretório, validam as somas de verificação dos registros,
  e reconstruir `ConnectQueueState`. A corrupção faz com que o histórico ofensivo seja
  ignorado, sinalizado por telemetria e opcionalmente colocado em quarentena para despejos de suporte.
- Como o texto cifrado já satisfaz o envelope de privacidade Norito, o único
  metadados adicionais registrados são o ID da sessão com hash. Aplicativos querendo mais
  privacidade pode optar pelo `telemetry_opt_in = false`, que armazena diários, mas
  edita exportações de profundidade de fila e desativa o compartilhamento de hash `sid` em logs.
- SDKs expõem `ConnectQueueObserver` para que carteiras/dApps possam inspecionar a profundidade da fila,
  drenos e resultados de GC; este gancho alimenta UIs de status sem analisar logs.

### Repetir e retomar a semântica

1. Ao reconectar, os SDKs emitem `Control::Resume` com `{seq_app_max,
   seq_wallet_max, queued_app, queued_wallet, journal_hash}`. O hash é o
   Resumo Blake3 do diário somente anexado para que pares incompatíveis possam detectar desvios.
2. O peer receptor compara a carga útil do currículo com seu estado, solicita
   retransmissão quando existem lacunas e reconhece quadros repetidos via
   `Control::ResumeAck`.
3. Os quadros reproduzidos sempre respeitam a ordem de inserção (`sequence` e depois o tempo de gravação).
   Os SDKs da carteira DEVEM aplicar contrapressão emitindo tokens `FlowControl` (também
   registrado em diário) para que os dApps não possam inundar a fila enquanto estiverem off-line.
4. Os diários armazenam texto cifrado literalmente, então a repetição simplesmente bombeia os bytes gravados
   de volta através do transporte e do decodificador. Nenhuma recodificação por SDK é permitida.

### Fluxo de reconexão1. O transporte restabelece o WebSocket e negocia um novo intervalo de ping.
2. O dApp reproduz os frames enfileirados em ordem, respeitando a contrapressão da carteira
   (`ConnectSession.nextControlFrame()` produz tokens `FlowControl`).
3. A carteira descriptografa os resultados armazenados em buffer, verifica a monotonicidade da sequência e
   reproduz aprovações/resultados pendentes.
4. Ambos os lados emitem um controle `resume` resumindo `seq_app_max`, `seq_wallet_max`,
   e profundidades de fila para telemetria.
5. Quadros duplicados (correspondentes a `sequence` + `payload_hash`) são reconhecidos e descartados; conflitos geram `ConnectError.Internal` e acionam uma reinicialização forçada da sessão.

### Modos de falha

- Se a sessão for considerada obsoleta (`offline_timeout_ms`, padrão 5 minutos),
  os quadros em buffer são eliminados e o SDK gera `ConnectError.sessionExpired`.
- Em caso de corrupção do diário, os SDKs tentam um único reparo de decodificação Norito; ligado
  falha, eles descartam o diário e emitem telemetria `connect.queue_repair_failed`.
- A incompatibilidade de sequência aciona `ConnectError.replayDetected` e força um novo
  handshake (reinicialização da sessão com o novo `sid`).

### Plano de buffer offline e controles do operador

A entrega do workshop requer um plano documentado para que cada SDK seja enviado da mesma forma
comportamento offline, fluxo de remediação e superfícies de evidências. O plano abaixo é
comum em Swift (`ConnectSessionDiagnostics`), Android
(`ConnectDiagnosticsSnapshot`) e JS (`ConnectQueueInspector`).

| Estado | Gatilho | Resposta automática | Substituição manual | Bandeira de telemetria |
|-------|---------|--------------------|-----------------|----------------|
| `Healthy` | Uso de fila  5/min | Pausar novas solicitações de sinal, emitir tokens de controle de fluxo pela metade | Os aplicativos podem chamar `clearOfflineQueue(.app|.wallet)`; SDK reidrata o estado do peer uma vez online | Medidor `connect.queue_state=\"throttled\"`, `connect.queue_watermark` |
| `Quarantined` | Uso ≥ `disk_watermark_drop` (padrão 85%), corrupção detectada duas vezes ou `offline_timeout_ms` excedido | Pare o buffer, aumente `ConnectError.QueueQuarantined`, exija reconhecimento do operador | `ConnectSessionDiagnostics.forceReset()` exclui diários após exportar pacote | Contador `connect.queue_state=\"quarantined\"`, `connect.queue_quarantine_total` |- Os limites residem em `ConnectFeatureConfig` (`disk_watermark_warn`,
  `disk_watermark_drop`, `max_disk_bytes`, `offline_timeout_ms`). Quando um anfitrião
  omite um valor, os SDKs voltam aos padrões e registram um aviso para que as configurações
  podem ser auditados por telemetria.
- SDKs expõem `ConnectQueueObserver` além de auxiliares de diagnóstico:
  - Swift: `ConnectSessionDiagnostics.snapshot()` produz `{estado, profundidade, bytes,
    reason}` and `exportJournalBundle(url:)` persiste ambas as filas para suporte.
  - Android: `ConnectDiagnostics.snapshot()` + `exportJournalBundle(path)`.
  - JS: `ConnectQueueInspector.read()` retorna a mesma estrutura e um identificador de blob
    esse código da UI pode ser carregado nas ferramentas de suporte Torii.
- Quando um aplicativo alterna `offline_queue_enabled=false`, os SDKs são imediatamente drenados e
  limpe ambos os diários, marque o estado como `Disabled` e emita um terminal
  evento de telemetria. A preferência voltada para o usuário é espelhada no Norito
  quadro de aprovação para que os pares saibam se podem retomar os quadros armazenados em buffer.
- Operadores executam `connect queue inspect --sid <sid>` (wrapper CLI em torno do SDK
  diagnóstico) durante testes de caos; este comando imprime as transições de estado,
  marcar o histórico e retomar as evidências para que as revisões de governança não dependam de
  ferramentas específicas da plataforma.

### Fluxo de trabalho do pacote de evidências

As equipes de suporte e conformidade confiam em evidências determinísticas durante a auditoria
comportamento off-line. Cada SDK, portanto, implementa a mesma exportação em três etapas:

1. `exportJournalBundle(..)` escreve `{app_to_wallet,wallet_to_app}.queue` mais um
   manifesto que descreve o hash de compilação, sinalizadores de recursos e marcas d’água de disco.
2. `exportQueueMetrics(..)` emite as últimas 1000 amostras de telemetria para painéis
   pode ser reconstruído offline. As amostras incluem o ID da sessão com hash quando o
   o usuário aceitou.
3. O auxiliar CLI compacta as exportações e anexa um arquivo de metadados Norito assinado
   (`ConnectQueueEvidenceV1`) para que a ingestão de Torii possa arquivar o pacote em SoraFS.

Pacotes que falham na validação são rejeitados com `connect.evidence_invalid`
telemetria para que a equipe do SDK possa reproduzir e corrigir o exportador.

## Telemetria e Diagnóstico- Emitir eventos JSON Norito por meio de exportadores OpenTelemetry compartilhados. Métricas obrigatórias:
  - `connect.queue_depth{direction}` (manômetro) alimentado por `ConnectQueueState`.
  - `connect.queue_bytes{direction}` (medidor) para área ocupada por disco.
  - `connect.queue_dropped_total{reason}` (contador) para `overflow|ttl|repair`.
  - `connect.offline_flush_total{direction}` (contador) incrementa quando há filas
    drenar sem transporte; incremento de falhas `connect.offline_flush_failed`.
  -`connect.replay_success_total`/`connect.replay_error_total`.
  - Histograma `connect.resume_latency_ms` (tempo entre reconexão e estabilização
    estado) mais `connect.resume_attempts_total`.
  - Histograma `connect.session_duration_ms` (por sessão concluída).
  - Eventos estruturados `connect.error` com `code`, `fatal`, `telemetry_profile`.
- Os exportadores DEVEM anexar etiquetas `{platform, sdk_version, feature_hash}` para
  os painéis podem ser divididos por build do SDK. O hash `sid` é opcional e somente
  emitido quando a aceitação da telemetria é verdadeira.
- Os ganchos no nível do SDK revelam os mesmos eventos para que os aplicativos possam exportar mais detalhes:
  - Rápido: `ConnectSession.addObserver(_:) -> ConnectEvent`.
  - Android: `Flow<ConnectEvent>`.
  - JS: iterador assíncrono ou retorno de chamada.
- CI gate: trabalhos Swift executam `make swift-ci`, Android usa `./gradlew sdkConnectCi`,
  e JS executa `npm run test:connect` para que telemetria/painel permaneçam verdes antes
  mesclando alterações do Connect.
- Os logs estruturados incluem o hash `sid`, `seq`, `queue_depth` e `sid_epoch`
  valores para que os operadores possam correlacionar problemas do cliente. Diários que falham no reparo emitem
  Eventos `connect.queue_repair_failed{reason}` mais um caminho de despejo de memória opcional.

### Ganchos de telemetria e evidências de governança

- `connect.queue_state` também funciona como indicador de risco do roteiro. Grupo de painéis
  por `{platform, sdk_version}` e renderizar o tempo no estado para que a governança possa provar
  evidências de exercícios mensais antes de aprovar implementações graduais.
- `connect.queue_watermark` e `connect.queue_bytes` alimentam a pontuação de risco do Connect
  (`risk.connect.offline_buffer`), que pagina automaticamente o SRE quando mais de
  5% das sessões passam >10 minutos em `Throttled`.
- Os exportadores anexam `feature_hash` a cada evento para que as ferramentas do auditor possam confirmar
  que o codec Norito + plano offline corresponda à versão revisada. CI do SDK falha
  rápido quando a telemetria relata um hash desconhecido.
- O espantalho ainda requer um apêndice de modelo de ameaça; quando as métricas excedem o
  limites de política, os SDKs emitem eventos `connect.policy_violation` resumindo o
  sid ofensivo (hashed), estado e ação resolvida (`drain|purge|quarantine`).
- As evidências capturadas via `exportQueueMetrics` chegam ao mesmo namespace SoraFS
  como os artefatos do runbook do Connect para que os revisores do conselho possam rastrear cada exercício
  retornar a amostras de telemetria específicas sem solicitar logs internos.

## Propriedade e responsabilidades do quadro| Quadro / Controle | Proprietário | Domínio de sequência | Diário persistiu? | Etiquetas de telemetria | Notas |
|-----------------|---|-----------------|--------------------|------------------|-------|
| `Control::Open` | dApp | `seq_app` | ✅ (`app_to_wallet`) | `event=open` | Carrega metadados + bitmap de permissão; as carteiras reproduzem a última abertura antes dos prompts. |
| `Control::Approve` | Carteira | `seq_wallet` | ✅ (`wallet_to_app`) | `event=approve` | Inclui conta, provas, assinaturas. Incrementos de versão de metadados registrados aqui. |
| `Control::Reject` | Carteira | `seq_wallet` | ✅ | `event=reject`, `reason` | Mensagem localizada opcional; dApp descarta solicitações de assinatura pendentes. |
| `Control::Close` (inicialização) | dApp | `seq_app` | ✅ | `event=close`, `initiator=app` | A carteira reconhece com seu próprio `Close`. |
| `Control::Close` (reconhecimento) | Carteira | `seq_wallet` | ✅ | `event=close`, `initiator=wallet` | Confirma a desmontagem; O GC remove os diários quando ambos os lados persistem no quadro. |
| `SignRequest` | dApp | `seq_app` | ✅ | `event=sign_request`, `payload_hash` | Hash de carga registrado para detecção de conflito de reprodução. |
| `SignResult` | Carteira | `seq_wallet` | ✅ | `event=sign_result`, `status=ok|err` | Inclui hash BLAKE3 de bytes assinados; falhas aumentam `ConnectError.Signing`. |
| `Control::Error` | Carteira (a maioria) / dApp (transporte) | domínio proprietário correspondente | ✅ | `event=error`, `code` | Erros fatais forçam o reinício da sessão; marcas de telemetria `fatal=true`. |
| `Control::RotateKeys` | Carteira | `seq_wallet` | ✅ | `event=rotate_keys`, `reason` | Anuncia novas chaves de direção; dApp responde com `RotateKeysAck` (registrado no diário no lado do aplicativo). |
| `Control::Resume` / `ResumeAck` | Ambos | apenas domínio local | ✅ | `event=resume`, `direction=app|wallet` | Resume a profundidade da fila + estado seq; resumo de diário com hash auxilia no diagnóstico. |

- As chaves de cifra direcionais permanecem simétricas por função (`app→wallet`, `wallet→app`).
  As propostas de rotação de carteira são anunciadas via `Control::RotateKeys` e dApps
  reconhecer emitindo `Control::RotateKeysAck`; ambos os quadros devem atingir o disco
  antes da troca de chaves para evitar lacunas de repetição.
- O anexo de metadados (ícones, nomes localizados, provas de conformidade) é assinado por
  a carteira e armazenada em cache pelo dApp; atualizações exigem um novo quadro de aprovação com
  incrementado `metadata_version`.
- A matriz de propriedade acima é referenciada nos documentos do SDK, portanto CLI/web/automation
  os clientes seguem os mesmos padrões de contrato e instrumentação.

## Perguntas abertas

1. **Descoberta de sessão**: precisamos de códigos QR/handshake fora de banda como o WalletConnect? (Trabalho futuro.)
2. **Multisig**: como as aprovações de múltiplas assinaturas são representadas? (Estenda o resultado do sinal para suportar múltiplas assinaturas.)
3. **Conformidade**: Quais campos são obrigatórios para fluxos regulamentados (por roadmap)? (Aguarde a orientação da equipe de conformidade.)
4. **Embalagem do SDK**: Devemos fatorar o código compartilhado (por exemplo, codecs Norito Connect) em uma caixa de plataforma cruzada? (A definir.)

## Próximas etapas- Divulgue este espantalho ao conselho do SDK (reunião de fevereiro de 2026).
- Colete feedback sobre questões abertas e atualize o documento adequadamente.
- Programar detalhamento de implementação por SDK (marcos Swift IOS7, Android AND7, JS Connect).
- Acompanhe o progresso por meio da lista de prioridades do roteiro; atualizar `status.md` assim que o espantalho for ratificado.