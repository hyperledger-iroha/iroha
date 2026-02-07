---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/chunker-profile-authoring.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: autoria de perfil chunker
título: Guia de autoria de perfis de chunker de SoraFS
sidebar_label: Guia de autoria do chunker
descrição: Checklist para propor novos perfis e acessórios de chunker de SoraFS.
---

:::nota Fonte canônica
Esta página reflete `docs/source/sorafs/chunker_profile_authoring.md`. Mantenha ambas as cópias sincronizadas até que o conjunto de documentação Sphinx herdado seja retirado.
:::

# Guia de autoria de perfis de chunker de SoraFS

Este guia explica como propor e publicar novos perfis de chunker para SoraFS.
Complementa a RFC de arquitetura (SF-1) e a referência de registro (SF-2a)
com requisitos concretos de autoridade, etapas de validação e etapas de proposta.
Para um exemplo canônico, consulte
`docs/source/sorafs/proposals/sorafs_sf1_profile_v1.json`
e o registro de simulação associado em
`docs/source/sorafs/reports/sf1_determinism.md`.

## Resumo

Cada perfil que entra no registro deve:

- anunciar parâmetros CDC deterministas e configurações de multihash idênticos entre
  arquitetura;
- entregar fixtures reproduzíveis (JSON Rust/Go/TS + corpora fuzz + testigos PoR) que
  os SDKs downstream podem verificar as ferramentas de acordo com a medida;
- incluir listas de metadados para governança (namespace, name, semver) junto com o guia de implementação
  e ventanas operativas; sim
- passar o conjunto de diferenças deterministas antes da revisão do conselho.

Siga a lista de verificação abaixo para preparar uma proposta que cumpra essas regras.

## Resumo da carta de registro

Antes de redigir uma proposta, confirme que a carta de registro aplicada está cumprida
por `sorafs_manifest::chunker_registry::ensure_charter_compliance()`:

- Os IDs de perfil são enteros positivos que aumentam de forma monótona sem tons.
- O identificador canônico (`namespace.name@semver`) deve aparecer na lista de alias
  e **debe** ser a primeira entrada. Siga o alias heredados (p. ej., `sorafs.sf1@1.0.0`).
- Nenhum alias pode colidir com outro identificador canônico e não aparecer mais de uma vez.
- Los alias devem ser não vazios e recortados de espaços em branco.

Ajudas úteis do CLI:

```bash
# Listado JSON de todos los descriptores registrados (ids, handles, aliases, multihash)
cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- --list-profiles

# Emitir metadatos para un perfil por defecto candidato (handle canónico + aliases)
cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- \
  --promote-profile=sorafs.sf1@1.0.0 --json-out=-
```

Esses comandos mantêm as propostas alinhadas com a carta de registro e as fornecem
metadados canônicos necessários nas discussões de governo.

## Metadados necessários| Campo | Descrição | Exemplo (`sorafs.sf1@1.0.0`) |
|-------|-------------|---------------|
| `namespace` | Agrupamento lógico de perfis relacionados. | `sorafs` |
| `name` | Etiqueta legível para humanos. | `sf1` |
| `semver` | Cadeia de versão semântica para o conjunto de parâmetros. | `1.0.0` |
| `profile_id` | Identificador numérico monótono atribuído uma vez que o perfil entra. Reserve o ID seguinte, mas não reutilize os números existentes. | `1` |
| `profile_aliases` | Lida com informações adicionais opcionais (nomes herdados, abreviaturas) expostas aos clientes durante a negociação. Inclua sempre o identificador canônico como a primeira entrada. | `["sorafs.sf1@1.0.0"]` |
| `profile.min_size` | Comprimento mínimo de chunk em bytes. | `65536` |
| `profile.target_size` | Longitude objetivo de chunk em bytes. | `262144` |
| `profile.max_size` | Longitud máxima de chunk em bytes. | `524288` |
| `profile.break_mask` | Máscara adaptativa usada pelo rolamento hash (hex). | `0x0000ffff` |
| `profile.polynomial` | Engrenagem constante do polinômio (hex). | `0x3da3358b4dc173` |
| `gear_seed` | Semente usada para derivar a tabela gear de 64 KiB. | `sorafs-v1-gear` |
| `chunk_multihash.code` | Código multihash para resumos por pedaço. | `0x1f` (BLAKE3-256) |
| `chunk_multihash.digest` | Resumo do pacote canônico de fixtures. | `13fa...c482` |
| `fixtures_root` | Diretório relativo ao que contém os equipamentos regenerados. | `fixtures/sorafs_chunker/sorafs.sf1@1.0.0/` |
| `por_seed` | Semente para o monumento PoR determinista (`splitmix64`). | `0xfeedbeefcafebabe` (exemplo) |

Os metadados devem aparecer tanto no documento de proposta como dentro dos jogos
regenerados para que o registro, as ferramentas de CLI e a automatização de governança possam
confirme os valores sem cruces manuais. Se houver, execute os CLIs do chunk-store e
manifest con `--json-out=-` para transmitir os metadados calculados nas notas de revisão.

### Pontos de contato da CLI e registro

- `sorafs_manifest_chunk_store --profile=<handle>` — retorna e executa metadados de pedaço,
  resume o manifesto e verifica o PoR com os parâmetros propostos.
