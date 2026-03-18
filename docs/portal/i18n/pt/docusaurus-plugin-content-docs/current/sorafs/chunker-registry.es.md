---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/chunker-registry.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: registro de chunker
título: Registro de perfis de chunker de SoraFS
sidebar_label: Registro do chunker
descrição: IDs de perfil, parâmetros e plano de negociação para o registro de chunker de SoraFS.
---

:::nota Fonte canônica
Esta página reflete `docs/source/sorafs/chunker_registry.md`. Mantenha ambas as cópias sincronizadas até que o conjunto de documentação Sphinx herdado seja retirado.
:::

## Registro de perfis de chunker de SoraFS (SF-2a)

A pilha de SoraFS negocia o comportamento de chunking por meio de um pequeno registro com espaços de nomes.
Cada perfil atribui parâmetros CDC deterministas, metadados sempre e o digest/multicodec esperado usado em manifestos e arquivos CAR.

Os autores de perfis devem ser consultados
[`docs/source/sorafs/chunker_profile_authoring.md`](./chunker-profile-authoring.md)
para os metadados necessários, a lista de verificação de validação e a planta de proposta antes de enviar novas entradas.
Uma vez que o governo experimente uma mudança, siga o
[lista de verificação de implementação do registro](./chunker-registry-rollout-checklist.md) e el
[manual de manifesto em preparação](./staging-manifest-playbook) para promoção
os jogos são encenação e produção.

### Perfis

| Espaço para nome | Nome | SemVer | ID do perfil | Mínimo (bytes) | Alvo (bytes) | Máx. (bytes) | Máscara de corte | Multihash | Alias ​​| Notas |
|-----------|--------|--------|------------|-------------|----------------|-------------|-----------------|-----------|-------|-------|
| `sorafs` | `sf1` | `1.0.0` | `1` | 65536 | 262144 | 524288 | `0x0000ffff` | `0x1f` (BLAKE3-256) | `["sorafs.sf1@1.0.0", "sorafs.sf1@1.0.0"]` | Perfil canônico usado em luminárias SF-1 |

O registro vive no código como `sorafs_manifest::chunker_registry` (governado por [`chunker_registry_charter.md`](./chunker-registry-charter.md)). Cada entrada
se expressa como um `ChunkerProfileDescriptor` com:

* `namespace` – agrupamento lógico de perfis relacionados (p. ex., `sorafs`).
* `name` – etiqueta legível para humanos (`sf1`, `sf1-fast`, …).
* `semver` – cadeia de versão semântica para o conjunto de parâmetros.
* `profile` – o `ChunkProfile` real (mín/alvo/máx/máscara).
* `multihash_code` – o multihash usado para produzir resumos de pedaços (`0x1f`
  para o padrão SoraFS).

O manifesto serializa perfis por meio de `ChunkingProfileV1`. Estrutura de registro
os metadados do registro (namespace, name, semver) junto com os parâmetros CDC
em bruto e a lista de alias mostrada arriba. Os consumidores deveriam tentar uma
procure no registro por `profile_id` e recorra aos parâmetros inline quando
aparezcan IDs desconhecidos; a lista de alias garante que os clientes HTTP possam
siga enviando identificadores heredados em `Accept-Chunker` sem aviso prévio. As regras da
carta de registro exige que o cabo canônico (`namespace.name@semver`) seja la
primeira entrada em `profile_aliases`, seguida de qualquer alias heredado.

Para inspecionar o registro a partir de ferramentas, execute o auxiliar CLI:

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
```Todas as sinalizações da CLI que descrevem JSON (`--json-out`, `--por-json-out`, `--por-proof-out`,
`--por-sample-out`) aceita `-` como rota, que transmite a carga útil para stdout em vez de
crie um arquivo. Isso facilita a canalização de dados para ferramentas enquanto você mantém o
por defeito de comportamento na impressão do relatório principal.

### Matriz de implementação e plano de implementação


A tabela a seguir captura o estado atual de suporte para `sorafs.sf1@1.0.0` en
componentes principais. "Bridge" é referência para carril CARv1 + SHA-256
que requer negociação explícita do cliente (`Accept-Chunker` + `Accept-Digest`).

| Componente | Estado | Notas |
|-----------|--------|-------|
| `sorafs_manifest_chunk_store` | ✅ Soportado | Valida o identificador canônico + alias, transmite relatórios via `--json-out=-` e aplica a carta de registro com `ensure_charter_compliance()`. |
| `sorafs_manifest_stub` | ⚠️ Retirado | Construtor de manifesto fora de suporte; usa `iroha app sorafs toolkit pack` para CAR/manifesto empaquetado e mantém `--plan=-` para revalidação determinista. |
| `sorafs_provider_advert_stub` | ⚠️ Retirado | Auxiliar de validação offline apenas; os anúncios do provedor devem ser produzidos pelo pipeline de publicação e validados via `/v1/sorafs/providers`. |
| `sorafs_fetch` (desenvolvedor orquestrador) | ✅ Soportado | Lee `chunk_fetch_specs`, contém cargas úteis de capacidade `range` e conjunto de saída CARv2. |
| Fixações do SDK (Rust/Go/TS) | ✅ Soportado | Regeneradas via `export_vectors`; o identificador canônico aparece primeiro em cada lista de alias e é confirmado por sobres del consejo. |
| Negociação de perfis no gateway Torii | ✅ Soportado | Implementa a gramática completa de `Accept-Chunker`, inclui cabeçalhos `Content-Chunker` e expõe a ponte CARv1 apenas em solicitações de downgrade explícitas. |

Despliegue de telemetria:

- **Telemetria de busca de pedaços** — a CLI de Iroha `sorafs toolkit pack` emite resumos de pedaços, metadados CAR e fontes PoR para ingestão em painéis.
- **Anúncios de provedores** — as cargas úteis de anúncios incluem metadados de capacidades e alias; cobertura válida via `/v1/sorafs/providers` (p. ej., presença da capacidade `range`).
- **Monitoramento de gateway** — os operadores devem reportar os pares `Content-Chunker`/`Content-Digest` para detectar downgrades inesperados; espera-se que o uso da ponte seja mantido a zero antes da descontinuação.

Política de depreciação: uma vez que se ratifique um perfil sucessor, programe uma janela de publicação dupla
(documentado na proposta) antes de marcar `sorafs.sf1@1.0.0` como obsoleto no registro e remover o
bridge CARv1 dos gateways em produção.

Para inspecionar um teste PoR específico, fornece índices de pedaço/segmento/hoja e opcionalmente
persiste la prueba a disco:

```
$ cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- ./docs.tar \
    --por-proof=0:0:0 --por-proof-out=leaf.proof.json
