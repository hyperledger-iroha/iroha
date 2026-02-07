---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/chunker-profile-authoring.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: autoria de perfil chunker
título: Guia de criação de perfis chunker SoraFS
sidebar_label: Guia de criação do chunker
descrição: Checklist para propor novos perfis, bloco SoraFS e luminárias.
---

:::nota Fonte canônica
Esta página reflete `docs/source/sorafs/chunker_profile_authoring.md`. Gardez as duas cópias sincronizadas junto com o retorno completo do conjunto Sphinx herité.
:::

# Guia de criação de perfis chunker SoraFS

Este guia proponente de comentários explícitos e editor de novos perfis para SoraFS.
A RFC de arquitetura (SF-1) completa e a referência de registro (SF-2a)
com exigências de redação concreta, etapas de validação e modelos de proposição.
Para um exemplo canônico, veja
`docs/source/sorafs/proposals/sorafs_sf1_profile_v1.json`
e o registro de simulação associado em
`docs/source/sorafs/reports/sf1_determinism.md`.

## Vista do conjunto

Cada perfil que está entre no registro doit :

- Anuncia parâmetros CDC determinados e regulamentos multihash idênticos entre
  arquiteturas;
- fornecer luminárias rejouables (JSON Rust/Go/TS + corpora fuzz + témoins PoR) que
  os SDKs disponíveis podem ser verificados sem ferramentas na medida;
- incluir metas pré-definidas para o governo (namespace, nome, semver) assim como
  conselhos de implementação e janelas operacionais; et
- passe o conjunto de diferenças determinado antes da revista.

Siga a lista de verificação aqui para preparar uma proposta que respeite essas regras.

## Aperçu de la charte du registre

Antes de registrar uma proposta, verifique se ela respeita a tabela de registro aplicada
par `sorafs_manifest::chunker_registry::ensure_charter_compliance()`:

- Les ID de perfil são todos os pontos positivos que aumentam de forma monótona sem problemas.
- Le handle canonique (`namespace.name@semver`) aparece na lista de apelidos
- Nenhum alias não pode entrar em colisão com outro identificador canônico ou aparelho mais de uma vez.
- Les alias doivent être non vides et trimés des espaces.

Utilitários CLI dos auxiliares:

```bash
# Listing JSON de tous les descripteurs enregistrés (ids, handles, aliases, multihash)
cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- --list-profiles

# Émettre des métadonnées pour un profil par défaut candidat (handle canonique + aliases)
cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- \
  --promote-profile=sorafs.sf1@1.0.0 --json-out=-
```

Estes comandos mantêm as propostas alinhadas com a tabela de registro e fornecimento
os métodos canônicos necessários às discussões de governo.

## Métadonnées requer| Campeão | Descrição | Exemplo (`sorafs.sf1@1.0.0`) |
|-------|-------------|---------------|
| `namespace` | Reagrupamento lógico de perfis liés. | `sorafs` |
| `name` | Libellé lisível. | `sf1` |
| `semver` | Cadeia de versão semântica para o conjunto de parâmetros. | `1.0.0` |
| `profile_id` | O identificador numérico monótono atribui um perfil integrado. Reserve o seguinte ID, mas não reutilize os números existentes. | `1` |
| `profile.min_size` | Longo mínimo de pedaços em bytes. | `65536` |
| `profile.target_size` | Longuure cible de chunk en bytes. | `262144` |
| `profile.max_size` | Longo máximo de pedaços em bytes. | `524288` |
| `profile.break_mask` | Máscara adaptada usada para hash rolante (hex). | `0x0000ffff` |
| `profile.polynomial` | Constante du polynôme gear (hex). | `0x3da3358b4dc173` |
| `gear_seed` | Semente utilizada para obter equipamento de mesa de 64 KiB. | `sorafs-v1-gear` |
| `chunk_multihash.code` | Codifique multihash para os resumos por parte. | `0x1f` (BLAKE3-256) |
| `chunk_multihash.digest` | Resumo do pacote canônico de fixtures. | `13fa...c482` |
| `fixtures_root` | O repertório relativo contém os jogos registrados. | `fixtures/sorafs_chunker/sorafs.sf1@1.0.0/` |
| `por_seed` | Semente para o encantamento PoR determinado (`splitmix64`). | `0xfeedbeefcafebabe` (exemplo) |

Os metadonos devem aparecer no documento de proposta e no interior do
luminárias geradas para registro, ferramentas CLI e automação de gerenciamento poderosa
confirme os valores sem recuperações manuais. Em caso de dúvida, execute as CLIs chunk-store e
manifesto com `--json-out=-` para transmitir os metadonées calculados nas notas de revista.

### Pontos de contato CLI e registro

- `sorafs_manifest_chunk_store --profile=<handle>` — relancear os metadados de pedaços,
  o resumo do manifesto e as verificações do PoR com os parâmetros propostos.
- `sorafs_manifest_chunk_store --json-out=-` — streamer le rapport chunk-store versão
  saída padrão para comparações automatizadas.
