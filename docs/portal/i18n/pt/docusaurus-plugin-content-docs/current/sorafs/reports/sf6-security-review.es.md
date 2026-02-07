---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/reports/sf6-security-review.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
título: Revisão de segurança SF-6
resumo: Hallazgos e tarefas de acompanhamento da avaliação independente de assinatura sem chave, streaming de prova e pipelines de envio de manifestos.
---

# Revisão de segurança SF-6

**Ventana de avaliação:** 10/02/2026 -> 18/02/2026  
**Líderes de revisão:** Security Engineering Guild (`@sec-eng`), Tooling Working Group (`@tooling-wg`)  
**Alcance:** SoraFS CLI/SDK (`sorafs_cli`, `sorafs_car`, `sorafs_manifest`), APIs de streaming de prova, gerenciamento de manifestos em Torii, integração Sigstore/OIDC, ganchos de liberação em CI.  
**Artefatos:**  
- Fonte do CLI e testes (`crates/sorafs_car/src/bin/sorafs_cli.rs`)  
- Manipuladores de manifesto/prova em Torii (`crates/iroha_torii/src/sorafs/api.rs`)  
- Automatização de liberação (`ci/check_sorafs_cli_release.sh`, `scripts/release_sorafs_cli.sh`)  
- Chicote de paridade determinista (`crates/sorafs_car/tests/sorafs_cli.rs`, [Relatório de Paridade GA do Orquestrador SoraFS](./orchestrator-ga-parity.md))

## Metodologia

1. **Workshops de modelagem de ameaças** mapeou capacidades de desafios para estações de trabalho de desenvolvedores, sistemas CI e nós Torii.  
2. **Revisão de código** foco em superfícies de credenciais (troca de tokens OIDC, assinatura sem chave), validação de manifestos Norito e contrapressão em streaming de prova.  
3. **Teste dinâmico** reprodução de manifestos de fixtures e modos de simulação de falha (replay de token, adulteração de manifesto, streams de prova truncados) usando o chicote de paridade e fuzz drives conforme medida.  
4. **Inspeção de configuração** padrões de validação de `iroha_config`, gerenciamento de sinalizadores da CLI e scripts de liberação para garantir execuções deterministas e auditáveis.  
5. **Entrevista de processo** confirma o fluxo de remediação, rotas de escalada e captura de evidências de auditorias com os proprietários de liberação do Tooling WG.

## Resumo de hallazgos| ID | Severidade | Área | Hallazgo | Resolução |
|----|----------|------|---------|------------|
| SF6-SR-01 | Alta | Assinatura sem chave | Os padrões de audiência do token OIDC estavam implícitos nos modelos de CI, com risco de repetição entre locatários. | É agregado o aplicativo explícito de `--identity-token-audience` em ganchos de lançamento e modelos de CI ([processo de lançamento](../developer-releases.md), `docs/examples/sorafs_ci.md`). CI agora falhará se omitir a audiência. |
| SF6-SR-02 | Mídia | Transmissão de prova | Os caminhos de contrapressão aceitam buffers de descritores sem limite, habilitando agotamiento de memória. | `sorafs_cli proof stream` impone tamanos de canal acotados com truncamento determinista, registrando retoma Norito e abortando o fluxo; o espelho Torii é atualizado para receber pedaços de resposta (`crates/iroha_torii/src/sorafs/api.rs`). |
| SF6-SR-03 | Mídia | Envio de manifestos | A CLI aceita manifestos sem verificar planos de pedaços incorporados quando `--plan` está ausente. | `sorafs_cli manifest submit` agora recomputa e compara resumos de CAR salvo que se provou `--expect-plan-digest`, rechazando incompatibilidades e mostrando pistas de remediação. Os testes registram casos de saída/falha (`crates/sorafs_car/tests/sorafs_cli.rs`). |
| SF6-SR-04 | Baixa | Trilha de auditoria | A lista de verificação de liberação contém um registro de aprovação firmado para a revisão de segurança. | Se agregar uma seção em [processo de liberação](../developer-releases.md) que requer hashes adicionais do memorando de revisão e URL do ticket de assinatura antes do GA. |

Todos os hallazgos altos/médios são corrigidos durante a janela de revisão e validados com o arnês de paridade existente. Nenhuma questão emite críticos latentes.

## Validação de controles

- **Alcance de credenciais:** Os modelos de CI agora exigem audiência e emissor explícitos; o CLI e o auxiliar de liberação caem rapidamente salvo que `--identity-token-audience` acompanha um `--identity-token-provider`.  
- **Replay determinista:** Testes atualizados cubren flujos positivos/negativos de envio de manifestos, assegurando que os digests desalineados sigan siendo fallas no deterministas e se detectem antes de tocar la red.  
- **Back-pression en proof streaming:** Torii agora transmite itens PoR/PoTR sobre canais acotados, e o CLI retém apenas mostras truncadas de latência + cinco exemplos de falha, evitando crescimento sem limite e mantendo currículos deterministas.  
- **Observabilidade:** Contadores de streaming de prova (`torii_sorafs_proof_stream_*`) e resumos do CLI capturando motivos de aborto, entregando migalhas de auditoria aos operadores.  
- **Documentação:** Guias para desenvolvedores ([índice do desenvolvedor](../developer-index.md), [referência CLI](../developer-cli.md)) indicam sinalizadores sensíveis, segurança e fluxos de trabalho de escalação.

## Adicionados ao checklist de lançamento

Os gerentes de lançamento **deben** adicionar a seguinte evidência ao promover um candidato GA:1. Hash do memorando mais recente de revisão de segurança (este documento).  
2. Link para o ticket de remediação seguido (por exemplo, `governance/tickets/SF6-SR-2026.md`).  
3. Saída de `scripts/release_sorafs_cli.sh --manifest ... --bundle-out ... --signature-out ...` mostrando argumentos explícitos de audiência/emissor.  
4. Logs capturados do arnês de segurança (`cargo test -p sorafs_car -- --nocapture sorafs_cli::proof_stream::bounded_channels`).  
5. Confirmação de que as notas de lançamento do Torii incluem contadores de telemetria de prova de streaming acotados.

Não recolha os artefatos anteriores que bloqueiam a assinatura do GA.

**Hashes de artefatos de referência (assinatura em 20/02/2026):**

- `sf6_security_review.md` — `66001d0b53d8e7ed5951a07453121c075dea931ca44c11f1fcd1571ed827342a`

## Seguimentos pendentes

- **Atualização do modelo de ameaça:** Repetir esta revisão trimestralmente ou antes de grandes adições de sinalizadores da CLI.  
- **Cobertura de fuzzing:** As codificações de transporte de prova de streaming são fuzzearon via `fuzz/proof_stream_transport`, identificando cargas úteis, gzip, deflate e zstd.  
- **Ensaio de incidentes:** Programar um exercício de operadores que simule o comprometimento do token e a reversão do manifesto, garantindo que a documentação reflita os procedimentos praticados.

## Aprovação

- Representante da Guilda de Engenharia de Segurança: @sec-eng (2026-02-20)  
- Representante do Grupo de Trabalho de Ferramentas: @tooling-wg (2026-02-20)

Armazene as aprovações firmadas junto com o pacote de artefatos de lançamento.