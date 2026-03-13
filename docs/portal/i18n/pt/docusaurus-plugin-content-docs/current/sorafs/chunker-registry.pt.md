---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/chunker-registry.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: registro de chunker
título: Registro de perfis de chunker da SoraFS
sidebar_label: Registro do chunker
description: IDs de perfil, parâmetros e plano de negociação para o registro de chunker da SoraFS.
---

:::nota Fonte canônica
Esta página espelha `docs/source/sorafs/chunker_registry.md`. Mantenha ambas as cópias sincronizadas.
:::

## Registro de perfis de chunker da SoraFS (SF-2a)

A pilha SoraFS negocia o comportamento de chunking através de um registro pequeno com namespace.
Cada perfil atribui parâmetros CDC deterministas, metadados semver e o digest/multicodec esperados usados ​​em manifestos e arquivos CAR.

Autores de perfis devem consultar
[`docs/source/sorafs/chunker_profile_authoring.md`](./chunker-profile-authoring.md)
para os metadados necessários, um checklist de validação e o modelo de proposta antes de submeter novas entradas.
Uma vez que a governança aprove uma mudança, siga o
[checklist de rollout do registro](./chunker-registry-rollout-checklist.md) e o
[manual de manifesto em staging](./staging-manifest-playbook) para promoção
os fixtures a encenação e produção.

###Perfis

| Espaço para nome | Nome | SemVer | ID do perfil | Mínimo (bytes) | Alvo (bytes) | Máx. (bytes) | Rímel de quebra | Multihash | Aliases | Notas |
|-----------|------|--------|------------|-------------|----------------|-------------|-------|-----------|--------|-------|
| `sorafs` | `sf1` | `1.0.0` | `1` | 65536 | 262144 | 524288 | `0x0000ffff` | `0x1f` (BLAKE3-256) | `["sorafs.sf1@1.0.0", "sorafs.sf1@1.0.0"]` | Perfil canônico usado em luminárias SF-1 |

O registro vive no código como `sorafs_manifest::chunker_registry` (governado por [`chunker_registry_charter.md`](./chunker-registry-charter.md)). Cada entrada
e expressa como um `ChunkerProfileDescriptor` com:

* `namespace` - agrupamento lógico de perfis relacionados (ex., `sorafs`).
* `name` - rotulo legível para humanos (`sf1`, `sf1-fast`, ...).
* `semver` - cadeia de versão semântica para o conjunto de parâmetros.
* `profile` - o `ChunkProfile` real (mín/alvo/máx/máscara).
* `multihash_code` - o multihash usado ao produzir digests de chunk (`0x1f`
  para o padrão de SoraFS).

O manifesto serializa perfis via `ChunkingProfileV1`. A estrutura registra os metadados
 do registro (namespace, name, semver) junto com os parâmetros CDC brutos
e a lista de aliases mostrada acima. Os consumidores devem primeiro tentar uma
busca no registro por `profile_id` e recorrendo aos parâmetros inline quando
IDs desconhecidos aparecem; uma lista de aliases garante que os clientes HTTP possam
continue enviando identificadores alternativos em `Accept-Chunker` sem adivinhar. Como as regras fazem
charter do registro desativar que o handle canonico (`namespace.name@semver`) seja a
primeira entrada em `profile_aliases`, seguida por quaisquer aliases alternativos.

Para operar o registro a partir do ferramental, execute o auxiliar CLI:

```
$ cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- --list-profiles
[
  {
    "namespace": "sorafs",
    "name": "sf1",
    "semver": "1.0.0",
    "handle": "sorafs.sf1@1.0.0",
    "profile_id": 1,
    "min_size": 65536,
    "target_size": 262144,
    "max_size": 524288,
    "break_mask": "0x0000ffff",
    "multihash_code": 31
  }
]
```Todas as flags do CLI que escrevem JSON (`--json-out`, `--por-json-out`, `--por-proof-out`,
`--por-sample-out`) aceitam `-` como caminho, ou que transmite o payload para stdout em vez de
criar um arquivo. Isso torna fácil encadear os dados para ferramentas mantendo o
comportamento padrão de impressão do relatório principal.

### Matriz de rollout e plano de implantação


A tabela abaixo captura o status atual do suporte para `sorafs.sf1@1.0.0` nos
componentes principais. "Bridge" refere-se à faixa CARv1 + SHA-256
que requer negociação explícita do cliente (`Accept-Chunker` + `Accept-Digest`).

| Componente | Estado | Notas |
|-----------|--------|-------|
| `sorafs_manifest_chunk_store` | ✅ Suportado | Valida o identificador canônico + aliases, faz stream de relatos via `--json-out=-` e aplica o alvará de registro via `ensure_charter_compliance()`. |
| `sorafs_manifest_stub` | ⚠️ Retirado | Construtor de manifestos para fóruns de suporte; use `iroha app sorafs toolkit pack` para empacotamento CAR/manifest e manter `--plan=-` para revalidação determinística. |
| `sorafs_provider_advert_stub` | ⚠️ Retirado | Helper de validação offline apenas; os anúncios de provedores devem ser produzidos pelo pipeline de publicação e validados via `/v2/sorafs/providers`. |
| `sorafs_fetch` (desenvolvedor orquestrador) | ✅ Suportado | Le `chunk_fetch_specs`, significa cargas úteis de capacidade `range` e monta dita CARv2. |
| Fixações do SDK (Rust/Go/TS) | ✅ Suportado | Regeneradas via `export_vectors`; o identificador canônico aparece primeiro em cada lista de aliases e é assinado por envelopes do conselho. |
| Negociação de perfil no gateway Torii | ✅ Suportado | Implementa a gramática completa de `Accept-Chunker`, inclui cabeçalhos `Content-Chunker` e expõe a ponte CARv1 apenas em solicitações explícitas de downgrade. |

