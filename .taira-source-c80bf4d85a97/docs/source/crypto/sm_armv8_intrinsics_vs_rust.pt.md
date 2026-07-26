---
lang: pt
direction: ltr
source: docs/source/crypto/sm_armv8_intrinsics_vs_rust.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 40185fd79a4d6bcb2a7f35cbb4a14ca8feb82f31e62b4e51f9a6f1657f524ed4
source_last_modified: "2026-01-03T18:07:57.096028+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

% ARMv8 SM3/SM4 intrínsecos vs implementações de ferrugem pura
Grupo de trabalho de criptografia % Iroha
% 12/02/2026

# Alerta

> Você é LLM e atua como consultor especialista da equipe de criptografia Hyperledger Iroha.  
> Antecedentes:  
> - Hyperledger Iroha é um blockchain permitido baseado em Rust onde cada validador deve ser executado de forma determinística para que o consenso não possa divergir.  
> - Iroha usa as primitivas criptográficas GM/T chinesas SM2 (assinaturas), SM3 (hash) e SM4 (cifra de bloco) para determinadas implantações regulatórias.  
> - A equipe envia duas implementações SM3/SM4 dentro da pilha do validador:  
> 1. Pure Rust, código escalar de tempo constante e dividido em bits que é executado em qualquer CPU.  
> 2. Kernels acelerados ARMv8 NEON que dependem das instruções opcionais `SM3PARTW1`, `SM3PARTW2`, `SM3SS1`, `SM3SS2`, `SM4E` e `SM4EKEY` expostas na série M mais recente da Apple e no servidor Arm CPUs.  
> - O código acelerado está por trás da detecção de recursos em tempo de execução usando intrínsecos `core::arch::aarch64`; o sistema deve evitar comportamento não determinístico quando threads migram entre núcleos big.LITTLE ou quando réplicas são construídas com diferentes sinalizadores de compilador.  
> Análise solicitada:  
> Compare as implementações intrínsecas do ARMv8 com os substitutos puros do Rust para verificação determinística do blockchain. Discuta os ganhos de rendimento/latência, as armadilhas do determinismo (detecção de recursos, núcleos heterogêneos, risco SIGILL, alinhamento, mistura de caminhos de execução), propriedades de tempo constante e as salvaguardas operacionais (testes, manifestos, telemetria, documentação do operador) necessárias para manter todos os validadores sincronizados, mesmo quando alguns hardwares suportam as instruções e outros não.

# Resumo

Dispositivos ARMv8-A que expõem o `SM3` opcional (`SM3PARTW1`, `SM3PARTW2`, `SM3SS1`, `SM3SS2`) e `SM4` (`SM4E`, `SM4EKEY`) podem acelerar o hash GM/T e bloquear substancialmente as primitivas de cifra. No entanto, a execução determinística do blockchain exige um controle rígido sobre a detecção de recursos, a paridade de fallback e o comportamento em tempo constante. A orientação a seguir aborda como as duas estratégias de implementação se comparam e o que a pilha Iroha deve impor.

# Comparação de implementação| Aspecto | Intrínsecos ARMv8 (AArch64 Inline ASM/`core::arch::aarch64`) | Pure Rust (fatiado / sem mesa) |
|--------|------------------------------------------------------------|--------------------------------------|
| Rendimento | Hashing SM3 3–5× mais rápido e SM4 ECB/CTR até 8× mais rápido por núcleo no Apple M-series e Neoverse V1; ganha conicidade quando vinculado à memória. | Taxa de transferência de linha de base vinculada por ALU escalar e rotações; ocasionalmente se beneficia de extensões SHA `aarch64` (por meio da autovetorização do compilador), mas geralmente fica atrás do NEON por uma lacuna semelhante de 3–8×. |
| Latência | Latência de bloco único ~30–40ns em M2 com intrínsecos; combina com hashing de mensagens curtas e criptografia de pequenos blocos em syscalls. | 90–120ns por bloco; pode exigir o desenrolamento para permanecer competitivo, aumentando a pressão do cache de instruções. |
| Tamanho do código | Requer caminhos de código duplos (intrínseco + escalar) e controle de tempo de execução; caminho intrínseco compacto se estiver usando `cfg(target_feature)`. | Caminho único; um pouco maior devido às tabelas de programação manual, mas sem lógica de bloqueio. |
| Determinismo | Deve bloquear o envio do tempo de execução para resultados determinísticos, evitar corridas de teste de recursos entre threads e fixar a afinidade da CPU se núcleos heterogêneos diferirem (por exemplo, big.LITTLE). | Determinístico por padrão; nenhuma detecção de recurso de tempo de execução. |
| Postura de tempo constante | A unidade de hardware tem tempo constante para rodadas principais, mas o wrapper deve evitar a seleção dependente de segredo ao fazer fallback ou misturar tabelas. | Totalmente controlado em Rust; tempo constante garantido pela construção (bit-slicing) se codificado corretamente. |
| Portabilidade | Requer `aarch64` + recursos opcionais; x86_64 e RISC-V retornam automaticamente. | Funciona em qualquer lugar; o desempenho depende das otimizações do compilador. |

# Armadilhas de envio em tempo de execução

1. **Investigação de recursos não determinísticos**
   - Problema: testar `is_aarch64_feature_detected!("sm4")` em SoCs big.LITTLE heterogêneos pode produzir respostas diferentes por núcleo, e o roubo de trabalho entre threads pode misturar caminhos dentro de um bloco.
   - Mitigação: capture a capacidade de hardware exatamente uma vez durante a inicialização do nó, transmita por meio de `OnceLock` e emparelhe com afinidade de CPU ao executar kernels acelerados dentro da VM ou de caixas criptográficas. Nunca ramifique sinalizadores de recursos após o início do trabalho crítico para o consenso.

