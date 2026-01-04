<!-- Auto-generated stub for Portuguese (pt) translation. Replace this content with the full translation. -->

---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/chunker-registry.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 2895818612da6e0afbeb5bb44da3a6ecef6cab0aefd401ed79b4236df1dd9d18
source_last_modified: "2025-11-08T20:22:25.757062+00:00"
translation_last_reviewed: 2025-12-29
---

---
id: chunker-registry
title: Registro de perfis de chunker da SoraFS
sidebar_label: Registro de chunker
description: IDs de perfil, parametros e plano de negociacao para o registro de chunker da SoraFS.
---

:::note Fonte canonica
Esta pagina espelha `docs/source/sorafs/chunker_registry.md`. Mantenha ambas as copias sincronizadas ate que o conjunto de documentacao Sphinx legado seja retirado.
:::

## Registro de perfis de chunker da SoraFS (SF-2a)

A stack SoraFS negocia o comportamento de chunking via um registro pequeno com namespace.
Cada perfil atribui parametros CDC deterministas, metadados semver e o digest/multicodec esperado usado em manifests e arquivos CAR.

Autores de perfis devem consultar
[`docs/source/sorafs/chunker_profile_authoring.md`](./chunker-profile-authoring.md)
para os metadados requeridos, a checklist de validacao e o modelo de proposta antes de submeter novas entradas.
Uma vez que a governanca aprove uma mudanca, siga o
[checklist de rollout do registro](./chunker-registry-rollout-checklist.md) e o
[playbook de manifest em staging](./staging-manifest-playbook) para promover
os fixtures a staging e producao.

### Perfis

| Namespace | Nome | SemVer | ID de perfil | Min (bytes) | Target (bytes) | Max (bytes) | Mascara de quebra | Multihash | Aliases | Notas |
|-----------|------|--------|-------------|-------------|----------------|-------------|------------------|-----------|--------|-------|
| `sorafs`  | `sf1` | `1.0.0` | `1` | 65536 | 262144 | 524288 | `0x0000ffff` | `0x1f` (BLAKE3-256) | `["sorafs.sf1@1.0.0", "sorafs.sf1@1.0.0"]` | Perfil canonico usado em fixtures SF-1 |

O registro vive no codigo como `sorafs_manifest::chunker_registry` (governado por [`chunker_registry_charter.md`](./chunker-registry-charter.md)). Cada entrada
e expressa como um `ChunkerProfileDescriptor` com:

* `namespace` - agrupamento logico de perfis relacionados (ex., `sorafs`).
* `name` - rotulo legivel para humanos (`sf1`, `sf1-fast`, ...).
* `semver` - cadeia de versao semantica para o conjunto de parametros.
* `profile` - o `ChunkProfile` real (min/target/max/mask).
* `multihash_code` - o multihash usado ao produzir digests de chunk (`0x1f`
  para o default da SoraFS).

O manifest serializa perfis via `ChunkingProfileV1`. A estrutura registra os metadados
 do registro (namespace, name, semver) junto com os parametros CDC brutos
e a lista de aliases mostrada acima. Consumidores devem primeiro tentar uma
busca no registro por `profile_id` e recorrer aos parametros inline quando
IDs desconhecidos aparecerem; a lista de aliases garante que clientes HTTP possam
continuar enviando handles legados em `Accept-Chunker` sem adivinhar. As regras do
charter do registro exigem que o handle canonico (`namespace.name@semver`) seja a
primeira entrada em `profile_aliases`, seguida por quaisquer aliases legados.

