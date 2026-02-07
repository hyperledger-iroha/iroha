---
lang: pt
direction: ltr
source: docs/portal/docs/nexus/overview.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: visão geral do nexo
título: Обзор Sora Nexus
description: Высокоуровневый обзор архитектуры Iroha 3 (Sora Nexus) с указателями на канонические документы монорепозитория.
---

Nexus (Iroha 3) расширяет Iroha 2 para uma operação multi-lane, fornecendo dados para a governança e use ferramentas para o SDK. Esta página está abrindo o novo `docs/source/nexus_overview.md` em um monitor, este é o portal que você está procurando, como элементы архитектуры сочетаются.

## Линейки релизов

- **Iroha 2** - самостоятельные развертывания для консорциумов ou частных сетей.
- **Iroha 3 / Sora Nexus** - публичная multi-lane сеть, где операторы регистрируют пространства данных (DS) и получают общие инструменты governança, liquidação e observabilidade.
- Esta linha é criada no espaço de trabalho diferente (IVM + conjunto de ferramentas Kotodama), mais SDK instalado, ABI padrão e фикстуры Norito остаются переносимыми. Операторы скачивают pacет `iroha3-<version>-<os>.tar.zst`, чтобы присоединиться к Nexus; обратитесь к `docs/source/sora_nexus_operator_onboarding.md` para полноэкранным чек-листом.

## Blocos de construção

| Componente | Edição | Portal de navegação |
|-----------|---------|-------------|
| Пространство данных (DS) | Определенная governança область исполнения/хранения, владеющая одним или несколькими lane, объявляющая наборы validação, classe privada e política de comunicação + DA. | Sim. [Especificação Nexus](./nexus-spec) para a fábrica. |
| Pista | Детерминированный шард исполнения; выпускает коммитменты, которые упорядочивает глобальное NPoS кольцо. A faixa de classe inclui `default_public`, `public_custom`, `private_permissioned` e `hybrid_confidential`. | [Modelo de pista](./nexus-lane-model) описывает геометрию, префиксы хранения и удержание. |
| Plano de transferência | Placeholder-идентификаторы, фазы маршрутизации e упаковка двойного профиля отслеживают, как одно-лейновые instale a atualização em Nexus. | [Notas de transição](./nexus-transition-notes) документируют каждую фазу миграции. |
| Diretório Espacial | Реестровый контракт, который хранит манифесты + версии DS. O operador irá selecionar o catálogo com o mesmo catálogo antes de usá-lo. | Трекер diff-ов манифестов находится в `docs/source/project_tracker/nexus_config_deltas/`. |
| Via de catálogo | A configuração de segurança `[nexus]` fornece a faixa de IDs com apelido, política de marketing e programa DA. `irohad --sora --config … --trace-config` contém um catálogo confiável para auditores. | Use `docs/source/sora_nexus_operator_onboarding.md` para passo a passo CLI. |
| Liquidação de roteadores | O operador XOR opera, conectando a pista CBDC com a pista de liquidez pública. | `docs/source/cbdc_lane_playbook.md` описывает политические ручки и телеметрические portão. |
| Telemetria/SLOs | Дашборды + алерты em `dashboards/grafana/nexus_*.json` фиксируют высоту lane, backlog DA, задержку liquidação e глубину governança очереди. | [Plano de remediação de telemetria](./nexus-telemetry-remediation) описывает дашборды, алерты и аудит-доказательства. |

## Снимок развертывания

| Faz | Foco | Critérios de avaliação |
|-------|-------|---------------|
| N0 - Fechar beta | O registrador pode ser atualizado (`.sora`), o operador de integração inicial, a via de catálogo estável. | Подписанные DS манифесты + отрепетированные передачи governança. |
| N1 - Публичный запуск | Добавляет суффиксы `.nexus`, аукционы, registrador de autoatendimento, проводку liquidação XOR. | Você testa o resolvedor/gateway de sincronização, o que significa que o provedor de serviços de Internet está configurado para isso. |
| N2 - Segurança | Вводит `.dao`, API de revendedor, análise, relatórios de portal, scorecards para administradores. | Версионированные compliance артефакты, kit de ferramentas on-line para júri de políticas, отчеты прозрачности казначейства. |
| Porta NX-12/13/14 | Mecanismo de conformidade, dados telefônicos e documentação são fornecidos pelos pilotos do parceiro. | [Visão geral Nexus](./nexus-overview) + [operações Nexus](./nexus-operations) опубликованы, дашборды подключены, mecanismo de política слит. |

## Operadores abertos1. **Configurações de configuração** - держите `config/config.toml` синхронизированным с опубликованным каталогом pista e espaços de dados; архивируйте вывод `--trace-config` com um bilhete válido.
2. **Exibir Manipuladores** - verifique o catálogo do pacote com o pacote de espaço do diretório de espaço antes de usá-lo ou atualizá-lo узлов.
3. **Exibir telemetria** - publicar `nexus_lanes.json`, `nexus_settlement.json` e instalar SDKs; envie alertas para o PagerDuty e forneça recursos para o plano de remediação.
4. **Obter informações ** - definir a matriz de serviço em [operações Nexus](./nexus-operations) e executar RCA na tecnologia funcionou em junho.
5. **Governança de Governança** - участвуйте в голосованиях совета Nexus, влияющих на ваши lane, и раз в квартал repita a reversão da instrução (repetir a reversão da instrução `docs/source/project_tracker/nexus_config_deltas/`).

## Sim. também

- Fornecedor canônico: `docs/source/nexus_overview.md`
- Especificação de especificação: [./nexus-spec](./nexus-spec)
- Faixa geométrica: [./nexus-lane-model](./nexus-lane-model)
- Plano de transferência: [./nexus-transition-notes](./nexus-transition-notes)
- Plano de remediação de telemetria: [./nexus-telemetry-remediation](./nexus-telemetry-remediation)
- Operação do runbook: [./nexus-operações](./nexus-operations)
- Método de operação de integração: `docs/source/sora_nexus_operator_onboarding.md`