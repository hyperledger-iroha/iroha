---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/chunker-registry.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: registro de chunker
título: Реестр профилей chunker SoraFS
sidebar_label: Реестр chunker
descrição: ID do perfil, parâmetros e plano de execução para restaurar o chunker SoraFS.
---

:::nota História Canônica
Esta página contém `docs/source/sorafs/chunker_registry.md`. Se você tiver uma cópia sincronizada, a estrela do Sphinx não estará disponível na configuração.
:::

## Реестр профилей chunker SoraFS (SF-2a)

O bloco SoraFS permite que o chunking seja um arquivo com namespaced.
O perfil do perfil permite determinar parâmetros de CDC, metadanos semver e organizar digest/multicodec, usar manifestos e CAR-ARхивах.

Авторы профилей должны обратиться к
[`docs/source/sorafs/chunker_profile_authoring.md`](./chunker-profile-authoring.md)
para obter mais informações, verifique a validação e a execução do procedimento
отправкой новых записей. Possível definição de governança para o gerenciamento de governança
[Restaurante de implementação](./chunker-registry-rollout-checklist.md) e
[manifesto do manual na preparação](./staging-manifest-playbook), este é o produto
luminárias em preparação e produção.

### Perfil

| Espaço para nome | Eu | SemVer | Perfil de identificação | Minha (байты) | Цель (байты) | Max (байты) | Máscara de proteção | Multihash | Alias ​​| Nomeação |
|-----------|-----|--------|------------|-------------|--------------|-------------|---------------|-----------|------------|------------|
| `sorafs` | `sf1` | `1.0.0` | `1` | 65536 | 262144 | 524288 | `0x0000ffff` | `0x1f` (BLAKE3-256) | `["sorafs.sf1@1.0.0", "sorafs.sf1@1.0.0"]` | Perfil canônico, usado em luminárias SF-1 |

Registre-se no código como `sorafs_manifest::chunker_registry` (regule [`chunker_registry_charter.md`](./chunker-registry-charter.md)). Каждая запись
Use o `ChunkerProfileDescriptor` com os seguintes parâmetros:

* `namespace` – perfil de perfil de grupo de log (por exemplo, `sorafs`).
* `name` – читаемый человеком ярлык профиля (`sf1`, `sf1-fast`, …).
* `semver` – versão semântica para definir parâmetros.
* `profile` – número `ChunkProfile` (mín/alvo/máx/máscara).
* `multihash_code` – multihash, usado na ferramenta de resumo (`0x1f`
  para SoraFS para uso).

Manifesto serializado perfil через `ChunkingProfileV1`. Структура записывает метаданные реестра
(namespace, nome, semver) é usado nos parâmetros do CDC e no endereço de e-mail que você usa. Potrebitel
должны сначала попытаться выполнить lookup в реестре по `profile_id` e использовать inline-parâmetros,
когда встречаются неизвестные ID; список алиасов гарантирует, que clientes HTTP podem produzir
alça canônica (`namespace.name@semver`) foi usada para referência em `profile_aliases`, para o carro

Para obter uma configuração de ferramentas, clique na CLI auxiliar:

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

Para esta CLI, digite JSON (`--json-out`, `--por-json-out`, `--por-proof-out`,
`--por-sample-out`), digite `-` em qualquer lugar, para definir a carga útil no stdout em vez de
создания файла. Esta ferramenta é necessária em ferramentas de acordo com o padrão de segurança
поведения печати основного отчета.### Matriz de implementação e implementação do plano


A tabela ainda está fora do status do dispositivo `sorafs.sf1@1.0.0` no componente de montagem.
"Bridge" относится к совместимому каналу CARv1 + SHA-256, который требует явной
número de cliente (`Accept-Chunker` + `Accept-Digest`).

| Componente | Status | Nomeação |
|-----------|--------|-----------|
| `sorafs_manifest_chunk_store` | ✅ Suporte | Валидирует канонический handle + алиасы, стримит отчеты через `--json-out=-` e применяет charter реестра через `ensure_charter_compliance()`. |
| `sorafs_fetch` (desenvolvedor orquestrador) | ✅ Suporte | Selecione `chunk_fetch_specs`, use payloads como `range` e atualize seu CARv2. |
| Acessórios SDK (Rust/Go/TS) | ✅ Suporte | Solução de problemas `export_vectors`; identificador canônico идет первым в каждом списке алиасов и подписан envelopes do conselho. |
| Perfil de negociação no gateway Torii | ✅ Suporte | Реализует полную грамматику `Accept-Chunker`, включает заголовки `Content-Chunker` e открывает ponte CARv1 только по явным rebaixamento. |

Развертывание telеметрии:

