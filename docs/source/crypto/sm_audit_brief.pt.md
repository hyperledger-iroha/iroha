---
lang: pt
direction: ltr
source: docs/source/crypto/sm_audit_brief.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 9cda4648f0af7f89022e9d9f4ea243bc22685d9356927bbf1417c77b2057d872
source_last_modified: "2026-01-03T18:07:57.113286+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

% SM2/SM3/SM4 Resumo de auditoria externa
Grupo de trabalho de criptografia % Iroha
% 30/01/2026

# Visão geral

Este resumo reúne o contexto de engenharia e conformidade necessário para um
revisão independente da habilitação SM2/SM3/SM4 do Iroha. Destina-se a equipes de auditoria
com experiência em criptografia Rust e familiaridade com o Chinese National
Padrões de criptografia. O resultado esperado é um relatório escrito cobrindo
riscos de implementação, lacunas de conformidade e orientações de remediação priorizadas
antes do lançamento do SM, passando da pré-visualização para a produção.

# Instantâneo do programa

- **Escopo de lançamento:** Iroha 2/3 base de código compartilhada, verificação determinística
  caminhos entre nós e SDKs, assinatura disponível atrás da proteção de configuração.
- **Fase atual:** SM-P3.2 (integração de backend OpenSSL/Tongsuo) com Rust
  implementações já enviadas para verificação e casos de uso simétricos.
- **Data prevista para decisão:** 30/04/2026 (as conclusões da auditoria informam avançar/não avançar para
  habilitando a assinatura SM em compilações de validadores).
- **Principais riscos monitorados:** pedigree de dependência de terceiros, determinístico
  comportamento sob hardware misto, prontidão para conformidade do operador.

# Referências de código e acessórios

- `crates/iroha_crypto/src/sm.rs` — Implementações Rust e OpenSSL opcional
  ligações (recurso `sm-ffi-openssl`).
- `crates/ivm/tests/sm_syscalls.rs` — IVM cobertura de syscall para hashing,
  verificação e modos simétricos.
- `crates/iroha_data_model/tests/sm_norito_roundtrip.rs` — carga útil Norito
  viagens de ida e volta para artefatos SM.
- `docs/source/crypto/sm_program.md` — histórico do programa, auditoria de dependência e
  guarda-corpos de implantação.
- `docs/source/crypto/sm_operator_rollout.md` — habilitação voltada para o operador e
  procedimentos de reversão.
- `docs/source/crypto/sm_compliance_brief.md` — resumo regulatório e exportação
  considerações.
-`scripts/sm_openssl_smoke.sh` / `crates/iroha_crypto/tests/sm_openssl_smoke.rs`
  — aproveitamento determinístico de fumaça para fluxos apoiados por OpenSSL.
- `fuzz/sm_*` corpora — sementes fuzz baseadas em RustCrypto cobrindo primitivas SM3/SM4.

# Escopo de auditoria solicitado1. **Conformidade com as especificações**
   - Validar verificação de assinatura SM2, cálculo ZA e canônico
     comportamento de codificação.
   - Confirmar que as primitivas SM3/SM4 seguem GM/T 0002-2012 e GM/T 0007-2012,
     incluindo invariantes de modo contador e manipulação IV.
2. **Determinismo e garantias de tempo constante**
   - Revise ramificações, pesquisas de tabelas e despacho de hardware para execução de nós
     permanece determinístico em todas as famílias de CPU.
   - Avaliar declarações de tempo constante para operações de chave privada e confirmar a
     Os caminhos OpenSSL/Tongsuo mantêm a semântica de tempo constante.
3. **Canal lateral e análise de falhas**
   - Inspecione os riscos de tempo, cache e canal lateral de energia em Rust e
     Caminhos de código apoiados por FFI.
   - Avaliar o tratamento de falhas e a propagação de erros para verificação de assinatura e
     falhas de criptografia autenticadas.
4. **Criação, dependência e revisão da cadeia de suprimentos**
   - Confirme compilações reproduzíveis e proveniência de artefatos OpenSSL/Tongsuo.
   - Revise o licenciamento da árvore de dependência e a cobertura de auditoria.
