---
lang: pt
direction: ltr
source: docs/source/fastpq_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 8324267c90cfbaf718760c4883427e85d81edcfa180dd9f64fd31a5e219749f4
source_last_modified: "2026-01-18T05:31:56.951617+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Análise do trabalho do provador FASTPQ

Este documento captura o plano em etapas para entregar um provador FASTPQ-ISI pronto para produção e conectá-lo ao pipeline de agendamento do espaço de dados. Cada definição abaixo é normativa, a menos que seja marcada como TODO. A solidez estimada usa limites DEEP-FRI no estilo Cairo; testes automatizados de amostragem de rejeição em CI falham se o limite medido cair abaixo de 128 bits.

## Estágio 0 - Hash Placeholder (desembarcado)
- Codificação determinística Norito com compromisso BLAKE2b.
- Back-end de espaço reservado retornando `BackendUnavailable`.
- Tabela de parâmetros canônicos fornecida por `fastpq_isi`.

## Estágio 1 — Protótipo do Trace Builder

> **Status (09/11/2025):** `fastpq_prover` agora expõe empacotamento canônico
> ajudantes (`pack_bytes`, `PackedBytes`) e o Poseidon2 determinístico
> compromisso de pedido em vez de Cachinhos Dourados. Constantes são fixadas em
> `ark-poseidon2` commit `3f2b7fe`, encerrando o acompanhamento sobre a troca do BLAKE2 provisório
> o espaço reservado está fechado. Luminárias douradas (`tests/fixtures/packing_roundtrip.json`,
> `tests/fixtures/ordering_hash.json`) agora ancoram o conjunto de regressão.

### Objetivos
- Implemente o construtor de rastreamento FASTPQ para o AIR de atualização KV. Cada linha deve codificar:
  - `key_limbs[i]`: membros de base 256 (7 bytes, little-endian) do caminho da chave canônica.
  - `value_old_limbs[i]`, `value_new_limbs[i]`: mesma embalagem para valores pré/pós.
  - Colunas seletoras: `s_active`, `s_transfer`, `s_mint`, `s_burn`, `s_role_grant`, `s_role_revoke`, `s_meta_set`, `s_perm`.
  - Colunas auxiliares: `delta = value_new - value_old`, `running_asset_delta`, `metadata_hash`, `supply_counter`.
  - Colunas de ativos: `asset_id_limbs[i]` usando membros de 7 bytes.
  - Colunas SMT por nível `ℓ`: `path_bit_ℓ`, `sibling_ℓ`, `node_in_ℓ`, `node_out_ℓ`, mais `neighbour_leaf` para não-membros.
  - Colunas de metadados: `dsid`, `slot`.
- **Ordenação determinística.** Classifique as linhas lexicograficamente por `(key_bytes, op_rank, original_index)` usando uma classificação estável. Mapeamento `op_rank`: `transfer=0`, `mint=1`, `burn=2`, `role_grant=3`, `role_revoke=4`, `meta_set=5`. `original_index` é o índice baseado em 0 antes da classificação. Persista o hash de ordenação Poseidon2 resultante (tag de domínio `fastpq:v1:ordering`). Codifique a pré-imagem de hash como `[domain_len, domain_limbs…, payload_len, payload_limbs…]`, onde os comprimentos são elementos de campo u64, de forma que os zero bytes finais permaneçam distinguíveis.
- Testemunha de pesquisa: produza `perm_hash = Poseidon2(role_id || permission_id || epoch_u64_le)` quando a coluna armazenada `s_perm` (OR lógico de `s_role_grant` e `s_role_revoke`) for 1. IDs de função/permissão são strings LE de largura fixa de 32 bytes; época é LE de 8 bytes.
- Aplicar invariantes antes e dentro do AIR: seletores mutuamente exclusivos, conservação por ativo, constantes dsid/slot.
- `N_trace = 2^k` (`pow2_ceiling` de contagem de linhas); `N_eval = N_trace * 2^b` onde `b` é o expoente de expansão.
- Fornecer luminárias e testes de propriedades:
  - Embalagem de ida e volta (`fastpq_prover/tests/packing.rs`, `tests/fixtures/packing_roundtrip.json`).
  - Pedido de hash de estabilidade (`tests/fixtures/ordering_hash.json`).
  - Luminárias em lote (`trace_transfer.json`, `trace_mint.json`, `trace_duplicate_update.json`).### Esquema de colunas do AIR
| Grupo de Colunas | Nomes | Descrição |
| ----------------- | -------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------- |
| Atividade | `s_active` | 1 para linhas reais, 0 para preenchimento.                                                                                       |
| Principal | `key_limbs[i]`, `value_old_limbs[i]`, `value_new_limbs[i]` | Elementos Goldilocks compactados (little-endian, membros de 7 bytes).                                                             |
| Ativo | `asset_id_limbs[i]` | Identificador de ativo canônico compactado (little-endian, membros de 7 bytes).                                                      |
| Seletores | `s_transfer`, `s_mint`, `s_burn`, `s_role_grant`, `s_role_revoke`, `s_meta_set`, `s_perm` | 0/1. Restrição: seletores Σ (incluindo `s_perm`) = `s_active`; `s_perm` espelha linhas de concessão/revogação de função.              |
| Auxiliar | `delta`, `running_asset_delta`, `metadata_hash`, `supply_counter` | Estado usado para restrições, conservação e trilhas de auditoria.                                                           |
| SMT | `path_bit_ℓ`, `sibling_ℓ`, `node_in_ℓ`, `node_out_ℓ`, `neighbour_leaf` | Entradas/saídas Poseidon2 por nível mais testemunha vizinha para não-membros.                                         |
| Pesquisa | `perm_hash` | Hash Poseidon2 para pesquisa de permissão (restringido apenas quando `s_perm = 1`).                                            |
| Metadados | `dsid`, `slot` | Constante entre linhas.                                                                                                 |### Matemática e restrições
- **Embalagem de campo:** os bytes são divididos em membros de 7 bytes (little-endian). Cada membro `limb_j = Σ_{k=0}^{6} byte_{7j+k} * 256^k`; rejeitar membros ≥ módulo Cachinhos Dourados.
- **Equilíbrio/conservação:** deixe `δ = value_new - value_old`. Agrupe linhas por `asset_id`. Defina `r_asset_start = 1` na primeira linha de cada grupo de ativos (0 caso contrário) e restrinja
  ```
  running_asset_delta = (1 - r_asset_start) * running_asset_delta_prev + δ.
  ```
  Na última linha de cada grupo de ativos, afirme
  ```
  running_asset_delta = Σ (s_mint * δ) - Σ (s_burn * δ).
  ```
  As transferências satisfazem a restrição automaticamente porque seus valores δ somam zero em todo o grupo. Exemplo: se `value_old = 100` e `value_new = 120` em uma linha da casa da moeda, δ = 20, então a soma da casa da moeda contribui com +20 e a verificação final é resolvida para zero quando não ocorrem queimaduras.
- **Preenchimento:** introduza `s_active`. Multiplique todas as restrições de linha por `s_active` e aplique um prefixo contíguo: `s_active[i] ≥ s_active[i+1]`. As linhas de preenchimento (`s_active=0`) devem manter valores constantes, mas são irrestritas.
- **Hash de pedido:** Hash Poseidon2 (domínio `fastpq:v1:ordering`) sobre codificações de linha; armazenado em Public IO para auditabilidade.

## Estágio 2 – Núcleo do Provador STARK

### Objetivos
- Construir compromissos Poseidon2 Merkle sobre vetores de avaliação de rastreamento e pesquisa. Parâmetros: taxa=2, capacidade=1, rodadas completas=8, rodadas parciais=57, constantes fixadas em `ark-poseidon2` commit `3f2b7fe` (v0.3.0).
- Extensão de baixo grau: avalie cada coluna no domínio `D = { g^i | i = 0 .. N_eval-1 }`, onde `N_eval = 2^{k+b}` divide a capacidade 2-ádica de Cachinhos Dourados. Seja `g = ω^{(p-1)/N_eval}` com `ω` a raiz primitiva fixa de Cachinhos Dourados e `p` seu módulo; use o subgrupo base (sem coset). Registre `g` na transcrição (tag `fastpq:v1:lde`).
- Polinômios de composição: para cada restrição `C_j`, forme `F_j(X) = C_j(X) / Z_N(X)` com margens de grau listadas abaixo.
- Argumento de pesquisa (permissões): amostra `γ` da transcrição. Rastrear produto `Z_0 = 1`, `Z_i = Z_{i-1} * (perm_hash_i - γ)^{s_perm_i}`. Produto de tabela `T = ∏_j (table_perm_j - γ)`. Restrição de limite: `Z_final / T = 1`.
- DEEP-FRI com aridade `r ∈ {8, 16}`: para cada camada, absorva a raiz com tag `fastpq:v1:fri_layer_ℓ`, amostra `β_ℓ` (tag `fastpq:v1:beta_ℓ`), e dobre via `H_{ℓ+1}(i) = Σ_{k=0}^{r-1} H_ℓ(r*i + k) * β_ℓ^k`.
- Objeto de prova (codificado em Norito):
  ```
  Proof {
      protocol_version: u16,
      params_version: u16,
      parameter_set: String,
      public_io: PublicIO,
      trace_root: [u8; 32],
      lookup_root: [u8; 32],
      fri_layers: Vec<[u8; 32]>,
      alphas: Vec<Field>,
      betas: Vec<Field>,
      queries: Vec<QueryOpening>,
  }
  ```
- Verificador de espelhos provador; execute o conjunto de regressão em rastreamentos de 1k/5k/20k linhas com transcrições douradas.

### Graduação em Contabilidade
| Restrição | Grau antes da divisão | Grau após seletores | Margem vs `deg(Z_N)` |
|------------|------------------------|------------------------|----------------------|
| Conservação de transferência/hortelã/queima | ≤1 | ≤1 | `deg(Z_N) - 2` |
| Pesquisa de concessão/revogação de função | ≤2 | ≤2 | `deg(Z_N) - 3` |
| Conjunto de metadados | ≤1 | ≤1 | `deg(Z_N) - 2` |
| Hash SMT (por nível) | ≤3 | ≤3 | `deg(Z_N) - 4` |
| Grande produto de pesquisa | relação de produto | N/A | Restrição de limite |
| Raízes limite/totais de oferta | 0 | 0 | exato |

As linhas de preenchimento são tratadas por meio de `s_active`; linhas fictícias estendem o rastreamento para `N_trace` sem violar restrições.## Codificação e transcrição (global)
- **Embalagem de bytes:** base-256 (membros de 7 bytes, little-endian). Testes em `fastpq_prover/tests/packing.rs`.
- **Codificação de campo:** Cachinhos Dourados canônicos (little-endian membro de 64 bits, rejeição ≥ p); Saídas Poseidon2/raízes SMT serializadas como matrizes little-endian de 32 bytes.
- **Transcrição (Fiat – Shamir):**
  1. BLAKE2b absorve `protocol_version`, `params_version`, `parameter_set`, `public_io` e tag de commit Poseidon2 (`fastpq:v1:init`).
  2. Absorva `trace_root`, `lookup_root` (`fastpq:v1:roots`).
  3. Derive o desafio de pesquisa `γ` (`fastpq:v1:gamma`).
  4. Derive os desafios de composição `α_j` (`fastpq:v1:alpha_j`).
  5. Para cada raiz da camada FRI, absorva com `fastpq:v1:fri_layer_ℓ`, derive `β_ℓ` (`fastpq:v1:beta_ℓ`).
  6. Derive índices de consulta (`fastpq:v1:query_index`).

  As tags são ASCII minúsculas; os verificadores rejeitam incompatibilidades antes dos desafios de amostragem. Dispositivo de transcrição dourada: `tests/fixtures/transcript_v1.json`.
- **Versionamento:** `protocol_version = 1`, `params_version` corresponde ao conjunto de parâmetros `fastpq_isi`.

## Argumento de pesquisa (permissões)
- Tabela confirmada ordenada lexicograficamente por `(role_id_bytes, permission_id_bytes, epoch_le)` e confirmada via árvore Merkle Poseidon2 (`perm_root` em `PublicIO`).
- A testemunha de rastreamento usa `perm_hash` e o seletor `s_perm` (OU de concessão/revogação de função). A tupla é codificada como `role_id_bytes || permission_id_bytes || epoch_u64_le` com larguras fixas (32, 32, 8 bytes).
- Relação do produto:
  ```
  Z_0 = 1
  for each row i: Z_i = Z_{i-1} * (perm_hash_i - γ)^{s_perm_i}
  T = ∏_j (table_perm_j - γ)
  ```
  Asserção de limite: `Z_final / T = 1`. Consulte `examples/lookup_grand_product.md` para obter um passo a passo do acumulador de concreto.

## Restrições esparsas da árvore Merkle
- Definir `SMT_HEIGHT` (número de níveis). As colunas `path_bit_ℓ`, `sibling_ℓ`, `node_in_ℓ`, `node_out_ℓ`, `neighbour_leaf` aparecem para todos os `ℓ ∈ [0, SMT_HEIGHT)`.
- Parâmetros Poseidon2 fixados em `ark-poseidon2` commit `3f2b7fe` (v0.3.0); etiqueta de domínio `fastpq:v1:poseidon_node`. Todos os nós usam codificação de campo little-endian.
- Atualizar regras por nível:
  ```
  if path_bit_ℓ == 0:
      node_out_ℓ = Poseidon2(node_in_ℓ, sibling_ℓ)
  else:
      node_out_ℓ = Poseidon2(sibling_ℓ, node_in_ℓ)
  ```
- Conjunto de pastilhas `(node_in_0 = 0, node_out_0 = value_new)`; exclui o conjunto `(node_in_0 = value_old, node_out_0 = 0)`.
- As provas de não adesão fornecem `neighbour_leaf` para mostrar que o intervalo consultado está vazio. Consulte `examples/smt_update.md` para obter um exemplo prático e layout JSON.
- Restrição de limite: o hash final é igual a `old_root` para pré-linhas e `new_root` para pós-linhas.

## Parâmetros de solidez e SLOs
| N_traço | explosão | Sexta-feira | camadas | consultas | bits | Tamanho da prova (≤) | RAM (≤) | Latência P95 (≤) |
| ------- | ------ | --------- | ------ | ------- | -------- | --------------- | ------- | ---------------- |
| 2^15 | 8 | 8 | 5 | 52 | ~190 | 300 KB | 1,5 GB | 0,40s (A100) |
| 2^16 | 8 | 8 | 6 | 58 | ~132 | 420 KB | 2,5 GB | 0,75s (A100) |
| 2^17 | 16 | 16 | 5 | 64 | ~142 | 550 KB | 3,5 GB | 1,20s (A100) |

As derivações seguem o Apêndice A. O chicote CI produz provas malformadas e falha se os bits estimados forem <128.## Esquema IO público
| Campo | Bytes | Codificação | Notas |
|-----------------|-------|------------------------------------------|-------------------------------------|
| `dsid` | 16 | UUID little-endian | ID do Dataspace para a pista da entrada (global para pista padrão), com hash com tag `fastpq:v1:dsid`. |
| `slot` | 8 | pequeno-endian u64 | Nanossegundos desde a época.            |
| `old_root` | 32 | bytes do campo Poseidon2 little-endian | Raiz SMT antes do lote.              |
| `new_root` | 32 | bytes do campo Poseidon2 little-endian | Raiz SMT após lote.               |
| `perm_root` | 32 | bytes do campo Poseidon2 little-endian | Raiz da tabela de permissões para o slot. |
| `tx_set_hash` | 32 | BLAKE2b | Identificadores de instrução classificados.     |
| `parameter` | var | UTF-8 (por exemplo, `fastpq-lane-balanced`) | Nome do conjunto de parâmetros.                 |
| `protocol_version`, `params_version` | 2 cada | Little Endian U16 | Valores de versão.                      |
| `ordering_hash` | 32 | Poseidon2 (little-endian) | Hash estável de linhas classificadas.         |

