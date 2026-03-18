---
lang: pt
direction: ltr
source: docs/portal/docs/soranet/testnet-rollout.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: lançamento testnet
título: Testnet SoraNet (SNNet-10)
sidebar_label: Testnet de teste (SNNet-10)
descrição: Plano de ativação, kit de integração e portas de telemetria para testar a rede de teste SoraNet.
---

:::nota História Canônica
Esta página está planejando o lançamento do SNNet-10 em `docs/source/soranet/testnet_rollout_plan.md`. Selecione uma cópia sincronizada, pois os documentos não serão exibidos na configuração.
:::

SNNet-10 координирует поэтапную активацию sobreposição de anonimato SoraNet em nossa página. Используйте этот план, чтобы перевести roadmap bullet em конкретные entregas, runbooks e portas de telemetria, чтобы каждый operador понимал ожидания до Além disso, como SoraNet fornece transporte para uso.

## Faça uma pausa

| Faz | Таймлайн (цель) | Sim | Artefactos de construção |
|-------|-------------------|-------|---------|
| **T0 - Testnet Fechado** | 4º trimestre de 2026 | 20-50 relés em >=3 ASNs, principais contribuidores. | Kit de integração Testnet, suíte de fumaça de fixação de proteção, latência de linha de base + métricas PoW, registro de perfuração de brownout. |
| **T1 - Beta Público** | 1º trimestre de 2027 | >=100 relés, rotação de guarda ativada, ligação de saída definida, SDK betas para uso SoraNet com `anon-guard-pq`. | Kit de integração completo, lista de verificação de verificação do operador, SOP de publicação de diretório, pacote de painel de telemetria, relatórios de ensaio de incidentes. |
| **T2 - Mainnet Padrão** | 2º trimestre de 2027 (com acesso ao SNNet-6/7/9) | Produção сеть по умолчанию SoraNet; включены transportes obfs/MASQUE e catraca PQ de aplicação. | Governança de aprovação de protocolos, procedimento de reversão somente direto, alarmes de downgrade, relatórios de métricas de sucesso de relatórios. |

**Пути пропуска нет** - каждая фаза обязана доставить telemetria e artefatos de governança предыдущей стадии до повышения.

## Kit de integração Testnet

O operador de retransmissão Каждый получает детерминированный pacote com os dados definidos:

| Artefato | Descrição |
|----------|------------|
| `01-readme.md` | Obrigado, contatos e таймлайн. |
| `02-checklist.md` | Lista de verificação pré-voo (hardware, acessibilidade da rede, verificação da política de proteção). |
| `03-config-example.toml` | Минимальная конфигурация relé + orquestrador SoraNet, согласованная с blocos de conformidade SNNet-9, включая блок `guard_directory`, который pin-it hash последнего instantâneo de guarda. |
| `04-telemetry.md` | Instruções para usar painéis de métricas de privacidade SoraNet e limites de alerta. |
| `05-incident-playbook.md` | Proceda a reações de brownout/downgrade com escalas de matriz. |
| `06-verification-report.md` | Shablon, seu operador realizou e realizou vários testes de fumaça. |

Faça uma cópia legal em `docs/examples/soranet_testnet_operator_kit/`. Kit completo; número da versão correta (por exemplo, `testnet-kit-vT0.1`).

Para o operador public-beta (T1), o resumo de integração está escrito em `docs/source/soranet/snnet10_beta_onboarding.md` resumindo pré-requisitos, entregas de telemetria e fluxo de trabalho, você pode usar o kit de determinação e ajudantes do validador.`cargo xtask soranet-testnet-feed` gera feed JSON, janela de promoção de agregação, lista de retransmissão, relatório de métricas, evidências de exercício e hashes de anexo, além de modelo de stage-gate definido. Сначала подпишите logs de perfuração e acessórios через `cargo xtask soranet-testnet-drill-bundle`, чтобы feed зафиксировал `drill_log.signed = true`.

## Метрики успеха

