---
lang: pt
direction: ltr
source: docs/source/connect_architecture_feedback.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 097ea58d49f48d059cda762cd719bc62f0b2d6f6ddecedef3f9bac030ae46aec
source_last_modified: "2026-01-03T18:07:58.674563+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

#! Lista de verificação de feedback da arquitetura do Connect

Esta lista de verificação captura as questões abertas da arquitetura de sessão do Connect
espantalho que exigem entrada dos leads Android e JavaScript antes do
Workshop sobre vários SDKs de fevereiro de 2026. Use-o para coletar comentários de forma assíncrona, rastrear
propriedade e desbloquear a agenda do workshop.

> A coluna Status/Notas capturou respostas finais de leads Android e JS a partir de
> a sincronização pré-workshop de fevereiro de 2026; vincular novas questões de acompanhamento inline se as decisões
> evoluir.

## Ciclo de vida e transporte da sessão

| Tópico | Proprietário do Android | Proprietário JS | Status/Notas |
|-------|---------------|----------|----------------|
| Estratégia de back-off de reconexão do WebSocket (exponencial vs. linear limitado) | Rede Android TL | Líder JS | ✅ Acordado back-off exponencial com jitter, limitado a 60s; JS espelha as mesmas constantes para paridade navegador/nó. |
| Padrões de capacidade de buffer offline (espantalho atual: 32 quadros) | Rede Android TL | Líder JS | ✅ Padrão de 32 quadros confirmado com substituição de configuração; O Android persiste via `ConnectQueueConfig`, JS respeita `window.connectQueueMax`. |
| Notificações de reconexão estilo push (FCM/APNS vs. polling) | Rede Android TL | Líder JS | ✅ O Android exporá o gancho FCM opcional para aplicativos de carteira; JS permanece baseado em pesquisas com recuo exponencial, observando as restrições de push do navegador. |
| Protetores de cadência de pingue-pongue para clientes móveis | Rede Android TL | Líder JS | ✅ Ping 30s padronizado com tolerância a erros de 3×; O Android equilibra o impacto do Soneca, o JS é fixado em ≥15s para evitar o afogamento do navegador. |

## Criptografia e gerenciamento de chaves

| Tópico | Proprietário do Android | Proprietário JS | Status/Notas |
|-------|---------------|----------|----------------|
| Expectativas de armazenamento de chave X25519 (StrongBox, contextos seguros WebCrypto) | Criptografia Android TL | Líder JS | ✅ Android armazena X25519 no StrongBox quando disponível (volta para TEE); JS exige WebCrypto de contexto seguro para dApps, recorrendo à ponte `iroha_js_host` nativa no Node. |
| Compartilhamento de gerenciamento nonce ChaCha20-Poly1305 entre SDKs | Criptografia Android TL | Líder JS | ✅ Adote API de contador `sequence` compartilhada com proteção wrap de 64 bits e testes compartilhados; JS usa contadores BigInt para corresponder ao comportamento do Rust. |
| Esquema de carga útil de atestado apoiado por hardware | Criptografia Android TL | Líder JS | ✅ Esquema finalizado: `attestation { platform, evidence_b64, statement_hash }`; JS opcional (navegador), Node usa gancho de plug-in HSM. |
| Fluxo de recuperação de carteiras perdidas (handshake de rotação de chaves) | Criptografia Android TL | Líder JS | ✅ Handshake de rotação de carteira aceito: dApp emite controle `rotate`, carteira responde com nova pubkey + confirmação assinada; JS recodifica o material WebCrypto imediatamente. |

## Permissões e pacotes de provas| Tópico | Proprietário do Android | Proprietário JS | Status/Notas |
|-------|---------------|----------|----------------|
| Esquema de permissão mínima (métodos/eventos/recursos) para GA | Modelo de dados Android TL | Líder JS | ✅ Linha de base GA: `methods`, `events`, `resources`, `constraints`; JS alinha os tipos TypeScript com o manifesto Rust. |
| Carga útil de rejeição de carteira (`reason_code`, mensagens localizadas) | Rede Android TL | Líder JS | ✅ Códigos finalizados (`user_declined`, `permissions_mismatch`, `compliance_failed`, `internal_error`) mais `localized_message` opcional. |
| Campos opcionais do pacote de provas (anexos de conformidade/KYC) | Modelo de dados Android TL | Líder JS | ✅ Todos os SDKs aceitam `attachments[]` (Norito `AttachmentRef`) e `compliance_manifest_id` opcionais; nenhuma mudança de comportamento necessária. |
| Alinhamento no esquema JSON Norito vs. estruturas geradas por ponte | Modelo de dados Android TL | Líder JS | ✅ Decisão: prefira estruturas geradas por pontes; O caminho JSON permanece apenas para depuração, JS mantém o adaptador `Value`. |