A exclusão é codificada por membros de valor zero; chaves ausentes usam folha zero + testemunha vizinha.

`FastpqTransitionBatch.public_inputs` é a portadora canônica para `dsid`, `slot` e compromissos raiz;
os metadados do lote são reservados para contabilidade de contagem de hash/transcrição de entrada.

## Codificação de hashes
- Hash de pedido: Poseidon2 (tag `fastpq:v1:ordering`).
- Hash de artefato em lote: BLAKE2b sobre `PublicIO || proof.commitments` (tag `fastpq:v1:artifact`).

## Definições de estágio de concluído (DoD)
- **DoD Estágio 1**
  - Embalagem de testes de ida e volta e acessórios mesclados.
  - A especificação AIR (`docs/source/fastpq_air.md`) inclui `s_active`, colunas de ativos/SMT, definições de seletor (incluindo `s_perm`) e restrições simbólicas.
  - Hash de pedido registrado no PublicIO e verificado por meio de fixtures.
  - Geração de testemunhas SMT/lookup implementada com vetores de membros e não membros.
  - Os testes de conservação abrangem transferência, cunhagem, queima e lotes mistos.
- **Estágio 2 do Departamento de Defesa**
  - Especificação de transcrição implementada; transcrição dourada (`tests/fixtures/transcript_v1.json`) e tags de domínio verificadas.
  - Confirmação do parâmetro Poseidon2 `3f2b7fe` fixado no provador e verificador com testes de endianidade em arquiteturas.
  - Proteção CI sólida ativa; SLOs de tamanho de prova/RAM/latência registrados.
- **Estágio 3 do Departamento de Defesa**
  - API do Agendador (`SubmitProofRequest`, `ProofResult`) documentada com chaves de idempotência.
  - Prove artefatos armazenados de forma endereçável com nova tentativa/retirada.
  - Telemetria exportada para profundidade da fila, tempo de espera da fila, latência de execução do provador, contagens de novas tentativas, contagens de falhas de back-end e utilização de GPU/CPU, com painéis e limites de alerta para cada métrica.## Estágio 5 – Aceleração e Otimização de GPU
- Kernels alvo: LDE (NTT), hashing Poseidon2, construção de árvore Merkle, dobramento FRI.
- Determinismo: desative a matemática rápida, garanta saídas idênticas em bits na CPU, CUDA, Metal. O CI deve comparar raízes de prova entre dispositivos.
- Conjunto de benchmark comparando CPU vs GPU em hardware de referência (por exemplo, Nvidia A100, AMD MI210).
- Back-end de metal (Apple Silicon):
  - O script de construção compila o conjunto de kernel (`metal/kernels/ntt_stage.metal`, `metal/kernels/poseidon2.metal`) em `fastpq.metallib` via `xcrun metal`/`xcrun metallib`; certifique-se de que as ferramentas de desenvolvedor do macOS incluam o conjunto de ferramentas Metal (`xcode-select --install` e, em seguida, `xcodebuild -downloadComponent MetalToolchain`, se necessário).【crates/fastpq_prover/build.rs:98】【crates/fastpq_prover/build.rs:189】
  - Reconstrução manual (espelhos `build.rs`) para aquecimentos de CI ou embalagens determinísticas:
    ```bash
    export OUT_DIR=$PWD/target/metal && mkdir -p "$OUT_DIR"
    xcrun metal -std=metal3.0 -O3 -c metal/kernels/ntt_stage.metal -o "$OUT_DIR/ntt_stage.air"
    xcrun metal -std=metal3.0 -O3 -c metal/kernels/poseidon2.metal -o "$OUT_DIR/poseidon2.air"
    xcrun metallib "$OUT_DIR/ntt_stage.air" "$OUT_DIR/poseidon2.air" -o "$OUT_DIR/fastpq.metallib"
    export FASTPQ_METAL_LIB="$OUT_DIR/fastpq.metallib"
    ```
    Compilações bem-sucedidas emitem `FASTPQ_METAL_LIB=<path>` para que o tempo de execução possa carregar o metallib de forma determinística.【crates/fastpq_prover/build.rs:188】【crates/fastpq_prover/src/metal.rs:42】
  - O kernel LDE agora assume que o buffer de avaliação é inicializado com zero no host. Mantenha o caminho de alocação `vec![0; ..]` existente ou explicitamente zero buffers ao reutilizá-los.【crates/fastpq_prover/src/metal.rs:233】【crates/fastpq_prover/metal/kernels/ntt_stage.metal:141】
  - A multiplicação Coset é incorporada ao estágio final da FFT para evitar uma passagem extra; quaisquer alterações no teste LDE devem preservar essa invariante.【crates/fastpq_prover/metal/kernels/ntt_stage.metal:193】
  - O kernel FFT/LDE de memória compartilhada agora para na profundidade do bloco e entrega as borboletas restantes mais qualquer escala inversa para uma passagem `fastpq_fft_post_tiling` dedicada. O host Rust encadeia os mesmos lotes de colunas em ambos os kernels e só inicia o despacho pós-tile quando `log_len` excede o limite do bloco, portanto, a telemetria de profundidade da fila, as estatísticas do kernel e o comportamento de fallback permanecem determinísticos enquanto a GPU lida inteiramente com o trabalho de estágio amplo no dispositivo.【crates/fastpq_prover/metal/kernels/ntt_stage.metal:447】【crates/fastpq_prover/src/metal.rs:654】
  - Para experimentar formas de lançamento, defina `FASTPQ_METAL_THREADGROUP=<width>`; o caminho de despacho fixa o valor ao limite do dispositivo e registra a substituição para que as execuções de perfil possam varrer os tamanhos dos grupos de threads sem recompilar.【crates/fastpq_prover/src/metal.rs:321】- Ajuste o bloco FFT diretamente: o host agora deriva pistas de grupos de threads (16 para rastreamentos curtos, 32 uma vez `log_len ≥ 6`, 64 uma vez `log_len ≥ 10`, 128 uma vez `log_len ≥ 14` e 256 em `log_len ≥ 18`) e profundidade do bloco (5 estágios para pequenos rastreamentos, 4 quando `log_len ≥ 12`, e quando o domínio atinge `log_len ≥ 18/20/22`, a passagem de memória compartilhada agora executa 14/12/16 estágios antes de entregar o controle ao kernel pós-tile) do domínio solicitado mais a largura/máximo de threads de execução do dispositivo. Substitua por `FASTPQ_METAL_FFT_LANES` (potência de dois entre 8 e 256) e `FASTPQ_METAL_FFT_TILE_STAGES` (1–16) para fixar formatos de lançamento específicos; ambos os valores fluem através de `FftArgs`, são fixados na janela suportada e são registrados para varreduras de perfil.
- O lote de colunas FFT/IFFT e LDE agora deriva da largura do grupo de threads resolvido: o host tem como alvo aproximadamente 4.096 threads lógicos por buffer de comando, funde até 64 colunas por vez com a preparação do bloco de buffer circular e apenas desce através de 64 → 32 → 16 → 8 → 4 → 2 → 1 colunas conforme o domínio de avaliação cruza os limites de 2¹⁶/2¹⁸/2²⁰/2²². Isso mantém a captura de 20 mil linhas em ≥64 colunas por despacho, ao mesmo tempo que garante que cosets longos ainda terminem de forma determinística. O agendador adaptativo ainda dobra a largura da coluna até que os despachos se aproximem da meta de ≈2ms e agora divide o lote pela metade automaticamente sempre que um despacho amostrado chega ≥30% acima dessa meta, de modo que as transições de pista/bloco que inflacionam o custo por coluna recuam sem substituições manuais. As permutações do Poseidon compartilham o mesmo agendador adaptativo e o bloco `metal_heuristics.batch_columns.poseidon` em `fastpq_metal_bench` agora registra a contagem de estados resolvidos, o limite, a última duração e o sinalizador de substituição para que a telemetria de profundidade da fila possa ser vinculada diretamente ao ajuste do Poseidon. Substitua por `FASTPQ_METAL_FFT_COLUMNS` (1–64) para fixar um tamanho de lote FFT determinístico e use `FASTPQ_METAL_LDE_COLUMNS` (1–64) quando precisar que o despachante LDE honre uma contagem de colunas fixa; o banco de metal apresenta as entradas `kernel_profiles.*.columns` resolvidas em cada captura para que os experimentos de ajuste permaneçam reproduzíveis.- O envio de múltiplas filas agora é automático em Macs discretos: o host inspeciona `is_low_power`, `is_headless` e a localização do dispositivo para decidir se deve ativar duas filas de comando Metal, apenas se espalha quando a carga de trabalho carrega pelo menos 16 colunas (escalada pelo fan-out resolvido) e faz round-robins dos lotes de colunas para que traços longos mantenham ambas as pistas de GPU ocupadas sem sacrificar determinismo. O semáforo de buffer de comando agora impõe um piso de “dois em andamento por fila”, e a telemetria da fila registra a janela de medição agregada (`window_ms`) mais taxas de ocupação normalizadas (`busy_ratio`) para o semáforo global e cada entrada de fila para que os artefatos de liberação possam provar que ambas as filas permaneceram ≥50% ocupadas no mesmo intervalo de tempo. Substitua os padrões por `FASTPQ_METAL_QUEUE_FANOUT` (1–4 pistas) e `FASTPQ_METAL_COLUMN_THRESHOLD` (total mínimo de colunas antes da distribuição); os testes de paridade Metal forçam as substituições para que os Macs multi-GPU permaneçam cobertos e a política resolvida seja registrada junto com a telemetria de profundidade da fila e o novo `metal_dispatch_queue.queues[*]` bloco.【crates/fastpq_prover/src/metal.rs:620】【crates/fastpq_prover/src/metal.rs:900】【crates/fastpq_prover/src/metal.rs:2254】【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:871】
- A detecção de metal agora investiga `MTLCreateSystemDefaultDevice`/`MTLCopyAllDevices` diretamente (aquecendo CoreGraphics em shells sem cabeça) antes de voltar para `system_profiler`, e `FASTPQ_DEBUG_METAL_ENUM` imprime os dispositivos enumerados quando configurado para que execuções de CI sem cabeça possam explicar por que `FASTPQ_GPU=gpu` ainda foi rebaixado para a CPU caminho. Quando a substituição é definida como `gpu`, mas nenhum acelerador é detectado, `fastpq_metal_bench` agora apresenta erros imediatamente com um ponteiro para o botão de depuração em vez de continuar silenciosamente na CPU. Isso restringe a classe de “substituição silenciosa da CPU” definida no WP2‑E e dá aos operadores um botão para capturar logs de enumeração dentro de benchmarks agrupados.
  - Os tempos de GPU Poseidon agora se recusam a tratar substitutos de CPU como dados de “GPU”. `hash_columns_gpu` relata se o acelerador realmente foi executado, `measure_poseidon_gpu` descarta amostras (e registra um aviso) sempre que o pipeline retorna e o filho do microbench Poseidon sai com um erro se o hash da GPU não estiver disponível. Como resultado, `gpu_recorded=false` sempre que a execução do Metal retrocede, o resumo da fila ainda registra a janela de despacho com falha e os resumos do painel sinalizam imediatamente a regressão. O wrapper (`scripts/fastpq/wrap_benchmark.py`) agora falha quando `metal_dispatch_queue.poseidon.dispatch_count == 0`, então os pacotes Stage7 não podem ser assinados sem o envio real do GPU Poseidon evidência.【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:1123】【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:2200】【scripts/fastpq/wrap_benchmark.py:912】- O hash de Poseidon agora reflete esse contrato de teste. `PoseidonColumnBatch` produz buffers de carga útil nivelados mais descritores de deslocamento/comprimento, o host rebase esses descritores por lote e executa um buffer duplo `COLUMN_STAGING_PIPE_DEPTH` para que os uploads de carga + descritor se sobreponham ao trabalho da GPU, e ambos os kernels Metal/CUDA consomem os descritores diretamente para que cada despacho absorva todos os blocos de taxa preenchidos no dispositivo antes de emitir os resumos da coluna. `hash_columns_from_coefficients` agora transmite esses lotes por meio de um thread de trabalho de GPU, mantendo mais de 64 colunas em andamento por padrão em GPUs discretas (ajustáveis ​​via `FASTPQ_POSEIDON_PIPE_COLUMNS`/`FASTPQ_POSEIDON_PIPE_DEPTH`). O banco Metal registra as configurações de pipeline resolvidas + contagens de lote em `metal_dispatch_queue.poseidon_pipeline`, e `kernel_profiles.poseidon.bytes` inclui o tráfego do descritor para que as capturas do Stage7 comprovem a nova ABI ponta a ponta.【crates/fastpq_prover/src/trace.rs:604】【crates/fastpq_prover/src/trace.rs:809】【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:196 3】【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:2675】【crates/fastpq_prover/src/metal.rs:2290】【crates/fastpq_prover/cuda/fastpq_cuda.cu:351】
