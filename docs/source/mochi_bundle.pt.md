---
lang: pt
direction: ltr
source: docs/source/mochi_bundle.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: f2dd292b7d15b449f3cec1b79343387a8c23beef3a163367bd5fa8ced8593aae
source_last_modified: "2026-01-03T18:08:00.656311+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Ferramentas de pacote MOCHI

O MOCHI vem com um fluxo de trabalho de embalagem leve para que os desenvolvedores possam produzir um
pacote de desktop portátil sem fiação de scripts CI personalizados. O `xtask`
subcomando lida com compilação, layout, hash e (opcionalmente) arquivamento
criação de uma só vez.

## Gerando um pacote

```bash
cargo xtask mochi-bundle
```

Por padrão, o comando cria binários de lançamento, monta o pacote em
`target/mochi-bundle/` e emite um arquivo `mochi-<os>-<arch>-release.tar.gz`
ao lado de um `manifest.json` determinístico. O manifesto lista todos os arquivos com
seu tamanho e hash SHA-256 para que os pipelines de CI possam executar novamente a verificação ou publicar
atestados. O auxiliar garante o shell de desktop `mochi` e o
O binário do espaço de trabalho `kagami` está presente, então a geração do genesis funciona fora do
caixa.

### Bandeiras

| Bandeira | Descrição |
|---------------------|------------------------------------------------------------------------------------------|
| `--out <dir>` | Substitua o diretório de saída (o padrão é `target/mochi-bundle`).         |
| `--profile <name>` | Construa com um perfil Cargo específico (por exemplo, `debug` para testes).              |
| `--no-archive` | Ignore o arquivo `.tar.gz`, deixando apenas a pasta preparada.               |
| `--kagami <path>` | Use um binário `kagami` explícito em vez de construir `iroha_kagami`.         |
| `--matrix <path>` | Anexe metadados de pacote a uma matriz JSON para rastreamento de proveniência de CI.         |
| `--smoke` | Execute `mochi --help` do pacote configurável como uma porta de execução básica.      |
| `--stage <dir>` | Copie o pacote finalizado (e arquive, quando presente) em uma pasta de teste. |

`--stage` destina-se a pipelines de CI onde cada agente de construção carrega seu
artefatos para um local compartilhado. O auxiliar recria o diretório do pacote configurável e
copia o arquivo gerado no diretório temporário para que os trabalhos de publicação possam
colete resultados específicos da plataforma sem scripts de shell.

O layout dentro do pacote é intencionalmente simples:

```
bin/mochi              # egui desktop executable
bin/kagami             # kagami helper for genesis generation
config/sample.toml     # starter supervisor configuration
docs/README.md         # bundle overview and verification guide
LICENSE                # repository licence
manifest.json          # generated file manifest with SHA-256 digests
```

### Substituições de tempo de execução

O executável `mochi` empacotado aceita substituições de linha de comando para a maioria
configurações comuns do supervisor. Use esses sinalizadores em vez de editar
`config/local.toml` ao experimentar:

```
./bin/mochi --data-root ./data --profile four-peer-bft \
    --torii-start 12000 --p2p-start 14000 \
    --irohad /path/to/irohad --kagami /path/to/kagami
```

Qualquer valor CLI tem precedência sobre entradas e ambiente `config/local.toml`
variáveis.

## Automação de instantâneo

`manifest.json` registra o timestamp de geração, alvo triplo, perfil de carga,
e o inventário completo do arquivo. Pipelines podem diferenciar o manifesto para detectar quando
novos artefatos aparecem, carregue o JSON junto com os ativos de lançamento ou audite o
hashes antes de promover um pacote para as operadoras.

O auxiliar é idempotente: executar novamente o comando atualiza o manifesto e
substitui o arquivo anterior, mantendo `target/mochi-bundle/` como o único
fonte de verdade para o pacote mais recente na máquina atual.