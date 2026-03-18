---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/reports/sf6-security-review.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
título: Revisão de segurança SF-6
resumo: Achados e itens de acompanhamento da avaliação independente de assinatura sem chave, streaming de prova e pipelines de envio de manifestos.
---

# Revisão de segurança SF-6

**Janela de avaliação:** 10/02/2026 -> 18/02/2026  
**Líderes de revisão:** Security Engineering Guild (`@sec-eng`), Tooling Working Group (`@tooling-wg`)  
**Escopo:** CLI/SDK SoraFS (`sorafs_cli`, `sorafs_car`, `sorafs_manifest`), APIs de streaming de prova, manipulação de manifesto Torii, integração Sigstore/OIDC, ganchos de liberação CI.  
**Artefatos:**  
- Fonte do CLI e testes (`crates/sorafs_car/src/bin/sorafs_cli.rs`)  
- Manipuladores de manifesto/prova Torii (`crates/iroha_torii/src/sorafs/api.rs`)  
- Automação de liberação (`ci/check_sorafs_cli_release.sh`, `scripts/release_sorafs_cli.sh`)  
- Chicote de paridade determinística (`crates/sorafs_car/tests/sorafs_cli.rs`, [SoraFS Orchestrator GA Parity Report](./orchestrator-ga-parity.md))

## Metodologia

1. **Workshops de modelagem de ameaças** mapearam capacidades de ataque para estações de trabalho de desenvolvedores, sistemas CI e nós Torii.  
2. **Revisão de código** com foco em superfícies de credenciais (troca de token OIDC, assinatura sem chave), validação de manifestos Norito e contrapressão em streaming de prova.  
3. **Teste dinâmico** reexecutou manifestos de fixture e modos de falha simulados (repetição de token, adulteração de manifesto, streams de prova truncados) usando chicote de paridade e unidades fuzz sob medida.  
4. **Inspeção de configuração** validou os padrões `iroha_config`, manipulação de flag CLI e scripts de liberação para garantir execuções determinísticas e auditáveis.  
5. **Entrevista de processo** Fluxo de remediação confirmado, caminhos de escalonamento e captura de evidências de auditoria com os proprietários da liberação do Tooling WG.

## Resumo das descobertas| ID | Gravidade | Área | Encontrando | Resolução |
|----|----------|------|---------|------------|
| SF6-SR-01 | Alto | Assinatura sem chave | Os padrões de público do token OIDC estavam implícitos nos modelos de CI, com risco de reprodução entre locatários. | Foi adicionada aplicação explícita de `--identity-token-audience` em release hooks e CI templates ([release process](../developer-releases.md), `docs/examples/sorafs_ci.md`). O CI agora falha quando a audiência é omitida. |
| SF6-SR-02 | Médio | Transmissão de prova | Caminhos de contrapressão aceitavam buffers de assinantes sem limite, permitindo esgotamento de memória. | `sorafs_cli proof stream` aplica tamanhos de canais limitados com truncamento determinístico, registra resumos Norito e aborta o stream; o O espelho Torii foi atualizado para blocos de resposta limitados (`crates/iroha_torii/src/sorafs/api.rs`). |
| SF6-SR-03 | Médio | Envio de manifesto | O CLI aceitou manifestos sem verificar os planos de chunk incorporados quando `--plan` estava ausente. | `sorafs_cli manifest submit` agora recomputa e compara CAR digests a menos que `--expect-plan-digest` seja fornecido, eliminando incompatibilidades e exibindo dicas de remediação. Testes cobrem casos de sucesso/falha (`crates/sorafs_car/tests/sorafs_cli.rs`). |
| SF6-SR-04 | Baixo | Trilha de auditoria | O release checklist não tinha um log de aprovação assinado para a revisão de segurança. | Foi adicionada uma seção em [release process](../developer-releases.md) exigência de hashes do memorando de revisão e URL do ticket de sign-off antes de GA. |

Todos os achados altos/médios foram corrigidos durante uma janela de revisão e validados pelo chicote de paridade existente. Nenhum problema crítico latente permanece.

## Validação de controle

- **Escopo da credencial:** Modelos de CI agora desativar público e emissor explícitos; o CLI e o release helper falham rapidamente, a menos que `--identity-token-audience` acompanhe `--identity-token-provider`.  
- **Replay determinístico:** Testes atualizados cobrem fluxos positivos/negativos de envio de manifesto, garantindo que digests incompatíveis continuem sendo falhas não determinísticas e expostas sejam antes de tocar a rede.  
- **Proof streaming back-pression:** Torii agora faz stream de itens PoR/PoTR em canais limitados, e o CLI retém apenas amostras de latência truncadas + cinco exemplares de falha, evitando crescimento sem limite e mantendo resumos determinísticos.  
- **Observabilidade:** Contadores de streaming de prova (`torii_sorafs_proof_stream_*`) e resumos CLI capturam motivos de aborto, fornecem migalhas de auditoria aos operadores.  
- **Documentação:** Guias do desenvolvedor ([índice do desenvolvedor](../developer-index.md), [referência CLI](../developer-cli.md)) destacam sinalizadores sensíveis a segurança e escalação de fluxos de trabalho.

## Adições à lista de verificação de lançamento

Os gerentes de lançamento **devem** anexar a seguinte evidência ao promover um candidato GA:1. Hash do memorando de revisão de segurança mais recente (este documento).  
2. Link para o ticket de remediação rastreado (ex.: `governance/tickets/SF6-SR-2026.md`).  
3. Saída de `scripts/release_sorafs_cli.sh --manifest ... --bundle-out ... --signature-out ...` mostrando argumentos explícitos de audiência/emissor.  
4. Logs capturados do chicote de paridade (`cargo test -p sorafs_car -- --nocapture sorafs_cli::proof_stream::bounded_channels`).  
5. Confirmação de que as notas de lançamento do Torii incluem contadores de telemetria de streaming de prova limitada.

Não foram coletados os artefatos acima bloqueados na assinatura do GA.

**Hashes de artefato de referência (aprovação em 20/02/2026):**

- `sf6_security_review.md` - `66001d0b53d8e7ed5951a07453121c075dea931ca44c11f1fcd1571ed827342a`

## Acompanhamentos excelentes

- **Atualização do modelo de ameaça:** Repita esta revisão trimestralmente ou antes de grandes recomendações de sinalizadores CLI.  
- **Cobertura de difusão:** Codificações de transporte de streaming de prova são fuzzed via `fuzz/proof_stream_transport`, cobrindo identidade de cargas úteis, gzip, deflate e zstd.  
- **Ensaio de incidente:** Agende um exercício de operadores simulando comprometimento de token e reversão de manifesto, garantindo que a documentação reflita procedimentos praticados.

## Aprovação

- Representante da Guilda de Engenharia de Segurança: @sec-eng (2026-02-20)  
- Representante do Grupo de Trabalho de Ferramentas: @tooling-wg (2026-02-20)

Guarde as aprovações assinadas junto com o pacote de artefatos de lançamento.