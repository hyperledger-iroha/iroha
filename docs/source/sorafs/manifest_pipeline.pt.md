<!-- Auto-generated stub for Portuguese (pt) translation. Replace this content with the full translation. -->

---
lang: pt
direction: ltr
source: docs/source/sorafs/manifest_pipeline.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 2572648c9c5aa1d4c346e66440fd14bff98afd55232ba1a7ba1c5fcd505559c6
source_last_modified: "2025-11-02T17:57:27.798590+00:00"
translation_last_reviewed: 2025-12-28
---

# Chunking de SoraFS → Pipeline de manifestos

Esta nota captura os passos mínimos necessários para transformar um payload de bytes em
um manifesto codificado em Norito adequado para pinning no registro do SoraFS.

1. **Faça chunking do payload de forma determinística**
   - Use `sorafs_car::CarBuildPlan::single_file` (internamente usa o chunker SF-1)
     para derivar offsets de chunks, comprimentos e digests BLAKE3.
   - O plano expõe o digest do payload e os metadados de chunks que o tooling downstream
     pode reutilizar para montagem de CAR e agendamento de Proof-of-Replication.
   - Alternativamente, o protótipo `sorafs_car::ChunkStore` ingere bytes e
     registra metadados determinísticos de chunks para construção posterior de CAR.
     O store agora deriva a árvore de amostragem PoR de 64 KiB / 4 KiB (marcada por domínio,
     alinhada a chunks) para que os agendadores possam solicitar provas Merkle sem reler
     o payload.
    Use `--por-proof=<chunk>:<segment>:<leaf>` para emitir um testemunho JSON de uma
    folha amostrada e `--por-json-out` para gravar o snapshot do digest raiz para
    verificação posterior. Combine `--por-proof` com `--por-proof-out=path` para persistir
    o testemunho e use `--por-proof-verify=path` para confirmar que uma prova existente
    corresponde ao `por_root_hex` calculado para o payload atual. Para múltiplas
    folhas, `--por-sample=<count>` (com `--por-sample-seed` e
    `--por-sample-out` opcionais) produz amostras determinísticas enquanto sinaliza
    `por_samples_truncated=true` sempre que a solicitação excede as folhas disponíveis.
   - Persista os offsets/comprimentos/digests de chunks se você pretende construir
     provas de bundle (manifestos CAR, agendas PoR).
   - Consulte [`sorafs/chunker_registry.md`](chunker_registry.md) para as
     entradas canônicas do registro e orientação de negociação.

2. **Empacote um manifesto**
   - Alimente os metadados de chunking, CID raiz, compromissos CAR, política de pin, claims de alias
     e assinaturas de governança em `sorafs_manifest::ManifestBuilder`.
   - Chame `ManifestV1::encode` para obter os bytes Norito e
     `ManifestV1::digest` para obter o digest canônico registrado no Pin
     Registry.

3. **Publicar**
   - Submeta o digest do manifesto via governança (assinatura do conselho, provas de alias)
     e pinne os bytes do manifesto no SoraFS usando o pipeline determinístico.
   - Garanta que o arquivo CAR (e o índice CAR opcional) referenciado pelo manifesto esteja
     armazenado no mesmo conjunto de pins do SoraFS.

### Quickstart da CLI

```bash
cargo run -p sorafs_manifest --bin sorafs-manifest-stub   ./docs.tar   --root-cid=0155aa   --car-cid=017112...   --alias-file=docs:sora:alias_proof.bin   --council-signature-file=0123...cafe:council.sig   --metadata=build:ci-123   --manifest-out=docs.manifest   --manifest-signatures-out=docs.manifest.signatures.json   --car-out=docs.car   --json-out=docs.report.json
```