Lançamento de telemetria:

- **Telemetria de busca de chunks** - o CLI Iroha `sorafs toolkit pack` emite resumos de chunk, metadados CAR e raízes PoR para ingestão em dashboards.
- **Anúncios de provedores** - os payloads de anúncios incluem metadados de capacidade e aliases; cobertura válida via `/v2/sorafs/providers` (ex., presença da capacidade `range`).
- **Monitoramento de gateway** - os operadores devem reportar os pareamentos `Content-Chunker`/`Content-Digest` para detectar downgrades inesperados; espera-se que o uso do bridge tenda a zero antes da depreciação.

Política de depreciação: uma vez que um perfil sucessor seja ratificado, agende uma janela de publicação dupla
bridge CARv1 dos gateways em produção.

Para executar um testemunho PoR específico, forneca índices de chunk/segmento/folha e opcionalmente
persista a prova no disco:

```
$ cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- ./docs.tar \
    --por-proof=0:0:0 --por-proof-out=leaf.proof.json
```

Você pode selecionar um perfil por id numérico (`--profile-id=1`) ou por identificador de registro
(`--profile=sorafs.sf1@1.0.0`); um formato de identificador e conveniente para scripts que
encadeiam namespace/nome/semver diretamente dos metadados de governança.

Use `--promote-profile=<handle>` para emitir um bloco JSON de metadados (incluindo todos os aliases
registrados) que pode ser colado em `chunker_registry_data.rs` ao promover um novo perfil padrão:

```
$ cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- \
    --promote-profile=sorafs.sf1@1.0.0
```O relatorio principal (e o arquivo de prova opcional) inclui o resumo raiz, os bytes de folha amostrados
(codificados em hexadecimal) e os resumos de irmãos de segmento/chunk para que os verificadores possam
recalcular o hash das camadas de 64 KiB/4 KiB contra o valor `por_root_hex`.

Para validar uma prova existente contra um payload, passe o caminho via
`--por-proof-verify` (o CLI adiciona `"por_proof_verified": true` quando o testemunho
corresponde a raiz calculada):

```
$ cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- ./docs.tar \
    --por-proof-verify=leaf.proof.json
```

Para amostragem em lote, utilize `--por-sample=<count>` e opcionalmente forneca um caminho de seed/saida.
O CLI garante ordenação determinística (seeded com `splitmix64`) e trunca automaticamente quando
a requisição excede as folhas disponíveis:

```
$ cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- ./docs.tar \
    --por-sample=8 --por-sample-seed=0xfeedface --por-sample-out=por.samples.json
```
```

O manifest stub espelha os mesmos dados, o que e conveniente ao automatizar a selecao de
`--chunker-profile-id` em pipelines. Ambos os CLIs de chunk store tambem aceitam a forma de handle canonico
(`--profile=sorafs.sf1@1.0.0`) para que scripts de build evitem hard-codear IDs numericos:

```
$ carga run -p sorafs_manifest --bin sorafs_manifest_stub -- --list-chunker-profiles
[
  {
    "perfil_id": 1,
    "namespace": "sorafs",
    "nome": "sf1",
    "semver": "1.0.0",
    "handle": "sorafs.sf1@1.0.0",
    "tamanho_mín": 65536,
    "tamanho_alvo": 262144,
    "tamanho máximo": 524288,
    "break_mask": "0x0000ffff",
    "código_multihash": 31
  }
]
```

O campo `handle` (`namespace.name@semver`) corresponde ao que os CLIs aceitam via
`--profile=...`, tornando seguro copiar direto para automacao.

### Negociar chunkers

Gateways e clientes anunciam perfis suportados via provider adverts:

```
ProviderAdvertBodyV1 {
    ...
    chunk_profile: profile_id (implícito via registro)
    capacidades: [...]
}
```

O agendamento de chunks multi-source e anunciado via a capacidade `range`. O CLI aceita isso com
`--capability=range[:streams]`, onde o sufixo numerico opcional codifica a concorrencia preferida
para fetch por range do provider (por exemplo, `--capability=range:64` anuncia um budget de 64 streams).
Quando omitido, consumidores recorrem ao hint geral `max_streams` publicado em outro ponto do advert.

Ao solicitar dados CAR, clientes devem enviar um header `Accept-Chunker` listando tuplas
`(namespace, name, semver)` em ordem de preferencia:

```

Gateways selecionam um perfil apoiado mutuamente (padrão `sorafs.sf1@1.0.0`)
e refletem a decisão através do cabeçalho de resposta `Content-Chunker`. Manifestos
embutem o perfil escolhido para que nossos downstream possam validar o layout de chunks
sem depender da negociação HTTP.

###Suporte CAR

mantemos um caminho de exportação CARv1+SHA-2:

* **Caminho primário** - CARv2, resumo da carga útil BLAKE3 (`0x1f` multihash),
  `MultihashIndexSorted`, perfil de pedaço registrado como acima.
  PODEM expor esta variante quando o cliente omite `Accept-Chunker` ou solicita
  `Accept-Digest: sha2-256`.

adicionais para transição, mas não devem substituir o digest canonico.

### Conformidade

* O perfil `sorafs.sf1@1.0.0` mapeia para os fixtures públicos em
  `fixtures/sorafs_chunker` e os corpora registrados em
  `fuzz/sorafs_chunker`. A paridade ponta a ponta e exercitada em Rust, Go e Node
  através dos testes fornecidos.
* `chunker_registry::lookup_by_profile` afirma que os parâmetros do descritor
  cobre a `ChunkProfile::DEFAULT` para evitar divergência acidental.
* Os manifestos produzidos por `iroha app sorafs toolkit pack` e `sorafs_manifest_stub` incluem os metadados do registro.