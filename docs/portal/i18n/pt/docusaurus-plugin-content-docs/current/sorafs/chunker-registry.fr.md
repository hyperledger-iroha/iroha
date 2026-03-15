---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/chunker-registry.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: registro de chunker
título: Registro de perfis chunker SoraFS
sidebar_label: Registrar chunker
descrição: IDs de perfil, parâmetros e plano de negociação para o bloco de registro SoraFS.
---

:::nota Fonte canônica
Esta página reflete `docs/source/sorafs/chunker_registry.md`. Gardez as duas cópias sincronizadas junto com o retorno completo do conjunto Sphinx herité.
:::

## Registrar o bloco de perfis SoraFS (SF-2a)

A pilha SoraFS negocia o comportamento de chunking por meio de um pequeno namespace de registro.
Cada perfil atribui parâmetros CDC determinados, metadados sempre e o resumo/multicodec atendido usado nos manifestos e arquivos CAR.

Os autores dos perfis devem ser consultados
[`docs/source/sorafs/chunker_profile_authoring.md`](./chunker-profile-authoring.md)
para os requisitos de metado, a lista de verificação de validação e o modelo de proposta anterior
de soumettre de novas entradas. Uma vez que uma modificação foi aprovada por
governança, suivez la
[lista de verificação de implementação do registro](./chunker-registry-rollout-checklist.md) e o
[manual de manifesto em teste](./staging-manifest-playbook) para promoção
os equipamentos para encenação e produção.

### Perfis

| Espaço para nome | Nome | SemVer | ID do perfil | Min (octetos) | Cible (octetos) | Máx. (octetos) | Máscara de ruptura | Multihash | Alias ​​| Notas |
|-----------|-----|--------|------------|-------------|----------------|-------------|-------|-----------|-------|-------|
| `sorafs` | `sf1` | `1.0.0` | `1` | 65536 | 262144 | 524288 | `0x0000ffff` | `0x1f` (BLAKE3-256) | `["sorafs.sf1@1.0.0", "sorafs.sf1@1.0.0"]` | Perfil canônico utilizado nas luminárias SF-1 |

Registre-o no código sob `sorafs_manifest::chunker_registry` (registrado por [`chunker_registry_charter.md`](./chunker-registry-charter.md)). Entrada Chaque
foi escrito como um `ChunkerProfileDescriptor` com:

* `namespace` – reagrupamento logístico de perfis locais (ex., `sorafs`).
* `name` – libellé lisível (`sf1`, `sf1-fast`,…).
* `semver` – cadeia de versão semântica para o jogo de parâmetros.
* `profile` – o rolo `ChunkProfile` (mín/alvo/máx/máscara).
* `multihash_code` – o multihash utilizado na produção de resumos de pedaços (`0x1f`
  para o padrão SoraFS).

O manifesto serializa os perfis via `ChunkingProfileV1`. A estrutura registra os metadonées
você registra (namespace, name, semver) nos parâmetros brutos do CDC e na lista de alias ci-dessus.
Os consumidores devem tentar uma pesquisa no registro por `profile_id` e retornar aux
parâmetros inline quando IDs desconhecidos são exibidos; a lista de alias garante que os clientes HTTP
du registre exige que le handle canonique (`namespace.name@semver`) seja a estreia de
`profile_aliases`, suivi de alias hereditários.

Para inspecionar o registro a partir das ferramentas, execute o auxiliar CLI:

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
```Todos os sinalizadores da CLI criados em JSON (`--json-out`, `--por-json-out`, `--por-proof-out`,
`--por-sample-out`) aceita `-` como chemin, ce qui streame le payload vers stdout em vez de
crie um arquivo. Isso facilita a tubulação dos dados em todas as ferramentas, mantendo-os
comportamento por padrão de imprimir o relacionamento principal.

### Matriz de implementação e plano de implantação


O quadro acima captura o status de suporte atual para `sorafs.sf1@1.0.0` nos
componentes principais. "Bridge" designa a voz CARv1 + SHA-256 aqui
precisa de uma negociação explícita para o cliente (`Accept-Chunker` + `Accept-Digest`).

| Composto | Estatuto | Notas |
|-----------|--------|-------|
| `sorafs_manifest_chunk_store` | ✅ Apoiado | Valide o identificador canônico + alias, transmita os relatórios via `--json-out=-` e aplique a tabela de registro via `ensure_charter_compliance()`. |
| `sorafs_fetch` (desenvolvedor orquestrador) | ✅ Apoiado | Lit `chunk_fetch_specs`, compreende as cargas úteis da capacidade `range` e monta uma surtida CARv2. |
| SDK de luminárias (Rust/Go/TS) | ✅ Apoiado | Régenérées via `export_vectors` ; o identificador canônico aparece primeiro em cada lista de alias e é assinado pelos envelopes do conselho. |
| Negociação de perfis do gateway Torii | ✅ Apoiado | Implemente toda a gramática `Accept-Chunker`, inclua os cabeçalhos `Content-Chunker` e não exponha a ponte CARv1 que está nas demandas de downgrade explícitas. |

Implantação da televisão:

- **Telemetria de busca de pedaços** — CLI Iroha `sorafs toolkit pack` fornece resumos de pedaços, metadados CAR e racines PoR para ingestão nos painéis.
- **Anúncios de provedores** — as cargas úteis de anúncios incluem metadados de capacidade e alias; valide a cobertura via `/v2/sorafs/providers` (ex., presença da capacidade `range`).
- **Gateway de vigilância** — os operadores devem relatar os acoplamentos `Content-Chunker`/`Content-Digest` para detectar downgrades inatender; O uso da ponte é censurado e tende a zero antes da depreciação.

Política de depreciação: uma vez que um perfil de sucesso foi ratificado, planeje uma janela de dupla publicação
(documentado na proposta) antes de marcar `sorafs.sf1@1.0.0` como descontinuado no registro e retirado
ponte CARv1 des gateways em produção.

Para inspecionar um número PoR específico, forneça índices de chunk/segment/feuille e, opcionalmente,
persista a verificação no disco:

```
$ cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- ./docs.tar \
    --por-proof=0:0:0 --por-proof-out=leaf.proof.json