```

Você pode selecionar um perfil por id numérico (`--profile-id=1`) ou por identificador de registro
(`--profile=sorafs.sf1@1.0.0`); o formato com handle é conveniente para scripts que
pasen namespace/name/semver diretamente dos metadados de governo.Usa `--promote-profile=<handle>` para emitir um bloco JSON de metadados (incluindo todos os alias
registrados) que você pode pegar em `chunker_registry_data.rs` ao promover um novo perfil por defeito:

```
$ cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- \
    --promote-profile=sorafs.sf1@1.0.0
```

O relatório principal (e o arquivo de teste opcional) inclui a raiz do resumo, os bytes de hoje mostrados
(codificados em hexadecimal) e os resumos de segmentos/chunk para que os verificadores possam rehashear
as capas de 64 KiB/4 KiB contra o valor `por_root_hex`.

Para validar um teste existente contra uma carga útil, passe pela rota via
`--por-proof-verify` (o CLI anexado `"por_proof_verified": true` quando o teste
coincide com a razão calculada):

```
$ cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- ./docs.tar \
    --por-proof-verify=leaf.proof.json
```

Para mostre em lote, use `--por-sample=<count>` e opcionalmente fornece uma rota de semente/salida.
El CLI garante uma ordem determinista (sembrada com `splitmix64`) e truncará a forma transparente quando
a solicitação supere as horas disponíveis:

```
$ cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- ./docs.tar \
    --por-sample=8 --por-sample-seed=0xfeedface --por-sample-out=por.samples.json
```
```

El manifest stub refleja los mismos datos, lo que es conveniente al automatizar la selección de
`--chunker-profile-id` en pipelines. Ambos CLIs de chunk store también aceptan la forma de handle canónico
(`--profile=sorafs.sf1@1.0.0`) para que los scripts de build puedan evitar hard-codear IDs numéricos:

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

El campo `handle` (`namespace.name@semver`) coincide con lo que aceptan los CLIs vía
`--profile=…`, por lo que es seguro copiarlo directamente a la automatización.

### Negociar chunkers

Gateways y clientes anuncian perfiles soportados vía provider adverts:

```
ProviderAdvertBodyV1 {
    ...
    chunk_profile: profile_id (implícito via registro)
    capacidades: [...]
}
```

La programación de chunks multi-source se anuncia vía la capacidad `range`. El CLI la acepta con
`--capability=range[:streams]`, donde el sufijo numérico opcional codifica la concurrencia preferida
de fetch por rango del proveedor (por ejemplo, `--capability=range:64` anuncia un presupuesto de 64 streams).
Cuando se omite, los consumidores vuelven al hint general `max_streams` publicado en otra parte del advert.

Al solicitar datos CAR, los clientes deben enviar un header `Accept-Chunker` que liste tuplas
`(namespace, name, semver)` en orden de preferencia:

```

Os gateways selecionam um perfil suportado mutuamente (por defeito `sorafs.sf1@1.0.0`)
e reflita sobre a decisão por meio do cabeçalho de resposta `Content-Chunker`. Os manifestos
embeben o perfil escolhido para que os nós downstream possam validar o layout dos chunks
não depende da negociação HTTP.

### Suporte CAR

manteremos uma rota de exportação CARv1+SHA-2:

* **Ruta primária** – CARv2, resumo da carga útil BLAKE3 (`0x1f` multihash),
  `MultihashIndexSorted`, perfil de pedaço registrado como arriba.
  PUEDEN exponente esta variante quando o cliente omite `Accept-Chunker` ou solicita
  `Accept-Digest: sha2-256`.

adicionais para transição, mas não é necessário substituir o resumo canônico.

### Conformidade

* O perfil `sorafs.sf1@1.0.0` é atribuído aos equipamentos públicos em
  `fixtures/sorafs_chunker` e os corpos registrados em
  `fuzz/sorafs_chunker`. A paridade ponta a ponta é executada em Rust, Go e Node
  mediante as tentativas provisórias.
* `chunker_registry::lookup_by_profile` afirma que os parâmetros do descritor
  coincide com `ChunkProfile::DEFAULT` para evitar divergências acidentais.
* Os manifestos produzidos por `iroha app sorafs toolkit pack` e `sorafs_manifest_stub` incluem os metadados do registro.