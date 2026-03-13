---
lang: he
direction: rtl
source: docs/portal/docs/sorafs/chunker-registry.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
מזהה: chunker-registry
כותרת: Registro de perfis de chunker da SoraFS
sidebar_label: Registro de chunker
תיאור: מזהי פרפיל, פרמטרים e plano de negociacao לרישום chunker da SoraFS.
---

:::שים לב Fonte canonica
Esta pagina espelha `docs/source/sorafs/chunker_registry.md`. Mantenha ambas as copias sincronizadas.
:::

## רישום של chunker da SoraFS (SF-2a)

מחסנית SoraFS או רכיב חלוקה דרך מרחב השמות.
Cada perfil atribui parametros CDC deterministas, metadados semver e o digest/multicodec esperado usado em manifests e arquivos CAR.

Autores de perfis devem יועץ
[`docs/source/sorafs/chunker_profile_authoring.md`](./chunker-profile-authoring.md)
לרשימת בדיקות מתקדמת, רשימת בדיקה של validacao e o modelo de proposta antes de submeter novas entradas.
Uma vez que a governanca מאשרת uma mudanca, siga o
[רשימת בדיקה לרישום](./chunker-registry-rollout-checklist.md) e o
[playbook de manifest em staging](./staging-manifest-playbook) לקידום מכירות
OS מתקין בימוי e producao.

### פרפיס

| מרחב שמות | Nome | SemVer | ID de perfil | דקות (בתים) | יעד (בתים) | מקסימום (בתים) | מסקרה דה קוברה | Multihash | כינויים | Notas |
|-----------|------|--------|---------------------------------------------------|
| `sorafs` | `sf1` | `1.0.0` | `1` | 65536 | 262144 | 524288 | `0x0000ffff` | `0x1f` (BLAKE3-256) | `["sorafs.sf1@1.0.0", "sorafs.sf1@1.0.0"]` | פרופיל canonico usado em fixtures SF-1 |

O registro vive no codigo como `sorafs_manifest::chunker_registry` (governado por [`chunker_registry_charter.md`](./chunker-registry-charter.md)). קאדה אנטרדה
e expressa como um `ChunkerProfileDescriptor` com:

* `namespace` - agrupamento logico de perfis relacionados (לדוגמה, `sorafs`).
* `name` - rotulo legivel para humanos (`sf1`, `sf1-fast`, ...).
* `semver` - cadeia de versao semantica para o conjunto de parametros.
* `profile` - o `ChunkProfile` אמיתי (מינימום/יעד/מקסימום/מסכה).
* `multihash_code` - o multihash usado ao produzir digests de chunk (`0x1f`
  para o default da SoraFS).

O manifest serializa perfis דרך `ChunkingProfileV1`. A estrutura registra os metadados
 do registro (מרחב שם, שם, semver) junto com os parametros CDC brutos
e a list de aliases mostrada acima. Consumidores devem primeiro tentar uma
busca no registro por `profile_id` e recorrer aos parametros inline quando
תעודות זהות desconhecidos aparecerem; רשימה של כינויים מבטיחים ללקוחות HTTP possam
continuar enviando מטפל ב-Alternatives em `Accept-Chunker` sem adivinhar. כמו שרגרס עושים
charter do registro exigem que o handle canonico (`namespace.name@semver`) seja a
primeira entrada em `profile_aliases`, seguida por quaisquer aliases alternativos.

עבור בדיקה או רישום כדי לבצע כלים, בצע את עוזר CLI:

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
`--por-sample-out`) aceitam `-` como caminho, o que transmite o payload para stdout em vez de
קריאר um arquivo. Iso torna facil encadear os dados para tooling mantendo o
comportamento padrao de imprimir או relatorio principal.

### מאטריז דה פריסה ותכנון אימפלנטאקאו


טבלה אבאיקסו קפטורה או סטטוס תמיכה עבור `sorafs.sf1@1.0.0` nos
רכיבים עיקריים. "גשר" מפנה ל-faixa CARv1 + SHA-256
que requer negociacao explicita do cliente (`Accept-Chunker` + `Accept-Digest`).

| רכיב | סטטוס | Notas |
|-----------|--------|-------|
| `sorafs_manifest_chunk_store` | ✅ סופורטדו | תקף או לטפל ב-Canonico + כינויים, זרם קשרים דרך `--json-out=-` ושימוש ב-Charter לעשות רישום דרך `ensure_charter_compliance()`. |
| `sorafs_manifest_stub` | ⚠️ Retirado | Builder de manifest fora de suporte; השתמש ב-`iroha app sorafs toolkit pack` para empacotamento CAR/manifest e mantenha `--plan=-` para revalidacao deterministica. |
| `sorafs_provider_advert_stub` | ⚠️ Retirado | Helper de validacao אפנים לא מקוונים; פרסומות ספק devem ser produzidos pelo pipeline de publicacao e validados דרך `/v2/sorafs/providers`. |
| `sorafs_fetch` (מתזמר מפתח) | ✅ סופורטדו | Le `chunk_fetch_specs`, מטענים רווחים של capacidade `range` e monta saida CARv2. |
| Fixtures de SDK (Rust/Go/TS) | ✅ סופורטדו | Regeneradas דרך `export_vectors`; o לטפל canonico aparece primeiro em cada list de aliases e e assinado por envelopes do conselho. |
| Negociacao de perfil no gateway Torii | ✅ סופורטדו | יישום גרמטית של `Accept-Chunker`, כולל כותרות `Content-Chunker` e expoe o bridge CARv1 apenas em solicitacoes explicitas degrade down. |