O comando imprime digests de chunks e detalhes do manifesto; quando `--manifest-out`
e/ou `--car-out` são fornecidos, ele grava o payload Norito e um arquivo CARv2 compatível
com a especificação (pragma + header + blocos CARv1 + índice Multihash) em disco. Se você
passar um caminho de diretório, a ferramenta percorre-o recursivamente (ordem lexicográfica),
faz chunking de cada arquivo e emite uma árvore dag-cbor enraizada em diretório cujo CID
aparece como raiz tanto do manifesto quanto do CAR. O relatório JSON inclui o digest calculado
do payload CAR, o digest completo do arquivo, o tamanho, o CID bruto e a raiz (separando
o codec dag-cbor), junto com as entradas de alias/metadata do manifesto. Use `--root-cid`/`--dag-codec`
para *verificar* a raiz ou o codec calculado durante execuções de CI, `--car-digest` para
impor o hash do payload, `--car-cid` para impor um identificador CAR bruto pré-computado
(CIDv1, codec `raw`, multihash BLAKE3) e `--json-out` para persistir o JSON impresso junto
aos artefatos de manifesto/CAR para automação downstream.

Quando `--manifest-signatures-out` é fornecido (junto com pelo menos um flag
`--council-signature*`) a ferramenta também grava um envelope
`manifest_signatures.json` contendo o digest BLAKE3 do manifesto, o digest SHA3-256
agregado do plano de chunks (offsets, comprimentos e digests BLAKE3 de chunks) e as
assinaturas do conselho fornecidas. O envelope agora registra o perfil de chunker na
forma canônica `namespace.name@semver`. A automação downstream pode publicar o envelope nos logs de
governança ou distribuí-lo com os artefatos de manifesto e CAR. Quando você receber um
envelope de um signatário externo, adicione `--manifest-signatures-in=<path>` para que a
CLI confirme os digests e verifique cada assinatura Ed25519 contra o digest do manifesto
recém-calculado.

Quando vários perfis de chunker são registrados, você pode selecionar um explicitamente
com `--chunker-profile-id=<id>`. O flag mapeia para os identificadores numéricos em
[`chunker_registry`](chunker_registry.md) e garante que tanto a passagem de chunking quanto
o manifesto emitido referenciem a mesma tupla `(namespace, name, semver)`. Prefira a forma
de handle canônico na automação (`--chunker-profile=sorafs.sf1@1.0.0`), o que evita
codificar IDs numéricos. Execute `sorafs_manifest_chunk_store --list-profiles` para ver as
entradas atuais do registro (a saída espelha a listagem fornecida por
`sorafs_manifest_chunk_store`), ou use `--promote-profile=<handle>` para exportar o handle
canônico e os metadados de alias ao preparar uma atualização do registro.

Auditores podem solicitar a árvore completa de Proof-of-Retrievability via
`--por-json-out=path`, que serializa digests de chunk/segment/leaf para verificação de
amostragem. Testemunhas individuais podem ser exportadas com
`--por-proof=<chunk>:<segment>:<leaf>` (e validadas com `--por-proof-verify=path`), enquanto
`--por-sample=<count>` gera amostras determinísticas e sem duplicação para checks pontuais.

Qualquer flag que escreva JSON (`--json-out`, `--chunk-fetch-plan-out`, `--por-json-out`,
etc.) também aceita `-` como caminho, permitindo transmitir o payload diretamente
para stdout sem criar arquivos temporários.

Use `--chunk-fetch-plan-out=path` para persistir a especificação ordenada de fetch de chunks
(índice do chunk, offset do payload, comprimento, digest BLAKE3) que acompanha o plano do
manifesto. Clientes multi-origem podem alimentar o JSON resultante diretamente no
orquestrador de fetch do SoraFS sem reler o payload de origem. O relatório JSON impresso
pela CLI também inclui esse array em `chunk_fetch_specs`. Tanto a seção `chunking` quanto o
objeto `manifest` expõem `profile_aliases` ao lado do handle `profile` canônico.

Ao reexecutar o stub (por exemplo em CI ou em um pipeline de release) você pode
passar `--plan=chunk_fetch_specs.json` ou `--plan=-` para importar a especificação
gerada anteriormente. A CLI verifica se o índice, offset, comprimento e digest BLAKE3 de
cada chunk ainda correspondem ao plano CAR recém-derivado antes de continuar a ingestão,
o que protege contra planos obsoletos ou adulterados.

### Smoke-test de orquestração local

O crate `sorafs_car` agora fornece `sorafs-fetch`, uma CLI de desenvolvimento que consome o
array `chunk_fetch_specs` e simula a recuperação multi-provedor a partir de arquivos locais.
Aponte-o para o JSON emitido por `--chunk-fetch-plan-out`, forneça um ou mais caminhos de
payload de provedores (opcionalmente com `#N` para aumentar a concorrência) e ele verificará
os chunks, remontará o payload e imprimirá um relatório JSON resumindo contagens de
sucesso/falha por provedor e recibos por chunk:

