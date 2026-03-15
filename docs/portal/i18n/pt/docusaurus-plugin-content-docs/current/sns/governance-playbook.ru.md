---
lang: pt
direction: ltr
source: docs/portal/docs/sns/governance-playbook.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::nota História Canônica
Esta página contém `docs/source/sns/governance_playbook.md` e temperatura
служит канонической копией портала. Este arquivo foi criado para PR переводов.
:::

# Плейбук управления Sora Name Service (SN-6)

**Estado:** Finalizado em 24/03/2026 — Versão atualizada para o projeto SN-1/SN-6  
**Roteiro de Ссылки:** SN-6 "Conformidade e Resolução de Disputas", SN-7 "Resolver e Sincronização de Gateway", политика адресов ADDR-1/ADDR-5  
**Explicações:** Rede de registro em [`registry-schema.md`](./registry-schema.md), API de registrador de contrato em [`registrar-api.md`](./registrar-api.md), UX-endereços de acesso em [`address-display-guidelines.md`](./address-display-guidelines.md), e as estruturas existentes em [`docs/account_structure.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/account_structure.md).

Esta é a descrição de como a organização está atualizando o Sora Name Service (SNS)
чартеры, утверждают регистрации, эскалируют споры e подтверждают синхронность
resolver resolvedor e gateway. Em seu roteiro de trabalho, esta CLI
`sns governance ...`, манифесты Norito e artefatos de auditoria usados ​​ediный
операторский источник до N1 (публичного запуска).

## 1. Observância e auditoria

Documento fornecido para:

- Conselho de Governança de Членов, голосующих по чартеру, политикам суффиксов и
  результатам споров.
- Conselho guardião de Членов, которые ввводят экстренные заморозки и пересматривают
  откаты.
- Steward по суффиксам, ведущих очереди registrador, утверждающих аукционы и
  управляющих распределением дохода.
- Operadores de resolvedor/gateway, abertos para a expansão do SoraDNS, habilitados
  Guarda-corpos GAR e телеметрические.
- Команд комплаенса, казначейства и поддержки, которые должны доказать, что
  каждое действие управления оставило аудируемые артефакты Norito.

Ao abrir as configurações de segurança (N0), segurança pública (N1) e segurança (N2),
перечисленные в `roadmap.md`, связывая каждый fluxo de trabalho com novos documentos,
дашбордами и путями эскалации.

## 2. Rolos e cartões de contato| Papel | Desenvolvimento de serviços | Artefatos e telemetria | Escalação |
|------|----------------------|----------------------------------|-----------|
| Conselho de Governança | Разрабатывает и утверждает чартеры, политики суффиксов, вердикты по спорам и ротации steward. | `docs/source/sns/governance_addenda/`, `artifacts/sns/governance/*`, бюллетени совета, сохраненные через `sns governance charter submit`. | Председатель совета + трекер повестки управления. |
| Conselho Tutelar | Выпускает soft/hard заморозки, экстренные каноны e 72 h обзоры. | Тикеты guardião, создаваемые `sns governance freeze`, override-манифесты в `artifacts/sns/guardian/*`. | Ротация guardião de plantão (<=15 min ACK). |
| Sufixo Administradores | Ведут очереди registrador, аукционы, ценовые уровни и коммуникации с клиентами; подтверждают комплаенс. | Политики steward em `SuffixPolicyV1`, ценовые листы, agradecimentos steward рядом с регуляторными мемо. | Lide com programas steward + PagerDuty para funcionar. |
| Operações de registro e cobrança | Abra o ponto de venda `/v2/sns/*`, instale as placas, publique a telemetria e sofra a CLI. | API do registrador ([`registrar-api.md`](./registrar-api.md)), métrica `sns_registrar_status_total`, placa de transferência em `artifacts/sns/payments/*`. | Registrador de gerente de serviço e contato казначейства. |
| Operadores de resolução e gateway | Fornecer SoraDNS, GAR e configurar gateway em um registrador de sincronização; стримят метрики прозрачности. | [`docs/source/soradns/deterministic_hosts.md`](../../../source/soradns/deterministic_hosts.md), [`docs/source/reports/soradns_transparency.md`](../../../source/reports/soradns_transparency.md), `dashboards/alerts/soradns_transparency_rules.yml`. | Resolver SRE de plantão + gateway de operações principais. |
| Tesouraria e Finanças | Taxa de aprovação 70/30, exceções de referência, licenças / certificados de segurança e certificados de SLA. | Манифесты начислений дохода, выгрузки Stripe/казначейства, квартальные KPI приложения в `docs/source/sns/regulatory/`. | Controlador financeiro + compressor. |
| Contato de Conformidade e Regulamentação | Garantir o desenvolvimento global (EU DSA e outros), definir acordos de KPI e fornecer proteção. | Regule o memorando em `docs/source/sns/regulatory/`, decks de referência, selecione `ops/drill-log.md` ou mesa-repetição. | Lide programas completos. |
| Suporte / SRE de plantão | Обрабатывает инциденты (коллизии, дрейф биллинга, простои resolvedor), координирует сообщения cliente e владеет runbook. | Шаблоны инцидентов, `ops/drill-log.md`, лабораторные доказательства, транскрипты Slack/war-room em `incident/`. | Rota SNS de plantão + SRE менеджмент. |

## 3. Artefatos canônicos e históricos| Artefato | Explosão | Atualizado |
|----------|-------------|-----------|
| Planejamento + KPI | `docs/source/sns/governance_addenda/` | Você pode criar contratos com versão de controle, convênios de KPI e atualização de definição, na interface CLI da interface. |
| Lista de exemplos | [`registry-schema.md`](./registry-schema.md) | Estrutura canônica Norito (`NameRecordV1`, `SuffixPolicyV1`, `RevenueAccrualEventV1`). |
| Registrador de contrato | [`registrar-api.md`](./registrar-api.md) | Cargas úteis REST/gRPC, métricas `sns_registrar_status_total` e gancho de governança confiável. |
| Endereços UX | [`address-display-guidelines.md`](./address-display-guidelines.md) | Канонические отображения I105 (предпочтительно) e сжатые (второй выбор), используемые кошельками/эксплорерами. |
| Documentos SoraDNS / GAR | [`docs/source/soradns/deterministic_hosts.md`](../../../source/soradns/deterministic_hosts.md), [`docs/source/reports/soradns_transparency.md`](../../../source/reports/soradns_transparency.md) | Definindo o host, você pode trabalhar com o alfaiate de transparência e fornecer alertas. |
| Memorando de regulamentação | `docs/source/sns/regulatory/` | Заметки приема по юрисдикциям (например, EU DSA), administrador de reconhecimentos, шаблонные приложения. |
| Registro de perfuração | `ops/drill-log.md` | Записи хаос- e IR-репетиций перед выходом из фаз. |
| Artistas humanos | `artifacts/sns/` | A placa de registro, o guardião dos tickets, o resolvedor de diferenças, os KPI são usados ​​e o CLI é fornecido em `sns governance ...`. |

Qual é o valor mínimo do seu artefato nas tabelas
então, esses auditores podem ter uma avaliação de desempenho em 24 horas.

## 4. Плейбуки жизненного цикла

### 4.1 Cartas e administradores

| Shag | Владелец | CLI / Documentação | Nomeação |
|-----|----------|----------------------|-----------|
| Черновик приложения и KPI delta | Докладчик совета + lider steward | Markdown definido em `docs/source/sns/governance_addenda/YY/` | Включить ID KPI convênio, ganchos de telemetria e ativação de uso. |
| Possibilidade de implementação | Председатель совета | `sns governance charter submit --input SN-CH-YYYY-NN.md` (como `CharterMotionV1`) | CLI envia o manifesto Norito para `artifacts/sns/governance/<id>/charter_motion.json`. |
| Reconhecimento e reconhecimento do tutor | Совет + guardiões | `sns governance ballot cast --proposal <id>` e `sns governance guardian-ack --proposal <id>` | Siga os protocolos de configuração e a entrega da cor. |
| Administrador da província | Administrador do programa | `sns governance steward-ack --proposal <id> --signature <file>` | Требуется до смены политик суффикса; soхранить конверт em `artifacts/sns/governance/<id>/steward_ack.json`. |
| Atividade | Operações de registrador | Обновить `SuffixPolicyV1`, сбросить кэши registrar, опубликовать заметку в `status.md`. | A ativação do registro foi feita em `sns_governance_activation_total`. |
| Registro de auditoria | Compensação | Coloque a informação em `docs/source/sns/regulatory/<jurisdiction>/<cycle>.md` e no registro de perfuração, exceto na mesa. | Selecione opções de dados telefônicos e diferenças de política. |

### 4.2 Registro de registro, autenticação e senha1. **Preflight:** registrador запрашивает `SuffixPolicyV1`, чтобы подтвердить ценовой
   уровень, доступные сроки и окна graça/redenção. Listas de opções de listas
   sincronizar com a tabela 3/4/5/6-9/10+ (базовый уровень +
   коэффициенты суффикса), описанной no roteiro.
2. **Auctions de lance selado:** Para o prêmio premium você terá um ciclo de 72 h commit / 24 h
   revelar через `sns governance auction commit` / `... reveal`. Abrir espião
   commit (только хеши) em `artifacts/sns/auctions/<name>/commit.json`, чтобы
   аудиторы могли проверить случайность.
3. **Plata de registro:** registro de validação `PaymentProofV1` proteção de segurança
   казначейства (70% tesouraria / 30% administrador com separação de referência 72 h, всплески ошибок registrador, дрейф ARPU).

### 4.4 Esportes, esportes e apelos| Faz | Владелец | Действие e доказательства | SLA |
|------|----------|---------------------------|-----|
| Congelamento suave | Administrador / поддержка | Compre o bilhete `SNS-DF-<id>` com uma placa de transferência, selecione um suporte de títulos e selecione uma seleção. | <=4 horas após a postagem. |
| Guardião ingresso | Quadro guardião | `sns governance freeze --selector <I105> --reason <text> --until <ts>` é igual a `GuardianFreezeTicketV1`. Сохранить JSON em `artifacts/sns/guardian/<id>.json`. | <=30 min ACK, <=2 h выполнение. |
| Ратификация совета | Conselho de governação | Утвердить или отклонить заморозки, задокументировать решение со ссылкой на Guardian тикет и Digest Bond спора. | A solução mais importante é o sucesso ou o valor assíncrono. |
| Painel de arbitragem | Complans + mordomo | Созвать панель из 7 instruções (roteiro detalhado) com хешированными бюллетенями через `sns governance dispute ballot`. Use o pacote anonimamente para receber o pacote. | Вердикт <=7 дней после внесения bond. |
| Apelação | Guardião + aviso | Апелляции удваивают bond e повторяют процесс присяжных; Selecione o manifesto Norito `DisputeAppealV1` e solicite o seu bilhete original. | <=10 de janeiro. |
| Reparação e reparação | Operações de registrador + resolvedor | Selecione `sns governance unfreeze --selector <I105> --ticket <id>`, registre o status do registrador e распространить diff GAR/resolver. | Essa é a verdade. |

Экстренные каноны (заморозки, инициированные guardião <=72 h) следуют тому же
потоку, но требуют ретроспективного обзора совета и заметки о прозрачности в
`docs/source/sns/regulatory/`.

### 4.5 Resolvedor e gateway de transferência

1. **Event hook:** каждое событие реестра отправляется в поток событий resolver
   (`tools/soradns-resolver` SSE). As operações do Resolver podem ser usadas e usadas no diff через
   alfaiate de transparência (`scripts/telemetry/run_soradns_transparency_tail.sh`).
2. **Обновление GAR шаблона:** gateways должны обновить GAR шаблоны, на которые
   selecione `canonical_gateway_suffix()` e verifique o código `host_pattern`.
   Verifique a diferença em `artifacts/sns/gar/<date>.patch`.
3. **Arquivo de zona de distribuição:** Use arquivo de zona de esqueleto, especificado em `roadmap.md`
   (nome, ttl, cid, prova), e отправьте его em Torii/SoraFS. Arquivar Norito JSON
   em `artifacts/sns/zonefiles/<name>/<version>.json`.
4. **Produção de processo:** Abra `promtool test rules dashboards/alerts/tests/soradns_transparency_rules.test.yml`
   чтобы убедиться, что алерты зеленые. Use o texto do Prometheus para
   еженедельному отчету о прозрачности.
5. **Gateway Аудит:** Запишите образцы заголовков `Sora-*` (política de cache, CSP, resumo GAR)
   e приложите их к журналу управления, seus operadores podem doar, seu gateway
   обслужил новое имя с нужными guarda-corpos.

## 5. Telemetria e abertura| Sinal | Estocado | Descrição / Desenvolvimento |
|--------|----------|----------------------|
| `sns_registrar_status_total{result,suffix}` | Registrador de registro Torii | Счетчик успех/ошибка para registro, proдлений, заморозок, трансферов; alerta para a posição `result="error"` por sufixo. |
| `torii_request_duration_seconds{route="/v2/sns/*"}` | Métricas Torii | SLO para dados latentes para API обработчиков; instalado em `torii_norito_rpc_observability.json`. |
| `soradns_bundle_proof_age_seconds` e `soradns_bundle_cid_drift_total` | Resolvedor de transparência | Выявляют устаревшие доказательства или дрейф GAR; guarda-corpos instalados em `dashboards/alerts/soradns_transparency_rules.yml`. |
| `sns_governance_activation_total` | CLI de governança | Счетчик, увеличивающийся при активации чартеров/приложений; используется для сверки решений совета e опубликованных adendos. |
| Medidor `guardian_freeze_active` | Guardião CLI | Отслеживает окна congelamento suave/forte no seletor; пейджить SRE, если значение `1` держится дольше SLA. |
| Definição de KPI | Finanças / Documentação | Ежемесячные rollup публикуются вместе с регуляTORными мемо; portal встраивает их через [SNS KPI dashboard](./kpi-dashboard.md), чтобы stewards и регуляторы видели одинаковый Grafana вид. |

## 6. Treine para doar e auditar

| Destino | Doação para arquivo | Oral |
|----------|----------------------------|-----------|
| Carta de definição / política | Подписанный Norito манифест, CLI транскрипт, KPI diff, reconhecimento de administração. | `artifacts/sns/governance/<proposal-id>/` + `docs/source/sns/governance_addenda/`. |
| Registo / Prodlение | Carga útil `RegisterNameRequestV1`, `RevenueAccrualEventV1`, placa de transferência. | `artifacts/sns/payments/<tx>.json`, API do registrador de log. |
| Aukцion | Comprometer/revelar манифесты, seed случайности, таблица расчета победителя. | `artifacts/sns/auctions/<name>/`. |
| Raspar / разморозка | Tíquete do guardião, sua política de privacidade, log de ocorrência de URL, cliente de comunicação de segurança. | `artifacts/sns/guardian/<ticket>/`, `incident/<date>-sns-*.md`. |
| Resolvedor de resolução | Zonefile/GAR diff, JSONL выписка tailer, снимок Prometheus. | `artifacts/sns/resolver/<date>/` + saída de processamento. |
| Ingestão regulada | Memorando de admissão, трекер дедлайнов, administrador de reconhecimento, сводка KPI изменений. | `docs/source/sns/regulatory/<jurisdiction>/<cycle>.md`. |

## 7. Verifique a configuração do seu carro

| Faz | Critérios de avaliação | Pacotes de entrega |
|------|------------------|--------------------|
| N0 — Abrir beta | Схема реестра SN-1/SN-2, ручной registrador CLI, завершенный Guardian Drill. | Charte motion + steward ACK, registrador de log de simulação, отчет о прозрачности resolvedor, verificado em `ops/drill-log.md`. |
| N1 — Публичный запуск | Аукционы + фиксированные ценовые уровни для `.sora`/`.nexus`, registrador de autoatendimento, resolvedor de sincronização automática, биллинг дашборды. | Diferentes listas de números, registradores de CI de resultados, registros de placa/KPI, alfaiates de transparência de seus clientes, registros de informações de repetição. |
| N2 — Расширение | `.dao`, API de revendedor, suportes de portal, scorecards de administrador, dados de análise. | Скриншоты портала, SLA métrica споров, выгрузки steward scorecards, обновленный чартер управления с политиками revendedor. |Você pode usar brocas de mesa para fazer uso de brocas de mesa (счастливый путь регистрации,
заморозка, solucionador de interrupção) с артефактами в `ops/drill-log.md`.

## 8. Registo de incidentes e escalações

| Trigger | Уровень | Немедленный владелец | Destino de execução |
|---------|---------|----------------------|-----------------------|
| Resolver/GAR ou instalar um arquivo | 1º de setembro | Resolver SRE + quadro guardião | Пейджить resolvedor de plantão, собрать вывод tailer, решить вопрос о заморозке затронутых имен, публиковать статус каждые 30 min. |
| Registrador de software, software de faturamento ou API de API | 1º de setembro | Gerente de serviço de registrador | Para criar novos leilões, acesse a CLI, use stewards/казначейство, registre o log Torii para incidente. |
| Esporte por nome, placa não solicitada ou escala de cliente | 2 de setembro | Administrador + suporte de liderança | Собрать доказательства платежа, определить необходимость soft freeze, ответить заявителю в SLA, записать результат в трекере спора. |
| Amplificador de áudio | 2 de setembro | Contato de conformidade | Para planejar a solução, registre a mensagem em `docs/source/sns/regulatory/` e instale a sessão correta. |
| Broca ou repetição | 3 de setembro | Programa PM | Verifique o cenário de `ops/drill-log.md`, архивировать артефакты, отметить пробелы как задачи roadmap. |

Quais são os incidentes que ocorrem com `incident/YYYY-MM-DD-sns-<slug>.md` na tabela
владения, журналами команд e ссылками на доказательства, собранные по этому
плейбуку.

## 9. Ссылки

- [`registry-schema.md`](./registry-schema.md)
- [`registrar-api.md`](./registrar-api.md)
- [`address-display-guidelines.md`](./address-display-guidelines.md)
- [`docs/account_structure.md`](../../../account_structure.md)
- [`docs/source/soradns/deterministic_hosts.md`](../../../source/soradns/deterministic_hosts.md)
- [`docs/source/reports/soradns_transparency.md`](../../../source/reports/soradns_transparency.md)
-`ops/drill-log.md`
- `roadmap.md` (SNS, DG, ADDR instalados)

Verifique este conector atual através da textura de texto, CLI поверхностей
ou contratos de telefonia; roteiro de elementos, seleção de
`docs/source/sns/governance_playbook.md`, должны всегда соответствовать
последней редакции.