השקת טלמטריה:

- **Telemetria de fetch de chunks** - o CLI Iroha `sorafs toolkit pack` emite digests de chunk, metadados CAR e מעלה PoR para ingestao em לוחות מחוונים.
- **פרסומות של ספקים** - עומסי פרסומות של פרסומות כוללים מטאדוסים וכינויים; valide cobertura via `/v2/sorafs/providers` (לדוגמה, presenca da capacidade `range`).
- **Monitormento de gateway** - Operatores devem reportar os pareamentos `Content-Chunker`/`Content-Digest` para detectar משדרג לאחור ב-esperados; espera-se que o uso do bridge tenda a zero antes da deprecacao.

Politica de deprecacao: uma vez que um perfil sucessor seja ratificado, agende uma janela de publicacao dupla
גשר CARv1 dos gateways em producao.

Para inspecionar um testemunho PoR especifico, forneca indices de chunk/segmento/folha e opcionalmente
persista a prova no disco:

```
$ cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- ./docs.tar \
    --por-proof=0:0:0 --por-proof-out=leaf.proof.json
```

Voce pode selecionar um perfil por id numerico (`--profile-id=1`) ou por handle de registro
(`--profile=sorafs.sf1@1.0.0`); ידית פורמה ונוחות עבור סקריפטים
encadeiam namespace/name/semver diretamente dos metadados de governanca.

השתמש ב-`--promote-profile=<handle>` para emitir um bloco JSON de metadados (כולל כינויים של todos OS
registrados) que pode ser colado em `chunker_registry_data.rs` ao promoter um novo perfil padrao:

```
$ cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- \
    --promote-profile=sorafs.sf1@1.0.0
```O relatorio principal (e o arquivo de prova opcional) כולל o digest raiz, os bytes de folha amostrados
(codificados em hex) e os digests irmaos de segmento/chunk para que os verificadores possam
חישוב מחדש o hash das caadas de 64 KiB/4 KiB contra o valor `por_root_hex`.

Para validar uma prova existente contra umloadload, pass o caminho via
`--por-proof-verify` (או CLI adiciona `"por_proof_verified": true` quando o testemunho
corresponde a raiz calculada):

```
$ cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- ./docs.tar \
    --por-proof-verify=leaf.proof.json
```

למטרות רבות, השתמש ב-`--por-sample=<count>` ואופציונלי פורנקה um caminho de seed/saida.
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
    "פרופיל_מזהה": 1,
    "namespace": "סורפים",
    "name": "sf1",
    "semver": "1.0.0",
    "handle": "sorafs.sf1@1.0.0",
    "גודל_מינימלי": 65536,
    "מטרת_גודל": 262144,
    "max_size": 524288,
    "break_mask": "0x0000ffff",
    "קוד multihash": 31
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
    chunk_profile: profile_id (משתמע דרך רישום)
    יכולות: [...]
}
```

O agendamento de chunks multi-source e anunciado via a capacidade `range`. O CLI aceita isso com
`--capability=range[:streams]`, onde o sufixo numerico opcional codifica a concorrencia preferida
para fetch por range do provider (por exemplo, `--capability=range:64` anuncia um budget de 64 streams).
Quando omitido, consumidores recorrem ao hint geral `max_streams` publicado em outro ponto do advert.

Ao solicitar dados CAR, clientes devem enviar um header `Accept-Chunker` listando tuplas
`(namespace, name, semver)` em ordem de preferencia:

```

Gateways selecionam um perfil suportado mutuamente (ברירת מחדל `sorafs.sf1@1.0.0`)
e refletem a decisao via o header de resposta `Content-Chunker`. מניפסטים
אבוטום o perfil escolhido para que nos downstream possam validar o layout de chunks
sem depender da negociacao HTTP.

### רכב תמיכה

mantemos um caminho de exportacao CARv1+SHA-2:

* **Caminho primario** - CARv2, digest de payload BLAKE3 (`0x1f` multihash),
  `MultihashIndexSorted`, קובץ רישום נתח כמו אסימא.
  PODEM expor esta variante quando o cliente omite `Accept-Chunker` או בקשה
  `Accept-Digest: sha2-256`.

adiconais para transicao, mas nao devem substituir o digest canonico.

### Conformidade

* O perfil `sorafs.sf1@1.0.0` mapeia para os fixtures publicos em
  `fixtures/sorafs_chunker` e os corpora registrados em
  `fuzz/sorafs_chunker`. אימון מקצה לקצה e exercitada em Rust, Go e Node
  דרך os testes fornecidos.
* `chunker_registry::lookup_by_profile` אfirma que os parametros do descriptor
  correspondem a `ChunkProfile::DEFAULT` para evitar divergencia acidental.
* מניפסט מוצרים על ידי `iroha app sorafs toolkit pack` ו-`sorafs_manifest_stub` כולל מטאדוסים לרישום.