- `sorafs_manifest_stub --chunker-profile=<handle>` — confirma que os manifestos e os arquivos
  planos CAR enviou o identificador canônico e os aliases.
- `sorafs_manifest_stub --plan=-` — reinjete o `chunk_fetch_specs` anterior para
  verifica os offsets/digests após a modificação.

Consignez la sortie des commandes (digests, racines PoR, hashes de manifest) na proposição afin
que os revisores possam reproduzi-los para mais.

## Checklist de determinação e validação1. **Regenerar os jogos**
   ```bash
   cargo run --locked -p sorafs_chunker --bin export_vectors \
     --signature-out=fixtures/sorafs_chunker/manifest_signatures.json
   ```
2. **Execute o conjunto de paridade** — `cargo test -p sorafs_chunker` e o chicote diferente
   cross-linguagem (`crates/sorafs_chunker/tests/vectors.rs`) deve ser verde com eles
   novos acessórios no local.
3. **Retire os corpos fuzz/back-pression** — execute `cargo fuzz list` e o chicote de
   streaming (`fuzz/sorafs_chunker`) com ativos recuperados.
4. **Verifique os termos de prova de recuperação** - execute
   `sorafs_manifest_chunk_store --por-sample=<n>` com o perfil proposto e confirme que
   racines correspondente ao manifesto de luminárias.
5. **CI de teste** — invoquez `ci/check_sorafs_fixtures.sh` localement ; o script
   faça isso usando os novos acessórios e o `manifest_signatures.json` existente.
6. **Confirmação de tempo de execução cruzado** — certifique-se de que as ligações Go/TS consomem o JSON
   régénéré et émettent des limites et digests identiques.

Documente os comandos e os resumos resultantes na proposta para que o Tooling WG possa utilizá-lo
les rejouer sans conjecture.

### Manifesto de confirmação/PoR

Após a regeneração dos fixtures, execute o manifesto do pipeline completo para garantir que eles
métadonnées CAR et les preuves PoR restent cohérentes :

```bash
# Valider les métadonnées chunk + PoR avec le nouveau profil
cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- \
  --profile=sorafs.sf2@1.0.0 \
  --json-out=- --por-json-out=- fixtures/sorafs_chunker/input.bin

# Générer manifest + CAR et capturer les chunk fetch specs
cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- \
  fixtures/sorafs_chunker/input.bin \
  --chunker-profile=sorafs.sf2@1.0.0 \
  --chunk-fetch-plan-out=chunk_plan.json \
  --manifest-out=sf2.manifest \
  --car-out=sf2.car \
  --json-out=sf2.report.json

# Relancer avec le plan de fetch sauvegardé (évite les offsets obsolètes)
cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- \
  fixtures/sorafs_chunker/input.bin \
  --chunker-profile=sorafs.sf2@1.0.0 \
  --plan=chunk_plan.json --json-out=-
```

Substitua o arquivo de entrada por um corpus representado usado por seus equipamentos
(por exemplo, o fluxo determinado de 1 GiB) e junta os resumos resultantes à proposta.

## Modelo de proposta

As proposições são armazenadas sob a forma de registros Norito `ChunkerProfileProposalV1` depositados em
`docs/source/sorafs/proposals/`. O modelo JSON ci-dessous ilustra a forma de participação
(substitua por seus valores se necessário):


Forneça um relacionamento Markdown correspondente (`determinism_report`) que captura a sorte de
comandos, os resumos do pedaço e todas as divergências encontradas durante a validação.

## Fluxo de governança

1. **Soumettre um PR com proposta + luminárias.** Inclua os ativos genéricos, la
   proposta Norito e as mises do dia `chunker_registry_data.rs`.
2. **Revue Tooling WG.** Os revisores enviaram a lista de verificação de validação e confirmação
   que a proposta respeite as regras do registro (pas de reutilização de id,
   determinismo satisfatório).
3. **Enveloppe du conseil.** Uma vez aprovado, os membros do conselho assinaram o resumo
   da proposta (`blake3("sorafs-chunker-profile-v1" || canonical_bytes)`) e complemento
   suas assinaturas no envelope do perfil armazenado com os equipamentos.
4. **Publicação do registro.** A mesclagem com o registro, os documentos e os acessórios.
   A CLI, por padrão, permanece no perfil anterior até que o governo declare o
   migração prête.
5. **Suivi de depreciação.** Após a janela de migração, registre hoje para
   de migração.

## Conselhos de criação- Préférez des bornes puissances de deux pairs para minimizar o comportamento de chunking en bord.
- Evite alterar o código multihash sem coordenar o manifesto e o gateway do consumidor;
  inclua uma nota operacional quando você fizer isso.
- Garanta as sementes de equipamentos de mesa mais globais e exclusivos para simplificar as auditorias.
- Estoque todos os artefatos de benchmarking (ex., comparações de débito) sob
  `docs/source/sorafs/reports/` para referência futura.

Para monitorar as operações durante a implementação, veja o registro de migração
(`docs/source/sorafs/migration_ledger.md`). Para as regras de conformidade do tempo de execução, veja
`docs/source/sorafs/chunker_conformance.md`.