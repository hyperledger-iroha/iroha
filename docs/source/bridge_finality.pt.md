---
lang: pt
direction: ltr
source: docs/source/bridge_finality.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 5e28e5c38283ad6be40a0fc48e0312797f490542a143f4cefdd209aaf8099ac5
source_last_modified: "2026-07-11T20:38:35.470900+00:00"
translation_last_reviewed: 2026-07-12
---

<!--
SPDX-License-Identifier: Apache-2.0
-->

# Provas de finalidade da bridge

Este documento define o formato da primeira versão. Ele transporta a evidência
durável exata produzida pelo Sumeragi v2. O envelope tem versão de esquema `1`,
mas o protocolo de consenso contido nele é a versão `2`. Não existe projeção,
decodificador nem fallback para Sumeragi v1.

## Formato exato

`BridgeFinalityProof` (Norito ou Norito JSON) contém exatamente três campos:

```text
{ version, block_header, finality_artifact }
```

- `version` deve ser `1`;
- `block_header` é o `BlockHeader` canônico;
- `finality_artifact` é o `V2FinalityArtifact` exato e imutável persistido pelo
  caminho de aplicação do Sumeragi v2; ele incorpora de forma durável um PoP
  BLS-normal por entrada, na ordem de seu roster.

O artefato é a única fonte dos fatos de consenso. Ele inclui versões de formato
e protocolo, altura, `HeightContext` imutável completo, `BlockSubject` exato,
hash do bloco, CommitQC e PoPs alinhados ao roster. O contexto congela chain id,
limites de época, modo, CommitQC pai, roster ordenado de `ValidatorPower`,
`DualQuorum`, compromisso Nexus/AMX, layout de DA e semente do líder. O contexto
do pai que encerra uma época também
incorpora o `next_epoch_snapshot` opcional; como ele faz parte do context id, o
CommitQC do pai o autentica antes que possa autorizar o roster filho. O snapshot
finalizado também vincula seu `epoch_end_height` e os `validator_set_pops`
alinhados do próximo roster, além dos parâmetros da época. O sujeito vincula
`parent_block_hash`, `block_hash` e `payload_hash`. A prova não aceita cópias duplicadas de altura,
chain, hash, roster ou certificado.

## Fonte durável e verificação

Após aplicar o bloco, Sumeragi v2 valida e grava o artefato como sidecar Kura
imutável. A gravação é idempotente e Kura rejeita artefatos conflitantes na
mesma altura. A recuperação pode completar um sidecar ausente sem executar o
bloco novamente. O construtor lê bloco e sidecar por altura, verifica a
associação e executa o verificador canônico. Os PoPs históricos vêm do sidecar
e nunca são substituídos pelos do estado mundial mutável. Ele não usa uma
janela recente de certificados.

`verify_bridge_finality_proof` exige:

1. esquema `1`, formato do artefato `1` e protocolo Sumeragi `2`;
2. contexto, roster ponderado, quorum, pai e transição de época válidos;
3. igualdade exata de altura, context id, sujeito, hash repetido e CommitQC,
   sempre na fase `Commit`;
4. chain id esperado e altura, hash, predecessor e view recalculados do header,
   todos vinculados exatamente ao artefato;
5. um PoP BLS-normal durável e válido no artefato para cada membro do roster;
6. índices de signatários estritamente crescentes e dentro do intervalo;
7. simultaneamente pelo menos `floor(2n/3) + 1` signatários distintos e poder
   assinado estritamente maior que dois terços do total;
8. a assinatura BLS agregada sobre o preimage exato do voto v2.

O preimage usa o domínio `iroha:sumeragi:v2:vote` e codifica em Norito
`{ protocol_version: 2, round: { context_id, height, view }, phase: Commit,
subject: { parent_block_hash, block_hash, payload_hash } }`. Índice e assinatura
individual ficam fora; a lista ordenada do CommitQC seleciona chaves e PoPs. A
verificação BLS/PoP é sempre obrigatória.

## Âncora de confiança e sucessores

Uma prova isolada demonstra coerência criptográfica sob o roster que carrega,
mas não que esse roster seja canônico. Por isso, `BridgeFinalityVerifier` exige
um `HeightContextId` explicitamente confiável antes da primeira prova e nunca
aprende confiança dela. Depois só aceita a altura imediatamente seguinte,
verifica o CommitQC pai com o contexto e PoPs anteriores e aplica as regras de
transição v2. Dentro de uma época, o filho copia os PoPs alinhados do artefato
anterior; no limite, época, roster, quorum, semente e PoPs devem corresponder ao
`next_epoch_snapshot` do contexto pai, incluindo seu `epoch_end_height`, tudo
autenticado pelo CommitQC pai.
Alturas antigas, puladas ou sem vínculo são rejeitadas.

## Limite de confiança do SCCP

`TairaSccpMessageProofV1.finality_proof` é a codificação Norito do mesmo tipo;
SCCP não possui outro transcript nem outro cálculo de quorum. Header, raiz SCCP
e ramo Merkle autenticam a mensagem. A prova bruta só estabelece coerência sob
seu roster congelado.

A confiança vem do `SccpSoraFinalityAnchorV1` governado: rede Taira exata,
protocolo `2`, hash do chain id, altura/hash do checkpoint,
`checkpoint_context_id` e hash com domínio separado do artefato durável. O
circuito semântico expõe o hash da âncora como último sinal público. A admissão
deve autenticar o artefato do checkpoint e verificar cada sucessor imediato até
o artefato da mensagem, ou comparar os mesmos artefatos locais confiáveis. Uma
assinatura válida sob roster fornecido pela mensagem não prova finalidade de
Taira.

## Bundle e API

`BridgeFinalityBundle` contém exatamente `{ commitment, finality_proof }`. O
compromisso é exatamente
`{ chain_id, height_context_id, block_height, block_hash }`. SCCP usa seu ramo
Merkle tipado e sua âncora governada.

- `GET /v1/bridge/finality/{height}` retorna `BridgeFinalityProof`.
- `GET /v1/bridge/finality/bundle/{height}` retorna `BridgeFinalityBundle`.

As duas rotas falham de forma fechada se o bloco ou sidecar v2 exato estiver
ausente ou inválido. Consumidores da primeira versão devem rejeitar formatos ou
versões desconhecidos; não há fallback de compatibilidade.