5. **Crítica do equipamento de teste e verificação**
   - Avaliar testes determinísticos de fumaça, chicotes fuzz e luminárias Norito.
   - Recomendar cobertura adicional (por exemplo, testes diferenciais,
     provas) se permanecerem lacunas.
6. **Validação de conformidade e orientação do operador**
   - Cruzar a documentação enviada com os requisitos legais e esperados
     controles do operador.

# Entregáveis e Logística

- **Início de início:** 24/02/2026 (virtual, 90 minutos).
- **Entrevistas:** Crypto WG, mantenedores IVM, operações de plataforma (conforme necessário).
- **Acesso ao artefato:** espelho de repositório somente leitura, logs de pipeline de CI, acessórios
  saídas e SBOMs de dependência (CycloneDX).
- **Atualizações provisórias:** status escrito semanal + chamadas de risco.
- **Entregas finais (prazo para 15/04/2026):**
  - Resumo executivo com classificação de risco.
  - Descobertas detalhadas (por problema: impacto, probabilidade, referências de código,
    orientação de remediação).
  - Plano de novo teste/verificação.
  - Declaração sobre determinismo, postura de tempo constante e alinhamento de compliance.

## Status de engajamento

| Fornecedor | Estado | Início | Janela de campo | Notas |
|----|--------|----------|-------------|-------|
| Trilha de Bits (prática CN) | Declaração de trabalho executada 2026-02-21 | 24/02/2026 | 24/02/2026–22/03/2026 | Entrega prevista para 15/04/2026; Hui Zhang liderando o envolvimento com Alexey M. como contraparte de engenharia. Chamada de status semanal às quartas-feiras às 09:00UTC. |
| Grupo NCC (APAC) | Slot de contingência reservado | N/A (em espera) | Provisório 2026-05-06–2026-05-31 | Ativação somente se descobertas de alto risco exigirem uma segunda passagem; prontidão confirmada por Priya N. (Segurança) e mesa de engajamento do Grupo NCC 22/02/2026. |

Nº anexos incluídos no pacote de divulgação-`docs/source/crypto/sm_program.md`
-`docs/source/crypto/sm_operator_rollout.md`
-`docs/source/crypto/sm_compliance_brief.md`
-`docs/source/crypto/sm_lock_refresh_plan.md`
-`docs/source/crypto/sm_rust_vector_check.md`
- `docs/source/crypto/attachments/sm_iroha_crypto_tree.txt` — Instantâneo `cargo tree -p iroha_crypto --no-default-features --features "sm sm-ffi-openssl"`.
- `docs/source/crypto/attachments/sm_iroha_crypto_metadata.json` — Exportação `cargo metadata` para a caixa `iroha_crypto` (gráfico de dependência bloqueada).
- `docs/source/crypto/attachments/sm_openssl_smoke.log` — execução mais recente do `scripts/sm_openssl_smoke.sh` (ignora caminhos SM2/SM4 quando o suporte do provedor está ausente).
- `docs/source/crypto/attachments/sm_openssl_provenance.md` — proveniência do kit de ferramentas local (notas de versão pkg-config/OpenSSL).
- Manifesto do corpus Fuzz (`fuzz/sm_corpus_manifest.json`).

> **Advertência sobre o ambiente:** O instantâneo de desenvolvimento atual usa o conjunto de ferramentas OpenSSL 3.x vendido (recurso `openssl` crate `vendored`), mas o macOS não possui intrínsecos de CPU SM3/SM4 e o provedor padrão não expõe SM4-GCM, portanto, o chicote de fumaça OpenSSL ainda ignora a cobertura SM4 e a análise SM2 do exemplo do anexo. Um ciclo de dependência do espaço de trabalho (`sorafs_manifest ↔ sorafs_car`) também força o script auxiliar a ignorar a execução após emitir a falha `cargo check`. Execute novamente o pacote dentro do ambiente de compilação da versão Linux (OpenSSL/Tongsuo com SM4 habilitado e sem o ciclo) para capturar a paridade total antes da auditoria externa.