- `sorafs_manifest_chunk_store --json-out=-` — transmite o relatório de chunk-store para
  stdout para comparações automatizadas.
- `sorafs_manifest_stub --chunker-profile=<handle>` — confirmar que manifesta e planeja CAR
  incorpore o identificador canônico além do alias.
- `sorafs_manifest_stub --plan=-` — voltar para alimentar o `chunk_fetch_specs` anterior para
  verifique compensações/resumos após a mudança.

Registre a saída de comandos (resumos, raízes PoR, hashes de manifesto) na proposta para que
os revisores podem reproduzi-los textualmente.

## Checklist de determinismo e validação1. **Regenerar equipamentos**
   ```bash
   cargo run --locked -p sorafs_chunker --bin export_vectors \
     --signature-out=fixtures/sorafs_chunker/manifest_signatures.json
   ```
2. **Executar o conjunto de paridade** — `cargo test -p sorafs_chunker` e o arnés diff
   entre idiomas (`crates/sorafs_chunker/tests/vectors.rs`) deve estar em verde com os
   novos jogos em seu lugar.
3. **Reproduzindo corpora fuzz/back-pression** — ejecuta `cargo fuzz list` e o arnés de
   streaming (`fuzz/sorafs_chunker`) contra ativos regenerados.
4. **Verificar testigos Prova de Recuperabilidade** — ejecuta
   `sorafs_manifest_chunk_store --por-sample=<n>` usando o perfil proposto e confirmando que
   raíces coincidem com o manifesto de fixtures.
5. **Teste de CI** — invoque `ci/check_sorafs_fixtures.sh` localmente; o roteiro
   você deve ter sucesso com os novos equipamentos e o `manifest_signatures.json` existente.
6. **Confirmação cross-runtime** — certifique-se de que as ligações Go/TS consomem o JSON regenerado
   e emitem limites e digere idénticos.

Documente os comandos e os resumos resultantes da proposta para que o Tooling WG possa
repetirlos sem conjecturas.

### Confirmação de manifesto / PoR

Depois de regenerar fixtures, execute o pipeline completo de manifesto para garantir que eles
metadados CAR e os testes PoR seguem consistentemente:

```bash
# Validar metadata de chunk + PoR con el nuevo perfil
cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- \
  --profile=sorafs.sf2@1.0.0 \
  --json-out=- --por-json-out=- fixtures/sorafs_chunker/input.bin

# Generar manifest + CAR y capturar chunk fetch specs
cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- \
  fixtures/sorafs_chunker/input.bin \
  --chunker-profile=sorafs.sf2@1.0.0 \
  --chunk-fetch-plan-out=chunk_plan.json \
  --manifest-out=sf2.manifest \
  --car-out=sf2.car \
  --json-out=sf2.report.json

# Reejecutar usando el plan de fetch guardado (evita offsets obsoletos)
cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- \
  fixtures/sorafs_chunker/input.bin \
  --chunker-profile=sorafs.sf2@1.0.0 \
  --plan=chunk_plan.json --json-out=-
```

Substitua o arquivo de entrada com qualquer corpus representativo usado por seus equipamentos
(p. ej., o fluxo determinista de 1 GiB) e complementa os resumos resultantes da proposta.

## Planta de propuesta

As propostas são enviadas como registros Norito `ChunkerProfileProposalV1` registrados em
`docs/source/sorafs/proposals/`. A planta JSON de baixo ilustra a forma esperada
(substitui seus valores conforme necessário):


Proporciona um relatório Markdown correspondente (`determinism_report`) que captura o
saída de comandos, resumos de pedaços e qualquer desvio encontrado durante a validação.

## Fluxo de governança

1. **Enviar PR com proposta + luminárias.** Incluir os ativos gerados, a proposta
   Norito e atualizações para `chunker_registry_data.rs`.
2. **Revisão do Tooling WG.** Os revisores analisam a lista de verificação de validação e
   confirme que a proposta está alinhada com as regras do registro (sem reutilização de id,
   determinismo satisfecho).
3. **Sobre del consejo.** Uma vez aprovado, os membros do conselho firmam o resumo de
   a proposta (`blake3("sorafs-chunker-profile-v1" || canonical_bytes)`) e anexan sus
   firmas no perfil salvo junto com os jogos.
4. **Publicação do registro.** A mesclagem atualiza o registro, os documentos e os equipamentos. El
   CLI por defeito permanece no perfil anterior até que o governo declare a migração
   lista.
5. **Seguimento da suspensão de uso.** Após a janela de migração, atualize o registro
   livro de migração.

## Conselhos de autoria- Prefira limites de potência de pares para minimizar o comportamento de chunking em casos borde.
- Evita alterar o código multihash sem coordenar consumidores de manifesto e gateway; inclua um
  nota operativa quando lo hagas.
- Mantenha as sementes da tabela legíveis para humanos, mas globalmente únicas para simplificar
  auditórios.
- Guarda qualquer artefato de benchmarking (p. ej., comparações de rendimento) abaixo
  `docs/source/sorafs/reports/` para referência futura.

Para expectativas operacionais durante a implementação, consulte o registro de migração
(`docs/source/sorafs/migration_ledger.md`). Para regras de conformidade em versão de tempo de execução
`docs/source/sorafs/chunker_conformance.md`.