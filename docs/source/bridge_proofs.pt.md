---
lang: pt
direction: ltr
source: docs/source/bridge_proofs.md
status: needs-review
generator: scripts/sync_docs_i18n.py
source_hash: 69c9a740261d0c367d52870fc1f48775ae48307056ba9b79d2f811e0c0849f20
source_last_modified: "2026-07-11T15:09:39+04:00"
translation_last_reviewed: 2026-07-11
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

## Limites do Torii

`/v1/bridge/proofs/submit` e `/v1/bridge/messages` aplicam limites de corpo HTTP
específicos por endpoint. Autenticação, rate limit e `Content-Length` são
verificados antes de ler o corpo; corpos chunked são lidos somente até o limite
rígido. Uma requisição grande demais retorna `413`; transport/JSON malformado
retorna `400` separadamente. O payload de transação detached é limitado a
16 MiB e o payload de assinatura a 16 KiB.