```
cargo run -p sorafs_car --bin sorafs_fetch --   --plan=chunk_fetch_specs.json   --provider=alpha=./providers/alpha.bin   --provider=beta=./providers/beta.bin#4@3   --output=assembled.bin   --json-out=fetch_report.json   --provider-metrics-out=providers.json   --scoreboard-out=scoreboard.json
```

Use esse fluxo para validar o comportamento do orquestrador ou comparar payloads de
provedores antes de conectar transportes de rede reais ao nó SoraFS.

Quando precisar acessar um gateway Torii ao vivo em vez de arquivos locais, substitua os
flags `--provider=/path` pelas novas opções orientadas a HTTP:

```
sorafs-fetch   --plan=chunk_fetch_specs.json   --gateway-provider=name=gw-a,provider-id=<hex>,base-url=https://gw-a.example/,stream-token=<base64>   --gateway-manifest-id=<manifest_id_hex>   --gateway-chunker-handle=sorafs.sf1@1.0.0   --gateway-client-id=ci-orchestrator   --json-out=gateway_fetch_report.json
```

A CLI valida o stream token, impõe o alinhamento de chunker/perfil e registra os
metadados do gateway junto aos recibos usuais de provedores para que operadores possam
arquivar o relatório como evidência de rollout (veja o handbook de deployment para o
fluxo blue/green completo).

Se você passar `--provider-advert=name=/path/to/advert.to`, a CLI agora decodifica o envelope
Norito, verifica a assinatura Ed25519 e exige que o provedor anuncie a capacidade
`chunk_range_fetch`. Isso mantém a simulação de fetch multi-origem alinhada com a política
de admissão de governança e evita o uso acidental de provedores que não conseguem
atender a requisições de chunk por intervalo.

O sufixo `#N` aumenta o limite de concorrência do provedor, enquanto `@W` define o peso de
agendamento (padrão 1 quando omitido). Quando adverts ou descritores de gateway são
fornecidos, a CLI agora avalia o scoreboard do orquestrador antes de iniciar um fetch:
provedores elegíveis herdam pesos conscientes de telemetria e o snapshot JSON persiste em
`--scoreboard-out=<path>` quando fornecido. Provedores que falham verificações de
capacidade ou prazos de governança são descartados automaticamente com um aviso para que
as execuções permaneçam alinhadas com a política de admissão. Veja
`docs/examples/sorafs_ci_sample/{telemetry.sample.json,scoreboard.json}` para um par de
entrada/saída de exemplo.

Passe `--expect-payload-digest=<hex>` e/ou `--expect-payload-len=<bytes>` para afirmar que o
payload montado corresponde às expectativas do manifesto antes de escrever saídas — útil
para smoke-tests de CI que querem garantir que o orquestrador não descartou nem reordenou
chunks silenciosamente.

Se você já tiver o relatório JSON criado por `sorafs-manifest-stub`, passe-o diretamente via
`--manifest-report=docs.report.json`. A CLI de fetch reutilizará os campos embutidos
`chunk_fetch_specs`, `payload_digest_hex` e `payload_len`, então você não precisa gerenciar
arquivos de plano ou validação separados.

O relatório de fetch também expõe telemetria agregada para ajudar no monitoramento:
`chunk_retry_total`, `chunk_retry_rate`, `chunk_attempt_total`,
`chunk_attempt_average`, `provider_success_total`, `provider_failure_total`,
`provider_failure_rate` e `provider_disabled_total` capturam a saúde geral de uma sessão
de fetch e são adequados para dashboards Grafana/Loki ou asserções de CI. Use
`--provider-metrics-out` para escrever apenas o array `provider_reports` se o tooling
downstream precisar apenas de estatísticas no nível de provedor.

### Próximos passos

- Capture os metadados CAR junto aos digests de manifesto nos logs de governança para que
  observadores possam verificar o conteúdo do CAR sem baixar o payload novamente.
- Integre o fluxo de publicação de manifesto e CAR ao CI para que cada build de docs/artefatos
  produza automaticamente um manifesto, obtenha assinaturas e pinne os payloads resultantes.