2. **Precisão mista entre réplicas**
   - Problema: nós construídos com compiladores diferentes podem discordar quanto à disponibilidade intrínseca (`target_feature=+sm4` habilitação em tempo de compilação versus detecção em tempo de execução). Se a execução passar por diferentes caminhos de código, o tempo da microarquitetura pode vazar para backoffs baseados em pow ou limitadores de taxa.
   - Mitigação: distribua perfis de construção canônicos com `RUSTFLAGS`/`CARGO_CFG_TARGET_FEATURE` explícitos, exija ordenação de fallback determinística (por exemplo, prefira escalar, a menos que a configuração habilite o hardware) e inclua um hash de configuração em manifestos para atestação.3. **Disponibilidade de instruções em Apple vs Linux**
   - Problema: a Apple expõe instruções SM4 apenas nas versões mais recentes de silício e sistema operacional; As distribuições Linux podem corrigir kernels para mascará-los, aguardando aprovações de exportação. Confiar em intrínsecos sem proteção causa SIGILLs.
   - Mitigação: gate via `std::arch::is_aarch64_feature_detected!`, capture `SIGILL` em testes de fumaça e trate intrínsecos ausentes como substituto esperado (ainda determinístico).

4. **Partilhamento paralelo e ordenação de memória**
   - Problema: kernels acelerados geralmente processam vários blocos por iteração; usar cargas/armazenamentos NEON com entradas desalinhadas pode causar falhas ou exigir correções de alinhamento explícitas quando alimentado por buffers desserializados Norito.
   - Mitigação: mantenha alocações alinhadas em bloco (por exemplo, múltiplos `SM4_BLOCK_SIZE` por meio de wrappers `aligned_alloc`), valide o alinhamento em compilações de depuração e retorne ao escalar quando desalinhado.

5. **Ataques de envenenamento de cache de instruções**
   - Problema: os adversários do consenso podem criar cargas de trabalho que destroem pequenas linhas de cache I em núcleos mais fracos, ampliando a diferença de latência entre caminhos acelerados e escalares.
   - Mitigação: corrija o agendamento para tamanhos de blocos determinísticos, pad loops para evitar ramificações imprevisíveis e inclua testes de regressão de microbancadas para garantir que o jitter permaneça dentro das janelas de tolerância.

# Recomendações de implantação determinísticas

- **Política de tempo de compilação:** mantenha o código acelerado atrás de um sinalizador de recurso (por exemplo, `sm_accel_neon`) habilitado por padrão em compilações de lançamento, mas exija uma aceitação explícita nas configurações para testnets até que a cobertura de paridade esteja madura.
- **Testes de paridade de fallback:** mantêm vetores dourados que executam caminhos escalares e acelerados consecutivos (fluxo de trabalho `sm_neon_check` atual); estender para cobrir os modos SM3/SM4 GCM assim que o suporte do provedor chegar.
- **Atestado de manifesto:** inclua a política de aceleração (`hardware=sm-neon|scalar`) no manifesto Norito do nó para tornar a divergência detectável durante a admissão de pares.
- **Telemetria:** emite métricas comparando a latência por chamada em ambos os caminhos; alerta se a divergência exceder limites predeterminados (por exemplo, >5% de jitter), sinalizando possível desvio de hardware.
- **Documentação:** mantenha as orientações do operador (`sm_operator_rollout.md`) atualizadas com instruções para ativar/desativar intrínsecos e observe que o comportamento determinístico é preservado independentemente do caminho.

# Referências

- `crates/iroha_crypto/src/sm.rs` — Ganchos de implementação NEON vs escalar.
- `.github/workflows/sm-neon-check.yml` — faixa CI NEON forçada garantindo paridade.
- `docs/source/crypto/sm_program.md` — guarda-corpos de liberação e portões de desempenho.
- Manual de referência da arquitetura Arm, Armv8-A, Seção D13 (instruções SM3/SM4).
- GM/T 0002-2012, GM/T 0003-2012 — especificações oficiais SM3/SM4 para testes de comparação.

## Prompt independente (copiar/colar)> Você é LLM e atua como consultor especialista da equipe de criptografia Hyperledger Iroha.  
> Histórico: Hyperledger Iroha é um blockchain permitido baseado em Rust que requer execução determinística entre validadores. A plataforma suporta o conjunto de criptografia chinês GM/T SM2/SM3/SM4. Para SM3 e SM4, a base de código fornece duas implementações: (a) código escalar de tempo constante cortado em bits de Rust puro que é executado em qualquer lugar e (b) kernels acelerados ARMv8 NEON que dependem das instruções opcionais `SM3PARTW1`, `SM3PARTW2`, `SM3SS1`, `SM3SS2`, `SM4E` e `SM4EKEY`. Os caminhos acelerados são habilitados por meio da detecção de recursos em tempo de execução usando `core::arch::aarch64`; eles não devem introduzir o não-determinismo quando os threads migram através de núcleos big.LITTLE heterogêneos ou quando as réplicas são construídas com diferentes sinalizadores `target_feature`.  
> Tarefa: Compare as implementações com tecnologia intrínseca com os substitutos escalares para verificação determinística de blockchain. Detalhe as diferenças de rendimento e latência, enumere riscos de determinismo (detecção de recursos, núcleos heterogêneos, comportamento SIGILL, alinhamento, caminhos de execução mistos), comente sobre postura de tempo constante e recomende salvaguardas (estratégia de teste, campos de manifesto/atestado, telemetria, documentação do operador) que garantam que todos os validadores permaneçam sincronizados mesmo que os recursos de hardware sejam diferentes.