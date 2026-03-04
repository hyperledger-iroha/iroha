---
lang: pt
direction: ltr
source: docs/source/gpuzstd_metal_pipeline.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 019b3aa25ae224c1595467ac809f2c53290813e91a78b78b94ca71c3dd950264
source_last_modified: "2026-01-31T19:25:45.072449+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Pipeline GPU Zstd (Metal)

Este documento descreve o pipeline determinístico de GPU usado pelo auxiliar Metal
para compactação zstd. É um guia de design e implementação para o
Auxiliar `gpuzstd_metal` que emite quadros zstd padrão e bytes determinísticos
para um determinado fluxo de sequência. As saídas devem ida e volta com decodificadores de CPU; byte por-
a paridade de bytes com o compressor da CPU não é necessária porque a geração de sequência
difere.

## Metas

- Emite quadros zstd padrão que decodificam de forma idêntica ao zstd da CPU; paridade de bytes
  com o compressor da CPU não é necessário.
- Saídas determinísticas em hardware, drivers e agendamento de threads.
- Verificações de limites explícitos e tempos de vida de buffer previsíveis.

## Nota de implementação atual

- A descoberta de correspondência e a geração de sequência são executadas na GPU.
- Montagem de quadro e codificação de entropia (Huffman/FSE) atualmente executadas no host
  usando o codificador embutido; Os kernels GPU Huffman/FSE são testados em termos de paridade, mas não
  ainda conectado ao caminho completo do quadro.
- Decode usa o decodificador de quadros na caixa com um substituto zstd da CPU para quadros não suportados;
  a decodificação completa do bloco GPU permanece em andamento.

## Pipeline de codificação (alto nível)

1. Preparação de entrada
   - Copie a entrada em um buffer de dispositivo.
   - Partição em pedaços de tamanho fixo (para geração de sequência) e blocos (para
     montagem da estrutura zstd).
2. Localização de correspondência e emissão de sequência
   - Os kernels da GPU examinam cada pedaço e emitem sequências (comprimento literal, correspondência
     comprimento, deslocamento).
   - A ordenação de sequências é estável e determinística.
3. Preparação literal
   - Colete literais referenciados por sequências.
   - Construa histogramas literais e selecione o modo de bloco literal (bruto, RLE ou
     Huffman) deterministicamente.
4. Tabelas Huffman (literais)
   - Gere comprimentos de código a partir do histograma.
   - Construa tabelas canônicas com desempate determinístico que corresponda à CPU
     saída zstd.
5. Tabelas FSE (LL/ML/OF)
   - Normalizar contagens de frequência.
   - Construir tabelas de decodificação/codificação FSE de forma determinística.
6. Escritor Bitstream
   - Pacote de bits little-endian (LSB primeiro).
   - Liberar limites de bytes; bloco apenas com zeros.
   - Mascarar valores para larguras de bits declaradas e impor verificações de capacidade.
7. Montagem de bloco e estrutura
   - Emite cabeçalhos de bloco (tipo, tamanho, sinalizador do último bloco).
   - Serialize literais e sequências em blocos compactados.
   - Emite cabeçalhos de quadro zstd padrão e somas de verificação opcionais.

## Pipeline de decodificação (alto nível)

1. Análise de quadro
   - Valide bytes mágicos, configurações de janela e campos de cabeçalho de quadro.
2. Leitor de fluxo de bits
   - Leia sequências de primeiro bit LSB com verificações de limites estritos.
3. Decodificação literal
   - Decodifique blocos literais (brutos, RLE ou Huffman) no buffer literal.
4. Decodificação de sequência
   - Decodifique valores LL/ML/OF usando tabelas FSE.
   - Reconstrua partidas usando a janela deslizante.
5. Saída e soma de verificação
   - Grave bytes reconstruídos no buffer de saída.
   - Verifique somas de verificação opcionais quando habilitadas.

## Vida útil e propriedade do buffer- Buffer de entrada: host -> dispositivo, somente leitura.
- Buffer de sequência: dispositivo, produzido por busca de correspondência e consumido pela entropia
  codificação; sem reutilização entre blocos.
- Buffer literal: dispositivo, produzido para cada bloco e liberado após bloco
  emissão.
- Buffer de saída: dispositivo, armazena os bytes finais do quadro até que o host os copie
  fora.
- Buffers scratch: reutilizados entre kernels, mas sempre sobrescritos de forma determinística.

## Responsabilidades do kernel

- Kernels de localização de correspondência: encontre correspondências e emita sequências (LL/ML/OF + literais).
- Kernels de construção Huffman: deriva comprimentos de código e tabelas canônicas.
- Kernels de construção FSE: construa tabelas LL/ML/OF e máquinas de estado.
- Bloquear kernels de codificação: serializar literais e sequências no fluxo de bits.
- Bloquear kernels de decodificação: analisar bitstream e reconstruir literais/sequências.

## Determinismo e restrições de paridade

- As compilações de tabelas canônicas devem usar a mesma ordem e desempate da CPU
  zstd.
- Sem atômicos ou reduções que dependam do agendamento de threads para qualquer byte de saída.
- O empacotamento Bitstream é little-endian, LSB-first; blocos de alinhamento de bytes com zeros.
- Todas as verificações de limites são explícitas; entradas inválidas falham deterministicamente.

## Validação

- Vetores dourados de CPU para o gravador/leitor de fluxo de bits.
- Testes de paridade de Corpus comparando saídas de GPU e CPU.
- Cobertura Fuzz para quadros malformados e condições de contorno.

## Comparativo de mercado

Execute `cargo test -p gpuzstd_metal gpu_vs_cpu_benchmark -- --ignored --nocapture` para
compare a latência de codificação de CPU versus GPU em tamanhos de carga útil. O teste pula nos hosts
sem dispositivo com capacidade de metal; capture a saída junto com detalhes de hardware
ao ajustar os limites de descarregamento da GPU. Norito impõe o mesmo corte em
`gpu_zstd::encode_all`, para que os chamadores diretos correspondam à porta heurística.