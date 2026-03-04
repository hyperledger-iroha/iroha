---
lang: pt
direction: ltr
source: docs/source/connect_error_taxonomy.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: bd6ab18725af866ffd2589a3fa7bd7130bab1d5efef2920408baa8e338d5d068
source_last_modified: "2026-01-03T18:08:01.837162+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Taxonomia de erros de conexão (linha de base do Swift)

Esta nota rastreia o IOS-CONNECT-010 e documenta a taxonomia de erro compartilhada para
Nexus Conectar SDKs. Swift agora exporta o wrapper canônico `ConnectError`,
que mapeia todas as falhas relacionadas ao Connect para uma das seis categorias, portanto telemetria,
painéis e cópia UX permanecem alinhados entre plataformas.

> Última atualização: 15/01/2026  
> Proprietário: Swift SDK Lead (administrador de taxonomia)  
> Status: Implementações Swift + Android + JavaScript **chegadas**; integração de fila pendente.

## Categorias

| Categoria | Finalidade | Fontes típicas |
|----------|------------|-----------------|
| `transport` | Falhas de transporte WebSocket/HTTP que encerram uma sessão. | `URLError(.cannotConnectToHost)`, `ConnectClient.ClientError.closed` |
| `codec` | Falhas de serialização/ponte durante a codificação/decodificação de quadros. | `ConnectEnvelopeError.invalidPayload`, `DecodingError` |
| `authorization` | Falhas de TLS/atestado/política que exigem correção do usuário ou operador. | `URLError(.secureConnectionFailed)`, Torii respostas 4xx |
| `timeout` | Expirações inativas/off-line e watchdogs (fila TTL, tempo limite de solicitação). | `URLError(.timedOut)`, `ConnectQueueError.expired` |
| `queueOverflow` | Sinais de contrapressão FIFO para que os aplicativos possam liberar carga normalmente. | `ConnectQueueError.overflow(limit:)` |
| `internal` | Todo o resto: uso indevido do SDK, ponte Norito ausente, diários corrompidos. | `ConnectSessionError.missingDecryptionKeys`, `ConnectCryptoError.*` |

Cada SDK publica um tipo de erro que está em conformidade com a taxonomia e expõe
atributos de telemetria estruturada: `category`, `code`, `fatal` e opcionais
metadados (`http_status`, `underlying`).

## Mapeamento rápido

Swift exporta `ConnectError`, `ConnectErrorCategory` e protocolos auxiliares em
`IrohaSwift/Sources/IrohaSwift/ConnectError.swift`. Todos os erros de conexão pública
tipos estão em conformidade com `ConnectErrorConvertible`, então os aplicativos podem chamar `error.asConnectError()`
e encaminhar o resultado para camadas de telemetria/registro.| Erro rápido | Categoria | Código | Notas |
|------------|----------|------|-------|
| `ConnectClient.ClientError.alreadyStarted` | `internal` | `client.already_started` | Indica duplo `start()`; erro do desenvolvedor. |
| `ConnectClient.ClientError.closed` | `transport` | `client.closed` | Gerado ao enviar/receber após um fechamento. |
| `ConnectClient.ClientError.unknownPayload` | `codec` | `client.unknown_payload` | O WebSocket entregou carga textual enquanto esperava binário. |
| `ConnectSessionError.streamEnded` | `transport` | `session.stream_ended` | A contraparte fechou o fluxo inesperadamente. |
| `ConnectSessionError.missingDecryptionKeys` | `internal` | `session.missing_direction_keys` | O aplicativo esqueceu de configurar chaves simétricas. |
| `ConnectEnvelopeError.invalidPayload` | `codec` | `envelope.invalid_payload` | Carga útil Norito faltando campos obrigatórios. |
| `ConnectEnvelopeError.unknownPayloadKind` | `codec` | `envelope.unknown_payload_kind` | Carga futura vista pelo SDK antigo. |
| `ConnectCodecError.*` | `codec` | `codec.bridge_unavailable` / `codec.encode_failed` / `codec.decode_failed` | Ponte Norito ausente ou falha ao codificar/decodificar bytes do quadro. |
| `ConnectCryptoError.*` | `internal` | `crypto.*` | Ponte indisponível ou comprimentos de chave incompatíveis. |
| `ConnectQueueError.overflow(limit:)` | `queueOverflow` | `queue.overflow` | O comprimento da fila offline excedeu o limite configurado. |
| `URLError(.timedOut)` | `timeout` | `network.timeout` | Surgido por `URLSessionWebSocketTask`. |
| `URLError` Estojos TLS | `authorization` | `network.tls_failure` | Falhas na negociação ATS/TLS. |
| `DecodingError` / `EncodingError` | `codec` | `codec.*` | A decodificação/codificação JSON falhou em outro lugar no SDK; mensagem usa o contexto do decodificador Swift. |
| Qualquer outro `Error` | `internal` | `unknown_error` | Pega-tudo garantido; espelhos de mensagem `LocalizedError`. |