## Fachadas SDK e formato de API

| Tópico | Proprietário do Android | Proprietário JS | Status/Notas |
|-------|---------------|----------|----------------|
| Paridade de interfaces assíncronas de alto nível (`Flow`, iteradores assíncronos) | Rede Android TL | Líder JS | ✅ Android expõe `Flow<ConnectEvent>`; JS usa `AsyncIterable<ConnectEvent>`; ambos mapeiam para `ConnectEventKind` compartilhado. |
| Mapeamento de taxonomia de erro (`ConnectError`, subclasses digitadas) | Rede Android TL | Líder JS | ✅ Adote enum compartilhado {`Transport`, `Codec`, `Authorization`, `Timeout`, `QueueOverflow`, `Internal`} com detalhes de carga útil específicos da plataforma. |
| Semântica de cancelamento para solicitações de sinalização em voo | Rede Android TL | Líder JS | ✅ Introduzido o controle `cancelRequest(hash)`; ambos os SDKs apresentam corrotinas/promessas canceláveis ​​respeitando o reconhecimento da carteira. |
| Ganchos de telemetria compartilhados (eventos, nomenclatura de métricas) | Rede Android TL | Líder JS | ✅ Nomes de métricas alinhados: `connect.queue_depth`, `connect.latency_ms`, `connect.reconnects_total`; exportadores de amostra documentados. |

## Persistência offline e registro no diário

| Tópico | Proprietário do Android | Proprietário JS | Status/Notas |
|-------|---------------|----------|----------------|
| Formato de armazenamento para quadros enfileirados (binário Norito vs. JSON) | Modelo de dados Android TL | Líder JS | ✅ Armazene o binário Norito (`.to`) em qualquer lugar; JS usa IndexedDB `ArrayBuffer`. |
| Política de retenção de periódicos e limites de tamanho | Rede Android TL | Líder JS | ✅ Retenção padrão 24h e 1MiB por sessão; configurável via `ConnectQueueConfig`. |
| Resolução de conflitos quando ambos os lados reproduzem frames | Rede Android TL | Líder JS | ✅ Utilizar `sequence` + `payload_hash`; duplicatas ignoradas, conflitos acionam `ConnectError.Internal` com evento de telemetria. |
| Telemetria para profundidade da fila e sucesso de reprodução | Rede Android TL | Líder JS | ✅ Emite medidor `connect.queue_depth` e contador `connect.replay_success_total`; ambos os SDKs se conectam ao esquema de telemetria Norito compartilhado. |

## Picos e referências de implementação- **Acessórios da ponte Rust:** `crates/connect_norito_bridge/src/lib.rs` e testes associados cobrem os caminhos canônicos de codificação/decodificação usados ​​por cada SDK.
- **Arnês de demonstração rápida:** Exercícios `examples/ios/NoritoDemoXcode/NoritoDemoXcodeTests/ConnectViewModelTests.swift` Conecte fluxos de sessão com transportes simulados.
- **Swift CI gating:** execute `make swift-ci` ao atualizar artefatos do Connect para validar a paridade de fixtures, feeds de painel e metadados Buildkite `ci/xcframework-smoke:<lane>:device_tag` antes de compartilhar com outros SDKs.
- **Testes de integração do JavaScript SDK:** `javascript/iroha_js/test/integrationTorii.test.js` valida auxiliares de status/sessão do Connect em relação a Torii.
- **Notas de resiliência do cliente Android:** `java/iroha_android/README.md:150` documenta os experimentos de conectividade atuais que inspiraram os padrões de fila/retirada.

## Itens de preparação para workshop

- [x] Android: atribua uma pessoa pontual para cada linha da tabela acima.
- [x] JS: atribui pessoa responsável para cada linha da tabela acima.
- [x] Colete links para picos de implementação ou experimentos existentes.
- [x] Agendar revisão pré-trabalho antes do conselho de fevereiro de 2026 (reservado para 29/01/2026 às 15h UTC com Android TL, JS Lead, Swift Lead).
- [x] Atualize `docs/source/connect_architecture_strawman.md` com respostas aceitas.

## Pacote de pré-leitura

- ✅ Pacote registrado em `artifacts/connect/pre-read/20260129/` (gerado via `make docs-html` após atualizar o espantalho, os guias do SDK e esta lista de verificação).
- 📄 Resumo + etapas de distribuição ao vivo em `docs/source/project_tracker/connect_architecture_pre_read.md`; inclua o link no convite do workshop de fevereiro de 2026 e no lembrete `#sdk-council`.
- 🔁 Ao atualizar o pacote, atualize o caminho e o hash dentro da nota de pré-leitura e arquive o anúncio em `status.md` nos logs de preparação do IOS7/AND7.