# Candidatos a parceiros de auditoria e escopo

| Empresa | Experiência relevante | Escopo e resultados típicos | Notas |
|------|---------------------|---------------------------------|-------|
| Trilha de Bits (prática de criptografia CN) | Revisões de código Rust (`ring`, zkVMs), avaliações GM/T anteriores para pilhas de pagamento móvel. | Diferença de conformidade de especificações (GM/T 0002/3/4), revisão em tempo constante de caminhos Rust + OpenSSL, difusão diferencial, revisão da cadeia de suprimentos, roteiro de remediação. | Já noivo; tabela retida para fins de integridade ao planejar ciclos de atualização futuros. |
| Grupo NCC APAC | Equipes vermelhas de criptografia de hardware/SOC + Rust, publicaram análises de primitivas RustCrypto e pontes HSM de pagamento. | Avaliação holística de ligações Rust + JNI/FFI, validação de política determinística, revisão de portão de desempenho/telemetria, passo a passo do manual do operador. | Reservado como contingência; também pode fornecer relatórios bilíngues para reguladores chineses. |
| Kudelski Security (equipe de Blockchain e criptografia) | Auditorias de Halo2, Mina, zkSync, esquemas de assinatura personalizados implementados em Rust. | Concentre-se na correção da curva elíptica, integridade da transcrição, modelagem de ameaças para aceleração de hardware e evidências de CI/lançamento. | Útil para segundas opiniões sobre aceleração de hardware (SM-5a) e interações FASTPQ para SM. |
| Menor Autoridade | Auditorias de protocolo criptográfico para blockchains baseados em Rust (Filecoin, Polkadot), consultoria de construções reproduzíveis. | Verificação determinística de construção, verificação de codec Norito, verificação cruzada de evidências de conformidade, revisão de comunicação do operador. | Adequado para resultados de transparência/relatórios de auditoria quando os reguladores solicitam verificação independente além da revisão do código. |

Todos os trabalhos solicitam o mesmo pacote de artefatos enumerado acima, além dos seguintes complementos opcionais, dependendo da empresa:- **Conformidade com especificações e comportamento determinístico:** Verificação linha por linha da derivação SM2 ZA, preenchimento SM3, funções redondas SM4 e porta de despacho de tempo de execução `sm_accel` para garantir que a aceleração nunca altere a semântica.
- **Revisão de canal lateral e FFI:** Inspeção de declarações de tempo constante, blocos de código inseguros e camadas de ponte OpenSSL/Tongsuo, incluindo testes de comparação com o caminho Rust.
- **CI / validação da cadeia de suprimentos:** Reprodução dos aproveitamentos `sm_interop_matrix`, `sm_openssl_smoke` e `sm_perf` junto com atestados SBOM/SLSA para que as descobertas da auditoria possam ser vinculadas diretamente às evidências de liberação.
- **Garantia voltada ao operador:** Verificação cruzada de `sm_operator_rollout.md`, modelos de arquivamento de conformidade e painéis de telemetria para confirmar se as mitigações prometidas na documentação são tecnicamente aplicáveis.

Ao definir o escopo de auditorias futuras, reutilize esta tabela para alinhar os pontos fortes do fornecedor com o marco específico do roteiro (por exemplo, favoreça Kudelski para versões pesadas de hardware/desempenho, Trilha de bits para correção de idioma/tempo de execução e Menor autoridade para garantias de compilação reproduzíveis).

# Pontos de contato

- **Proprietário técnico:** Líder do Crypto WG (Alexey M., `alexey@iroha.tech`)
- **Gerente de programa:** Coordenadora de operações da plataforma (Sarah K.,
  `sarah@iroha.tech`)
- **Contato de segurança:** Engenharia de segurança (Priya N., `security@iroha.tech`)
- **Contato de documentação:** Líder do Docs/DevRel (Jamila R.,
  `docs@iroha.tech`)