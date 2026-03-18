---
lang: pt
direction: ltr
source: docs/references/configuration.md
status: complete
translator: manual
source_hash: cff283a14bf65f185f81539f8fbcd78ddcc6447c5e9045e1b46493051febaf6a
source_last_modified: "2025-11-02T04:40:39.795595+00:00"
translation_last_reviewed: 2025-11-14
---

<!-- Tradução em português de docs/references/configuration.md (Acceleration) -->

# Aceleração

A seção `[accel]` controla a aceleração opcional por hardware para a IVM e
helpers associados. Todos os caminhos acelerados possuem fallbacks
determinísticos em CPU; se um backend falhar em um golden self‑test em
runtime, ele é desativado automaticamente e a execução continua na CPU.

- `enable_cuda` (padrão: `true`) – usar CUDA quando compilado e disponível.
- `enable_metal` (padrão: `true`) – usar Metal no macOS quando disponível.
- `max_gpus` (padrão: `0`) – número máximo de GPUs a inicializar; `0` significa
  auto/sem limite explícito.
- `merkle_min_leaves_gpu` (padrão: `8192`) – número mínimo de folhas a partir
  do qual o hashing de folhas Merkle é offloadado para a GPU. Reduza apenas em
  GPUs excepcionalmente rápidas.
- Avançado (opcional; normalmente herda valores sensatos):
  - `merkle_min_leaves_metal` (padrão: herda `merkle_min_leaves_gpu`).
  - `merkle_min_leaves_cuda` (padrão: herda `merkle_min_leaves_gpu`).
  - `prefer_cpu_sha2_max_leaves_aarch64` (padrão: `32768`) – preferir SHA‑2 em
    CPU até esse número de folhas em ARMv8 com suporte SHA2.
  - `prefer_cpu_sha2_max_leaves_x86` (padrão: `32768`) – preferir SHA‑NI em CPU
    até esse número de folhas em x86/x86_64.

Notas
- Determinismo em primeiro lugar: a aceleração nunca altera os resultados
  observáveis; os backends executam testes golden na inicialização e fazem
  fallback para caminhos escalares/SIMD quando são detectados desvios.
- Configure via `iroha_config`; evite variáveis de ambiente em produção.

