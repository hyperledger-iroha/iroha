---
lang: pt
direction: ltr
source: docs/source/ministry/agenda_council_proposal.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: d2a7a47fdf0c80d189c912baafa5d6ce81a17a4c90f2b1797e532989a56f5060
source_last_modified: "2026-01-03T18:07:57.726224+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Esquema de Proposta do Conselho da Agenda (MINFO-2a)

Referência do roteiro: **MINFO-2a — Validador de formato de proposta.**

O fluxo de trabalho do Conselho da Agenda agrupa listas negras enviadas pelos cidadãos e mudanças de política
propostas antes que os painéis de governação as analisem. Este documento define o
esquema de carga útil canônico, requisitos de evidência e regras de detecção de duplicação
consumido pelo novo validador (`cargo xtask ministry-agenda validate`) então
os proponentes podem enviar envios JSON localmente antes de carregá-los no portal.

## Visão geral da carga útil

As propostas de agenda usam o esquema `AgendaProposalV1` Norito
(`iroha_data_model::ministry::AgendaProposalV1`). Os campos são codificados como JSON quando
envio por meio de superfícies CLI/portal.

| Campo | Tipo | Requisitos |
|-------|------|-------------|
| `version` | `1` (u16) | Deve ser igual a `AGENDA_PROPOSAL_VERSION_V1`. |
| `proposal_id` | sequência (`AC-YYYY-###`) | Identificador estável; aplicado durante a validação. |
| `submitted_at_unix_ms` | u64 | Milissegundos desde a época Unix. |
| `language` | corda | Etiqueta BCP‑47 (`"en"`, `"ja-JP"`, etc.). |
| `action` | enumeração (`add-to-denylist`, `remove-from-denylist`, `amend-policy`) | Ação do Ministério solicitada. |
| `summary.title` | corda | ≤256 caracteres recomendados. |
| `summary.motivation` | corda | Por que a ação é necessária. |
| `summary.expected_impact` | corda | Resultados se a ação for aceita. |
| `tags[]` | strings minúsculas | Etiquetas de triagem opcionais. Valores permitidos: `csam`, `malware`, `fraud`, `harassment`, `impersonation`, `policy-escalation`, `terrorism`, `spam`. |
| `targets[]` | objetos | Uma ou mais entradas da família hash (veja abaixo). |
| `evidence[]` | objetos | Um ou mais anexos de evidências (veja abaixo). |
| `submitter.name` | corda | Nome de exibição ou organização. |
| `submitter.contact` | corda | E-mail, identificador Matrix ou telefone; editado de painéis públicos. |
| `submitter.organization` | string (opcional) | Visível na IU do revisor. |
| `submitter.pgp_fingerprint` | string (opcional) | Impressão digital maiúscula de 40 hexadecimais. |
| `duplicates[]` | cordas | Referências opcionais a IDs de propostas enviadas anteriormente. |

### Entradas de destino (`targets[]`)

Cada destino representa um resumo da família hash referenciado pela proposta.

| Campo | Descrição | Validação |
|-------|------------|------------|
| `label` | Nome amigável para contexto do revisor. | Não vazio. |
| `hash_family` | Identificador de hash (`blake3-256`, `sha256`, etc.). | Letras/dígitos ASCII/`-_.`, ≤48 caracteres. |
| `hash_hex` | Resumo codificado em hexadecimal minúsculo. | ≥16 bytes (32 caracteres hexadecimais) e deve ser hexadecimal válido. |
| `reason` | Breve descrição do motivo pelo qual o resumo deve ser acionado. | Não vazio. |

O validador rejeita pares `hash_family:hash_hex` duplicados dentro do mesmo
proposta e relata conflitos quando a mesma impressão digital já existe no
registro duplicado (veja abaixo).

### Anexos de evidências (`evidence[]`)

Documento de entradas de evidências onde os revisores podem buscar o contexto de apoio.| Campo | Tipo | Notas |
|-------|------|-------|
| `kind` | enumeração (`url`, `torii-case`, `sorafs-cid`, `attachment`) | Determina os requisitos de resumo. |
| `uri` | corda | URL HTTP(S), ID do caso Torii ou URI SoraFS. |
| `digest_blake3_hex` | corda | Obrigatório para os tipos `sorafs-cid` e `attachment`; opcional para outros. |
| `description` | corda | Texto opcional de formato livre para revisores. |

