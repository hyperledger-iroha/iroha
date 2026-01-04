---
lang: pt
direction: ltr
source: docs/README.md
status: complete
translator: manual
source_hash: 6d050b266fbcc3f0041ec554f89e397e56dc37e5ec3fb093af59e46eb52109e6
source_last_modified: "2025-11-02T04:40:28.809865+00:00"
translation_last_reviewed: 2025-11-14
---

# Documentação do Iroha

Para uma visão geral em japonês, consulte [`README.ja.md`](./README.ja.md).

Este workspace publica duas linhas de lançamento a partir do mesmo código‑fonte:
**Iroha 2** (implantações auto‑hospedadas) e **Iroha 3 / SORA Nexus** (o único
livro‑razão Nexus global). Ambas reutilizam a mesma Iroha Virtual Machine (IVM)
e a mesma toolchain Kotodama, de modo que contratos e bytecode permanecem
portáveis entre ambientes. A documentação se aplica às duas linhas, salvo
indicação em contrário.

Na [documentação principal do Iroha](https://docs.iroha.tech/) você encontra:

- [Guia de início rápido](https://docs.iroha.tech/get-started/)
- [Tutoriais de SDK](https://docs.iroha.tech/guide/tutorials/) para Rust, Python, JavaScript e Java/Kotlin
- [Referência da API](https://docs.iroha.tech/reference/torii-endpoints.html)

Whitepapers e especificações por versão:

- [Whitepaper do Iroha 2](./source/iroha_2_whitepaper.md): especificação para redes auto‑hospedadas.
- [Whitepaper do Iroha 3 (SORA Nexus)](./source/iroha_3_whitepaper.md): arquitetura de múltiplas faixas (lanes) e espaços de dados do Nexus.
- [Modelo de dados e especificação ISI (derivados da implementação)](./source/data_model_and_isi_spec.md): referência de comportamento reconstruída a partir do código.
- [Envelopes ZK (Norito)](./source/zk_envelopes.md): envelopes Norito nativos baseados em IPA/STARK e expectativas do verificador.

## Localização

Stubs de documentação em japonês (`*.ja.*`), hebraico (`*.he.*`), espanhol
(`*.es.*`), português (`*.pt.*`), francês (`*.fr.*`), russo (`*.ru.*`), árabe
(`*.ar.*`) e urdu (`*.ur.*`) ficam ao lado de cada arquivo‑fonte em inglês.
Consulte [`docs/i18n/README.md`](./i18n/README.md) para saber como gerar e
manter traduções, além de orientações para adicionar novos idiomas.

## Ferramentas

Neste repositório você encontra documentação para as ferramentas do Iroha 2:

- [Kagami](../crates/iroha_kagami/README.md)
- Macros [`iroha_derive`](../crates/iroha_derive/) para structs de configuração (veja o recurso `config_base`)
- [Etapas de compilação com perfil](./profile_build.md) para identificar partes lentas na compilação de `iroha_data_model`

## Referências do SDK Swift / iOS

- [Visão geral do SDK Swift](./source/sdk/swift/index.md): auxiliares de pipeline, toggles de aceleração e APIs Connect/WebSocket.
- [Guia rápido do Connect](./connect_swift_ios.md): passo a passo centrado no SDK, com a referência alternativa de CryptoKit.
- [Guia de integração com o Xcode](./connect_swift_integration.md): integração de NoritoBridgeKit/Connect em um aplicativo, com ChaChaPoly e auxiliares de frames.
- [Guia para contribuidores da demo em SwiftUI](./norito_demo_contributor.md): como executar a demo iOS contra um nó Torii local, com observações sobre aceleração.
- Execute `make swift-ci` antes de publicar artefatos Swift ou alterações no Connect; isso verifica a paridade de fixtures, feeds de dashboards e metadados `ci/xcframework-smoke:<lane>:device_tag` no Buildkite.

## Norito (codec de serialização)

Norito é o codec de serialização do workspace. Não usamos `parity-scale-codec`
(SCALE). Quando a documentação ou os benchmarks comparam com SCALE, é apenas
para contexto; todos os caminhos de produção usam Norito. As APIs
`norito::codec::{Encode, Decode}` fornecem uma payload Norito sem cabeçalho
("bare") para hashing e eficiência de rede — continua sendo Norito, não SCALE.

Estado atual:

- Codificação/decodificação determinística com um cabeçalho fixo (magic,
  versão, schema de 16 bytes, compressão, tamanho, CRC64, flags).
- Checksum CRC64‑ECMA com aceleração selecionada em tempo de execução:
  - PCLMULQDQ em x86_64 (multiplicação sem transporte) + redução Barrett sobre blocos de 32 bytes.
  - PMULL em aarch64 com o mesmo esquema de folding.
  - Implementações slicing‑by‑8 e bit‑a‑bit como caminho portátil.
- Dicas de tamanho codificado implementadas por derives e tipos centrais para reduzir alocações.
- Buffers de streaming maiores (64 KiB) e atualização incremental do CRC durante a decodificação.
- Compressão zstd opcional; a aceleração por GPU é protegida por feature flag e permanece determinística.
- Seleção de caminho adaptativa: `norito::to_bytes_auto(&T)` escolhe entre sem
  compressão, zstd via CPU ou zstd offload para GPU (quando compilado e
  disponível) com base no tamanho da payload e nas capacidades de hardware em
  cache. A seleção afeta apenas o desempenho e o byte `compression` do
  cabeçalho; a semântica da payload não muda.

Veja `crates/norito/README.md` para testes de paridade, benchmarks e exemplos de uso.

Nota: Alguns documentos de subsistemas (por exemplo, aceleração da IVM e
circuitos ZK) ainda estão em evolução. Quando a funcionalidade não está
completa, os arquivos indicam explicitamente o trabalho restante e a direção do
projeto.

Observações sobre a codificação do endpoint de status

- O corpo de `/status` no Torii usa Norito por padrão, com payload sem cabeçalho
  ("bare") para compactação. Clientes devem tentar decodificar via Norito
  primeiro.
- Servidores podem retornar JSON quando solicitado; clientes fazem fallback para
  JSON se o `content-type` for `application/json`.
- O formato de rede é Norito, não SCALE. As APIs `norito::codec::{Encode,Decode}` são usadas para a variante sem cabeçalho.