Antes de minha informação ser obtida por meio de telemetria, você precisará de um mínimo de dois dias:

- `soranet_privacy_circuit_events_total`: 95% dos circuitos estão desativados sem quedas de energia ou downgrade; оставшиеся 5% ограничены fornecem PQ.
- `sorafs_orchestrator_policy_events_total{outcome="brownout"}`: =99% em cada 3 s; relatório de referência `soranet_privacy_throttles_total{scope="congestion"}`.
- Latência (percentil 95) por região: <200 ms por circuitos de после полного построения, фиксируется через `soranet_privacy_rtt_millis{percentile="p95"}`.

Dashboard e modelos de alerta colocados em `dashboard_templates/` e `alert_templates/`; отзеркальте их вашем repositório de telemetria e добавьте в CI lint verificações. Utilize `cargo xtask soranet-testnet-metrics` para gerar governança-ориентированного отчета перед запросом повышения.

Os envios do stage-gate são baseados em `docs/source/soranet/snnet10_stage_gate_template.md`, que foram criados para obter o formulário de Markdown em `docs/examples/soranet_testnet_stage_gate/stage_gate_report_template.md`.

## Lista de verificação de verificação

Операторы должны подписаться под следующим перед входом в каждую фазу:

- ✅ Retransmitir anúncio подписан текущим envelope de admissão.
- ✅ Teste de fumaça de rotação da guarda (`tools/soranet-relay --check-rotation`) fornecido.
- ✅ `guard_directory` указывает на последний artefato `GuardDirectorySnapshotV2` e `expected_directory_hash_hex` совпадает com resumo do comitê (por iniciar relé логирует проверенный haxixe).
- ✅ Catraca métrica PQ (`sorafs_orchestrator_pq_ratio`) держатся выше целевых порогов для запрошенной стадии.
- ✅ Configuração de conformidade GAR fornecida com a tag последним (см. каталог SNNet-9).
- ✅ Alarme de downgrade de simulação (coletores de número, alerta de alerta na tecnologia 5 minutos).
- ✅ Broca PoW/DoS é usada para mitigação de problemas documentados.

O kit de integração está incluído no kit de integração. Os operadores usam o helpdesk de governança para obter credenciais de produção.

## Governança e implementação

- **Controle de alterações:** promoções требуют aprovação Conselho de Governança, зафиксированный в atas do conselho e приложенный к página de status.
- **Resumo de status:** публиковать еженедельные обновления с количеством relés, relação PQ, инцидентами brownout e открытыми itens de ação (хранится в `docs/source/status/soranet_testnet_digest.md` é o início da cadencia).
- **Rollbacks:** Você pode alterar o plano de reversão, configurar a configuração em um período pré-definido de 30 minutos, ativar invalidação de DNS/guard cache e modelos de comunicação com o cliente.

## Ativos de apoio- `cargo xtask soranet-testnet-kit [--out <dir>]` fornece kit de integração de `xtask/templates/soranet_testnet/` no catálogo principal (por usar `docs/examples/soranet_testnet_operator_kit/`).
- `cargo xtask soranet-testnet-metrics --input <metrics.json> [--out <path|->]` fornece métricas de sucesso do SNNet-10 e fornece uma estrutura de aprovação/reprovação para análises de governança. O primeiro snapshot foi colocado em `docs/examples/soranet_testnet_metrics_sample.json`.
- Modelos Grafana e Alertmanager colocados em `dashboard_templates/soranet_testnet_overview.json` e `alert_templates/soranet_testnet_rules.yml`; verifique-o no repositório de telemetria ou verifique as verificações de lint do CI.
- A comunicação de downgrade para SDK/mensagens do portal está disponível em `docs/source/soranet/templates/downgrade_communication_template.md`.
- Еженедельные status digests должны использовать `docs/source/status/soranet_testnet_weekly_digest.md` как каноническую форму.

Pull requests podem ser usados ​​​​no local em que você usa artefatos de cálculo ou telemetria, plano de implementação definido como canônico.