---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/chunker-profile-authoring.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: autoria de perfil chunker
título: Guia de criação de perfil chunker SoraFS
sidebar_label: Guia de criação do Chunker
description: SoraFS chunker profiles اور fixtures تجویز کرنے کے لیے checklist۔
---

:::nota مستند ماخذ
:::

# Guia de criação de perfil chunker SoraFS

یہ گائیڈ وضاحت کرتی ہے کہ SoraFS کے لیے نئے chunker profiles کیسے تجویز اور publicar کیے جائیں۔
Arquitetura RFC (SF-1) ou referência de registro (SF-2a)
requisitos concretos de autoria, etapas de validação e modelos de proposta
Canônico مثال کے لیے دیکھیں
`docs/source/sorafs/proposals/sorafs_sf1_profile_v1.json`
اور متعلقہ registro de simulação
`docs/source/sorafs/reports/sf1_determinism.md` میں۔

## Visão geral

ہر perfil جو registro میں داخل ہوتی ہے اسے یہ کرنا ہوگا:

- parâmetros CDC determinísticos e configurações multihash کو arquiteturas کے درمیان یکساں طور پر anunciar کرنا؛
- fixtures reproduzíveis (Rust/Go/TS JSON + fuzz corpora + testemunhas PoR) فراہم کرنا جنہیں SDKs downstream بغیر ferramentas sob medida کے verify کر سکیں؛
- revisão do conselho سے پہلے conjunto de diferenças determinísticas پاس کرنا۔

Lista de verificação پر عمل کریں تاکہ ایک ایسا proposta تیار ہو جو ان قواعد پر پورا اترے۔

## Instantâneo da carta de registro

Rascunho da proposta
`sorafs_manifest::chunker_registry::ensure_charter_compliance()` impor کرتا ہے:

- IDs de perfil مثبت inteiros ہوتے ہیں جو بغیر lacunas کے monotônico طور پر بڑھتے ہیں۔
- کوئی alias کسی دوسرے identificador canônico سے colidir نہیں کر سکتا اور ایک سے زیادہ بار ظاہر نہیں ہو سکتا۔
- Aliases não vazios ہوں اور espaço em branco سے trim ہوں۔

Ajudantes CLI úteis:

```bash
# تمام registered descriptors کی JSON listing (ids, handles, aliases, multihash)
cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- --list-profiles

# Candidate default profile کے لیے metadata emit کریں (canonical handle + aliases)
cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- \
  --promote-profile=sorafs.sf1@1.0.0 --json-out=-
```

یہ propostas de comandos کو carta de registro کے مطابق رکھتے ہیں اور discussões de governança کے لیے درکار metadados canônicos فراہم کرتے ہیں۔

## Metadados obrigatórios| Campo | Descrição | Exemplo (`sorafs.sf1@1.0.0`) |
|-------|-------------|---------------|
| `namespace` | متعلقہ perfis کے لیے agrupamento lógico۔ | `sorafs` |
| `name` | rótulo legível por humanos۔ | `sf1` |
| `semver` | conjunto de parâmetros کے لیے string de versão semântica۔ | `1.0.0` |
| `profile_id` | Identificador numérico monotônico جو perfil کے terreno ہونے پر atribuir ہوتا ہے۔ اگلا id reserve کریں مگر موجودہ نمبرز reutilizar نہ کریں۔ | `1` |
| `profile.min_size` | comprimento do pedaço کی mínimo حد bytes میں۔ | `65536` |
| `profile.target_size` | comprimento do pedaço کی alvo حد bytes میں۔ | `262144` |
| `profile.max_size` | comprimento do pedaço کی máximo حد bytes میں۔ | `524288` |
| `profile.break_mask` | hash rolante کے لیے máscara adaptativa (hex)۔ | `0x0000ffff` |
| `profile.polynomial` | constante polinomial de engrenagem (hex)۔ | `0x3da3358b4dc173` |
| `gear_seed` | Tabela de engrenagens de 64 KiB deriva کرنے کے لیے semente۔ | `sorafs-v1-gear` |
| `chunk_multihash.code` | resumos por bloco کے لیے código multihash۔ | `0x1f` (BLAKE3-256) |
| `chunk_multihash.digest` | pacote de luminárias canônicas کا digest۔ | `13fa...c482` |
| `fixtures_root` | fixtures regenerados رکھنے والی diretório relativo۔ | `fixtures/sorafs_chunker/sorafs.sf1@1.0.0/` |
| `por_seed` | amostragem PoR determinística کے لیے semente (`splitmix64`)۔ | `0xfeedbeefcafebabe` (exemplo) |

Metadados کو documento de proposta اور acessórios gerados دونوں میں شامل ہونا چاہیے تاکہ registro, ferramentas CLI اور automação de governança بغیر referência cruzada manual کے valores confirmados کر سکیں۔ اگر شک ہو تو chunk-store e CLIs de manifesto کو `--json-out=-` کے ساتھ چلائیں تاکہ notas de revisão de metadados computados میں stream ہو سکے۔

### CLI e pontos de contato do registro

- `sorafs_manifest_chunk_store --profile=<handle>` — parâmetros propostos کے ساتھ metadados de pedaços, resumo do manifesto e verificações de PoR دوبارہ چلائیں۔
- `sorafs_manifest_chunk_store --json-out=-` — relatório de armazenamento de blocos کو stdout پر stream کریں تاکہ comparações automatizadas ہو سکیں۔
- `sorafs_manifest_stub --chunker-profile=<handle>` — confirmar کریں کہ manifestos اور planos do CAR identificador canônico اور aliases incorporados کرتے ہیں۔
- `sorafs_manifest_stub --plan=-` — پچھلا `chunk_fetch_specs` واپس feed کریں تاکہ change کے بعد offsets/digests verify ہوں۔

