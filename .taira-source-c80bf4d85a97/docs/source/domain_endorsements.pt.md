---
lang: pt
direction: ltr
source: docs/source/domain_endorsements.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 7c337150e6de1efa9f9480ba8126ecd5ada4ed8ee7ee8b70a95fd7f6348f9016
source_last_modified: "2026-01-03T18:08:00.700192+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Endossos de domínio

Os endossos de domínio permitem que as operadoras controlem a criação e reutilização de domínios sob uma declaração assinada pelo comitê. A carga útil de endosso é um objeto Norito registrado em cadeia para que os clientes possam auditar quem atestou qual domínio e quando.

## Formato da carga útil

-`version`: `DOMAIN_ENDORSEMENT_VERSION_V1`
- `domain_id`: identificador de domínio canônico
- `committee_id`: etiqueta do comitê legível por humanos
-`statement_hash`: `Hash::new(domain_id.to_string().as_bytes())`
- `issued_at_height` / `expires_at_height`: validade limite das alturas dos blocos
- `scope`: espaço de dados opcional mais uma janela `[block_start, block_end]` opcional (inclusiva) que **deve** cobrir a altura do bloco de aceitação
- `signatures`: assinaturas acima de `body_hash()` (endosso com `signatures = []`)
- `metadata`: metadados Norito opcionais (IDs de propostas, links de auditoria, etc.)

## Aplicação

- Os endossos são necessários quando Nexus e `nexus.endorsement.quorum > 0` estão ativados, ou quando uma política por domínio marca o domínio como necessário.
- A validação impõe vinculação de hash de domínio/instrução, versão, janela de bloco, associação de espaço de dados, expiração/idade e quórum de comitê. Os signatários devem ter chaves de consenso ativas com a função `Endorsement`. As repetições são rejeitadas por `body_hash`.
- Os endossos anexados ao registro de domínio usam a chave de metadados `endorsement`. O mesmo caminho de validação é utilizado pela instrução `SubmitDomainEndorsement`, que registra endossos para auditoria sem registrar um novo domínio.

## Comitês e políticas

- Os comitês podem ser registrados na cadeia (`RegisterDomainCommittee`) ou derivados de padrões de configuração (`nexus.endorsement.committee_keys` + `nexus.endorsement.quorum`, id = `default`).
- As políticas por domínio são configuradas por meio de `SetDomainEndorsementPolicy` (ID do comitê, `max_endorsement_age`, sinalizador `required`). Quando ausente, os padrões Nexus são usados.

## Ajudantes CLI

- Construir/assinar um endosso (gera Norito JSON para stdout):

  ```
  iroha endorsement prepare \
    --domain wonderland \
    --committee-id default \
    --issued-at-height 5 \
    --expires-at-height 25 \
    --block-start 5 \
    --block-end 15 \
    --signer-key <PRIVATE_KEY> --signer-key <PRIVATE_KEY>
  ```

- Envie um endosso:

  ```
  iroha endorsement submit --file endorsement.json
  # or: cat endorsement.json | iroha endorsement submit
  ```

- Gerenciar governança:
  -`iroha endorsement register-committee --committee-id jdga --quorum 2 --member <PK> --member <PK> [--metadata path]`
  -`iroha endorsement set-policy --domain wonderland --committee-id jdga --max-endorsement-age 1000 --required`
  -`iroha endorsement policy --domain wonderland`
  -`iroha endorsement committee --committee-id jdga`
  -`iroha endorsement list --domain wonderland`

As falhas de validação retornam strings de erro estáveis (incompatibilidade de quorum, endosso obsoleto/expirado, incompatibilidade de escopo, espaço de dados desconhecido, comitê ausente).