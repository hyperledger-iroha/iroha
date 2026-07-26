---
lang: pt
direction: ltr
source: docs/source/bridge_proofs.md
status: needs-review
generator: scripts/sync_docs_i18n.py
source_hash: 74e29801129deccb6d5640d414289c47cf13fa9e0229fb55212b6c7710d7c5f7
source_last_modified: "2026-07-12T07:38:49.568351+00:00"
translation_last_reviewed: 2026-07-12
translator: machine-assisted
---

> Este é um resumo localizado e abreviado, atualizado em 2026-07-11, não uma
> tradução normativa integral. Para os tipos exatos, contratos de API e
> requisitos de lançamento, consulte a
> [página canónica em inglês](bridge_proofs.md).

# Provas de bridge SCCP V1 — resumo abreviado

## Limite do primeiro lançamento

- SCCP V1 é uma superfície fechada: apenas Ethereum mainnet, BSC mainnet e TRON
  mainnet são suportadas, e `sora-taira` é o único destino SORA. Qualquer outro
  perfil de rede ou identidade SORA é rejeitado.
- `SubmitBridgeProof` aceita somente provas tipadas `NativeProtocol` e
  `SccpDestination`, vinculadas à rota. A submissão de payloads genéricos `Ics`
  e `TransparentZk` não está disponível e é rejeitada de forma fail-closed.

## Registro tipado e histórico

- `SccpRegistryV1` é tipado e append-only. Cada lane retém no máximo 64 revisões
  de rota e 4.096 native trust anchors. Registros nunca são removidos
  implicitamente; a próxima inclusão além do limite é rejeitada atomicamente.
- O intervalo de anchor usa uma coordenada de consenso autenticada: o finalized
  beacon slot no Ethereum e o finalized native block height no BSC/TRON. Um
  anchor antigo permanece válido até o checkpoint sucessor, inclusive, e não
  depois dele.
- O registro inbound durável conserva separadamente event/finality height e
  `anchor_interval_height`. O high-water por lane+anchor só aumenta; um
  checkpoint sucessor não pode ficar abaixo dele. A hidratação do snapshot
  recalcula o índice por completo e rejeita valores ausentes, obsoletos ou
  excedentes. A reutilização de message id e qualquer replay também são
  rejeitados.

A route de origem TRON usa a ABI exata
`transferToTaira(bytes,uint256,uint64 expectedNonce)`. A execução só tem êxito
quando `expectedNonce == transferNonce`; em seguida, grava esse mesmo valor no
payload canônico antes de incrementar o storage. A admissão native reconstrói a
chamada ABI completa a partir do recipient do payload, do valor escalado e do
nonce. Assim, o selector descontinuado de dois argumentos, um nonce antigo ou
futuro e um nonce `uint64` esgotado são rejeitados de modo seguro.

## Verificação única e limites determinísticos

- Cada prova native ou destination é decodificada canonicamente uma só vez e
  passa uma só vez pela verificação criptográfica cara. Antes disso, o consenso
  reserva uma estimativa conservadora de trabalho independente do hardware.
- `[zk.sccp]` define limites obrigatórios e não nulos por prova, transação e
  bloco para quantidade/bytes de provas, native headers, atualizações do light
  client Ethereum, bytes de headers, recuperações secp256k1, verificações e
  contribuições BLS e verificações pairing-product BN254. Esses limites de
  admissão são vinculados ao consenso e devem ser idênticos em todos os
  validadores.

## Compromisso outbound, retenção e descoberta

Cada mensagem outbound bem-sucedida recebe um `commitment_index` denso na ordem
de execução do bloco (`0..=511`). V1 fixa os limites imutáveis em 512 mensagens por
bloco e 4.096 bytes de payload canônico por mensagem. `[zk.sccp]` limita em conjunto
os payloads pendentes por `max_pending_outbound_messages` (padrão `65536`) e
`max_pending_outbound_payload_bytes` (padrão `268435456`).

Antes de publicar a finalidade ou remover o corpo do bloco, Kura mantém de forma
imutável o header canônico exato e o arquivo SCCP autenticado pela raiz. A reconstrução
de proofs, bundles, proof requests e histórico recente não lê o corpo histórico nem
uma cópia mutável do payload no WSV. Ao aceitar a destination proof, o payload pendente
e sua cobrança são removidos atomicamente e substituídos por um descritor terminal de
tamanho fixo, preservando locator/index. O estado pendente é limitado; os registros
terminais e o histórico imutável de Kura crescem deliberadamente para proteção
permanente contra replay. `GET /v1/sccp/messages/recent` usa o cursor composto
`{ from, after_index }`. A evidência imutável conta no uso total/do operador do disco,
mas fica fora do orçamento de corpos removíveis.

## Limites do Torii

`/v1/bridge/proofs/submit` e `/v1/bridge/messages` aplicam limites de corpo HTTP
específicos por endpoint. Autenticação, rate limit e `Content-Length` são
verificados antes de ler o corpo; corpos chunked são lidos somente até o limite
rígido. Uma requisição grande demais retorna `413`; transport/JSON malformado
retorna `400` separadamente. O payload de transação detached é limitado a
16 MiB e o payload de assinatura a 16 KiB.