Saída de comando (resumos, raízes PoR, hashes de manifesto) کو proposta میں ریکارڈ کریں تاکہ revisores انہیں reproduzir literalmente کر سکیں۔

## Lista de verificação de determinismo e validação

1. **As luminárias regeneram کریں**
   ```bash
   cargo run --locked -p sorafs_chunker --bin export_vectors \
     --signature-out=fixtures/sorafs_chunker/manifest_signatures.json
   ```
2. **Suíte de paridade چلائیں** — `cargo test -p sorafs_chunker` e chicote de comparação entre idiomas (`crates/sorafs_chunker/tests/vectors.rs`) نئے fixtures کے ساتھ verde ہونا چاہیے۔
3. **Reprodução de corpora fuzz/contrapressão ** — `cargo fuzz list` e chicote de streaming (`fuzz/sorafs_chunker`) e ativos regenerados کے خلاف چلائیں۔
4. **Testemunhas de prova de recuperação verificam کریں** - `sorafs_manifest_chunk_store --por-sample=<n>` perfil proposto کے ساتھ چلائیں اور raízes کو manifesto de fixação سے correspondência کریں۔
5. **CI dry run** — `ci/check_sorafs_fixtures.sh` Módulo de teste script کو نئے fixtures اور موجودہ `manifest_signatures.json` کے ساتھ sucesso ہونا چاہیے۔
6. **Confirmação de tempo de execução cruzado** — یقینی بنائیں کہ Ligações Go/TS regeneradas JSON consumir کریں اور limites de pedaços idênticos اور digests emit کریں۔Comandos اور resumos resultantes کو proposta میں دستاویزی کریں تاکہ Ferramentas WG بغیر suposições کے انہیں دوبارہ چلا سکے۔

### Confirmação de Manifesto/PoR

Fixtures regeneram o pipeline de manifesto do pipeline de manifesto com metadados CAR e provas de PoR consistentes:

```bash
# نئے profile کے ساتھ chunk metadata + PoR validate کریں
cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- \
  --profile=sorafs.sf2@1.0.0 \
  --json-out=- --por-json-out=- fixtures/sorafs_chunker/input.bin

# manifest + CAR generate کریں اور chunk fetch specs capture کریں
cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- \
  fixtures/sorafs_chunker/input.bin \
  --chunker-profile=sorafs.sf2@1.0.0 \
  --chunk-fetch-plan-out=chunk_plan.json \
  --manifest-out=sf2.manifest \
  --car-out=sf2.car \
  --json-out=sf2.report.json

# محفوظ fetch plan کے ساتھ دوبارہ چلائیں (stale offsets سے بچاتا ہے)
cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- \
  fixtures/sorafs_chunker/input.bin \
  --chunker-profile=sorafs.sf2@1.0.0 \
  --plan=chunk_plan.json --json-out=-
```

Arquivo de entrada کو اپنے fixtures میں استعمال ہونے والے کسی corpus representativo سے بدلیں
(Mesmo fluxo determinístico de 1 GiB) e resumos resultantes کو proposta کے ساتھ anexar کریں۔

## Modelo de proposta

Propostas کو `ChunkerProfileProposalV1` Norito registros کے طور پر `docs/source/sorafs/proposals/` میں check in کیا جاتا ہے۔ O modelo JSON esperado é o que você precisa (os valores devem ser substituídos):


Relatório Markdown correspondente (`determinism_report`) فراہم کریں جس میں saída de comando, resumos de pedaços e validação کے دوران پائی گئی desvios شامل ہوں۔

## Fluxo de trabalho de governança

1. **Proposta + acessórios کے ساتھ PR submit کریں۔** Ativos gerados, proposta Norito, e atualizações `chunker_registry_data.rs` شامل کریں۔
2. **Revisão do Tooling WG۔** Lista de verificação de validação dos revisores دوبارہ چلاتے ہیں اور confirmar کرتے ہیں کہ regras de registro de proposta کے مطابق ہے (reutilização de id نہیں، determinismo satisfeito)۔
3. **Envelope do conselho۔** Aprovar ہونے کے بعد resumo da proposta dos membros do conselho (`blake3("sorafs-chunker-profile-v1" || canonical_bytes)`) پر assinar کرتے ہیں اور assinaturas کو envelope de perfil میں anexar کرتے ہیں جو luminárias کے ساتھ رکھا جاتا ہے۔
4. **Publicação de registro۔** Mesclar registro, documentos e atualização de equipamentos ہوتے ہیں۔ Perfil de CLI padrão پچھلے پر رہتا ہے جب تک migração de governança کو pronto قرار نہ دے۔

## Dicas de autoria

- Potência de dois کی limites pares ترجیح دیں تاکہ comportamento de fragmentação de casos extremos کم ہو۔
- Sementes de mesa de engrenagens کو legíveis por humanos مگر globalmente exclusivas رکھیں تاکہ trilhas de auditoria آسان ہوں۔
- Artefatos de benchmarking (comparações de rendimento) کو `docs/source/sorafs/reports/` میں محفوظ کریں تاکہ مستقبل میں referência ہو سکے۔

Rollout کے دوران expectativas operacionais کے لیے registro de migração دیکھیں
(`docs/source/sorafs/migration_ledger.md`)۔ Regras de conformidade de tempo de execução
`docs/source/sorafs/chunker_conformance.md` دیکھیں۔