Para inspecionar o registro a partir do tooling, execute o CLI helper:

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
```

Todas as flags do CLI que escrevem JSON (`--json-out`, `--por-json-out`, `--por-proof-out`,
`--por-sample-out`) aceitam `-` como caminho, o que transmite o payload para stdout em vez de
criar um arquivo. Isso torna facil encadear os dados para tooling mantendo o
comportamento padrao de imprimir o relatorio principal.

### Matriz de compatibilidade e plano de rollout


A tabela abaixo captura o status atual de suporte para `sorafs.sf1@1.0.0` nos
componentes principais. "Bridge" refere-se a faixa de compatibilidade CARv1 + SHA-256
que requer negociacao explicita do cliente (`Accept-Chunker` + `Accept-Digest`).

| Componente | Status | Notas |
|-----------|--------|-------|
| `sorafs_manifest_chunk_store` | ✅ Suportado | Valida o handle canonico + aliases, faz stream de relatorios via `--json-out=-` e aplica o charter do registro via `ensure_charter_compliance()`. |
| `sorafs_manifest_stub` | ⚠️ Legado | Builder de manifest legado; use `iroha sorafs toolkit pack` para empacotamento CAR/manifest e mantenha `--plan=-` para revalidacao deterministica. |
| `sorafs_provider_advert_stub` | ⚠️ Legado | Helper de validacao offline apenas; provider adverts devem ser produzidos pelo pipeline de publicacao e validados via `/v1/sorafs/providers`. |
| `sorafs_fetch` (developer orchestrator) | ✅ Suportado | Le `chunk_fetch_specs`, entende payloads de capacidade `range` e monta saida CARv2. |
| Fixtures de SDK (Rust/Go/TS) | ✅ Suportado | Regeneradas via `export_vectors`; o handle canonico aparece primeiro em cada lista de aliases e e assinado por envelopes do conselho. |
| Negociacao de perfil no gateway Torii | ✅ Suportado | Implementa a gramatica completa de `Accept-Chunker`, inclui headers `Content-Chunker` e expoe o bridge CARv1 apenas em solicitacoes explicitas de downgrade. |
| Bridge CARv1 (`sha2-256`) | ⚠️ Transicional | Disponivel para clientes legados quando a requisicao anuncia tanto o perfil canonico quanto `Accept-Digest: sha2-256`; as respostas incluem `Content-Chunker: ...;legacy=true`. |

Rollout de telemetria:

- **Telemetria de fetch de chunks** - o CLI Iroha `sorafs toolkit pack` emite digests de chunk, metadados CAR e raizes PoR para ingestao em dashboards.
- **Provider adverts** - os payloads de adverts incluem metadados de capacidade e aliases; valide cobertura via `/v1/sorafs/providers` (ex., presenca da capacidade `range`).
- **Monitoramento de gateway** - operadores devem reportar os pareamentos `Content-Chunker`/`Content-Digest` para detectar downgrades inesperados; espera-se que o uso do bridge tenda a zero antes da deprecacao.

Politica de deprecacao: uma vez que um perfil sucessor seja ratificado, agende uma janela de publicacao dupla
(documentada na proposta) antes de marcar `sorafs.sf1@1.0.0` como deprecated no registro e remover o
bridge CARv1 dos gateways em producao.

Para inspecionar um testemunho PoR especifico, forneca indices de chunk/segmento/folha e opcionalmente
persista a prova no disco:

```
$ cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- ./docs.tar \
    --por-proof=0:0:0 --por-proof-out=leaf.proof.json
```

Voce pode selecionar um perfil por id numerico (`--profile-id=1`) ou por handle de registro
(`--profile=sorafs.sf1@1.0.0`); a forma handle e conveniente para scripts que
encadeiam namespace/name/semver diretamente dos metadados de governanca.

Use `--promote-profile=<handle>` para emitir um bloco JSON de metadados (incluindo todos os aliases
registrados) que pode ser colado em `chunker_registry_data.rs` ao promover um novo perfil padrao:

```
$ cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- \
    --promote-profile=sorafs.sf1@1.0.0