- O hash Poseidon fundido Stage7-P2 agora chega em ambos os back-ends da GPU. O trabalhador de streaming alimenta fatias `PoseidonColumnBatch::column_window()` contíguas em `hash_columns_gpu_fused`, que as canaliza para `poseidon_hash_columns_fused` para que cada despacho grave `leaf_digests || parent_digests` com o mapeamento pai canônico `(⌈columns / 2⌉)`. `ColumnDigests` mantém ambas as fatias e `merkle_root_with_first_level` consome a camada pai imediatamente, de modo que a CPU nunca recalcula os nós de profundidade 1 e a telemetria do Stage7 pode afirmar que as capturas da GPU reportam zero pais “substitutos” sempre que o kernel fundido consegue.【crates/fastpq_prover/src/trace.rs:1070】【crates/fastpq_prover/src/gpu.rs:365】【crates/fastpq_prover/src/metal.rs:2422】【crates/fastpq_prover/cuda/fastpq_cuda.cu:631】
- `fastpq_metal_bench` agora emite um bloco `device_profile` com o nome do dispositivo Metal, ID de registro, sinalizadores `low_power`/`headless`, localização (integrado, slot, externo), indicador discreto, `hw.model` e o rótulo Apple SoC derivado (por exemplo, “M3 Max”). Os painéis do Stage7 consomem este campo para capturar capturas por M4/M3 versus GPUs discretas sem analisar nomes de host, e o JSON é enviado próximo à fila/evidência heurística para que cada artefato de lançamento prove qual classe de frota produziu a execução.- A sobreposição de host/dispositivo FFT agora usa uma janela de teste com buffer duplo: enquanto o lote *n* termina dentro de `fastpq_fft_post_tiling`, o host nivela o lote *n+1* no segundo buffer de teste e só pausa quando um buffer deve ser reciclado. O back-end registra quantos lotes foram nivelados mais o tempo gasto no nivelamento em comparação com a espera pela conclusão da GPU, e `fastpq_metal_bench` apresenta o bloco `column_staging.{batches,flatten_ms,wait_ms,wait_ratio}` agregado para que os artefatos de liberação possam provar a sobreposição em vez de paralisações silenciosas do host. O relatório JSON agora também divide os totais por fase em `column_staging.phases.{fft,lde,poseidon}`, permitindo que as capturas do Stage7 provem se o teste FFT/LDE/Poseidon está vinculado ao host ou aguardando a conclusão da GPU. As permutações do Poseidon reutilizam os mesmos buffers de teste agrupados, portanto, as capturas `--operation poseidon_hash_columns` agora emitem os deltas `column_staging` específicos do Poseidon junto com a evidência de profundidade da fila sem instrumentação personalizada. Os novos arrays `column_staging.samples.{fft,lde,poseidon}` registram as tuplas `batch/flatten_ms/wait_ms/wait_ratio` por lote, tornando trivial provar que a sobreposição `COLUMN_STAGING_PIPE_DEPTH` está sendo mantida (ou detectar quando o host começa a esperar pela GPU completações).【crates/fastpq_prover/src/metal.rs:319】【crates/fastpq_prover/src/metal.rs:330】【crates/fastpq_prover/src/metal.rs:1813】【crates/fas tpq_prover/src/metal.rs:2488】【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:1189】【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:1216】- A aceleração Poseidon2 agora é executada como um kernel Metal de alta ocupação: cada grupo de threads copia as constantes redondas e as linhas MDS na memória do grupo de threads, desenrola as rodadas completas/parciais e percorre vários estados por pista para que cada despacho lance pelo menos 4.096 threads lógicos. Substitua o formato de lançamento via `FASTPQ_METAL_POSEIDON_LANES` (potências de dois entre 32 e 256, fixadas no limite do dispositivo) e `FASTPQ_METAL_POSEIDON_BATCH` (1–32 estados por pista) para reproduzir experimentos de criação de perfil sem reconstruir `fastpq.metallib`; o host Rust encadeia o ajuste resolvido por meio de `PoseidonArgs` antes do envio. O host agora captura instantâneos de `MTLDevice::{is_low_power,is_headless,location}` uma vez por inicialização e direciona automaticamente GPUs discretas para lançamentos em camadas VRAM (`256×24` em peças ≥48GiB, `256×20` em 32GiB, `256×16` caso contrário) enquanto SoCs de baixo consumo se atêm a `256×8` (fallbacks para hardware de 128/64 pistas continuam a usar 8/6 estados por pista), para que os operadores obtenham a profundidade do pipeline de> 16 estados sem tocar em env vars. `fastpq_metal_bench` se reexecuta sob `FASTPQ_METAL_POSEIDON_MICRO_MODE={default,scalar}` para capturar um bloco `poseidon_microbench` dedicado comparando a pista escalar com o kernel multiestado para que os artefatos de liberação possam citar uma aceleração concreta. O mesmo captura a telemetria de superfície `poseidon_pipeline` (`chunk_columns`, `pipe_depth`, `batches`, `fallbacks`), portanto, as evidências do Stage7 comprovam a janela de sobreposição em cada GPU trace.【crates/fastpq_prover/metal/kernels/poseidon2.metal:1】【crates/fastpq_prover/src/metal_config.rs:78】【crates/fastpq_prover/src/metal.rs:1971】【cra tes/fastpq_prover/src/bin/fastpq_metal_bench.rs:1691】【crates/fastpq_prover/src/trace.rs:299】【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:1988】
  - A preparação do bloco LDE agora reflete a heurística FFT: rastreamentos pesados executam apenas 12 estágios na passagem de memória compartilhada uma vez `log₂(len) ≥ 18`, caem para 10 estágios no log₂20 e fixam-se em oito estágios no log₂22 para que as borboletas largas se movam para o kernel pós-tiling. Substitua por `FASTPQ_METAL_LDE_TILE_STAGES` (1–32) sempre que precisar de uma profundidade determinística; o host só inicia o despacho pós-tile quando a heurística para mais cedo, para que a profundidade da fila e a telemetria do kernel permaneçam determinísticas.【crates/fastpq_prover/src/metal.rs:827】
  - Micro-otimização do kernel: os blocos FFT/LDE de memória compartilhada agora reutilizam movimentos giratórios por faixa e avanços aproximados em vez de reavaliar `pow_mod*` para cada borboleta. Cada pista pré-calcula `w_seed`, `w_stride` e (quando necessário) o passo coset uma vez por bloco e, em seguida, transmite através dos deslocamentos, reduzindo as multiplicações escalares dentro de `apply_stage_tile`/`apply_stage_global` e reduzindo a média de LDE de 20 mil linhas para ~ 1,55s com as heurísticas mais recentes (ainda acima da meta de 950 ms, mas uma melhoria adicional de ~ 50 ms em relação ao ajuste somente em lote).- O conjunto de kernel agora tem uma referência dedicada (`docs/source/fastpq_metal_kernels.md`) que documenta cada ponto de entrada, os limites de threadgroup/tile aplicados em `fastpq.metallib` e as etapas de reprodução para compilar o metallib manualmente.【docs/source/fastpq_metal_kernels.md:1】
  - O relatório de benchmark agora emite um objeto `post_tile_dispatches` que registra quantos lotes FFT/IFFT/LDE foram executados no kernel pós-tiling dedicado (contagens de despacho por tipo mais os limites de estágio/log₂). `scripts/fastpq/wrap_benchmark.py` copia o bloco em `benchmarks.post_tile_dispatches`/`benchmarks.post_tile_summary`, e a porta de manifesto recusa capturas de GPU que omitem a evidência para que cada artefato de 20 mil linhas prove que o kernel multi-pass foi executado no dispositivo.【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:1048】【scripts/fastpq/wrap_benchmark.py:255】【xtask/src/fastpq.rs:280】
  - Defina `FASTPQ_METAL_TRACE=1` para emitir logs de depuração por despacho (rótulo do pipeline, largura do grupo de threads, grupos de lançamento, tempo decorrido) para correlação de rastreamento de instrumentos/metal.【crates/fastpq_prover/src/metal.rs:346】
- A fila de despacho agora é instrumentada: `FASTPQ_METAL_MAX_IN_FLIGHT` limita buffers de comando Metal simultâneos (padrão automático derivado da contagem de núcleos de GPU detectados via `system_profiler`, fixado pelo menos no piso de distribuição da fila com um fallback de paralelismo de host quando o macOS se recusa a relatar o dispositivo). O banco permite amostragem de profundidade de fila para que o JSON exportado carregue um objeto `metal_dispatch_queue` com campos `limit`, `dispatch_count`, `max_in_flight`, `busy_ms` e `overlap_ms` para evidência de liberação, adiciona um aninhado Bloco `metal_dispatch_queue.poseidon` sempre que uma captura somente Poseidon (`--operation poseidon_hash_columns`) é executada e emite um bloco `metal_heuristics` descrevendo o limite do buffer de comando resolvido mais as colunas de lote FFT/LDE (incluindo se as substituições forçaram os valores) para que os revisores possam auditar as decisões de agendamento junto com a telemetria. Os kernels Poseidon também alimentam um bloco `poseidon_profiles` dedicado, destilado das amostras do kernel, para que bytes/thread, ocupação e geometria de despacho sejam rastreados entre os artefatos. Se a execução primária não puder coletar a profundidade da fila ou as estatísticas de preenchimento zero do LDE (por exemplo, quando um despacho da GPU retorna silenciosamente para a CPU), o chicote dispara automaticamente um único despacho de sonda para coletar a telemetria ausente e agora sintetiza os tempos de preenchimento zero do host quando a GPU se recusa a relatá-los, portanto, as evidências publicadas sempre incluem o `zero_fill` bloco.【crates/fastpq_prover/src/metal.rs:2056】【crates/fastpq_prover/src/metal.rs:247】【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:1524】【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:2078】
  - Defina `FASTPQ_SKIP_GPU_BUILD=1` ao fazer compilação cruzada sem o conjunto de ferramentas Metal; o aviso registra o salto e o planejador continua no caminho da CPU.【crates/fastpq_prover/build.rs:45】【crates/fastpq_prover/src/backend.rs:195】- A detecção em tempo de execução usa `system_profiler` para confirmar o suporte ao Metal; se a estrutura, dispositivo ou metallib estiver faltando, o script de construção limpa `FASTPQ_METAL_LIB` e o planejador permanece na CPU determinística caminho.【crates/fastpq_prover/build.rs:29】【crates/fastpq_prover/src/backend.rs:293】【crates/fastpq_prover/src/backend.rs:605】【crates/fastpq_prover/src/metal.rs:43】
  - Lista de verificação do operador (hospedeiros metálicos):
    1. Confirme se o conjunto de ferramentas está presente e se `FASTPQ_METAL_LIB` aponta para um `.metallib` compilado (`echo $FASTPQ_METAL_LIB` não deve estar vazio após `cargo build --features fastpq-gpu`).【crates/fastpq_prover/build.rs:188】
    2. Execute testes de paridade com pistas de GPU habilitadas: `FASTPQ_GPU=gpu cargo test -p fastpq_prover --features fastpq-gpu --release`. Isso exercita os kernels do Metal e retorna automaticamente se a detecção falhar.【crates/fastpq_prover/src/backend.rs:114】【crates/fastpq_prover/src/metal.rs:418】
    3. Capture uma amostra de benchmark para painéis: localize a biblioteca Metal compilada
       (`fd -g 'fastpq.metallib' target/release/build | head -n1`), exporte-o via
       `FASTPQ_METAL_LIB` e execute
      `cargo run -p fastpq_prover --features fastpq-gpu --bin fastpq_metal_bench --release -- --rows 20000 --iterations 5 --output fastpq_metal_bench.json --trace-dir traces`.
       O conjunto canônico `fastpq-lane-balanced` agora preenche cada captura para 32.768 linhas, então o
       JSON reflete as 20 mil linhas solicitadas e o domínio preenchido que aciona a GPU
       grãos. Faça upload do JSON/log para seu armazenamento de evidências; os espelhos noturnos do fluxo de trabalho do macOS
      esta execução e arquiva os artefatos para referência. O relatório registra
     `fft_tuning.{threadgroup_lanes,tile_stage_limit}` junto com o `speedup` de cada operação, o
     A seção LDE adiciona `zero_fill.{bytes,ms,queue_delta}` para que os artefatos de lançamento provem o determinismo,
     sobrecarga de preenchimento zero do host e o uso incremental da fila da GPU (limite, contagem de despacho,
     pico em voo, tempo ocupado/sobreposição) e o novo bloco `kernel_profiles` captura por kernel
     taxas de ocupação, largura de banda estimada e intervalos de duração para que os painéis possam sinalizar a GPU
       regressões sem reprocessar amostras brutas.【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:697】
       Espere que o caminho do Metal LDE fique abaixo de 950 ms (alvo `<1 s` no hardware Apple série M);
4. Capture telemetria de uso de linha de um ExecWitness real para que os painéis possam mapear o gadget de transferência
   adoção. Obtenha uma testemunha de Torii
  (`iroha_cli audit witness --binary --out exec.witness`) e decodifique-o com
  `iroha_cli audit witness --decode exec.witness` (opcionalmente adicione
  `--fastpq-parameter fastpq-lane-balanced` para afirmar o conjunto de parâmetros esperado; Lotes FASTPQ
  emitir por padrão; passe `--no-fastpq-batches` somente se precisar cortar a saída).
   Cada entrada de lote agora emite um objeto `row_usage` (`total_rows`, `transfer_rows`,
   `non_transfer_rows`, contagens por seletor e `transfer_ratio`). Arquive esse snippet JSON
   reprocessando transcrições brutas.【crates/iroha_cli/src/audit.rs:209】 Compare a nova captura com
   a linha de base anterior com `scripts/fastpq/check_row_usage.py`, então o CI falha se as taxas de transferência ou
   total de linhas regride:

   ```bash
   python3 scripts/fastpq/check_row_usage.py \
     --baseline artifacts/fastpq_benchmarks/fastpq_row_usage_2025-02-01.json \
     --candidate fastpq_row_usage_2025-05-12.json \
     --max-transfer-ratio-increase 0.005 \
     --max-total-rows-increase 0
   ```Exemplos de blobs JSON para testes de fumaça estão disponíveis em `scripts/fastpq/examples/`. Localmente você pode executar `make check-fastpq-row-usage`
   (envolve `ci/check_fastpq_row_usage.sh`) e o CI executa o mesmo script via `.github/workflows/fastpq-row-usage.yml` para comparar o commit
   Instantâneos `artifacts/fastpq_benchmarks/fastpq_row_usage_*.json` para que o pacote de evidências falhe rapidamente sempre que
   as linhas de transferência voltam a subir. Passe `--summary-out <path>` se desejar uma comparação legível por máquina (o trabalho de CI carrega `fastpq_row_usage_summary.json`).
   Quando um ExecWitness não for útil, sintetize uma amostra de regressão com `fastpq_row_bench`
   (`crates/fastpq_prover/src/bin/fastpq_row_bench.rs:1`), que emite exatamente o mesmo `row_usage`
   objeto para contagens de seletores configuráveis (por exemplo, um teste de estresse de 65.536 linhas):

   ```bash
   cargo run -p fastpq_prover --bin fastpq_row_bench -- \
     --transfer-rows 65536 \
     --mint-rows 256 \
     --burn-rows 128 \
     --pretty \
     --output artifacts/fastpq_benchmarks/fastpq_row_usage_65k.json
   ```Os pacotes de implementação Stage7-3 também devem passar pelo `scripts/fastpq/validate_row_usage_snapshot.py`, que
   impõe que cada entrada `row_usage` contenha as contagens do seletor e que
   `transfer_ratio = transfer_rows / total_rows`; `ci/check_fastpq_rollout.sh` chama o ajudante
   automaticamente para que os pacotes sem esses invariantes falhem antes que as faixas da GPU sejam obrigatórias.【scripts/fastpq/validate_row_usage_snapshot.py:1】【ci/check_fastpq_rollout.sh:1】
       o portão de manifesto do banco impõe isso via `--max-operation-ms lde=950`, então atualize o
       capture sempre que sua evidência exceder esse limite.
      Quando você também precisar de provas de Instrumentos, passe `--trace-dir <dir>` para que o arnês
      se reinicia via `xcrun xctrace record` (modelo padrão “Metal System Trace”) e
      armazena um arquivo `.trace` com carimbo de data e hora junto com o JSON; você ainda pode substituir o local /
      modelo manualmente com `--trace-output <path>` mais `--trace-template` opcional /
      `--trace-seconds`. O JSON resultante anuncia `metal_trace_{template,seconds,output}` então
      pacotes de artefatos sempre identificam o traço capturado.【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:177】
      Enrole cada captura com
      `python3 scripts/fastpq/wrap_benchmark.py --require-lde-mean-ms 950 --require-poseidon-mean-ms 1000 --row-usage artifacts/fastpq_benchmarks/fastpq_row_usage_2025-05-12.json fastpq_metal_bench.json artifacts/fastpq_benchmarks/fastpq_metal_bench_<date>_macos14_arm64.json --sign-output`
       (adicione `--gpg-key <fingerprint>` se precisar fixar uma identidade de assinatura) para que o pacote falhe
       rápido sempre que a média LDE da GPU viola a meta de 950 ms, o Poseidon excede 1s ou o
       Faltam blocos de telemetria Poseidon, incorpora um `row_usage_snapshot`
      ao lado do JSON, aparece o resumo do microbench Poseidon em `benchmarks.poseidon_microbench`,
      e ainda carrega metadados para runbooks e o painel Grafana
    (`dashboards/grafana/fastpq_acceleration.json`). O JSON agora emite `speedup.ratio` /
     `speedup.delta_ms` por operação para que a evidência de liberação possa provar GPU vs.
     Ganhos de CPU sem reprocessar as amostras brutas, e o wrapper copia tanto o
     estatísticas de preenchimento zero (mais `queue_delta`) em `zero_fill_hotspots` (bytes, latência, derivado
     GB/s), registra os metadados dos Instrumentos em `metadata.metal_trace`, encadeia o opcional
     Bloco `metadata.row_usage_snapshot` quando `--row-usage <decoded witness>` é fornecido e nivela o
     contadores por kernel em `benchmarks.kernel_summary` para preencher gargalos, fila de metal
     utilização, ocupação do kernel e regressões de largura de banda são visíveis rapidamente, sem
     explorando o relatório bruto.【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:521】【scripts/fastpq/wrap_benchmark.py:1】【artifacts/fastpq_benchmarks/fastpq_metal_bench_2025-11-07T123018Z_macos14_arm64.json:1】
     Como o instantâneo de uso de linha agora viaja com o artefato empacotado, os tickets de implementação simplesmente
     faça referência ao pacote em vez de anexar um segundo snippet JSON, e o CI pode diferenciar o incorporado
    conta diretamente ao validar os envios do Stage7. Para arquivar os dados do microbanco por conta própria,
    execute `python3 scripts/fastpq/export_poseidon_microbench.py --bundle artifacts/fastpq_benchmarks/<metal>.json`
    e armazene o arquivo resultante em `benchmarks/poseidon/`. Mantenha o manifesto agregado atualizado com
    `python3 scripts/fastpq/aggregate_poseidon_microbench.py --input benchmarks/poseidon --output benchmarks/poseidon/manifest.json`
    portanto, os painéis/CI podem diferenciar o histórico completo sem percorrer cada arquivo manualmente.4. Valide a telemetria enrolando `fastpq_execution_mode_total{device_class="<matrix>", backend="metal"}` (endpoint Prometheus) ou procurando logs `telemetry::fastpq.execution_mode`; entradas `resolved="cpu"` inesperadas indicam que o host recuou apesar da intenção da GPU.【crates/iroha_telemetry/src/metrics.rs:8887】【crates/fastpq_prover/src/backend.rs:174】
    5. Use `FASTPQ_GPU=cpu` (ou o botão de configuração) para forçar a execução da CPU durante a manutenção e confirmar se os logs de fallback ainda aparecem; isso mantém os runbooks SRE alinhados com o caminho determinístico.【crates/fastpq_prover/src/backend.rs:308】【crates/iroha_config/src/parameters/user.rs:1357】