### Registro duplicado

Os operadores podem manter um registo das impressões digitais existentes para evitar a duplicação
casos. O validador aceita um arquivo JSON no formato:

```json
{
  "entries": [
    {
      "hash_family": "blake3-256",
      "hash_hex": "0d714bed4b7c63c23a2cf8ee9ce6c3cde1007907c427b4a0754e8ad31c91338d",
      "proposal_id": "AC-2025-014",
      "note": "Already handled in 2025-08 incident"
    }
  ]
}
```

Quando um alvo de proposta corresponde a uma entrada, o validador aborta, a menos que
`--allow-registry-conflicts` é especificado (avisos ainda são emitidos).
Use [`cargo xtask ministry-agenda impact`](impact_assessment_tooling.md) para
gerar o resumo pronto para referendo que faz referência cruzada à duplicata
instantâneos de registro e política.

## Uso da CLI

Lint uma única proposta e compare-a com um registro duplicado:

```bash
cargo xtask ministry-agenda validate \
  --proposal docs/examples/ministry/agenda_proposal_example.json \
  --registry docs/examples/ministry/agenda_duplicate_registry.json
```

Passe `--allow-registry-conflicts` para fazer downgrade de ocorrências duplicadas para avisos quando
realizando auditorias históricas.

A CLI depende do mesmo esquema Norito e dos auxiliares de validação enviados em
`iroha_data_model`, para que SDKs/portais possam reutilizar o `AgendaProposalV1::validate`
método para um comportamento consistente.

## CLI de classificação (MINFO-2b)

Referência do roteiro: **MINFO-2b — Classificação multislot e registro de auditoria.**

A lista do Conselho da Agenda agora é gerenciada por meio de classificação determinística para que os cidadãos
pode auditar de forma independente cada sorteio. Use o novo comando:

```bash
cargo xtask ministry-agenda sortition \
  --roster docs/examples/ministry/agenda_council_roster.json \
  --slots 3 \
  --seed 0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef \
  --out artifacts/ministry/agenda_sortition_2026Q1.json
```

- `--roster` — arquivo JSON descrevendo cada membro elegível:

  ```json
  {
    "format_version": 1,
    "members": [
      {
        "member_id": "citizen:ada",
        "weight": 2,
        "role": "citizen",
        "organization": "Artemis Cooperative"
      },
      {
        "member_id": "citizen:erin",
        "weight": 1,
        "role": "citizen",
        "eligible": false
      }
    ]
  }
  ```

  O arquivo de exemplo reside em
  `docs/examples/ministry/agenda_council_roster.json`. Campos opcionais (função,
  organização, contato, metadados) são capturados na folha Merkle para que os auditores
  pode comprovar a escalação que alimentou o sorteio.

- `--slots` — número de assentos no conselho a preencher.
- `--seed` — Semente BLAKE3 de 32 bytes (64 caracteres hexadecimais minúsculos) gravado no
  minutos de governança para o sorteio.
- `--out` — caminho de saída opcional. Quando omitido, o resumo JSON é impresso em
  saída padrão.

### Resumo de saída

O comando emite um blob JSON `SortitionSummary`. A saída da amostra é armazenada em
`docs/examples/ministry/agenda_sortition_summary_example.json`. Campos principais:

| Campo | Descrição |
|-------|------------|
| `algorithm` | Etiqueta de classificação (`agenda-sortition-blake3-v1`). |
| `roster_digest` | Resumos BLAKE3 + SHA-256 do arquivo de lista (usado para confirmar que as auditorias operam na mesma lista de membros). |
| `seed_hex` / `slots` | Faça eco das entradas da CLI para que os auditores possam reproduzir o sorteio. |
| `merkle_root_hex` | Raiz da árvore Merkle da lista (ajudantes `hash_node`/`hash_leaf` em `xtask/src/ministry_agenda.rs`). |
| `selected[]` | Entradas para cada slot, incluindo metadados canônicos de membros, índice elegível, índice de lista original, entropia de sorteio determinística, hash de folha e irmãos à prova de Merkle. |

### Verificando um empate1. Obtenha a lista referenciada por `roster_path` e verifique seu BLAKE3/SHA-256
   os resumos correspondem ao resumo.
2. Execute novamente a CLI com a mesma semente/slots/roster; o `selected[].member_id` resultante
   a ordem deve corresponder ao resumo publicado.
