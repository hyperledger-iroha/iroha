---
lang: pt
direction: ltr
source: docs/source/mochi/packaging.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: c7ab0877a6f43402d6ec13a44c4a7c2b68e4a49e6103bb50d7469d9e71aaa953
source_last_modified: "2026-01-03T18:07:57.001158+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Guia de embalagem MOCHI

Este guia explica como construir o pacote supervisor de desktop MOCHI, inspecionar
os artefatos gerados e ajuste as substituições de tempo de execução fornecidas com o
pacote. Ele complementa o início rápido com foco em embalagens reproduzíveis
e uso de CI.

## Pré-requisitos

- Conjunto de ferramentas Rust (edição 2024 / Rust 1.82+) com dependências de espaço de trabalho
  já construído.
- `irohad`, `iroha_cli` e `kagami` compilados para o destino desejado. O
  bundler reutiliza binários de `target/<profile>/`.
- Espaço em disco suficiente para a saída do pacote sob `target/` ou um personalizado
  destino.

Crie as dependências uma vez antes de executar o bundler:

```bash
cargo build -p irohad -p iroha_cli -p iroha_kagami
```

## Construindo o pacote

Invoque o comando `xtask` dedicado na raiz do repositório:

```bash
cargo xtask mochi-bundle
```

Por padrão, isso produz um pacote de lançamento sob `target/mochi-bundle/` com um
nome de arquivo derivado do sistema operacional host e da arquitetura (por exemplo,
`mochi-macos-aarch64-release.tar.gz`). Use os seguintes sinalizadores para personalizar
a construção:

- `--profile <name>` – escolha um perfil Cargo (`release`, `debug` ou um
  perfil personalizado).
- `--no-archive` – mantém o diretório expandido sem criar um `.tar.gz`
  arquivo (útil para testes locais).
- `--out <path>` – grava pacotes em um diretório personalizado em vez de
  `target/mochi-bundle/`.
- `--kagami <path>` – fornece um executável `kagami` pré-construído para incluir no
  arquivo. Quando omitido, o empacotador reutiliza (ou constrói) o binário a partir do
  perfil selecionado.
- `--matrix <path>` – anexa metadados do pacote a um arquivo de matriz JSON (criado se
  ausente) para que os pipelines de CI possam registrar cada artefato de host/perfil produzido em um
  correr. As entradas incluem o diretório do pacote, caminho do manifesto e SHA-256, opcional
  localização do arquivo e o resultado do teste de fumaça mais recente.
- `--smoke` – execute o pacote `mochi --help` como uma comporta de fumaça leve
  após empacotamento; falhas revelam dependências ausentes antes de publicar um
  artefato.
- `--stage <path>` – copie o pacote finalizado (e arquive quando produzido) em
  um diretório temporário para que compilações multiplataforma possam depositar artefatos em um
  localização sem scripts extras.

O comando copia `mochi-ui-egui`, `kagami`, `LICENSE`, a amostra
configuração e `mochi/BUNDLE_README.md` no pacote. Um determinístico
`manifest.json` é gerado junto com os binários para que os trabalhos de CI possam rastrear arquivos
hashes e tamanhos.

## Layout e verificação do pacote

Um pacote expandido segue o layout documentado em `BUNDLE_README.md`:

```
bin/mochi
bin/kagami
config/sample.toml
docs/README.md
manifest.json
LICENSE
```

O arquivo `manifest.json` lista cada artefato com seu hash SHA-256. Verifique
o pacote depois de copiá-lo para outro sistema:

```bash
jq -r '.files[] | "\(.sha256)  \(.path)"' manifest.json | sha256sum --check
```

Os pipelines de CI podem armazenar em cache o diretório expandido, assinar o arquivo ou publicar
o manifesto junto com as notas de lançamento. O manifesto inclui o gerador
perfil, triplo alvo e carimbo de data/hora de criação para auxiliar no rastreamento de procedência.

## Substituições de tempo de execução

MOCHI descobre binários auxiliares e locais de tempo de execução por meio de sinalizadores CLI ou
variáveis de ambiente:- `--data-root` / `MOCHI_DATA_ROOT` – substitui o espaço de trabalho usado para peer
  configurações, armazenamento e logs.
- `--profile` – alternar entre predefinições de topologia (`single-peer`,
  `four-peer-bft`).
- `--torii-start`, `--p2p-start` – altera as portas base usadas ao alocar
  serviços.
- `--irohad` / `MOCHI_IROHAD` – aponta para um binário `irohad` específico.
- `--kagami` / `MOCHI_KAGAMI` – substitui o `kagami` incluído.
- `--iroha-cli` / `MOCHI_IROHA_CLI` – substitui o auxiliar CLI opcional.
- `--restart-mode <never|on-failure>` – desativa reinicializações automáticas ou força o
  política de recuo exponencial.
- `--restart-max <attempts>` – substitui o número de tentativas de reinicialização quando
  executando no modo `on-failure`.
- `--restart-backoff-ms <millis>` – define o backoff base para reinicializações automáticas.
- `MOCHI_CONFIG` – fornece um caminho `config/local.toml` personalizado.

A ajuda CLI (`mochi --help`) imprime a lista completa de sinalizadores. Substituições de ambiente
entram em vigor na inicialização e podem ser combinados com a caixa de diálogo Configurações dentro do
IU.

## dicas de uso de CI

- Execute `cargo xtask mochi-bundle --no-archive` para gerar um diretório que possa
  ser compactado com ferramentas específicas da plataforma (ZIP para Windows, tarballs para
  Unix).
- Capture metadados do pacote com `cargo xtask mochi-bundle --matrix dist/matrix.json`
  portanto, os trabalhos de lançamento podem publicar um único índice JSON listando cada host/perfil
  artefato produzido no pipeline.
- Use `cargo xtask mochi-bundle --stage /mnt/staging/mochi` (ou similar) em cada
  agente de compilação para fazer upload do pacote e arquivar em um diretório compartilhado que o
  trabalho de publicação pode consumir.
- Publique o arquivo e `manifest.json` para que os operadores possam verificar o pacote
  integridade.
- Armazene o diretório gerado como um artefato de construção para semear testes de fumaça que
  exercitar o supervisor com binários empacotados deterministicamente.
- Grave hashes de pacote nas notas de lançamento ou no log `status.md` para futuro
  verificações de proveniência.