- **Телеметрия fetch чанков** — CLI Iroha `sorafs toolkit pack` эмитит digere чанков, метаданные CAR e корни PoR para ingestão em painéis.
- **Anúncios de provedores** — anúncios de cargas úteis usam recursos de metadaнные e aliases; teste a capacidade `/v1/sorafs/providers` (por exemplo, capacidade máxima `range`).
- **Мониторинг gateway** — операторы должны сообщать пары `Content-Chunker`/`Content-Digest`, чтобы обнаруживать неожиданные rebaixamentos; ожидается, что использование bridge снизится нуля до депрекации.

Política de deprecação: como usar o perfil profissional, definir uma nova publicação
Ponte CARv1 ou gateways de produção.

Isso fornece dados concretos sobre PoR, usando índices chunk/segment/leaf e необходимости
sofra a prova no disco:

```
$ cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- ./docs.tar \
    --por-proof=0:0:0 --por-proof-out=leaf.proof.json
```

Você pode precisar de um perfil de ID de identificação (`--profile-id=1`) ou de uma configuração de identificador
(`--profile=sorafs.sf1@1.0.0`); identificador de formato удобен для скриптов, которые
передают namespace/name/semver напрямую из metadados de governança.

Use `--promote-profile=<handle>` para seus metadados JSON (você pode
зарегистрированные алиасы), который можно вставить в `chunker_registry_data.rs` por
Prodвижении нового профиля по умолчанию:

```
$ cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- \
    --promote-profile=sorafs.sf1@1.0.0
```

Основной отчет (e необязательный файл prova) включает корневой digest, байты выбранного folha
(em hexadecimal) e resumos de irmãos сегмента/чанка, чтобы верификаторы могли пересчитать hash слоев
64 KiB/4 KiB относительно значения `por_root_hex`.

Чтобы валидировать существующий prova de carga útil относительно, передайте путь через
`--por-proof-verify` (CLI instalado `"por_proof_verified": true`, como mostrado
совпадает с вычисленным корнем):

```
$ cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- ./docs.tar \
    --por-proof-verify=leaf.proof.json
```

Para o pacote, você precisa usar `--por-sample=<count>` e primeiro colocar sementes/sementes.
CLI garante determinação de proporção (`splitmix64` semeado) e uso de proteção,
veja a lista de downloads disponíveis:```
$ cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- ./docs.tar \
    --por-sample=8 --por-sample-seed=0xfeedface --por-sample-out=por.samples.json
```
```

Manifest stub отражает те же данные, что удобно при скриптинге выбора `--chunker-profile-id`
в пайплайнах. Оба chunk store CLI также принимают канонический формат handle
(`--profile=sorafs.sf1@1.0.0`), поэтому build-скрипты могут избежать жесткого
хардкода числовых ID:

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

Поле `handle` (`namespace.name@semver`) совпадает с тем, что CLIs принимают через
`--profile=…`, поэтому его можно безопасно копировать в автоматизацию.

### Согласование chunker

Gateways и клиенты объявляют поддерживаемые профили через provider adverts:

```
ProviderAdvertBodyV1 {
    ...
    chunk_profile: profile_id (não definido para configuração)
    capacidades: [...]
}
```

Планирование multi-source чанков объявляется через capability `range`. CLI принимает ее с
`--capability=range[:streams]`, где опциональный числовой суффикс кодирует предпочтительную
параллельность range-fetch у провайдера (например, `--capability=range:64` объявляет бюджет 64 streams).
Когда суффикс отсутствует, потребители возвращаются к общему hint `max_streams`, опубликованному в другом
месте advert.

При запросе CAR-данных клиенты должны отправлять заголовок `Accept-Chunker`, перечисляя кортежи
`(namespace, name, semver)` в порядке предпочтения:

```

Os gateways são usados ​​no perfil do perfil (por `sorafs.sf1@1.0.0`) e abertos
решение в ответном заголовке `Content-Chunker`. Manifestos встраивают выбранный профиль, чтобы
O downstream pode ser validado por meio de transações HTTP.

### CARRO Совместимость

сохраняется путь экспорта CARv1+SHA-2:

* **Основной путь** – CARv2, resumo de carga útil BLAKE3 (`0x1f` multihash),
  `MultihashIndexSorted`, pedaço de perfil definido como você.
  Você pode usar esta variante se o cliente abrir `Accept-Chunker` ou fechar
  `Accept-Digest: sha2-256`.

заголовки для совместимости, não не должны заменять канонический digest.

### Соответствие

* O perfil `sorafs.sf1@1.0.0` contém luminárias públicas em
  `fixtures/sorafs_chunker` e corpora, registrados em
  `fuzz/sorafs_chunker`. Parceria de ponta a ponta testada em Rust, Go e Node
  через предоставленные тесты.
* `chunker_registry::lookup_by_profile` é compatível, este parâmetro é descrito
  Se você usar `ChunkProfile::DEFAULT`, verifique se há algum problema.
* Manifestos, производимые `iroha app sorafs toolkit pack` e `sorafs_manifest_stub`, включают метаданные реестра.