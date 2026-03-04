---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/chunker-registry.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: registro fragmentador
título: Registro de perfis de chunker da SoraFS
sidebar_label: Registro de fragmentador
descripción: ID de perfil, parámetros y plano de negociación para el registro de fragmentador de SoraFS.
---

:::nota Fuente canónica
Esta página espelha `docs/source/sorafs/chunker_registry.md`. Mantenha ambas como copias sincronizadas.
:::

## Registro de perfis de fragmentador de SoraFS (SF-2a)

Una pila SoraFS negocia el comportamiento de fragmentación a través de un registro pequeño con espacio de nombres.
Cada perfil atribuye parámetros CDC deterministas, metadados semver y o digest/multicodec esperados usados ​​en manifiestos y archivos CAR.

Autores de perfis deben consultar
[`docs/source/sorafs/chunker_profile_authoring.md`](./chunker-profile-authoring.md)
Para los metadados requeridos, una lista de verificación de validación y el modelo de propuesta antes de submeter novas entradas.
Una vez que un gobernador apruebe una mudanca, siga o
[checklist de rollout do registro](./chunker-registry-rollout-checklist.md) e o
[playbook de manifest em staging](./staging-manifest-playbook) para promover
os accesorios, puesta en escena y producción.

### Perfis| Espacio de nombres | Nombre | SemVer | ID de perfil | Mín. (bytes) | Destino (bytes) | Máx. (bytes) | Máscara de quebra | Multihash | Alias ​​| Notas |
|-----------|------|--------|-------------|-------------|----------------|-------------|------------------|-----------|--------|-------|
| `sorafs` | `sf1` | `1.0.0` | `1` | 65536 | 262144 | 524288 | `0x0000ffff` | `0x1f` (BLAKE3-256) | `["sorafs.sf1@1.0.0", "sorafs.sf1@1.0.0"]` | Perfil canónico usado en accesorios SF-1 |

El registro vive sin código como `sorafs_manifest::chunker_registry` (gobernado por [`chunker_registry_charter.md`](./chunker-registry-charter.md)). cada entrada
y expresar como un `ChunkerProfileDescriptor` com:

* `namespace` - agrupamiento lógico de daños relacionados (ej., `sorafs`).
* `name` - rotulo legivel para humanos (`sf1`, `sf1-fast`, ...).
* `semver` - cadena de versao semántica para el conjunto de parámetros.
* `profile` - o `ChunkProfile` real (mín/objetivo/máx/máscara).
* `multihash_code` - o multihash usado para producir resúmenes de fragmentos (`0x1f`
  por defecto de SoraFS).El manifiesto serializa perfis a través de `ChunkingProfileV1`. La estructura registra los metadados.
 hacer registro (namespace, name, semver) junto con los parámetros CDC brutos
e una lista de alias mostrada acima. Los consumidores deben tener el primer tentar uma
busca no registro por `profile_id` y recorrer aos parámetros inline cuando
IDs desconhecidos aparecerán; una lista de alias garantizados para que los clientes HTTP posean
Continuar enviando manijas alternativas em `Accept-Chunker` sin adivinhar. como lo hacen los regras
charter do registro exigem que o handle canonico (`namespace.name@semver`) seja a
primeira entrada em `profile_aliases`, seguida por quaisquer alias alternativos.

Para inspeccionar o registrar desde las herramientas, ejecute el asistente CLI:

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

Todas las banderas hacen CLI que elimina JSON (`--json-out`, `--por-json-out`, `--por-proof-out`,
`--por-sample-out`) aceitam `-` como camino, o que transmite o carga útil para stdout en vez de
criar un archivo. Isso torna facil encadear os dados para tooling mantendo o
comportamento padrao de imprimir o relatorio principal.

### Matriz de rollout y plano de implantación


A tabela abaixo captura o status actual de soporte para `sorafs.sf1@1.0.0` nos
componentes principales. "Puente" referencia a faixa CARv1 + SHA-256
que requer negociacao explicita do cliente (`Accept-Chunker` + `Accept-Digest`).| Componente | Estado | Notas |
|-----------|--------|-------|
| `sorafs_manifest_chunk_store` | ✅ Apoyado | Valida o handle canonico + alias, faz stream de relatorios via `--json-out=-` y aplica o charter do registro via `ensure_charter_compliance()`. |
| `sorafs_manifest_stub` | ⚠️Retirado | Constructor de manifiestos foros de apoyo; use `iroha app sorafs toolkit pack` para empacotamento CAR/manifest e mantenha `--plan=-` para revalidacao deterministica. |
| `sorafs_provider_advert_stub` | ⚠️Retirado | Ayudante de validación sin conexión apenas; Los anuncios de proveedores deben ser producidos por el pipeline de publicacao y validados a través de `/v1/sorafs/providers`. |
| `sorafs_fetch` (organizador de desarrollo) | ✅ Apoyado | El `chunk_fetch_specs`, comprende cargas útiles de capacidad `range` y monta dicho CARv2. |
| Accesorios de SDK (Rust/Go/TS) | ✅ Apoyado | Regeneradas vía `export_vectors`; o handle canonico aparece primero en cada lista de alias y e assinado por sobres do conselho. |
| Negociacao de perfil no gateway Torii | ✅ Apoyado | Implemente la gramática completa de `Accept-Chunker`, incluidos los encabezados `Content-Chunker` y la exposición del puente CARv1 apenas en solicitudes explícitas de degradación. |