Testes de unidade (`IrohaSwift/Tests/IrohaSwiftTests/ConnectErrorTests.swift`) bloqueados
o mapeamento para que futuros refatoradores não possam alterar silenciosamente categorias ou códigos.

### Exemplo de uso

```swift
do {
    try await session.nextEnvelope()
} catch {
    let connectError = error.asConnectError()
    telemetry.emit(event: "connect.error",
                   attributes: connectError.telemetryAttributes(fatal: true))
    logger.error("Connect failure: \(connectError.code) – \(connectError.message)")
}
```

## Telemetria e painéis

O Swift SDK fornece `ConnectError.telemetryAttributes(fatal:httpStatus:)`
que retorna o mapa de atributos canônicos. Os exportadores devem encaminhar estes
atributos em eventos `connect.error` OTEL com extras opcionais:

```
attributes = {
  "category": "transport",
  "code": "client.closed",
  "fatal": "true",
  "http_status": "101" // optional
  "underlying": "URLError(Code=-1005 ...)" // optional
}
```

Os painéis correlacionam os contadores `connect.error` com a profundidade da fila (`connect.queue_depth`)
e reconecte histogramas para detectar regressões sem explorar registros.

## Mapeamento AndroidO Android SDK exporta `ConnectError`, `ConnectErrorCategory`, `ConnectErrorTelemetryOptions`,
`ConnectErrorOptions` e os utilitários auxiliares em
`org.hyperledger.iroha.android.connect.error`. Ajudantes no estilo Builder convertem qualquer `Throwable`
em uma carga útil compatível com a taxonomia, inferir categorias de exceções de transporte/TLS/codec,
e expor atributos de telemetria determinísticos para que as pilhas OpenTelemetry/sampling possam consumir o
resultado sem adaptadores personalizados.【java/iroha_android/src/main/java/org/hyperledger/iroha/android/connect/error/ConnectError.java:8】【java/iroha_android/src/main/java/org/hyperledger/iroha/android/connect/error/ConnectErrors.java:25】
`ConnectQueueError` já implementa `ConnectErrorConvertible`, emitindo o queueOverflow/timeout
categorias para condições de estouro/expiração para que a instrumentação da fila offline possa se conectar ao mesmo fluxo.【java/iroha_android/src/main/java/org/hyperledger/iroha/android/connect/error/ConnectQueueError.java:5】
O README do Android SDK agora faz referência à taxonomia e mostra como encapsular exceções de transporte
antes de emitir a telemetria, mantendo a orientação do dApp alinhada com a linha de base do Swift.【java/iroha_android/README.md:167】

## Mapeamento JavaScript

Clientes Node.js/browser importam `ConnectError`, `ConnectQueueError`, `ConnectErrorCategory` e
`connectErrorFrom()` de `@iroha/iroha-js`. O auxiliar compartilhado inspeciona códigos de status HTTP,
Códigos de erro de nó (socket, TLS, timeout), nomes `DOMException` e falhas de codec para emitir o
mesmas categorias/códigos documentados nesta nota, enquanto as definições do TypeScript modelam a telemetria
substituições de atributos para que as ferramentas possam emitir eventos OTEL sem conversão manual.【javascript/iroha_js/src/connectError.js:1】【javascript/iroha_js/index.d.ts:840】
O SDK README documenta o fluxo de trabalho e vincula-se a esta taxonomia para que as equipes de aplicativos possam
copie os trechos de instrumentação literalmente.【javascript/iroha_js/README.md:1387】

## Próximas etapas (cross-SDK)

- **Integração de filas:** assim que a fila off-line for enviada, garanta a lógica de desenfileiramento/descarte
  apresenta valores `ConnectQueueError` para que a telemetria de overflow permaneça confiável.