```

Você pode selecionar um perfil por ID numérico (`--profile-id=1`) ou por identificador de registro
(`--profile=sorafs.sf1@1.0.0`); o formato do identificador é prático para os scripts aqui
namespace/nome/semver transmetente diretamente dos métodos de governo.

Use `--promote-profile=<handle>` para criar um bloco de metadados JSON (e que inclui todos os alias
registrado) que pode ser coletado em `chunker_registry_data.rs` durante a promoção de um novo perfil
por padrão:

```
$ cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- \
    --promote-profile=sorafs.sf1@1.0.0
```O relacionamento principal (e o arquivo de teste opcional) inclui o resumo racine, os octetos de folha échantillonnés
(codificados em hexadécimal) e resumos de segmentos/pedaços para que os verificadores possam rehacher
os sofás de 64 KiB/4 KiB estão voltados para o valor `por_root_hex`.

Para validar uma verificação existente contra uma carga útil, passe pelo caminho
`--por-proof-verify` (o CLI adiciona `"por_proof_verified": true` quando o telefone está
corresponde à racine calculada):

```
$ cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- ./docs.tar \
    --por-proof-verify=leaf.proof.json
```

Para o cultivo por lotes, use `--por-sample=<count>` e forneça eventualmente um caminho de semente/sorteio.
O CLI garante uma ordem determinada (consultada com `splitmix64`) e um troque automático quando
la requête dépasse les feuilles disponibles :

```
$ cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- ./docs.tar \
    --por-sample=8 --por-sample-seed=0xfeedface --por-sample-out=por.samples.json
```
```

Le manifest stub reflète les mêmes données, ce qui est pratique pour scripter la sélection de
`--chunker-profile-id` dans les pipelines. Les deux CLIs de chunk store acceptent aussi la forme de handle canonique
(`--profile=sorafs.sf1@1.0.0`) afin que les scripts de build évitent de coder en dur des IDs numériques :

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

Le champ `handle` (`namespace.name@semver`) correspond à ce que les CLIs acceptent via
`--profile=…`, ce qui permet de le copier directement dans l'automatisation.

### Négocier les chunkers

Les gateways et les clients annoncent les profils supportés via des provider adverts :

```
ProviderAdvertBodyV1 {
    ...
    chunk_profile: profile_id (implícito via registro)
    capacidades: [...]
}
```

La planification multi-source des chunks est annoncée via la capacité `range`. Le CLI l'accepte avec
`--capability=range[:streams]`, où le suffixe numérique optionnel encode la concurrence de fetch par range préférée
par le provider (par exemple, `--capability=range:64` annonce un budget de 64 streams).
Lorsqu'il est omis, les consommateurs reviennent à l'indication générale `max_streams` publiée ailleurs dans l'advert.

Lorsqu'ils demandent des données CAR, les clients doivent envoyer un header `Accept-Chunker` listant des tuples
`(namespace, name, semver)` par ordre de préférence :

```

Os gateways selecionam um perfil com suporte mútuo (por padrão `sorafs.sf1@1.0.0`)
e reflete a decisão por meio do cabeçalho de resposta `Content-Chunker`. Os manifestos
integre o perfil escolhido para que os nódulos downstream possam validar o layout dos pedaços
sem usar a negociação HTTP.

### Suporte CAR

Nós mantemos uma rota de exportação CARv1+SHA-2 :

* **Chemin principal** – CARv2, resumo da carga útil BLAKE3 (`0x1f` multihash),
  `MultihashIndexSorted`, perfil de pedaço registrado como ci-dessus.
  PEUVENT expositor esta variante quando o cliente omete `Accept-Chunker` ou demanda
  `Accept-Digest: sha2-256`.

suplementares para a transição, mas não deve substituir o resumo canônico.

### Conformidade

* O perfil `sorafs.sf1@1.0.0` corresponde a luminárias públicas em
  `fixtures/sorafs_chunker` e aux corpora registrados sob
  `fuzz/sorafs_chunker`. A paridade de luta em luta é exercida em Rust, Go et Node
  através dos testes fornecidos.
* `chunker_registry::lookup_by_profile` afirma que os parâmetros do descritor
  correspondente a `ChunkProfile::DEFAULT` para evitar qualquer divergência acidental.
* Os manifestos produzidos por `iroha app sorafs toolkit pack` e `sorafs_manifest_stub` incluem os metados do registro.