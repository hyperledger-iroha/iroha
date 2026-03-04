---
lang: pt
direction: ltr
source: docs/source/README.md
status: complete
translator: manual
source_hash: 4a6e55a3232ff38c5c2f45b0a8a3d97471a14603bea75dc2034d7c9c4fb3f862
source_last_modified: "2025-11-10T19:43:50.185052+00:00"
translation_last_reviewed: 2025-11-14
---

# Índice de documentação da Iroha VM e Kotodama

Este índice reúne os principais documentos de projeto e referência para a IVM,
Kotodama e o pipeline com foco em IVM. Para a versão em japonês, consulte
[`README.ja.md`](./README.ja.md).

- Arquitetura da IVM e mapeamento de linguagem: `../../ivm.md`
- ABI de syscalls da IVM: `ivm_syscalls.md`
- Constantes de syscalls geradas: `ivm_syscalls_generated.md` (execute `make docs-syscalls` para atualizar)
- Cabeçalho de bytecode da IVM: `ivm_header.md`
- Gramática e semântica de Kotodama: `kotodama_grammar.md`
- Exemplos de Kotodama e mapeamento de syscalls: `kotodama_examples.md`
- Pipeline de transações (IVM‑first): `../../new_pipeline.md`
- API de contratos do Torii (manifests): `torii_contracts_api.md`
- Envelope de consulta JSON (CLI / ferramentas): `query_json.md`
- Referência do módulo de streaming Norito: `norito_streaming.md`
- Amostras de ABI em tempo de execução: `samples/runtime_abi_active.md`, `samples/runtime_abi_hash.md`, `samples/find_active_abi_versions.md`
- API de aplicações ZK (anexos, prover, apuração de votos): `zk_app_api.md`
- Runbook de anexos/prover ZK no Torii: `zk/prover_runbook.md`
- Guia operacional da API ZK App do Torii (anexos/prover; documentação do crate): `../../crates/iroha_torii/docs/zk_app_api.md`
- Ciclo de vida de VK/proofs (registro, verificação, telemetria): `zk/lifecycle.md`
- Ajuda operacional do Torii (endpoints de visibilidade): `references/operator_aids.md`
- Quickstart da lane padrão do Nexus: `quickstart/default_lane.md`
- Quickstart e arquitetura do supervisor MOCHI: `mochi/index.md`
- Guias do SDK JavaScript (quickstart, configuração, publicação): `sdk/js/index.md`
- Painéis de métricas/paridade do Swift SDK: `references/ios_metrics.md`
- Governança: `../../gov.md`
- Prompts de coordenação para esclarecimentos: `coordination_llm_prompts.md`
- Roadmap: `../../roadmap.md`
- Uso da imagem de build Docker: `docker_build.md`

Dicas de uso

- Compile e execute exemplos em `examples/` usando as ferramentas externas
  (`koto_compile`, `ivm_run`):
  - `make examples-run` (e `make examples-inspect` se `ivm_tool` estiver
    disponível).
- Testes de integração opcionais (ignorados por padrão) para exemplos e
  verificações de cabeçalho ficam em `integration_tests/tests/`.

Configuração do pipeline

- Todo o comportamento de runtime é configurado por arquivos `iroha_config`. Não
  usamos variáveis de ambiente para operadores.
- Valores padrão razoáveis são fornecidos; a maioria das implantações não
  precisará de ajustes.
- Chaves relevantes em `[pipeline]`:
  - `dynamic_prepass`: ativa o prepass somente‑leitura na IVM para derivar
    conjuntos de acesso (padrão: true).
  - `access_set_cache_enabled`: armazena em cache conjuntos de acesso
    derivados por `(code_hash, entrypoint)`; desative para depurar hints
    (padrão: true).
  - `parallel_overlay`: constrói overlays em paralelo; o commit continua
    determinístico (padrão: true).
  - `gpu_key_bucket`: bucketização opcional de chaves para o prepass do
    agendador usando radix estável em `(key, tx_idx, rw_flag)`; o caminho
    determinístico em CPU permanece sempre ativo (padrão: false).
  - `cache_size`: capacidade do cache global de pré‑decodificação da IVM
    (streams já decodificados). Padrão: 128. Aumentar esse valor pode reduzir o
    tempo de decodificação para execuções repetidas.

Verificações de sincronização de docs

- Constantes de syscalls (`docs/source/ivm_syscalls_generated.md`)
  - Regenerar: `make docs-syscalls`
  - Somente verificar: `bash scripts/check_syscalls_doc.sh`
- Tabela de ABI de syscalls (`crates/ivm/docs/syscalls.md`)
  - Somente verificar: `cargo run -p ivm --bin gen_syscalls_doc -- --check --no-code`
  - Atualizar a seção gerada (e a tabela nos docs de código):
    `cargo run -p ivm --bin gen_syscalls_doc -- --write`
- Tabelas de pointer‑ABI (`crates/ivm/docs/pointer_abi.md` e `ivm.md`)
  - Somente verificar: `cargo run -p ivm --bin gen_pointer_types_doc -- --check`
  - Atualizar seções: `cargo run -p ivm --bin gen_pointer_types_doc -- --write`
- Política de cabeçalho da IVM e hashes de ABI (`docs/source/ivm_header.md`)
  - Somente verificar: `cargo run -p ivm --bin gen_header_doc -- --check` e
    `cargo run -p ivm --bin gen_abi_hash_doc -- --check`
  - Atualizar seções: `cargo run -p ivm --bin gen_header_doc -- --write` e
    `cargo run -p ivm --bin gen_abi_hash_doc -- --write`

CI

- O workflow do GitHub Actions `.github/workflows/check-docs.yml` executa essas
  verificações em cada push/PR e falhará se os documentos gerados ficarem fora
  de sincronia com a implementação.
- [Guia de governança](governance_playbook.md)