Implementación de telemetría:- **Telemetría de recuperación de fragmentos** - o CLI Iroha `sorafs toolkit pack` emite resúmenes de fragmentos, metadados CAR y eleva PoR para ingestao en paneles de control.
- **Anuncios del proveedor** - Las cargas útiles de anuncios incluyen metadados de capacidad y alias; valide cobertura vía `/v1/sorafs/providers` (ej., presenteca da capacidade `range`).
- **Monitoramento de gateway** - los operadores deben reportar los pares `Content-Chunker`/`Content-Digest` para detectar downgrades inesperados; espera-se que o uso do bridge tenda a cero antes da deprecacao.

Politica de deprecacao: uma vez que um perfil sucesor seja ratificado, agenda uma janela de publicacao dupla
puente CARv1 dos puertas de enlace en producción.

Para inspeccionar un testemunho PoR específico, forneca indices de chunk/segmento/folha y opcionalmente
persista a prueba no disco:

```
$ cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- ./docs.tar \
    --por-proof=0:0:0 --por-proof-out=leaf.proof.json
```

Puede seleccionar un perfil por ID numérico (`--profile-id=1`) o por identificador de registro.
(`--profile=sorafs.sf1@1.0.0`); una forma manejable y conveniente para scripts que
encadeiam namespace/name/semver directamente dos metadados degobernanza.

Utilice `--promote-profile=<handle>` para emitir un bloque JSON de metadados (incluidos todos los alias
registrados) que pueden ser colados en `chunker_registry_data.rs` para promover un nuevo perfil padrao:

```
$ cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- \
    --promote-profile=sorafs.sf1@1.0.0
```El relatorio principal (y el archivo de prueba opcional) incluye el resumen raiz, los bytes de folha demostrados.
(codificados en hexadecimal) e os digests irmaos de segmento/chunk para que os verificadores possam
recalcular o hash das camadas de 64 KiB/4 KiB contra el valor `por_root_hex`.

Para validar una prueba existente contra una carga útil, pase el camino a través
`--por-proof-verify` (o CLI adicional `"por_proof_verified": true` cuando se prueba
corresponde a raiz calculada):

```
$ cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- ./docs.tar \
    --por-proof-verify=leaf.proof.json
```

Para amostragem em lote, use `--por-sample=<count>` y opcionalmente forneca um caminho de seed/saida.
La CLI garantiza orden determinística (seeded com `splitmix64`) y se trunca automáticamente cuando
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
$ ejecución de carga -p sorafs_manifest --bin sorafs_manifest_stub --list-chunker-profiles
[
  {
    "id_perfil": 1,
    "espacio de nombres": "sorafs",
    "nombre": "sf1",
    "semver": "1.0.0",
    "manejar": "sorafs.sf1@1.0.0",
    "tamaño_mínimo": 65536,
    "tamaño_objetivo": 262144,
    "tamaño_máximo": 524288,
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
ProveedorAdvertBodyV1 {
    ...
    chunk_profile: perfil_id (implícito vía registro)
    capacidades: [...]
}
```

O agendamento de chunks multi-source e anunciado via a capacidade `range`. O CLI aceita isso com
`--capability=range[:streams]`, onde o sufixo numerico opcional codifica a concorrencia preferida
para fetch por range do provider (por exemplo, `--capability=range:64` anuncia um budget de 64 streams).
Quando omitido, consumidores recorrem ao hint geral `max_streams` publicado em outro ponto do advert.

Ao solicitar dados CAR, clientes devem enviar um header `Accept-Chunker` listando tuplas
`(namespace, name, semver)` em ordem de preferencia:

```Las puertas de enlace seleccionan un perfil compatible entre sí (predeterminado `sorafs.sf1@1.0.0`)
e refleja la decisión a través del encabezado de respuesta `Content-Chunker`. Manifiestos
Embutem o perfil escondido para que nos downstream pueda validar o diseño de trozos
Sem depende de la negociación HTTP.

### Soporte COCHE

Mantemos un camino de exportación CARv1+SHA-2:

* **Caminho primario** - CARv2, resumen de carga útil BLAKE3 (`0x1f` multihash),
  `MultihashIndexSorted`, perfil de trozo registrado como acima.
  PODEM exporta esta variante cuando el cliente omite `Accept-Chunker` o solicita
  `Accept-Digest: sha2-256`.

Adicionais para transicao, mas nao devem substituir o digest canonico.

### Conformidad

* O perfil `sorafs.sf1@1.0.0` mapeia para los accesorios públicos en
  `fixtures/sorafs_chunker` y los cuerpos registrados en
  `fuzz/sorafs_chunker`. Paridad de extremo a extremo y ejercitada en Rust, Go y Node
  a través de los testículos formados.
* `chunker_registry::lookup_by_profile` afirma que los parámetros del descriptor
  corresponde a `ChunkProfile::DEFAULT` para evitar divergencia accidental.
* Los manifiestos producidos por `iroha app sorafs toolkit pack` e `sorafs_manifest_stub` incluyen los metadados del registro.