3. Para um membro específico, calcule a folha Merkle usando o membro serializado JSON
   (`norito::json::to_vec(&sortition_member)`) e dobre cada hash de prova. A final
   o resumo deve ser igual a `merkle_root_hex`. O auxiliar no resumo do exemplo mostra
   como combinar `eligible_index`, `leaf_hash_hex` e `merkle_proof[]`.

Esses artefatos satisfazem o requisito MINFO-2b de aleatoriedade verificável,
seleção k-of-m e registros de auditoria somente anexados até que a API on-chain seja conectada.

## Referência de erro de validação

`AgendaProposalV1::validate` emite variantes `AgendaProposalValidationError`
sempre que uma carga útil falha no linting. A tabela abaixo resume os mais comuns
erros para que os revisores do portal possam traduzir a saída da CLI em orientações acionáveis.| Erro | Significado | Remediação |
|-------|------------|-------------|
| `UnsupportedVersion { expected, found }` | A carga útil `version` difere do esquema suportado pelo validador. | Gere novamente o JSON usando o pacote de esquema mais recente para que a versão corresponda a `expected`. |
| `MissingProposalId` / `InvalidProposalIdFormat { value }` | `proposal_id` está vazio ou não está no formato `AC-YYYY-###`. | Preencha um identificador exclusivo seguindo o formato documentado antes de reenviar. |
| `MissingSubmissionTimestamp` | `submitted_at_unix_ms` é zero ou está ausente. | Registre o carimbo de data/hora do envio em milissegundos Unix. |
| `InvalidLanguageTag { value }` | `language` não é uma tag BCP‑47 válida. | Use uma tag padrão como `en`, `ja-JP` ou outra localidade reconhecida pelo BCP‑47. |
| `MissingSummaryField { field }` | Um de `summary.title`, `.motivation` ou `.expected_impact` está vazio. | Forneça texto não vazio para o campo de resumo indicado. |
| `MissingSubmitterField { field }` | `submitter.name` ou `submitter.contact` ausente. | Forneça os metadados do remetente ausentes para que os revisores possam entrar em contato com o proponente. |
| `InvalidTag { value }` | A entrada `tags[]` não está na lista de permissões. | Remova ou renomeie a tag para um dos valores documentados (`csam`, `malware`, etc.). |
| `MissingTargets` | A matriz `targets[]` está vazia. | Forneça pelo menos uma entrada da família hash de destino. |
| `MissingTargetLabel { index }` / `MissingTargetReason { index }` | Entrada de destino sem os campos `label` ou `reason`. | Preencha o campo obrigatório para a entrada indexada antes de reenviar. |
| `InvalidHashFamily { index, value }` | Etiqueta `hash_family` não suportada. | Restrinja nomes de famílias hash a alfanuméricos ASCII mais `-_`. |
| `InvalidHashHex { index, value }` / `TargetDigestTooShort { index }` | O resumo não é hexadecimal válido ou tem menos de 16 bytes. | Forneça um resumo hexadecimal em letras minúsculas (≥32 caracteres hexadecimais) para o destino indexado. |
| `DuplicateTarget { index, fingerprint }` | O resumo de destino duplica uma entrada anterior ou impressão digital do registro. | Remova duplicatas ou mescle as evidências de apoio em um único alvo. |
| `MissingEvidence` | Nenhum anexo de evidência foi fornecido. | Anexe pelo menos um registro de evidência vinculado ao material de reprodução. |
| `MissingEvidenceUri { index }` | Entrada de evidência faltando no campo `uri`. | Forneça o URI buscável ou o identificador de caso para a entrada de evidência indexada. |
| `MissingEvidenceDigest { index }` / `InvalidEvidenceDigest { index, value }` | A entrada de evidência que requer um resumo (SoraFS CID ou anexo) está ausente ou tem `digest_blake3_hex` inválido. | Forneça um resumo BLAKE3 minúsculo de 64 caracteres para a entrada indexada. |

## Exemplos

- `docs/examples/ministry/agenda_proposal_example.json` — canônico,
  Carga útil da proposta lint-clean com dois anexos de evidências.
- `docs/examples/ministry/agenda_duplicate_registry.json` — registro inicial
  contendo uma única impressão digital e justificativa do BLAKE3.

Reutilize esses arquivos como modelos ao integrar ferramentas do portal ou escrever CI
verifica envios automatizados.