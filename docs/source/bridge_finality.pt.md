---
lang: pt
direction: ltr
source: docs/source/bridge_finality.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 2e4c6ed5974f623906f51259a634bcad5df703bcec899630ae29f4669b289ab6
source_last_modified: "2026-01-08T21:52:45.509525+00:00"
translation_last_reviewed: 2026-01-08
---

<!--
SPDX-License-Identifier: Apache-2.0
-->

# Provas de finalizacao de bridge

Este documento descreve a superficie inicial de provas de finalizacao de bridge para Iroha.
O objetivo e permitir que cadeias externas ou light clients verifiquem que um bloco Iroha
esta finalizado sem computacao off-chain ou relays confiaveis.

## Formato de prova

`BridgeFinalityProof` (Norito/JSON) contem:

- `height`: altura do bloco.
- `chain_id`: identificador de cadeia Iroha para evitar replay entre cadeias.
- `block_header`: `BlockHeader` canonico.
- `block_hash`: hash do header (clientes recomputam para validar).
- `commit_certificate`: conjunto de validadores + assinaturas que finalizaram o bloco.
- `validator_set_pops`: provas de posse (PoP) alinhadas com a ordem do validator set
  (necessarias para verificacao BLS agregada).

A prova e autocontida; nenhum manifest externo ou blob opaco e necessario.
Retencao: Torii serve provas de finalizacao para a janela recente de commit-certificate
(limitada pelo cap de historico configurado; padrao de 512 entradas via
`sumeragi.commit_cert_history_cap` / `SUMERAGI_COMMIT_CERT_HISTORY_CAP`). Clientes
devem cachear ou ancorar provas se precisarem de horizontes mais longos.
A tupla canonica e `(block_header, block_hash, commit_certificate)`: o hash do header deve
corresponder ao hash dentro do commit certificate, e o chain id vincula a prova a um unico
ledger. Servidores rejeitam e registram `CommitCertificateHashMismatch` quando o certificate
aponta para um hash de bloco diferente.

## Commitment bundle

`BridgeFinalityBundle` (Norito/JSON) estende a prova basica com um commitment e justificativa
explicitos:

- `commitment`: `{ chain_id, authority_set { id, validator_set, validator_set_hash, validator_set_hash_version }, block_height, block_hash, mmr_root?, mmr_leaf_index?, mmr_peaks?, next_authority_set? }`
- `justification`: assinaturas do authority set sobre o payload do commitment
  (reutiliza as assinaturas do commit certificate).
- `block_header`, `commit_certificate`: iguais a prova basica.

Placeholder atual: `mmr_root`/`mmr_peaks` sao derivados recomputando um MMR de hash de bloco em
memoria; proofs de inclusao ainda nao sao retornadas. Clientes ainda podem verificar o mesmo
hash via o payload de commitment hoje.

MMR peaks are ordered left to right. Recompute `mmr_root` by bagging peaks
from right to left: `root = H(p_n, H(p_{n-1}, ... H(p_1, p_0)))`.

API: `GET /v2/bridge/finality/bundle/{height}` (Norito/JSON).

A verificacao e analoga a prova basica: recompute `block_hash` a partir do header, verifique
as assinaturas do commit certificate e confirme que os campos do commitment correspondem ao
certificate e ao hash do bloco. O bundle adiciona um wrapper de commitment/justification para
protocolos de bridge que preferem a separacao.

## Passos de verificacao

1. Recompute `block_hash` a partir de `block_header`; rejeite se nao coincidir.
2. Verifique que `commit_certificate.block_hash` corresponde ao `block_hash` recomputado;
   rejeite pares header/commit certificate em mismatch.
3. Verifique que `chain_id` corresponde a cadeia Iroha esperada.
4. Recompute `validator_set_hash` de `commit_certificate.validator_set` e verifique que
   corresponde ao hash/versao registrados.
5. Verifique que o comprimento de `validator_set_pops` corresponde ao validator set e valide
   cada PoP contra sua chave publica BLS.
6. Verifique assinaturas no commit certificate contra o hash do header usando as chaves
   publicas e indices de validadores referenciados; imponha quorum (`2f+1` quando `n>3`,
   caso contrario `n`) e rejeite indices duplicados/fora do intervalo.
7. Opcionalmente vincule a um checkpoint confiavel comparando o hash do validator set com
   um valor ancorado (anchor de weak-subjectivity).
8. Opcionalmente vincule a um anchor de epoch esperado para rejeitar provas de epochs mais
   antigos/novos ate que o anchor seja rotacionado intencionalmente.

`BridgeFinalityVerifier` (em `iroha_data_model::bridge`) aplica essas checagens, rejeitando
chain-id/height drift, mismatches de hash/versao do validator set, PoPs ausentes ou invalidas,
signatarios duplicados/fora do intervalo, assinaturas invalidas e epochs inesperados antes de
contar quorum para que light clients possam reutilizar um unico verificador.

## Verificador de referencia

`BridgeFinalityVerifier` aceita um `chain_id` esperado mais anchors opcionais de validator set
e epoch. Ele aplica o tuple header/block-hash/commit-certificate, valida hash/versao do validator
set, verifica assinaturas/quorum contra o roster de validadores anunciado e rastreia a ultima
altura para rejeitar provas antigas/puladas. Quando anchors sao fornecidos, ele rejeita replays
entre epochs/rosters com erros `UnexpectedEpoch`/`UnexpectedValidatorSet`; sem anchors ele adota
o hash do validator set e o epoch da primeira prova antes de continuar a impor erros
 deterministas para assinaturas duplicadas/fora do intervalo/insuficientes.

## Superficie de API

- `GET /v2/bridge/finality/{height}` - retorna `BridgeFinalityProof` para a altura de bloco
  solicitada. A negociacao de conteudo via `Accept` suporta Norito ou JSON.
- `GET /v2/bridge/finality/bundle/{height}` - retorna `BridgeFinalityBundle`
  (commitment + justification + header/certificate) para a altura solicitada.

## Notas e follow-ups

- As provas atualmente sao derivadas de commit certificates armazenados. O historico limitado
  segue a janela de retencao do commit certificate; clientes devem cachear provas de ancoragem
  se precisarem de horizontes mais longos. Requisicoes fora da janela retornam
  `CommitCertificateNotFound(height)`; exponha o erro e recorra a um checkpoint ancorado.
- Uma prova reusada ou forjada com `block_hash` em mismatch (header vs. certificate) e rejeitada
  com `CommitCertificateHashMismatch`; clientes devem executar a mesma checagem de tupla antes de
  verificar assinaturas e descartar payloads em mismatch.
- Trabalho futuro pode adicionar MMR/authority-set commitment chains para reduzir o tamanho das
  provas para historicos muito longos. O commit certificate passa a ser encapsulado em envelopes
  de commitment mais ricos.