- Telemetria e reserva:
  - Logs do modo de execução (`telemetry::fastpq.execution_mode`) e contadores (`fastpq_execution_mode_total{device_class="…", backend="metal"|…}`) expõem o modo solicitado versus o modo resolvido para que substitutos silenciosos sejam visíveis nos painéis.【crates/fastpq_prover/src/backend.rs:174】【crates/iroha_telemetry/src/metrics.rs:5397】
  - A placa `FASTPQ Acceleration Overview` Grafana (`dashboards/grafana/fastpq_acceleration.json`) visualiza a taxa de adoção do Metal e vincula-se aos artefatos de benchmark, enquanto as regras de alerta emparelhadas (`dashboards/alerts/fastpq_acceleration_rules.yml`) permitem lançamentos em downgrades sustentados.
  - As substituições `FASTPQ_GPU={auto,cpu,gpu}` permanecem suportadas; valores desconhecidos geram avisos, mas ainda se propagam para telemetria para auditoria.【crates/fastpq_prover/src/backend.rs:308】【crates/fastpq_prover/src/backend.rs:349】
  - Os testes de paridade de GPU (`cargo test -p fastpq_prover --features fastpq_prover/fastpq-gpu`) devem passar para CUDA e Metal; CI pula normalmente quando o metallib está ausente ou a detecção falha.【crates/fastpq_prover/src/gpu.rs:49】【crates/fastpq_prover/src/backend.rs:346】
  - Evidência de prontidão do metal (arquive os artefatos abaixo com cada implementação para que a auditoria do roteiro possa provar determinismo, cobertura de telemetria e comportamento alternativo):| Etapa | Meta | Comando / Evidência |
    | ---- | ---- | ------------------ |
    | Construir metallib | Certifique-se de que `xcrun metal`/`xcrun metallib` estejam disponíveis e emita o `.metallib` determinístico para este commit | `xcrun metal -std=metal3.0 -O3 -c metal/kernels/ntt_stage.metal -o "$OUT_DIR/ntt_stage.air"`; `xcrun metal -std=metal3.0 -O3 -c metal/kernels/poseidon2.metal -o "$OUT_DIR/poseidon2.air"`; `xcrun metallib "$OUT_DIR/ntt_stage.air" "$OUT_DIR/poseidon2.air" -o "$OUT_DIR/fastpq.metallib"`; exportar `FASTPQ_METAL_LIB=$OUT_DIR/fastpq.metallib`.【crates/fastpq_prover/build.rs:98】【crates/fastpq_prover/build.rs:188】
    | Verifique env var | Confirme se o Metal permanece ativado verificando o env var registrado pelo script de construção | `echo $FASTPQ_METAL_LIB` (deve retornar um caminho absoluto; vazio significa que o backend foi desativado).【crates/fastpq_prover/build.rs:188】【crates/fastpq_prover/src/metal.rs:43】
    | Conjunto de paridade GPU | Provar que os kernels são executados (ou emitem logs determinísticos de downgrade) antes do envio | `FASTPQ_GPU=gpu cargo test -p fastpq_prover --features fastpq-gpu --release` e armazene o snippet de log resultante que mostra `backend="metal"` ou o aviso de fallback.【crates/fastpq_prover/src/backend.rs:114】【crates/fastpq_prover/src/backend.rs:195】
    | Amostra de referência | Capture o par JSON/log que registra o ajuste `speedup.*` e FFT para que os painéis possam ingerir evidências do acelerador | `FASTPQ_METAL_LIB=$(fd -g 'fastpq.metallib' target/release/build | head -n1) cargo run -p fastpq_prover --features fastpq-gpu --bin fastpq_metal_bench --release -- --rows 20000 --iterations 5 --output fastpq_metal_bench.json --trace-dir traces`; arquivar o JSON, o carimbo de data / hora `.trace` e o stdout junto com as notas de lançamento para que a placa Grafana pegue a execução do Metal (o relatório registra as 20 mil linhas solicitadas mais o domínio preenchido de 32.768 linhas para que os revisores possam confirmar o `<1 s` LDE alvo).【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:697】
    | Embrulhar e assinar relatório | Falha no lançamento se a média LDE da GPU violar 950ms, Poseidon exceder 1s ou blocos de telemetria Poseidon estiverem faltando e produzir um pacote de artefato assinado | `python3 scripts/fastpq/wrap_benchmark.py --require-lde-mean-ms 950 --require-poseidon-mean-ms 1000 fastpq_metal_bench.json artifacts/fastpq_benchmarks/fastpq_metal_bench_<date>_macos14_arm64.json --sign-output [--gpg-key <fingerprint>]`; envia o JSON empacotado e a assinatura `.json.asc` gerada para que os auditores possam verificar as métricas de menos de um segundo sem executar novamente a carga de trabalho.【scripts/fastpq/wrap_benchmark.py:714】【scripts/fastpq/wrap_benchmark.py:732】 |
    | Manifesto de banco assinado | Aplique evidências LDE `<1 s` em pacotes Metal/CUDA e capture resumos assinados para aprovação de lançamento | `cargo xtask fastpq-bench-manifest --bench metal=artifacts/fastpq_benchmarks/fastpq_metal_bench_<date>_macos14_arm64.json --bench cuda=artifacts/fastpq_benchmarks/fastpq_cuda_bench_<date>_sm80.json --matrix artifacts/fastpq_benchmarks/matrix/matrix_manifest.json --signing-key secrets/fastpq_bench.ed25519 --out artifacts/fastpq_bench_manifest.json`; anexe o manifesto + assinatura ao ticket de lançamento para que a automação downstream possa validar as métricas de prova em menos de um segundo.【xtask/src/fastpq.rs:1】【artifacts/fastpq_benchmarks/README.md:65】
| Pacote CUDA | Mantenha a captura SM80 CUDA em sincronia com a evidência Metal para que os manifestos cubram ambas as classes de GPU. | `FASTPQ_GPU=gpu cargo run -p fastpq_prover --bin fastpq_cuda_bench --release -- --rows 20000 --iterations 5 --column-count 16 --device 0 --row-usage artifacts/fastpq_benchmarks/fastpq_row_usage_2025-05-12.json` no host Xeon+RTX → `python3 scripts/fastpq/wrap_benchmark.py --require-lde-mean-ms 950 --require-poseidon-mean-ms 1000 fastpq_cuda_bench.json artifacts/fastpq_benchmarks/fastpq_cuda_bench_<date>_sm80.json --label device_class=xeon-rtx-sm80 --sign-output`; anexe o caminho encapsulado a `artifacts/fastpq_benchmarks/matrix/devices/xeon-rtx-sm80.txt`, mantenha o par `.json`/`.asc` próximo ao pacote Metal e cite o `fastpq_cuda_bench_2025-11-12T090501Z_ubuntu24_x86_64.json` propagado quando os auditores precisarem de uma referência layout.【scripts/fastpq/wrap_benchmark.py:714】【artifacts/fastpq_benchmarks/matrix/devices/xeon-rtx-sm80.txt:1】
| Verificação de telemetria | Valide as superfícies Prometheus/OTEL refletindo `device_class="<matrix>", backend="metal"` (ou registre o downgrade) | `curl -s http://<host>:8180/metrics | rg 'fastpq_execution_mode_total{device_class'` e copie o log `telemetry::fastpq.execution_mode` emitido na inicialização.【crates/iroha_telemetry/src/metrics.rs:8887】【crates/fastpq_prover/src/backend.rs:174】| Exercício de fallback forçado | Documente o caminho determinístico da CPU para manuais SRE | Execute uma carga de trabalho curta com `FASTPQ_GPU=cpu` ou `zk.fastpq.execution_mode = "cpu"` e capture o log de downgrade para que os operadores possam ensaiar o procedimento de reversão.【crates/fastpq_prover/src/backend.rs:308】【crates/iroha_config/src/parameters/user.rs:1357】
    | Captura de rastreamento (opcional) | Ao criar perfil, capture rastreamentos de despacho para que as substituições de pista/bloco do kernel possam ser revisadas posteriormente | Execute novamente um teste de paridade com `FASTPQ_METAL_TRACE=1 FASTPQ_GPU=gpu …` e anexe o log de rastreamento produzido aos seus artefatos de lançamento.【crates/fastpq_prover/src/metal.rs:346】【crates/fastpq_prover/src/backend.rs:208】

    Arquive as evidências com o tíquete de lançamento e espelhe a mesma lista de verificação em `docs/source/fastpq_migration_guide.md` para que as implementações de preparação/produção sigam um manual idêntico.【docs/source/fastpq_migration_guide.md:1】

### Liberar aplicação da lista de verificação

Adicione os seguintes portões a cada ticket de lançamento do FASTPQ. As liberações são bloqueadas até que todos os itens sejam
completos e anexados como artefatos assinados.

1. **Métricas de prova em subsegundos** — A captura canônica do benchmark Metal
   (`fastpq_metal_bench_*.json`) deve provar que a carga de trabalho de 20.000 linhas (32.768 linhas preenchidas) termina em
   <1s. Concretamente, a entrada `benchmarks.operations` onde `operation = "lde"` e o correspondente
   A amostra `report.operations` deve mostrar `gpu_mean_ms ≤ 950`. As execuções que excedem o teto exigem
   investigação e uma recaptura antes que a lista de verificação possa ser assinada.
2. **Manifesto de benchmark assinado** — Depois de gravar novos pacotes Metal + CUDA, execute
   `cargo xtask fastpq-bench-manifest … --signing-key <path>` para emitir
   `artifacts/fastpq_bench_manifest.json` e a assinatura separada
   (`artifacts/fastpq_bench_manifest.sig`). Anexe os dois arquivos mais a impressão digital da chave pública ao
   liberar ticket para que os revisores possam verificar o resumo e a assinatura de forma independente.【xtask/src/fastpq.rs:1】
3. **Anexos de evidências** — Armazene o benchmark bruto JSON, log stdout (ou rastreamento de instrumentos, quando
   capturado) e o par manifesto/assinatura com o ticket de liberação. A lista de verificação é apenas
   considerado verde quando o ticket está vinculado a esses artefatos e o revisor de plantão confirma o
   o resumo gravado em `fastpq_bench_manifest.json` corresponde aos arquivos enviados.【artifacts/fastpq_benchmarks/README.md:1】

## Etapa 6 – Proteção e Documentação
- Backend de espaço reservado retirado; o pipeline de produção é enviado por padrão sem alternância de recursos.
- Construções reproduzíveis (conjuntos de ferramentas de pinos, imagens de contêiner).
- Fuzzers para estruturas de rastreamento, SMT e pesquisa.
- Os testes de fumaça em nível de provador cobrem concessões de votação de governança e transferências de remessas para manter os equipamentos do Stage6 estáveis antes das implementações completas do IVM.【crates/fastpq_prover/tests/realistic_flows.rs:1】
- Runbooks com limites de alerta, procedimentos de remediação, diretrizes de planejamento de capacidade.
- Reprodução de prova entre arquiteturas (x86_64, ARM64) em CI.

### Manifesto do banco e portão de lançamento

As evidências de lançamento agora incluem um manifesto determinístico cobrindo tanto Metal quanto
Pacotes de benchmark CUDA. Execute:

```bash
cargo xtask fastpq-bench-manifest \
  --bench metal=artifacts/fastpq_benchmarks/fastpq_metal_bench_<date>_macos14_arm64.json \
  --bench cuda=artifacts/fastpq_benchmarks/fastpq_cuda_bench_<date>_sm80.json \
  --matrix artifacts/fastpq_benchmarks/matrix/matrix_manifest.json \
  --signing-key secrets/fastpq_bench.ed25519 \
  --out artifacts/fastpq_bench_manifest.json
```O comando valida os pacotes empacotados, impõe limites de latência/aceleração,
emite resumos BLAKE3 + SHA-256 e (opcionalmente) assina o manifesto com um
Chave Ed25519 para que as ferramentas de liberação possam verificar a procedência. Veja
`xtask/src/fastpq.rs`/`xtask/src/main.rs` para a implementação e
`artifacts/fastpq_benchmarks/README.md` para orientação operacional.

> **Nota:** Feixes de metal que omitem `benchmarks.poseidon_microbench` agora causam
> a geração do manifesto falha. Execute novamente `scripts/fastpq/wrap_benchmark.py`
> (e `scripts/fastpq/export_poseidon_microbench.py` se você precisar de um autônomo
> resumo) sempre que a evidência de Poseidon estiver faltando, então libere manifestos
> sempre capture a comparação escalar versus padrão.【xtask/src/fastpq.rs:409】

O sinalizador `--matrix` (padrão para `artifacts/fastpq_benchmarks/matrix/matrix_manifest.json`
quando presente) carrega as medianas entre dispositivos capturadas por
`scripts/fastpq/capture_matrix.sh`. O manifesto codifica o piso de 20.000 linhas e
limites de latência/aceleração por operação para cada classe de dispositivo, de forma personalizada
As substituições `--require-rows`/`--max-operation-ms`/`--min-operation-speedup` não são
não é mais necessário, a menos que você esteja depurando uma regressão específica.