```

O relatorio principal (e o arquivo de prova opcional) inclui o digest raiz, os bytes de folha amostrados
(codificados em hex) e os digests irmaos de segmento/chunk para que os verificadores possam
recalcular o hash das camadas de 64 KiB/4 KiB contra o valor `por_root_hex`.

Para validar uma prova existente contra um payload, passe o caminho via
`--por-proof-verify` (o CLI adiciona `"por_proof_verified": true` quando o testemunho
corresponde a raiz calculada):

```
$ cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- ./docs.tar \
    --por-proof-verify=leaf.proof.json
```

Para amostragem em lote, use `--por-sample=<count>` e opcionalmente forneca um caminho de seed/saida.
O CLI garante ordenacao deterministica (seeded com `splitmix64`) e truncara automaticamente quando
a requisicao exceder as folhas disponiveis:

```
$ cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- ./docs.tar \
    --por-sample=8 --por-sample-seed=0xfeedface --por-sample-out=por.samples.json
```
```

O manifest stub espelha os mesmos dados, o que e conveniente ao automatizar a selecao de
`--chunker-profile-id` em pipelines. Ambos os CLIs de chunk store tambem aceitam a forma de handle canonico
(`--profile=sorafs.sf1@1.0.0`) para que scripts de build evitem hard-codear IDs numericos:

```
$ cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- --list-chunker-profiles
[
  {
    "profile_id": 1,
    "namespace": "sorafs",
    "name": "sf1",
    "semver": "1.0.0",
    "handle": "sorafs.sf1@1.0.0",
    "min_size": 65536,
    "target_size": 262144,
    "max_size": 524288,
    "break_mask": "0x0000ffff",
    "multihash_code": 31
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
    chunk_profile: profile_id (implicito via registro)
    capabilities: [...]
}
```

O agendamento de chunks multi-source e anunciado via a capacidade `range`. O CLI aceita isso com
`--capability=range[:streams]`, onde o sufixo numerico opcional codifica a concorrencia preferida
para fetch por range do provider (por exemplo, `--capability=range:64` anuncia um budget de 64 streams).
Quando omitido, consumidores recorrem ao hint geral `max_streams` publicado em outro ponto do advert.

Ao solicitar dados CAR, clientes devem enviar um header `Accept-Chunker` listando tuplas
`(namespace, name, semver)` em ordem de preferencia:

```
Accept-Chunker: sorafs.sf1;version=1.0.0, legacy.fastcdc;version=0.9.0
```

Gateways selecionam um perfil suportado mutuamente (default `sorafs.sf1@1.0.0`)
e refletem a decisao via o header de resposta `Content-Chunker`. Manifests
embutem o perfil escolhido para que nos downstream possam validar o layout de chunks
sem depender da negociacao HTTP.

### Compatibilidade CAR

O envelope canonico do manifest usa raizes CIDv1 com `dag-cbor` (`0x71`). Para compatibilidade legacy
mantemos um caminho de exportacao CARv1+SHA-2:

* **Caminho primario** - CARv2, digest de payload BLAKE3 (`0x1f` multihash),
  `MultihashIndexSorted`, perfil de chunk registrado como acima.
* **Bridge legacy** - CARv1, digest de payload SHA-256 (`0x12` multihash). Servidores
  PODEM expor esta variante quando o cliente omite `Accept-Chunker` ou solicita
  `Accept-Digest: sha2-256`.

Manifests sempre anunciam o commitment CARv2/BLAKE3. As faixas legacy fornecem headers
adicionais para compatibilidade, mas nao devem substituir o digest canonico.

### Conformidade

* O perfil `sorafs.sf1@1.0.0` mapeia para os fixtures publicos em
  `fixtures/sorafs_chunker` e os corpora registrados em
  `fuzz/sorafs_chunker`. A paridade end-to-end e exercitada em Rust, Go e Node
  via os testes fornecidos.
* `chunker_registry::lookup_by_profile` afirma que os parametros do descriptor
  correspondem a `ChunkProfile::DEFAULT` para evitar divergencia acidental.
* Manifests produzidos por `iroha sorafs toolkit pack` e `sorafs_manifest_stub` incluem os metadados do registro.
