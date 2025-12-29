---
lang: pt
direction: ltr
source: docs/profile_build.md
status: complete
translator: manual
source_hash: 9698d31da47926ae882dc1c93152ecd3865767be6262f14b71253dbd8b2a0fa9
source_last_modified: "2025-11-02T04:40:28.811778+00:00"
translation_last_reviewed: 2025-11-14
---

<!-- Tradução para português de docs/profile_build.md (Profiling iroha_data_model Build) -->

# Fazendo profile do build de `iroha_data_model`

Para localizar etapas lentas na compilação de `iroha_data_model`, execute o script helper:

```sh
./scripts/profile_build.sh
```

Esse script executa `cargo build -p iroha_data_model --timings` e grava os relatórios de
tempo em `target/cargo-timings/`.
Abra `cargo-timing.html` em um navegador e ordene as tarefas por duração para ver quais
crates ou etapas de build consomem mais tempo.

Use esses timings para concentrar os esforços de otimização nas tarefas mais lentas.