Atualize a matriz anexando caminhos de benchmark agrupados ao
Listas `artifacts/fastpq_benchmarks/matrix/devices/<label>.txt` e execução
`scripts/fastpq/capture_matrix.sh`. O script captura as medianas por dispositivo,
emite o `matrix_manifest.json` consolidado e imprime o caminho relativo que
`cargo xtask fastpq-bench-manifest` irá consumir. O AppleM4, Xeon + RTX e
Listas de captura Neoverse+MI300 (`devices/apple-m4-metal.txt`,
`devices/xeon-rtx-sm80.txt`, `devices/neoverse-mi300.txt`) mais seus embrulhados
pacotes de referência
(`fastpq_metal_bench_2025-11-07T123018Z_macos14_arm64.json`,
`fastpq_cuda_bench_2025-11-12T090501Z_ubuntu24_x86_64.json`,
`fastpq_opencl_bench_2025-11-18T074455Z_ubuntu24_aarch64.json`) agora estão verificados
em, então cada versão impõe as mesmas medianas entre dispositivos antes do manifesto
é assinado.【artifacts/fastpq_benchmarks/matrix/devices/apple-m4-metal.txt:1】【artifacts/fastpq_benchmarks/matrix/devices/x eon-rtx-sm80.txt:1】【artifacts/fastpq_benchmarks/matrix/devices/neoverse-mi300.txt:1】【artifacts/fastpq_benchmarks/fastp q_metal_bench_2025-11-07T123018Z_macos14_arm64.json:1】【artifacts/fastpq_benchmarks/fastpq_cuda_bench_2025-11-12T09050 1Z_ubuntu24_x86_64.json:1】【artifacts/fastpq_benchmarks/fastpq_opencl_bench_2025-11-18T074455Z_ubuntu24_aarch64.json:1】

---

## Resumo da crítica e ações abertas

## Etapa 7 — Evidências de adoção e implementação da frota

Stage7 leva o provador de “documentado e comparado” (Stage6) para
“pronto para frotas de produção”. O foco está na ingestão de telemetria,
paridade de captura entre dispositivos e pacotes de evidências do operador para aceleração da GPU
pode ser mandatado de forma determinística.- **Etapa 7-1 — Ingestão de telemetria de frota e SLOs.** Painéis de produção
  (`dashboards/grafana/fastpq_acceleration.json`) deve ser conectado à tensão
  Prometheus/OTel alimenta com cobertura Alertmanager para paralisações de profundidade de fila,
  regressões de preenchimento zero e substitutos silenciosos de CPU. O pacote de alerta permanece abaixo
  `dashboards/alerts/fastpq_acceleration_rules.yml` e alimenta a mesma evidência
  pacote necessário no Stage6.【dashboards/grafana/fastpq_acceleration.json:1】【dashboards/alerts/fastpq_acceleration_rules.yml:1】
  O painel agora expõe variáveis de modelo para `device_class`, `chip_family`,
  e `gpu_kind`, permitindo que as operadoras dinamizem a adoção do Metal pela matriz exata
  rótulo (por exemplo, `apple-m4-max`), por família de chips Apple ou por chip discreto vs.
  classes de GPU integradas sem editar as consultas.
  Nós macOS criados com `irohad --features fastpq-gpu` agora emitem
  `fastpq_execution_mode_total{device_class,chip_family,gpu_kind,...}`,
  `fastpq_metal_queue_ratio{device_class,chip_family,gpu_kind,queue,metric}`
  (proporções de ocupação/sobreposição) e
  `fastpq_metal_queue_depth{device_class,chip_family,gpu_kind,metric}`
  (limit, max_in_flight, dispatch_count, window_seconds) para que os painéis e
  As regras do Alertmanager podem ler o ciclo de trabalho/headroom do semáforo de metal diretamente do
  Prometheus sem esperar por um pacote de benchmark. Hosts agora exportam
  `fastpq_zero_fill_duration_ms{device_class,chip_family,gpu_kind}` e
  `fastpq_zero_fill_bandwidth_gbps{device_class,chip_family,gpu_kind}` sempre que
  o auxiliar LDE zera os buffers de avaliação da GPU e o Alertmanager ganhou o
  `FastpqQueueHeadroomLow` (altura livre  0,40 ms acima de 15 m) regras para que haja espaço na fila e
  operadores de página de regressões de preenchimento zero imediatamente, em vez de esperar pelo
  próximo benchmark empacotado. Um novo alerta de nível de página `FastpqCpuFallbackBurst` rastreia
  Solicitações de GPU que chegam ao back-end da CPU para mais de 5% da carga de trabalho,
  forçando os operadores a capturar evidências e causar falhas transitórias de GPU
  antes de tentar novamente a implementação.【crates/irohad/src/main.rs:2345】【crates/iroha_telemetry/src/metrics.rs:4436】【dashboards/alerts/fastpq_acceleration_rules.yml:1】【dashboards/alerts/tests/fastpq_acceleration_rules.test.yml:1】
  O SLO definido agora também impõe a meta de ciclo de trabalho de ≥50% do metal por meio do
  Regra `FastpqQueueDutyCycleDrop`, que calcula a média
  `fastpq_metal_queue_ratio{metric="busy"}` em uma janela contínua de 15 minutos e
  avisa sempre que o trabalho da GPU ainda está sendo agendado, mas uma fila não consegue manter o
  ocupação necessária. Isso mantém o contrato de telemetria ao vivo alinhado com o
  evidência de benchmark antes que as faixas de GPU sejam obrigatórias.【dashboards/alerts/fastpq_acceleration_rules.yml:1】【dashboards/alerts/tests/fastpq_acceleration_rules.test.yml:1】
- **Estágio 7-2 — Matriz de captura entre dispositivos.** O novo
  Compilações `scripts/fastpq/capture_matrix.sh`
  `artifacts/fastpq_benchmarks/matrix/matrix_manifest.json` por dispositivo
  listas de captura em `artifacts/fastpq_benchmarks/matrix/devices/`. AppleM4,
  As medianas Xeon + RTX e Neoverse + MI300 agora vivem no repositório junto com seus
  pacotes agrupados, então `cargo xtask fastpq-bench-manifest` carrega o manifesto
  automaticamente, impõe o limite mínimo de 20.000 linhas e aplica por dispositivo
  limites de latência/aceleração sem sinalizadores CLI personalizados antes que um pacote de lançamento sejaaprovado.【scripts/fastpq/capture_matrix.sh:1】【artifacts/fastpq_benchmarks/matrix/matrix_manifest.json:1】【artifacts/fastpq_benchmarks/matrix/devices/apple-m4-met al.txt:1】【artifacts/fastpq_benchmarks/matrix/devices/xeon-rtx-sm80.txt:1】【artifacts/fastpq_benchmarks/matrix/devices/neoverse-mi300.txt:1】【xtask/src/fastpq.rs:1】
Motivos de instabilidade agregada agora acompanham a matriz: passar
`--reason-summary-out` a `scripts/fastpq/geometry_matrix.py` para emitir um
Histograma JSON de causas de falha/aviso codificadas por rótulo de host e origem
resumo, para que os revisores do Stage7-2 possam ver substitutos de CPU ou falta de telemetria em
uma olhada sem examinar a tabela Markdown completa. O mesmo ajudante agora
aceita `--host-label chip_family:Chip` (repita para várias chaves) para que o
As saídas Markdown/JSON incluem colunas de rótulos de host selecionadas em vez de enterrar
esses metadados no resumo bruto, tornando trivial filtrar compilações de sistema operacional ou
Versões do driver Metal ao compilar o pacote de evidências Stage7-2.【scripts/fastpq/geometry_matrix.py:1】
As varreduras de geometria também carimbam os campos ISO8601 `started_at` / `completed_at` no
resultados de resumo, CSV e Markdown para que os pacotes de captura possam provar a janela para
cada host quando as matrizes do Stage7-2 mesclam várias execuções de laboratório.【scripts/fastpq/launch_geometry_sweep.py:1】
`scripts/fastpq/stage7_bundle.py` agora une a matriz geométrica com
Instantâneos `row_usage/*.json` em um único pacote Stage7 (`stage7_bundle.json`
+ `stage7_geometry.md`), validando taxas de transferência via
`validate_row_usage_snapshot.py` e resumos persistentes de host/env/motivo/fonte
para que os tickets de lançamento possam anexar um artefato determinístico em vez de fazer malabarismos
tabelas por host.【scripts/fastpq/stage7_bundle.py:1】【scripts/fastpq/validate_row_usage_snapshot.py:1】
- **Etapa 7-3 — Evidências de adoção do operador e exercícios de reversão.** O novo
  `docs/source/fastpq_rollout_playbook.md` descreve o pacote de artefatos
  (`fastpq_bench_manifest.json`, capturas de Metal/CUDA embrulhadas, exportação Grafana,
  Instantâneo do Alertmanager, logs de reversão) que devem acompanhar cada ticket de implementação
  além do cronograma escalonado (piloto → rampa → padrão) e exercícios de fallback forçados.
  `ci/check_fastpq_rollout.sh` valida esses pacotes para que o CI aplique o Stage7
  portão antes que os lançamentos avancem. O pipeline de lançamento agora pode extrair o mesmo
  agrupados em `artifacts/releases/<version>/fastpq_rollouts/…` via
  `scripts/run_release_pipeline.py --fastpq-rollout-bundle <path>`, garantindo o
  manifestos assinados e evidências de implementação permanecem juntos. Um pacote de referência vive
  sob `artifacts/fastpq_rollouts/20250215T101500Z/fleet-alpha/canary/` para manter
  o fluxo de trabalho do GitHub (`.github/workflows/fastpq-rollout.yml`) verde enquanto real
  os envios de lançamento são revisados.

### Distribuição da fila Stage7 FFT`crates/fastpq_prover/src/metal.rs` agora instancia um `QueuePolicy` que
gera automaticamente várias filas de comando do Metal sempre que o host relata um
GPU discreta. GPUs integradas mantêm o caminho de fila única
(`MIN_QUEUE_FANOUT = 1`), enquanto os dispositivos discretos têm como padrão duas filas e apenas
distribua-se quando uma carga de trabalho abrange pelo menos 16 colunas. Ambas as heurísticas podem ser ajustadas
através do novo `FASTPQ_METAL_QUEUE_FANOUT` e `FASTPQ_METAL_COLUMN_THRESHOLD`
variáveis de ambiente e o agendador faz round-robins em lotes FFT/LDE em todo o
filas ativas antes de emitir o despacho pós-tiling emparelhado na mesma fila
para preservar as garantias de pedido.【crates/fastpq_prover/src/metal.rs:620】【crates/fastpq_prover/src/metal.rs:772】【crates/fastpq_prover/src/metal.rs:900】
Os operadores de nós não precisam mais exportar esses env vars manualmente: o
O perfil `iroha_config` expõe `fastpq.metal_queue_fanout` e
`fastpq.metal_queue_column_threshold` e `irohad` os aplicam via
`fastpq_prover::set_metal_queue_policy` antes que o backend do Metal seja inicializado, então
os perfis da frota permanecem reproduzíveis sem embalagens de lançamento personalizadas.【crates/irohad/src/main.rs:1879】【crates/fastpq_prover/src/lib.rs:60】
Os lotes FFT inversos agora ficam em uma única fila sempre que a carga de trabalho apenas
atinge o limite de distribuição (por exemplo, a captura balanceada de faixa de 16 colunas), que
restaura ≥1,0× paridade para WP2-D enquanto deixa FFT/LDE/Poseidon de coluna grande
despachos no caminho multi-fila.【crates/fastpq_prover/src/metal.rs:2018】

Os testes auxiliares exercitam os limites da política de fila e a validação do analisador para que o CI possa
provar a heurística Stage7 sem exigir hardware GPU em cada construtor,
e os testes específicos da GPU forçam substituições de distribuição para manter a cobertura de repetição em
sincronizar com os novos padrões.【crates/fastpq_prover/src/metal.rs:2163】【crates/fastpq_prover/src/metal.rs:2236】

### Etiquetas de dispositivos Stage7-1 e contrato de alerta

`scripts/fastpq/wrap_benchmark.py` agora investiga `system_profiler` na captura do macOS
hospeda e registra rótulos de hardware em cada benchmark empacotado para que a telemetria da frota
e a matriz de captura pode girar por dispositivo sem planilhas personalizadas. Um
A captura de metal de 20.000 linhas agora traz entradas como:

```json
"labels": {
  "device_class": "apple-m4-pro",
  "chip_family": "m4",
  "chip_bin": "pro",
  "gpu_kind": "integrated",
  "gpu_vendor": "apple",
  "gpu_bus": "builtin",
  "gpu_model": "Apple M4 Pro"
}
```

Esses rótulos são ingeridos junto com `benchmarks.zero_fill_hotspots` e
`benchmarks.metal_dispatch_queue` então o instantâneo Grafana, matriz de captura
(`artifacts/fastpq_benchmarks/matrix/devices/<label>.txt`) e Alertmanager
todas as evidências concordam com a classe de hardware que produziu as métricas. O
O sinalizador `--label` ainda permite substituições manuais quando falta um host de laboratório
`system_profiler`, mas os identificadores testados automaticamente agora cobrem AppleM1–M4 e
GPUs PCIe discretas prontas para uso.【scripts/fastpq/wrap_benchmark.py:1】

As capturas do Linux recebem o mesmo tratamento: `wrap_benchmark.py` agora inspeciona
`/proc/cpuinfo`, `nvidia-smi`/`rocm-smi` e `lspci` para que CUDA e OpenCL sejam executados
derivar `cpu_model`, `gpu_model` e um `device_class` canônico (`xeon-rtx-sm80`
para o host Stage7 CUDA, `neoverse-mi300` para o laboratório MI300A). Os operadores podem
ainda substituem os valores detectados automaticamente, mas os pacotes de evidências do Stage7 não
exigem edições manuais para marcar capturas Xeon/Neoverse com o dispositivo correto
metadados.Em tempo de execução, cada host define `fastpq.device_class`, `fastpq.chip_family` e
`fastpq.gpu_kind` (ou as variáveis de ambiente `FASTPQ_*` correspondentes) para o
mesmos rótulos de matriz que aparecem no pacote de captura para exportação Prometheus
`fastpq_execution_mode_total{device_class="…",chip_family="…",gpu_kind="…"}` e
o painel de aceleração FASTPQ pode filtrar por qualquer um dos três eixos. O
As regras do Alertmanager são agregadas no mesmo conjunto de rótulos, permitindo que os operadores criem gráficos
adoção, downgrades e fallbacks por perfil de hardware em vez de um único
proporção para toda a frota.【crates/iroha_config/src/parameters/user.rs:1224】【dashboards/grafana/fastpq_acceleration.json:1】【dashboards/alerts/fastpq_acceleration_rules.yml:1】

O contrato de alerta/SLO de telemetria agora vincula as métricas capturadas ao Stage7
portões. A tabela abaixo resume os sinais e pontos de aplicação:

| Sinal | Fonte | Alvo / Gatilho | Aplicação |
| ------ | ------ | ---------------- | ----------- |
| Taxa de adoção de GPU | Prometheus `fastpq_execution_mode_total{requested=~"auto|gpu", device_class="…", chip_family="…", gpu_kind="…", backend="metal"}` | ≥95% das resoluções por (device_class, chip_family, gpu_kind) devem chegar a `resolved="gpu", backend="metal"`; página quando qualquer trigêmeo cair abaixo de 50% em 15 milhões | Alerta `FastpqMetalDowngrade` (página)【dashboards/alerts/fastpq_acceleration_rules.yml:1】 |
| Lacuna de back-end | Prometheus `fastpq_execution_mode_total{backend="none", device_class="…", chip_family="…", gpu_kind="…"}` | Deve permanecer em 0 para cada trigêmeo; avisar após quaisquer rajadas sustentadas (>10m) | Alerta `FastpqBackendNoneBurst` (aviso)【dashboards/alerts/fastpq_acceleration_rules.yml:21】 |
| Taxa de fallback da CPU | Prometheus `fastpq_execution_mode_total{requested=~"auto|gpu", backend="cpu", device_class="…", chip_family="…", gpu_kind="…"}` | ≤5% das provas solicitadas pela GPU podem chegar ao back-end da CPU para qualquer trio; página quando um trio excede 5% para ≥10m | Alerta `FastpqCpuFallbackBurst` (página)【dashboards/alerts/fastpq_acceleration_rules.yml:32】 |
| Ciclo de trabalho da fila de metal | Prometheus `fastpq_metal_queue_ratio{metric="busy", device_class="…", chip_family="…", gpu_kind="…"}` | A média móvel de 15m deve permanecer ≥50% sempre que os trabalhos de GPU estiverem na fila; avisa quando a utilização cai abaixo da meta enquanto as solicitações de GPU persistem | Alerta `FastpqQueueDutyCycleDrop` (aviso)【dashboards/alerts/fastpq_acceleration_rules.yml:98】 |
| Profundidade da fila e orçamento de preenchimento zero | Blocos de referência empacotados `metal_dispatch_queue` e `zero_fill_hotspots` | `max_in_flight` deve permanecer pelo menos um slot abaixo de `limit` e a média de preenchimento zero do LDE deve permanecer ≤0,4ms (≈80GB/s) para o rastreamento canônico de 20.000 linhas; qualquer regressão bloqueia o pacote de implementação | Revisado por meio da saída `scripts/fastpq/wrap_benchmark.py` e anexado ao pacote de evidências Stage7 (`docs/source/fastpq_rollout_playbook.md`). |
| Espaço livre na fila de tempo de execução | Prometheus `fastpq_metal_queue_depth{metric="limit|max_in_flight", device_class="…", chip_family="…", gpu_kind="…"}` | `limit - max_in_flight ≥ 1` para cada trigêmeo; avisar após 10m sem altura livre | Alerta `FastpqQueueHeadroomLow` (aviso)【dashboards/alerts/fastpq_acceleration_rules.yml:41】 |
| Latência de preenchimento zero em tempo de execução | Prometheus `fastpq_zero_fill_duration_ms{device_class="…", chip_family="…", gpu_kind="…"}` | A última amostra de preenchimento zero deve permanecer ≤0,40 ms (limite do Estágio 7) | Alerta `FastpqZeroFillRegression` (página)【dashboards/alerts/fastpq_acceleration_rules.yml:58】 |O wrapper impõe a linha de preenchimento zero diretamente. Passe
`--require-zero-fill-max-ms 0.40` a `scripts/fastpq/wrap_benchmark.py` e
falhará quando o JSON do banco não tiver telemetria de preenchimento zero ou quando o mais quente
a amostra de preenchimento zero excede o orçamento do Stage7, evitando que pacotes de lançamento sejam
envio sem as evidências obrigatórias.【scripts/fastpq/wrap_benchmark.py:1008】

#### Lista de verificação de tratamento de alertas Stage7-1

Cada alerta listado acima alimenta um exercício de plantão específico para que os operadores reúnam os
mesmos artefatos que o pacote de lançamento exige:

1. **`FastpqQueueHeadroomLow` (aviso).** Execute uma consulta Prometheus instantânea
   para `fastpq_metal_queue_depth{metric=~"limit|max_in_flight",device_class="<matrix>"}` e
   capture o painel Grafana “Queue headroom” do `fastpq-acceleration`
   placa. Registre o resultado da consulta em
   `artifacts/fastpq_rollouts/<stamp>/<fleet>/<lane>/metrics_headroom.prom`
   junto com o ID do alerta para que o pacote de lançamento prove que o aviso foi
   reconhecido antes que a fila morresse de fome.【dashboards/grafana/fastpq_acceleration.json:1】
2. **`FastpqZeroFillRegression` (página).** Inspecione
   `fastpq_zero_fill_duration_ms{device_class="<matrix>"}` e, se a métrica for
   barulhento, execute novamente `scripts/fastpq/wrap_benchmark.py` no banco JSON mais recente
   para atualizar o bloco `zero_fill_hotspots`. Anexe a saída do promQL,
   capturas de tela e arquivo de bancada atualizado para o diretório de implementação; isso cria
   a mesma evidência que `ci/check_fastpq_rollout.sh` espera durante o lançamento
   validação.【scripts/fastpq/wrap_benchmark.py:1】【ci/check_fastpq_rollout.sh:1】
3. **`FastpqCpuFallbackBurst` (página).** Confirme
   `fastpq_execution_mode_total{requested="gpu",backend="cpu"}` excede 5%
   floor e, em seguida, amostra dos logs `irohad` para as mensagens de downgrade correspondentes
   (`telemetry::fastpq.execution_mode resolved="cpu"`). Armazene o dump do promQL
   além de trechos de log em `metrics_cpu_fallback.prom`/`rollback_drill.log` para que o
   O pacote demonstra tanto o impacto quanto o reconhecimento do operador.
4. **Embalagem de evidências.** Depois que qualquer alerta for eliminado, execute novamente as etapas do Estágio 7-3 em
   o manual de implementação (exportação Grafana, instantâneo de alerta, análise de reversão) e
   revalidar o pacote via `ci/check_fastpq_rollout.sh` antes de reconectá-lo
   para o ticket de lançamento.【docs/source/fastpq_rollout_playbook.md:114】

Operadores que preferem automação podem operar
`scripts/fastpq/capture_alert_evidence.sh --device-class <label> --out <bundle-dir>`
para consultar a API Prometheus para o headroom da fila, preenchimento zero e substituto da CPU
métricas listadas acima; o auxiliar grava o JSON capturado (prefixado com o
promQL original) em `metrics_headroom.prom`, `metrics_zero_fill.prom` e
`metrics_cpu_fallback.prom` no diretório de implementação escolhido para que esses arquivos
pode ser anexado ao pacote sem invocações manuais de curl.`ci/check_fastpq_rollout.sh` agora impõe o headroom da fila e o preenchimento zero
orçamento diretamente. Ele analisa cada bancada `metal` referenciada por
`fastpq_bench_manifest.json`, inspeciona
`benchmarks.metal_dispatch_queue.{limit,max_in_flight}` e
`benchmarks.zero_fill_hotspots[]` e falha no pacote quando o headroom cai
abaixo de um slot ou quando qualquer ponto de acesso LDE reportar `mean_ms > 0.40`. Isto mantém o
Guarda de telemetria Stage7 em CI, correspondendo à revisão manual realizada no
Instantâneo Grafana e evidência de liberação.【ci/check_fastpq_rollout.sh#L1】
Como parte da mesma validação, o script agora insiste que todos os pacotes
benchmark carrega os rótulos de hardware detectados automaticamente (`metadata.labels.device_class`
e `metadata.labels.gpu_kind`). Pacotes sem esses rótulos falham imediatamente,
garantindo que os artefatos de lançamento, manifestos de matriz Stage7-2 e tempo de execução
todos os painéis referem-se exatamente aos mesmos nomes de classe de dispositivo.

O painel “Latest Benchmark” Grafana e o pacote de implementação associado agora citam o
`device_class`, orçamento sem preenchimento e instantâneo da profundidade da fila para engenheiros de plantão
pode correlacionar a telemetria de produção com a classe de captura exata usada durante a sinalização
desligado. As entradas futuras da matriz herdam os mesmos rótulos, ou seja, o dispositivo Stage7-2
listas e os painéis Prometheus compartilham um único esquema de nomenclatura para AppleM4,
M3 Max e futuras capturas MI300/RTX.

### Runbook de telemetria da frota Stage7-1

Siga esta lista de verificação antes de ativar as pistas de GPU por padrão para que a telemetria da frota
e as regras do Alertmanager refletem as mesmas evidências capturadas durante a preparação da versão:

1. **Captura de rótulo e hosts de tempo de execução.** `python3 scripts/fastpq/wrap_benchmark.py`
   já emite `metadata.labels.device_class`, `chip_family` e `gpu_kind`
   para cada JSON empacotado. Mantenha esses rótulos sincronizados com
   `fastpq.{device_class,chip_family,gpu_kind}` (ou o
   `FASTPQ_{DEVICE_CLASS,CHIP_FAMILY,GPU_KIND}` env vars) dentro de `iroha_config`
   então as métricas de tempo de execução são publicadas
   `fastpq_execution_mode_total{device_class="…",chip_family="…",gpu_kind="…"}`
   e os medidores `fastpq_metal_queue_*` com os mesmos identificadores que aparecem
   em `artifacts/fastpq_benchmarks/matrix/devices/*.txt`. Ao encenar um novo
   classe, regenerar o manifesto da matriz via
   `scripts/fastpq/capture_matrix.sh --devices artifacts/fastpq_benchmarks/matrix/devices`
   para que o CI e os painéis entendam o rótulo adicional.
2. **Verifique os medidores de fila e as métricas de adoção.** Execute `irohad --features fastpq-gpu`
   nos hosts Metal e raspar o endpoint de telemetria para confirmar a fila ao vivo
   medidores estão exportando:

   ```bash
   curl -sf http://$IROHA_PROM/metrics | rg 'fastpq_metal_queue_(ratio|depth)'
   curl -sf http://$IROHA_PROM/metrics | rg 'fastpq_execution_mode_total'
   ```O primeiro comando prova que o amostrador de semáforo está emitindo o `busy`,
   Série `overlap`, `limit` e `max_in_flight` e o segundo mostra se
   cada classe de dispositivo está resolvendo para `backend="metal"` ou voltando para
   `backend="cpu"`. Conecte o alvo de raspagem através de Prometheus/OTel antes
   importando o painel para que Grafana possa traçar a visualização da frota imediatamente.
3. **Instale o painel + pacote de alertas.** Importar
   `dashboards/grafana/fastpq_acceleration.json` em Grafana (guarde o
   variáveis de modelo de classe de dispositivo, família de chips e tipo de GPU integradas) e carga
   `dashboards/alerts/fastpq_acceleration_rules.yml` no Alertmanager juntos
   com seu dispositivo de teste de unidade. O pacote de regras vem com um chicote `promtool`; correr
   `promtool test rules dashboards/alerts/tests/fastpq_acceleration_rules.test.yml`
   sempre que as regras mudam para provar `FastpqMetalDowngrade` e
   `FastpqBackendNoneBurst` ainda dispara nos limites documentados.
4. **Gate liberações com o pacote de evidências.** Mantenha
   `docs/source/fastpq_rollout_playbook.md` útil ao gerar uma implementação
   envio para que cada pacote carregue os benchmarks agrupados, exportação Grafana,
   pacote de alertas, prova de telemetria de fila e logs de reversão. A CI já aplica o
   contrato: `make check-fastpq-rollout` (ou invocando
   `ci/check_fastpq_rollout.sh --bundle <path>`) valida o pacote configurável, executa novamente
   os testes de alerta e se recusa a assinar quando há espaço livre na fila ou preenchimento zero
   os orçamentos regridem.
5. **Anexar alertas à correção.** Quando o Alertmanager paginar, use o Grafana
   placa e os contadores Prometheus brutos da etapa 2 para confirmar se
   downgrades resultam de falta de fila, fallbacks de CPU ou rajadas de backend=none.
O runbook vive em
este documento mais `docs/source/fastpq_rollout_playbook.md`; atualize o
liberar ticket com o `fastpq_execution_mode_total` relevante,
Trechos `fastpq_metal_queue_ratio` e `fastpq_metal_queue_depth` juntos
com links para o painel Grafana e o instantâneo do alerta para que os revisores possam ver
exatamente qual SLO foi acionado.

### WP2-E — Instantâneo de perfil de metal estágio por estágio

`scripts/fastpq/src/bin/metal_profile.rs` resume as capturas de metal embrulhadas
para que a meta abaixo de 900 ms possa ser rastreada ao longo do tempo (executar
`cargo run --manifest-path scripts/fastpq/Cargo.toml --bin metal_profile -- <capture.json>`).
O novo ajudante Markdown
`scripts/fastpq/metal_capture_summary.py fastpq_metal_bench_20k_latest.json --label "20k snapshot (pre-override)"`
gera as tabelas de estágio abaixo (imprime o Markdown junto com um texto
resumo para que os tickets WP2-E possam incorporar a evidência literalmente). Duas capturas são rastreadas
agora:

> **Nova instrumentação WP2-E:** `fastpq_metal_bench --gpu-probe ...` agora emite um
> instantâneo de detecção (modo de execução solicitado/resolvido, `FASTPQ_GPU`
> substituições, back-end detectado e os IDs de registro/dispositivos Metal enumerados)
> antes de qualquer kernel ser executado. Capture este log sempre que uma GPU forçada ainda estiver em execução
> volta para o caminho da CPU para que o pacote de evidências registre que os hosts veem
> `MTLCopyAllDevices` retorna zero e quais substituições estavam em vigor durante o
> benchmark.【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:603】【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:2616】> **Ajudante de captura de palco:** `cargo xtask fastpq-stage-profile --trace --out-dir artifacts/fastpq_stage_profiles/<label>`
> agora aciona `fastpq_metal_bench` para FFT, LDE e Poseidon individualmente,
> armazena as saídas JSON brutas em diretórios por estágio e emite um único
> Pacote `stage_profile_summary.json` que registra tempos de CPU/GPU, profundidade da fila
> telemetria, estatísticas de preparação de colunas, perfis de kernel e rastreamento associado
> artefatos. Passe `--stage fft --stage lde --stage poseidon` para direcionar um subconjunto,
> `--trace-template "Metal System Trace"` para escolher um modelo xctrace específico,
> e `--trace-dir` para rotear pacotes configuráveis `.trace` para um local compartilhado. Anexe o
> resumo JSON mais os arquivos de rastreamento gerados para cada edição do WP2-E para revisores
> pode diferenciar a ocupação da fila (`metal_dispatch_queue.*`), taxas de sobreposição e o
> capturou a geometria de lançamento em execuções sem espeleologia manual de múltiplas
> Invocações `fastpq_metal_bench`.【xtask/src/fastpq.rs:721】【xtask/src/main.rs:3187】

