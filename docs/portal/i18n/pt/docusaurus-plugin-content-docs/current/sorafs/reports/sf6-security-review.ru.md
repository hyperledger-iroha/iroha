---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/reports/sf6-security-review.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
título: Отчет по безопасности SF-6
resumo: Результаты и последующие действия по независимой оценке assinatura sem chave, prova de streaming e пайплайнов отправки manifestos.
---

# Отчет по безопасности SF-6

**Configurações:** 10/02/2026 → 18/02/2026  
**Provérbios:** Security Engineering Guild (`@sec-eng`), Grupo de Trabalho de Ferramentas (`@tooling-wg`)  
**Explicado:** CLI/SDK SoraFS (`sorafs_cli`, `sorafs_car`, `sorafs_manifest`), APIs de streaming de prova, manifestos de armazenamento em Torii, integração Sigstore/OIDC, ganchos de liberação CI.  
**Artigos:**  
- CLI e testes padrão (`crates/sorafs_car/src/bin/sorafs_cli.rs`)  
- Manifesto/prova dos manipuladores Torii (`crates/iroha_torii/src/sorafs/api.rs`)  
- Automação de liberação (`ci/check_sorafs_cli_release.sh`, `scripts/release_sorafs_cli.sh`)  
- Chicote de paridade determinística (`crates/sorafs_car/tests/sorafs_cli.rs`, [Отчет о паритете GA SoraFS Orchestrator](./orchestrator-ga-parity.md))

## Metodologia

1. **Workshops de modelagem de ameaças** отразили возможности атакующих для рабочих станций разработчиков, CI систем и Torii узлов.  
2. **Revisão de código** informações de segurança do seu site (token OIDC, assinatura sem chave), validação Norito manifesta e contrapressão em streaming de prova.  
3. **Teste dinâmico** você pode exibir manifestos de fixação e simulações (repetição de token, adulteração de manifesto, fluxos de prova de uso) com chicote de paridade e unidades fuzz especiais.  
4. **Inspeção de configuração** padrões comprovados `iroha_config`, обработку флагов CLI e scripts de liberação, чтобы обеспечить детерминированные и аудируемые programas.  
5. **Entrevista do processo** fornece fluxo de remediação, caminhos de escalonamento e evidências de auditoria fornecidas pelos proprietários da versão Tooling WG.

## Сводка находок| ID | Gravidade | Área | Encontrando | Resolução |
|----|----------|------|---------|------------|
| SF6-SR-01 | Alto | Assinatura sem chave | Os tokens OIDC de auditoria padrão são necessários em modelos de CI, o que representa o risco de reprodução entre locatários. | Você pode usar `--identity-token-audience` em ganchos de liberação e modelos de CI ([processo de liberação](../developer-releases.md), `docs/examples/sorafs_ci.md`). CI теперь падает, если аудитория не указана. |
| SF6-SR-02 | Médio | Transmissão de prova | Os caminhos de contrapressão são principalmente prejudiciais aos buffers que podem ser usados. | `sorafs_cli proof stream` ограничивает размеры каналов с детерминированным truncamento, логирует Norito resumos e abortar поток; O espelho Torii é definido, fornecendo blocos de resposta (`crates/iroha_torii/src/sorafs/api.rs`). |
| SF6-SR-03 | Médio | Manifestos de manifestação | Os manifestos primários da CLI são baseados em planos de blocos padrão, criados por `--plan`. | `sorafs_cli manifest submit` теперь пересчитывает e сравнивает CAR digests, если не указан `--expect-plan-digest`, отклоняя incompatibilidades e выдавая dicas de remediação. Os testes foram feitos com sucesso/armazenamento (`crates/sorafs_car/tests/sorafs_cli.rs`). |
| SF6-SR-04 | Baixo | Trilha de auditoria | A lista de verificação de lançamento não contém uma revisão de segurança do diário. | Добавлен раздел [processo de liberação](../developer-releases.md), требующий приложить hashes memo review e URL тикета sign-off por GA. |

Os valores altos/médios são fornecidos no momento certo e podem ser usados ​​com o chicote de paridade. O problema do script crítico não foi resolvido.

## Controle de validação

- **Escopo da credencial:** Modelos de CI теперь требуют явных público e emissor утверждений; CLI e release helper são usados, como `--identity-token-audience`, não usados ​​com `--identity-token-provider`.  
- **Repetição determinística:** Обновленные тесты покрывают положительные/отрицательные потоки отправки manifestos, гарантируя, что resumos incompatíveis остаются недетерминированными ошибками и выявляются до обращения к сети.  
- **Prova de contrapressão de streaming:** Torii теперь стримит Itens PoR/PoTR через ограниченные каналы, e CLI хранит только усеченные amostras latência + пять примеров отказов, предотвращая неограниченный рост подписчиков e сохраняя детерминированные resumos.  
- **Observabilidade:** Streaming de prova de streaming (`torii_sorafs_proof_stream_*`) e resumos CLI фиксируют причины abort, давая операторам audit breadcrumbs.  
- **Documentação:** Гайды для разработчиков ([índice do desenvolvedor](../developer-index.md), [referência CLI](../developer-cli.md)) отмечают флаги sensíveis à segurança e fluxos de trabalho de escalonamento.

## Дополнения к lista de verificação de lançamento

Gerentes de lançamento **обязаны** приложить следующие доказательства при продвижении GA кандидата:

1. Revisão de segurança do memorando hash последнего (этот документ).  
2. Selecione a remediação do bilhete (exemplo, `governance/tickets/SF6-SR-2026.md`).  
3. Saída `scripts/release_sorafs_cli.sh --manifest ... --bundle-out ... --signature-out ...` с явными аргументами audiência/emissor.  
4. Chicote de paridade Логи (`cargo test -p sorafs_car -- --nocapture sorafs_cli::proof_stream::bounded_channels`).  
5. Em seguida, estas notas de lançamento Torii usam contadores de telemetria streaming de prova limitada.Непредоставление артефактов выше блокирует GA sign-off.

**Hashes de referência артефактов (assinatura em 20/02/2026):**

- `sf6_security_review.md` — `66001d0b53d8e7ed5951a07453121c075dea931ca44c11f1fcd1571ed827342a`

## Оставшиеся acompanhamentos

- **Atualização do modelo de ameaça:** Повторять эту ревизию ежеквартально или перед крупными добавлениями флагов CLI.  
- **Cobertura difusa:** Código de transmissão à prova de streaming fuzz'ятся через `fuzz/proof_stream_transport`, identidade de cargas úteis охватывая, gzip, deflate e zstd.  
- **Ensaio de incidente:** Запланировать операторское упражнение, симулирующее компрометацию токена e manifesto de reversão, чтобы документация отражала отработанные процедуры.

## Aprovação

- Представитель Security Engineering Guild: @sec-eng (2026-02-20)  
- Grupo de Trabalho de Ferramentas: @tooling-wg (2026-02-20)

Obtenha aprovações para liberar o pacote de artefatos.