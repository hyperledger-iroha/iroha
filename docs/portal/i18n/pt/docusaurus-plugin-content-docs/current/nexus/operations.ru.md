---
lang: pt
direction: ltr
source: docs/portal/docs/nexus/operations.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: operações de nexo
título: Runbook para operação Nexus
Descrição: A operação prática do operador Nexus, operação `docs/source/nexus_operations.md`.
---

Use esta página como a fonte de alimentação para `docs/source/nexus_operations.md`. Ao configurar a operação de verificação, você pode atualizar a configuração e transferir a telefonia, enquanto use o operador Nexus.

## Чек-list жизненного цикла

| Etap | Destino | Documentar |
|-------|--------|----------|
| Primeiro | Verifique a configuração/referência da chave, verifique `profile = "iroha3"` e configure a configuração. | Use `scripts/select_release_profile.py`, soma de verificação diária, pacotes de pacotes disponíveis. |
| Catálogo de visualização | Obtenha o catálogo `[nexus]`, verifique a política de marketing e forneça-a para a manutenção da União Soviética, para garantir a segurança `--trace-config`. | Вывод `irohad --sora --config ... --trace-config`, сохраненный с тикетом onboarding. |
| Fumaça e corte | Запустить `irohad --sora --config ... --trace-config`, выполнить CLI smoke (`FindNetworkStatus`), проверить эксport телеметрии и запросить admissão. | Teste de fumaça + подтверждение Alertmanager. |
| Regime de estado | Confie em painéis/alertas, controles de gerenciamento de governança gráfica e configurações de sincronização/runbooks por meio de manuais personalizados. | Протоколы квартального обзора, скриншоты дашбордов, ID тикетов ротации. |

Possível onboarding (sem chave, número de mercado, serviço de perfil de usuário) localizado em `docs/source/sora_nexus_operator_onboarding.md`.

## Atualizações de atualização

1. **Rede de Confiança** - envie-nos para `status.md`/`roadmap.md`; прикладывайте чек-лист onboarding к каждому PR релиза.
2. **Gerenciar faixa de gerenciamento** - forneça pacotes configuráveis ​​no Diretório do Espaço e arquive-os em `docs/source/project_tracker/nexus_config_deltas/`.
3. **Delьты конфигурации** - каждое изменение `config/config.toml` требует тикет сссылкой на lane/data-space. Сохраняйте редактированную копию эффективной конфигурации при join/upgrade узлов.
4. **Reversão de Тренировки** - ежеквартально репетируйте parar/restaurar/fumaça; фиксируйте результаты em `docs/source/project_tracker/nexus_config_deltas/<date>-rollback.md`.
5. **Conformidade de conformidade** - faixas privadas/CBDC должны получить одобрение conformidade перед изменением политики DA ou botões редактирования telefone (veja `docs/source/cbdc_lane_playbook.md`).

## Telemetria e SLOs

- Dados: `dashboards/grafana/nexus_lanes.json`, `nexus_settlement.json`, e também vídeos do SDK (por exemplo, `android_operator_console.json`).
- Alertas: `dashboards/alerts/nexus_audit_rules.yml` e transporte Torii/Norito (`dashboards/alerts/torii_norito_rpc_rules.yml`).
- Métricas para monitoramento:
  - `nexus_lane_height{lane_id}` - alerta de progresso de três slots.
  - `nexus_da_backlog_chunks{lane_id}` - алерт выше порогов для lane (em 64 público / 8 privado).
  - `nexus_settlement_latency_seconds{lane_id}` - алерт когда P99 demora 900 ms (público) ou 1200 ms (privado).
  - `torii_request_failures_total{scheme="norito_rpc"}` - alerta если 5 minutos antes de 2%.
  - `telemetry_redaction_override_total` - 2 de setembro de 2016; Claro, isso substitui a conformidade com os tíquetes.
- Выполняйте чек-лист телеметрии из [Plano de remediação de telemetria Nexus](./nexus-telemetry-remediation) минимум ежеквартально и прикладывайте заполненную форму к заметкам операционного обзора.

##Matriz de incidentes

| Gravidade | Expedição | Exibição |
|----------|------------|----------|
| 1º de setembro | Нарушение изоляции espaço de dados, liquidação остановка >15 минут или порча голосования governança. | Пейджинг Nexus Primário + Engenharia de Liberação + Conformidade, admissão de segurança, artefactos de construção, comunicação de выпустить <=60 minutos, RCA <=5 рабочих дней. |
| 2 de setembro | Faixa de backlog de SLA aberta, intervalo de telemetria >30 minutos, manual de implementação provado. | Пейджинг Nexus Primário + SRE, смягчение <=4 horas, оформить acompanhamentos na técnica 2 dias de semana. |
| 3 de setembro | Não блокирующий дрейф (documentos, alertas). | Зафиксировать в tracker e планировать исправление в спринте. |

Тикеты инцидентов должны фиксировать затронутые IDs lane/data-space, хэши манифестов, таймлайн, поддерживающие métricas/logs, e acompanhamento de avaliações/ответственных.

##Archiv доказательств

- Consulte pacotes/manifestos/exportações telefónicas em `artifacts/nexus/<lane>/<date>/`.
- Сохраняйте редактированные configs + вывод `--trace-config` para каждого релиза.
- Abra o protocolo de configuração + configure a configuração ou a manifestação.
- Храните еженедельные snapshots Prometheus na métrica Nexus na técnica 12 meses.
- Фиксируйте правки runbook em `docs/source/project_tracker/nexus_config_deltas/README.md`, чтобы аудиторы знали, когда менялись обязанности.

## Materiais úteis- Opção: [Visão geral Nexus](./nexus-overview)
Especificação: [especificação Nexus] (./nexus-spec)
- Pista geométrica: [modelo de pista Nexus] (./nexus-lane-model)
- Correção e calços de roteamento: [Nexus notas de transição](./nexus-transition-notes)
- Operadores de integração: [integração do operador Sora Nexus] (./nexus-operator-onboarding)
- Ремедиация телеметрии: [Nexus plano de remediação de telemetria](./nexus-telemetry-remediation)