> **Ajudante de evidência de fila/preparação (09/05/2026):** `scripts/fastpq/profile_queue.py` agora
> ingere uma ou mais capturas JSON `fastpq_metal_bench` e emite uma tabela Markdown e
> um resumo legível por máquina (`--markdown-out/--json-out`) para profundidade da fila, taxas de sobreposição e
> a telemetria de teste do lado do host pode acompanhar todos os artefatos WP2-E. Correndo
> `python3 scripts/fastpq/profile_queue.py fastpq_metal_bench_poseidon.json fastpq_metal_bench_20k_new.json --json-out artifacts/fastpq_benchmarks/fastpq_queue_profile_20260509.json` produziu a tabela abaixo e sinalizou que as capturas de Metal arquivadas ainda reportam
> `dispatch_count = 0` e `column_staging.batches = 0`—WP2-E.1 permanece aberto até o Metal
> a instrumentação é reconstruída com a telemetria habilitada. Os artefatos JSON/Markdown gerados estão ativos
> sob `artifacts/fastpq_benchmarks/fastpq_queue_profile_20260509.{json,md}` para auditoria.
> O auxiliar agora (19/05/2026) também apresenta a telemetria do pipeline Poseidon (`pipe_depth`,
> `batches`, `chunk_columns` e `fallbacks`) dentro da tabela Markdown e do resumo JSON,
> para que os revisores do WP2-E.4/6 possam provar se a GPU permaneceu no caminho do pipeline e se algum
> fallbacks ocorreram sem abrir a captura bruta.【scripts/fastpq/profile_queue.py:1】> **Resumidor do perfil do estágio (30/05/2026):** `scripts/fastpq/stage_profile_report.py` consome
> o pacote `stage_profile_summary.json` emitido por `cargo xtask fastpq-stage-profile` e
> renderiza resumos Markdown e JSON para que os revisores do WP2-E possam copiar evidências em tickets
> sem transcrever manualmente os tempos. Invocar
> `python3 scripts/fastpq/stage_profile_report.py artifacts/fastpq_stage_profiles/<stamp>/stage_profile_summary.json --label "m3-lab" --markdown-out artifacts/fastpq_stage_profiles/<stamp>/stage_profile_summary.md --json-out artifacts/fastpq_stage_profiles/<stamp>/stage_profile_summary.jsonl`
> para produzir tabelas determinísticas listando médias de GPU/CPU, deltas de aceleração, cobertura de rastreamento e
> lacunas de telemetria por etapa. A saída JSON espelha a tabela e registra tags de problemas por estágio
> (`trace missing`, `queue telemetry missing`, etc.) para que a automação de governança possa diferenciar o host
> execuções referenciadas em WP2-E.1 a WP2-E.6.
> **Proteção de sobreposição de host/dispositivo (04/06/2026):** `scripts/fastpq/profile_queue.py` agora anota
> Taxas de espera FFT/LDE/Poseidon junto com os totais de milissegundos de nivelamento/espera por estágio e emite um
> problema sempre que `--max-wait-ratio <threshold>` detecta sobreposição ruim. Usar
> `python3 scripts/fastpq/profile_queue.py --max-wait-ratio 0.20 fastpq_metal_bench_20k_latest.json --markdown-out artifacts/fastpq_benchmarks/<stamp>/queue.md`
> para capturar a tabela Markdown e o pacote JSON com taxas de espera explícitas para tickets WP2-E.5
> pode mostrar se a janela de buffer duplo manteve a GPU alimentada. A saída do console em texto simples também
> lista as proporções por fase para facilitar as investigações de plantão.
> **Guarda de telemetria + status de execução (09/06/2026):** `fastpq_metal_bench` agora emite um bloco `run_status`
> (rótulo de back-end, contagem de despacho, motivos) e o novo sinalizador `--require-telemetry` falha na execução
> sempre que faltam temporizações de GPU ou telemetria de fila/preparação. `profile_queue.py` renderiza a execução
> status como uma coluna dedicada e apresenta estados não `ok` na lista de problemas, e
> `launch_geometry_sweep.py` encadeia o mesmo estado em avisos/classificação para que as matrizes não possam
> admitem capturas que retornam silenciosamente à CPU ou ignoram a instrumentação da fila.
> **Ajuste automático Poseidon/LDE (12/06/2026):** `metal_config::poseidon_batch_multiplier()` agora é dimensionado
> com as dicas do conjunto de trabalho Metal e `lde_tile_stage_target()` aumenta a profundidade do bloco em GPUs discretas.
> O multiplicador aplicado e o limite de blocos estão incluídos no bloco `metal_heuristics` de
> Saídas `fastpq_metal_bench` e renderizadas por `scripts/fastpq/metal_capture_summary.py`, então WP2-E
> os pacotes registram os botões exatos do pipeline usados em cada captura sem pesquisar no JSON bruto.

| Etiqueta | Despacho | Ocupado | Sobreposição | Profundidade Máxima | FFT achatar | FFT espere | Espera FFT% | LDE achatar | LDE espere | Espera LDE% | Poseidon achatado | Poseidon espere | Poseidon espere % | Profundidade do tubo | Lotes de tubos | Reservas de tubos |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| fastpq_metal_bench_poseidon | 0 | 0,0% | 0,0% | 0 | – | – | – | – | – | – | – | – | – | – | – | – |
| fastpq_metal_bench_20k_new | 0 | 0,0% | 0,0% | 0 | – | – | – | – | – | – | – | – | – | – | – | – |

#### Instantâneo de 20k (pré-substituição)

`fastpq_metal_bench_20k_latest.json`| Palco | Colunas | Len de entrada | Média de GPU (ms) | Média de CPU (ms) | Compartilhamento de GPU | Aceleração | ΔCPU (ms) |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| FFT | 16 | 32768 | 130,986ms (115,761–167,755) | 112,616ms (95,335–132,929) | 2,4% | 0,860× | −18.370 |
| IFF | 16 | 32768 | 129,296ms (111,127–142,955) | 158,144ms (126,847–237,887) | 2,4% | 1,223× | +28.848 |
| LDE | 16 | 262144 | 1570,656ms (1544,397–1584,502) | 1752,523ms (1548,807–2191,930) | 29,2% | 1,116× | +181.867 |
| Poseidon | 16 | 524288 | 3548,329ms (3519,881–3576,041) | 3642,706ms (3539,055–3758,279) | 66,0% | 1,027× | +94.377 |

Principais observações:1. O total da GPU é de 5,379s, o que representa **4,48s acima** da meta de 900ms. Poseidon
   hashing ainda domina o tempo de execução (≈66%) com o kernel LDE em segundo
   local (≈29%), então o WP2-E precisa atacar tanto a profundidade do pipeline Poseidon quanto
   o plano de residência/lado a lado da memória LDE antes que os substitutos da CPU desapareçam.
2. A FFT permanece uma regressão (0,86×), embora a IFFT esteja >1,22× acima do escalar
   caminho. Precisamos de uma varredura na geometria de lançamento
   (`FASTPQ_METAL_{FFT,LDE}_COLUMNS` + `FASTPQ_METAL_QUEUE_FANOUT`) para entender
   se a ocupação da FFT pode ser recuperada sem prejudicar o já melhor
   Horários do IFFT. O auxiliar `scripts/fastpq/launch_geometry_sweep.py` agora dirige
   esses experimentos de ponta a ponta: passe substituições separadas por vírgulas (por exemplo,
   `--fft-columns 16,32 --queue-fanout 1,2` e
   `--poseidon-lanes auto,256`) e invocará
   `fastpq_metal_bench` para cada combinação, armazene as cargas JSON em
   `artifacts/fastpq_geometry/<timestamp>/` e persistir um pacote `summary.json`
   descrevendo as proporções de fila de cada execução, opções de inicialização FFT/LDE, tempos de GPU vs CPU,
   e os metadados do host (nome do host/rótulo, plataforma tripla, dispositivo detectado
   classe, fornecedor/modelo de GPU) para que as comparações entre dispositivos sejam determinísticas
   proveniência. O auxiliar agora também escreve `reason_summary.json` próximo ao
   resumo por padrão, usando o mesmo classificador da matriz geométrica para rolar
   até fallbacks de CPU e falta de telemetria. Use `--host-label staging-m3` para marcar
   capturas de laboratórios compartilhados.
   A ferramenta complementar `scripts/fastpq/geometry_matrix.py` agora ingere um ou
   mais pacotes de resumo (`--summary hostA/summary.json --summary hostB/summary.json`)
   e emite tabelas Markdown/JSON que rotulam cada formato de lançamento como *estável*
   (temporizações de GPU FFT/LDE/Poseidon capturadas) ou *instável* (tempo limite, fallback de CPU,
   back-end não metálico ou telemetria ausente) ao lado das colunas do host. O
   tabelas agora incluem o `execution_mode`/`gpu_backend` resolvido mais um
   Coluna `Reason`, portanto, substitutos de CPU e tempos de GPU ausentes são óbvios em
   Matrizes Stage7 mesmo quando blocos de tempo estão presentes; uma linha de resumo conta
   as execuções estáveis versus totais. Passe `--operation fft|lde|poseidon_hash_columns`
   quando a varredura precisa isolar um único estágio (por exemplo, para perfilar
   Poseidon separadamente) e mantenha `--extra-args` livre para sinalizadores específicos de bancada.
   O ajudante aceita qualquer
   prefixo de comando (padrão `cargo run … fastpq_metal_bench`) mais opcional
   `--halt-on-error` / `--timeout-seconds` protege para que os engenheiros de desempenho possam
   reproduzir a varredura em máquinas diferentes enquanto coleta comparáveis,
   pacotes de evidências de vários dispositivos para Stage7.
3. `metal_dispatch_queue` relatou `dispatch_count = 0`, portanto, ocupação da fila
   a telemetria estava faltando, embora os kernels da GPU estivessem em execução. O tempo de execução Metal agora usa
   adquirir/liberar barreiras para a preparação de fila/coluna alterna para que os threads de trabalho
   observe os sinalizadores de instrumentação e o relatório da matriz geométrica chama
   formas de inicialização instáveis sempre que os tempos de GPU FFT/LDE/Poseidon estão ausentes. Mantenha
   anexando a matriz Markdown/JSON aos tickets WP2-E para que os revisores possam ver
   quais combinações ainda falharão quando a telemetria da fila estiver disponível.O guarda `run_status` e o sinalizador `--require-telemetry` agora falham na captura
   sempre que os tempos da GPU estiverem faltando ou a telemetria de fila/preparação estiver ausente, então
   As execuções de dispatch_count=0 não podem mais passar despercebidas para os pacotes WP2-E.
   `fastpq_metal_bench` agora expõe `--require-gpu` e
   `launch_geometry_sweep.py` habilita por padrão (desativar com
   `--allow-cpu-fallback`) para que falhas de CPU e falhas de detecção de metal sejam abortadas
   imediatamente, em vez de poluir matrizes Stage7 com telemetria não-GPU.【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs】【scripts/fastpq/launch_geometry_sweep.py】
4. As métricas de preenchimento zero desapareceram anteriormente pelo mesmo motivo; o conserto da cerca
   mantém a instrumentação do host ativa, portanto a próxima captura deve incluir o
   Bloco `zero_fill` sem temporizações sintéticas.

#### Instantâneo de 20k com `FASTPQ_GPU=gpu`

`fastpq_metal_bench_20k_refresh.json`

| Palco | Colunas | Len de entrada | Média de GPU (ms) | Média de CPU (ms) | Compartilhamento de GPU | Aceleração | ΔCPU (ms) |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| FFT | 16 | 32768 | 79,951ms (65,645–93,193) | 83,289ms (59,956–107,585) | 0,3% | 1,042× | +3.338 |
| IFF | 16 | 32768 | 78,605ms (69,986–83,726) | 93,898ms (80,656–119,625) | 0,3% | 1,195× | +15.293 |
| LDE | 16 | 262144 | 657,673ms (619,219–712,367) | 669,537ms (619,716–723,285) | 2,1% | 1,018× | +11.864 |
| Poseidon | 16 | 524288 | 30004,898ms (27284,117–32945,253) | 29087,532ms (24969,810–33020,517) | 97,4% | 0,969× | −917.366 |

Observações:

1. Mesmo com `FASTPQ_GPU=gpu`, esta captura ainda reflete o substituto da CPU:
   ~30s por iteração com `metal_dispatch_queue` preso em zero. Quando o
   a substituição está definida, mas o host não consegue descobrir um dispositivo Metal, a CLI agora sai
   antes de executar qualquer kernel e imprime o modo solicitado/resolvido mais o
   rótulo de back-end para que os engenheiros possam saber se a detecção, os direitos ou o
   A pesquisa metallib causou o downgrade. Execute `fastpq_metal_bench --gpu-probe
   --rows…` with `FASTPQ_DEBUG_METAL_ENUM=1` para capturar o log de enumeração e
   corrija o problema de detecção subjacente antes de executar novamente o criador de perfil.【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:1965】【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:2636】
2. A telemetria de preenchimento zero agora registra uma amostra real (18,66 ms em 32 MiB), provando
   a correção da cerca funciona, mas os deltas da fila permanecem ausentes até o envio da GPU
   ter sucesso.
3. Como o back-end continua fazendo downgrade, a porta de telemetria do Stage7 ainda está
   bloqueado: evidência de espaço na fila e sobreposição de poseidon exigem uma GPU genuína
   correr.

Essas capturas agora ancoram o backlog do WP2-E. Próximas ações: coletar perfil
flamecharts e logs de fila (depois que o back-end é executado na GPU), direcionam o
Gargalos Poseidon/LDE antes de revisitar a FFT e desbloquear o substituto de back-end
portanto, a telemetria Stage7 possui dados reais da GPU.

### Pontos fortes
- Preparação incremental, design trace-first, pilha STARK transparente.### Itens de ação de alta prioridade
1. Implemente acessórios de embalagem/pedido e atualize as especificações do AIR.
2. Finalize o commit do Poseidon2 `3f2b7fe` e publique exemplos de vetores SMT/pesquisa.
3. Mantenha os exemplos trabalhados (`lookup_grand_product.md`, `smt_update.md`) ao lado dos equipamentos.
4. Adicione o Apêndice A documentando a derivação de solidez e a metodologia de rejeição de IC.

### Decisões de design resolvidas
- ZK desativado (somente correção) em P1; revisitar em estágio futuro.
- Raiz da tabela de permissões derivada do estado de governança; os lotes tratam a tabela como somente leitura e comprovam a associação por meio de pesquisa.
- As provas de chave ausente usam folha zero mais testemunha vizinha com codificação canônica.
- Excluir semântica = valor da folha definido como zero no keyspace canônico.

Use este documento como referência canônica; atualize-o junto com o código-fonte, acessórios e apêndices para evitar desvios.

## Apêndice A - Derivação de Solidez

Este apêndice explica como a tabela “Soundness & SLOs” é produzida e como o CI impõe o limite mínimo de ≥128 bits mencionado anteriormente.

### Notação
- `N_trace = 2^k` — comprimento do traço após classificação e preenchimento até uma potência de dois.
- `b` — fator de explosão (`N_eval = N_trace × b`).
- `r` — FRI aridade (8 ou 16 para os conjuntos canônicos).
- `ℓ` — número de reduções de FRI (coluna `layers`).
- `q` — consultas do verificador por prova (coluna `queries`).
- `ρ` — taxa de código efetiva informada pelo planejador de coluna: `ρ = max_i(degree_i / domain_i)` sobre as restrições que sobrevivem à primeira rodada FRI.

O campo base Cachinhos Dourados tem `|F| = 2^64 - 2^32 + 1`, então as colisões Fiat-Shamir são limitadas por `q / 2^64`. A moagem adiciona um fator `2^{-g}` ortogonal, com `g = 23` para `fastpq-lane-balanced` e `g = 21` para o perfil de latência.【crates/fastpq_isi/src/params.rs:65】

### Limite analítico

Com DEEP-FRI de taxa constante, a probabilidade de falha estatística satisfaz

```
p_fri ≤ Σ_{j=0}^{ℓ-1} ρ^{q} = ℓ · ρ^{q}
```

porque cada camada reduz o grau polinomial e a largura do domínio pelo mesmo fator `r`, mantendo `ρ` constante. A coluna `est bits` da tabela informa `⌊-log₂ p_fri⌋`; Fiat-Shamir e retificação servem como margem extra de segurança.

### Saída do planejador e cálculo trabalhado

