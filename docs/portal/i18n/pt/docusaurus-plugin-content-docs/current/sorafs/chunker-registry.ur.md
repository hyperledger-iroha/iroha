---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/chunker-registry.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: registro de chunker
título: Registro de perfil do chunker SoraFS
sidebar_label: Registro Chunker
descrição: Registro de chunker SoraFS کے لیے IDs de perfil, parâmetros e plano de negociação۔
---

:::nota مستند ماخذ
:::

## Registro de perfil do chunker SoraFS (SF-2a)

Comportamento de agrupamento de pilha SoraFS کو ایک چھوٹے registro com namespace کے ذریعے negociar کرتا ہے۔
ہر parâmetros CDC determinísticos de perfil, semver metadados اور atribuição esperada de digest/multicodec کرتا ہے جو manifestos اور arquivos CAR میں استعمال ہوتا ہے۔

Autores de perfil کو
[`docs/source/sorafs/chunker_profile_authoring.md`](./chunker-profile-authoring.md)
میں مطلوبہ metadados, checklist de validação e modelo de proposta دیکھنا چاہیے قبل اس کے کہ وہ نئی entradas enviar
جب governança تبدیلی aprovar کر دے تو
[lista de verificação de implementação do registro](./chunker-registry-rollout-checklist.md) aqui
[manual de manifesto de preparação](./staging-manifest-playbook) کے مطابق fixtures کو preparação اور produção میں promover کریں۔

### Perfis

| Espaço para nome | Nome | SemVer | ID do perfil | Mínimo (bytes) | Alvo (bytes) | Máx. (bytes) | Quebrar máscara | Multihash | Aliases | Notas |
|-----------|------|--------|------------|-------------|----------------|-------------|------------|-----------|-------------|-------|
| `sorafs` | `sf1` | `1.0.0` | `1` | 65536 | 262144 | 524288 | `0x0000ffff` | `0x1f` (BLAKE3-256) | `["sorafs.sf1@1.0.0", "sorafs.sf1@1.0.0"]` | SF-1 fixtures میں استعمال ہونے والا perfil canônico |

Código de registro میں `sorafs_manifest::chunker_registry` کے طور پر موجود ہے (جسے [`chunker_registry_charter.md`](./chunker-registry-charter.md) governar کرتا ہے)۔ ہر entrada ایک `ChunkerProfileDescriptor` کے طور پر ظاہر ہوتی ہے جس میں:

* `namespace` – متعلقہ perfis کی agrupamento lógico (مثلاً `sorafs`)۔
* `name` – انسان کے لیے etiqueta de perfil legível (`sf1`, `sf1-fast`, …)۔
* `semver` – conjunto de parâmetros کے لیے string de versão semântica۔
* `profile` – صل `ChunkProfile` (mín/alvo/máx/máscara)۔
* `multihash_code` – chunk digests بناتے وقت استعمال ہونے والا multihash (`0x1f`
  SoraFS padrão کے لیے)۔

Manifesto `ChunkingProfileV1` کے ذریعے profiles کو serialize کرتا ہے۔ یہ estruturar metadados de registro
(namespace, nome, semver) کو parâmetros CDC brutos اور اوپر دکھائی گئی lista de alias کے ساتھ registro کرتا ہے۔
Consumidores کو پہلے `profile_id` کے ذریعے pesquisa de registro کرنا چاہیے اور اگر IDs desconhecidos آئیں تو parâmetros inline پر fallback کرنا چاہیے؛

Registro e ferramentas para inspecionar کرنے کے لیے auxiliar CLI چلائیں:

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

CLI کے وہ تمام flags جو JSON لکھتے ہیں (`--json-out`, `--por-json-out`, `--por-proof-out`,
`--por-sample-out`) path کے طور پر `-` قبول کرتے ہیں, جس سے payload stdout پر stream ہوتا ہے بجائے فائل بنانے کے۔
یہ ferramentas میں tubo de dados کرنا آسان بناتا ہے جبکہ relatório principal کو پرنٹ کرنے والا comportamento padrão برقرار رہتا ہے۔

```
$ cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- ./docs.tar \
    --por-proof=0:0:0 --por-proof-out=leaf.proof.json
```
```
$ cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- \
    --promote-profile=sorafs.sf1@1.0.0
```
```
$ cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- ./docs.tar \
    --por-proof-verify=leaf.proof.json
```
```
$ cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- ./docs.tar \
    --por-sample=8 --por-sample-seed=0xfeedface --por-sample-out=por.samples.json
```
```

Manifest stub یہی data mirror کرتا ہے، جو pipelines میں `--chunker-profile-id` selection کو script کرنے کے لیے convenient ہے۔ دونوں chunk store CLIs canonical handle form (`--profile=sorafs.sf1@1.0.0`) بھی accept کرتے ہیں تاکہ build scripts numeric IDs hard-code کرنے سے بچ سکیں:

```
```

`handle` field (`namespace.name@semver`) وہی ہے جو CLIs `--profile=…` کے ذریعے accept کرتے ہیں، اس لیے اسے automation میں براہ راست copy کرنا محفوظ ہے۔

### Negotiating chunkers

Gateways اور clients provider adverts کے ذریعے supported profiles advertise کرتے ہیں:

```
```

Multi-source chunk scheduling `range` capability کے ذریعے announce ہوتی ہے۔ CLI اسے `--capability=range[:streams]` کے ساتھ accept کرتا ہے، جہاں optional numeric suffix provider کی preferred range-fetch concurrency encode کرتا ہے (مثلاً `--capability=range:64` 64-stream budget advertise کرتا ہے)۔ جب یہ omit ہو تو consumers advert میں کہیں اور شائع شدہ general `max_streams` hint پر fallback کرتے ہیں۔

CAR data request کرتے وقت clients کو `Accept-Chunker` header بھیجنا چاہیے جو preference order میں `(namespace, name, semver)` tuples list کرے:

```Perfil de gateways com suporte mútuo Manifestos منتخب perfil incorporado کرتے ہیں تاکہ nós downstream Negociação HTTP پر انحصار کیے بغیر validação do layout do bloco کر سکیں۔



* **Caminho principal** – CARv2, resumo da carga útil BLAKE3 (`0x1f` multihash)،
  `MultihashIndexSorted`, o perfil do pedaço é o registro do registro ہوتا ہے۔


### Conformidade

* `sorafs.sf1@1.0.0` perfil public fixtures (`fixtures/sorafs_chunker`) اور `fuzz/sorafs_chunker` کے تحت registrar corpora سے match کرتا ہے۔ Paridade ponta a ponta Rust, Go اور Node میں دیے گئے testes سے exercício کی جاتی ہے۔
* `chunker_registry::lookup_by_profile` assert کرتا ہے کہ parâmetros do descritor `ChunkProfile::DEFAULT` سے match کریں تاکہ divergência acidental سے بچا جا سکے۔
* `iroha app sorafs toolkit pack` e `sorafs_manifest_stub` سے بنے manifestos میں metadados de registro شامل ہوتی ہے۔