Executando o planejador de coluna Stage1 em rendimentos de lotes representativos:

| Conjunto de parâmetros | `N_trace` | `b` | `N_eval` | `ρ` (planejador) | Grau efetivo (`ρ × N_eval`) | `ℓ` | `q` | `-log₂(ℓ · ρ^{q})` |
| ------------- | --------- | --- | -------- | ------------- | -------------------------------- | --- | --- | ------------------ |
| Lote balanceado de 20k | `2^15` | 8 | 262144 | 0,077026 | 20192 | 5 | 52 | 190 bits |
| Lote de rendimento de 65k | `2^16` | 8 | 524288 | 0,200208 | 104967 | 6 | 58 | 132 bits |
| Lote de latência 131k | `2^17` | 16 | 2097152 | 0,209492 | 439337 | 5 | 64 | 142 bits |Exemplo (lote balanceado de 20k):
1. `N_trace = 2^15`, então `N_eval = 2^15 × 8 = 2^18`.
2. A instrumentação do planejador relata `ρ = 0.077026`, portanto `p_fri = 5 × ρ^{52} ≈ 6.4 × 10^{-58}`.
3. `-log₂ p_fri = 190 bits`, correspondendo à entrada da tabela.
4. As colisões Fiat-Shamir adicionam no máximo `2^{-58.3}`, e a retificação (`g = 23`) subtrai outro `2^{-23}`, mantendo a solidez total confortavelmente acima de 160 bits.

### Chicote de amostragem de rejeição de CI

Cada execução de CI executa um chicote de Monte Carlo para garantir que as medições empíricas permaneçam dentro de ±0,6 bits do limite analítico:
1. Desenhe um conjunto de parâmetros canônicos e sintetize um `TransitionBatch` com a contagem de linhas correspondente.
2. Construa o rastreamento, inverta uma restrição escolhida aleatoriamente (por exemplo, perturbe o produto principal da pesquisa ou um irmão SMT) e tente produzir uma prova.
3. Execute novamente o verificador, reamostrando os desafios Fiat-Shamir (trituração incluída) e registre se a prova adulterada foi rejeitada.
4. Repita para 16.384 sementes por conjunto de parâmetros e converta o limite inferior Clopper-Pearson de 99% da taxa de rejeição observada em bits.

O trabalho falhará imediatamente se o limite inferior medido cair abaixo de 128 bits, portanto, as regressões no planejador, no loop de dobramento ou na fiação de transcrição serão capturadas antes da mesclagem.

## Apêndice B — Derivação da raiz do domínio

Stage0 fixa os geradores de rastreamento e avaliação em constantes derivadas de Poseidon para que todas as implementações compartilhem os mesmos subgrupos.

### Procedimento
1. **Seleção de sementes.** Absorva a tag UTF‑8 `fastpq:v1:domain_roots` na esponja Poseidon2 usada em outro lugar no FASTPQ (largura do estado=3, taxa=2, quatro rodadas completas + 57 rodadas parciais). As entradas reutilizam a codificação `[len, limbs…]` de `pack_bytes`, produzindo o gerador base `g_base = 7`.【crates/fastpq_prover/src/packing.rs:44】【scripts/fastpq/src/bin/poseidon_gen.rs:1】
2. **Gerador de rastreamento.** Calcule `trace_root = g_base^{(p-1)/2^{trace_log_size}} mod p` e verifique `trace_root^{2^{trace_log_size}} = 1` enquanto a meia potência não é 1.
3. **Gerador LDE.** Repita a mesma exponenciação com `lde_log_size` para derivar `lde_root`.
4. **Seleção de coset.** Stage0 usa o subgrupo base (`omega_coset = 1`). Cosets futuros podem absorver uma tag adicional como `fastpq:v1:domain_roots:coset`.
5. **Tamanho da permutação.** Persista `permutation_size` explicitamente para que os agendadores nunca infiram regras de preenchimento a partir de potências implícitas de dois.

### Reprodução e validação
- Ferramentas: `cargo run --manifest-path scripts/fastpq/Cargo.toml --bin poseidon_gen -- domain-roots` emite fragmentos de Rust ou uma tabela Markdown (consulte `--format table`, `--seed`, `--filter`).【scripts/fastpq/src/bin/poseidon_gen.rs:1】
- Testes: `canonical_sets_meet_security_target` mantém os conjuntos de parâmetros canônicos alinhados com as constantes publicadas (raízes diferentes de zero, emparelhamento de explosão/aridade, dimensionamento de permutação), então `cargo test -p fastpq_isi` detecta desvios imediatamente.【crates/fastpq_isi/src/params.rs:138】
- Fonte da verdade: atualize a tabela Stage0 e `fastpq_isi/src/params.rs` juntas sempre que novos pacotes de parâmetros forem introduzidos.

## Apêndice C — Detalhes do pipeline de compromisso### Streaming do fluxo de compromisso do Poseidon
Stage2 define o compromisso de rastreamento determinístico compartilhado pelo provador e verificador:
1. **Normalizar transições.** `trace::build_trace` classifica cada lote, preenche-o para `N_trace = 2^{⌈log₂ rows⌉}` e emite vetores de coluna na ordem abaixo.【crates/fastpq_prover/src/trace.rs:123】
2. **Colunas hash.** `trace::column_hashes` transmite as colunas por meio de esponjas Poseidon2 dedicadas marcadas como `fastpq:v1:trace:column:<name>`. Quando o recurso `fastpq-prover-preview` está ativo, o mesmo percurso recicla os coeficientes IFFT/LDE exigidos pelo backend, portanto, nenhuma cópia extra da matriz é alocada.【crates/fastpq_prover/src/trace.rs:474】
3. **Eleve em uma árvore Merkle.** `trace::merkle_root` dobra os resumos da coluna com nós Poseidon marcados como `fastpq:v1:trace:node`, duplicando a última folha sempre que um nível tiver uma distribuição estranha para evitar casos especiais.【crates/fastpq_prover/src/trace.rs:656】
4. **Finalize o resumo.** `digest::trace_commitment` prefixa a tag de domínio (`fastpq:v1:trace_commitment`), nome do parâmetro, dimensões preenchidas, resumos de coluna e raiz Merkle usando a mesma codificação `[len, limbs…]` e, em seguida, faz hash da carga útil com SHA3-256 antes de incorporá-la em `Proof::trace_commitment`.【crates/fastpq_prover/src/digest.rs:25】

O verificador recalcula o mesmo resumo antes de amostrar os desafios Fiat-Shamir, portanto, as incompatibilidades abortam as provas antes de qualquer abertura.

### Controles alternativos de Poseidon- O provador agora expõe uma substituição de pipeline Poseidon dedicada (`zk.fastpq.poseidon_mode`, env `FASTPQ_POSEIDON_MODE`, CLI `--fastpq-poseidon-mode`) para que os operadores possam misturar GPU FFT/LDE com hashing CPU Poseidon em dispositivos que não conseguem atingir o alvo Stage7 <900ms. Os valores suportados espelham o botão do modo de execução (`auto`, `cpu`, `gpu`), padronizando para o modo global quando não especificado. O tempo de execução encadeia esse valor por meio da configuração da pista (`FastpqPoseidonMode`) e o propaga no provador (`Prover::canonical_with_modes`) para que as substituições sejam determinísticas e auditáveis na configuração dumps.【crates/iroha_config/src/parameters/user.rs:1488】【crates/fastpq_prover/src/proof.rs:138】【crates/iroha_core/src/fastpq/lane.rs:123】
- A telemetria exporta o modo de pipeline resolvido por meio do novo contador `fastpq_poseidon_pipeline_total{requested,resolved,path,device_class,chip_family,gpu_kind}` (e do OTLP gêmeo `fastpq.poseidon_pipeline_resolutions_total`). Os painéis `sorafs`/operador podem, portanto, confirmar quando uma implementação está executando hashing fundido/pipeline de GPU versus o substituto forçado da CPU (`path="cpu_forced"`) ou downgrades de tempo de execução (`path="cpu_fallback"`). A sonda CLI é instalada automaticamente em `irohad`, portanto, pacotes de lançamento e telemetria ao vivo compartilham o mesmo fluxo de evidências.【crates/iroha_telemetry/src/metrics.rs:4780】【crates/irohad/src/main.rs:2504】
- A evidência de modo misto também é estampada em cada placar por meio da porta de adoção existente: o provador emite o rótulo de modo resolvido + caminho para cada lote, e o contador `fastpq_poseidon_pipeline_total` aumenta junto com o contador de modo de execução sempre que uma prova chega. Isso satisfaz o WP2-E.6, tornando as quedas de energia visíveis e fornecendo uma opção limpa para downgrades determinísticos enquanto a otimização continua.
- `scripts/fastpq/wrap_benchmark.py --poseidon-metrics metrics_poseidon.prom` agora analisa arranhões Prometheus (Metal ou CUDA) e incorpora um resumo `poseidon_metrics` dentro de cada pacote empacotado. O auxiliar filtra as linhas do contador por `metadata.labels.device_class`, captura as amostras `fastpq_execution_mode_total` correspondentes e falha no encapsulamento quando as entradas `fastpq_poseidon_pipeline_total` estão faltando, para que os pacotes WP2-E.6 sempre enviem evidências CUDA/Metal reproduzíveis em vez de ad-hoc notas.【scripts/fastpq/wrap_benchmark.py:1】【scripts/fastpq/tests/test_wrap_benchmark.py:1】

#### Política determinística de modo misto (WP2-E.6)1. **Detecte deficiência de GPU.** Sinalize qualquer classe de dispositivo cuja captura Stage7 ou instantâneo Grafana ao vivo mostre a latência do Poseidon mantendo o tempo total de prova >900ms enquanto FFT/LDE permanece abaixo do alvo. Os operadores anotam a matriz de captura (`artifacts/fastpq_benchmarks/matrix/devices/<label>.txt`) e paginam o plantão quando `fastpq_poseidon_pipeline_total{device_class="<label>",path="gpu"}` estagna enquanto `fastpq_execution_mode_total{backend="metal"}` ainda registra despachos GPU FFT/LDE.【scripts/fastpq/wrap_benchmark.py:1】【dashboards/grafana/fastpq_acceleration.json:1】
2. **Mude para CPU Poseidon apenas para os hosts afetados.** Defina `zk.fastpq.poseidon_mode = "cpu"` (ou `FASTPQ_POSEIDON_MODE=cpu`) na configuração local do host junto com os rótulos da frota, mantendo `zk.fastpq.execution_mode = "gpu"` para que FFT/LDE continue a usar o acelerador. Registre a diferença de configuração no tíquete de implementação e adicione a substituição por host ao pacote como `poseidon_fallback.patch` para que os revisores possam reproduzir a alteração de forma determinística.
3. **Prove o downgrade.** Raspe o contador Poseidon imediatamente após reiniciar o nó:
   ```bash
   curl -s http://<host>:8180/metrics | rg 'fastpq_poseidon_pipeline_total{.*device_class="<label>"'
   ```
   O dump deve mostrar `path="cpu_forced"` crescendo em sincronia com o contador de execução da GPU. Armazene o scrape como `metrics_poseidon.prom` próximo ao instantâneo `metrics_cpu_fallback.prom` existente e capture as linhas de log `telemetry::fastpq.poseidon` correspondentes em `poseidon_fallback.log`.
4. **Monitore e saia.** Continue alertando em `fastpq_poseidon_pipeline_total{path="cpu_forced"}` enquanto o trabalho de otimização continua. Depois que um patch traz o tempo de execução por prova de volta para menos de 900 ms no host de teste, reverta a configuração para `auto`, execute novamente o scrape (mostrando `path="gpu"` novamente) e anexe as métricas antes/depois ao pacote para fechar o exercício de modo misto.

**Contrato de telemetria.**

| Sinal | PromQL / Fonte | Finalidade |
|--------|-----------------|-----|
| Contador do modo Poseidon | `fastpq_poseidon_pipeline_total{device_class="<label>",path=~"cpu_.*"}` | Confirma que o hash da CPU é intencional e tem como escopo a classe de dispositivo sinalizada. |
| Contador do modo de execução | `fastpq_execution_mode_total{device_class="<label>",backend="metal"}` | Prova que FFT/LDE ainda funciona em GPU mesmo durante o downgrade do Poseidon. |
| Evidência de registro | Entradas `telemetry::fastpq.poseidon` capturadas em `poseidon_fallback.log` | Fornece prova por prova de que o host resolveu o hash da CPU com o motivo `cpu_forced`. |

O pacote de implementação deve agora incluir `metrics_poseidon.prom`, o config diff e o trecho de log sempre que o modo misto estiver ativo para que a governança possa auditar a política determinística de fallback juntamente com a telemetria FFT/LDE. `ci/check_fastpq_rollout.sh` já impõe os limites de fila/preenchimento zero; o portão de acompanhamento verificará a integridade do contador Poseidon assim que o modo misto chegar à automação de liberação.

As ferramentas de captura Stage7 já lidam com CUDA: envolva cada pacote `fastpq_cuda_bench` com `--poseidon-metrics` (apontando para o `metrics_poseidon.prom` raspado) e a saída agora carrega os mesmos contadores de pipeline/resumo de resolução usados ​​no Metal para que a governança possa verificar substitutos de CUDA sem ferramentas personalizadas.### Ordem das colunas
O pipeline de hash consome colunas nesta ordem determinística:
1. Sinalizadores seletores: `s_active`, `s_transfer`, `s_mint`, `s_burn`, `s_role_grant`, `s_role_revoke`, `s_meta_set`, `s_perm`.
2. Colunas de membros compactados (cada uma preenchida com zeros no comprimento do traço): `key_limb_{i}`, `value_old_limb_{i}`, `value_new_limb_{i}`, `asset_id_limb_{i}`.
3. Escalares auxiliares: `delta`, `running_asset_delta`, `metadata_hash`, `supply_counter`, `perm_hash`, `neighbour_leaf`, `dsid`, `slot`.
4. Testemunhas Merkle esparsas para cada nível `ℓ ∈ [0, SMT_HEIGHT)`: `path_bit_ℓ`, `sibling_ℓ`, `node_in_ℓ`, `node_out_ℓ`.

`trace::column_hashes` percorre as colunas exatamente nesta ordem, de modo que o back-end do espaço reservado e a implementação do Stage2 STARK permanecem estáveis em termos de rastreamento entre as versões.【crates/fastpq_prover/src/trace.rs:474】

### Transcrição de tags de domínio
Stage2 corrige o catálogo Fiat-Shamir abaixo para manter a geração de desafios determinística:

| Etiqueta | Finalidade |
| --- | ------- |
| `fastpq:v1:init` | Absorva a versão do protocolo, conjunto de parâmetros e `PublicIO`. |
| `fastpq:v1:roots` | Confirme o rastreamento e procure as raízes de Merkle. |
| `fastpq:v1:gamma` | Experimente o desafio do grande produto de pesquisa. |
| `fastpq:v1:alpha:<i>` | Exemplos de desafios de composição polinomial (`i = 0, 1`). |
| `fastpq:v1:lookup:product` | Absorva o grande produto de pesquisa avaliado. |
| `fastpq:v1:beta:<round>` | Experimente o desafio de dobrar para cada rodada do FRI. |
| `fastpq:v1:fri_layer:<round>` | Confirme a raiz Merkle para cada camada FRI. |
| `fastpq:v1:fri:final` | Registre a camada FRI final antes de abrir consultas. |
| `fastpq:v1:query_index:0` | Derive deterministicamente índices de consulta do verificador. |