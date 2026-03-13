---
lang: pt
direction: ltr
source: docs/portal/docs/soranet/privacy-metrics-pipeline.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: pipeline de métricas de privacidade
título: Conversor de métricas privado SoraNet (SNNet-8)
sidebar_label: Conversor de métricas privadas
descrição: Сбор телеметрии сохранением приватности para relé e orquestrador SoraNet.
---

:::nota História Canônica
Verifique `docs/source/soranet/privacy_metrics_pipeline.md`. Selecione uma cópia sincronizada, mas não será necessário usar a documentação atual.
:::

# Conversor de métricas privadas SoraNet

SNNet-8 é orientado para o relé de tempo de execução. Теперь relé агрегирует события handshake e circuito em минутные baldes e экспортирует только грубые счетчики Prometheus, оставляя Os circuitos externos não são adequados e o funcionamento prático é exibido.

## Обзор агрегатора

- Runtime-realization находится в `tools/soranet-relay/src/privacy.rs` como `PrivacyAggregator`.
- Buckets индексируются durante um minuto no final do período (`bucket_secs`, durante 60 segundos) e хранятся em ограниченном coluna (`max_completed_buckets`, em 120). Coletor compartilha имеют собственный ограниченный backlog (`max_share_lag_buckets`, em умолчанию 12), чтобы устаревшие окна Prio сбрасывались Como baldes suprimidos, e não usados ​​em coletores ou coletores de máscara.
- `RelayConfig::privacy` é compatível com `PrivacyConfig`, conjunto de configuração (`bucket_secs`, `min_handshakes`, `flush_delay_buckets`, `force_flush_buckets`, `max_completed_buckets`, `max_share_lag_buckets`, `expected_shares`). No tempo de execução de produção, você pode usar o SNNet-8a para obter uma agregação segura.
- O tempo de execução do módulo fornece ajudantes de tipo: `record_circuit_accepted`, `record_circuit_rejected`, `record_throttle`, `record_throttle_cooldown`, `record_capacity_reject`, `record_active_sample`, `record_verified_bytes` e `record_gar_category`.

## Retransmissor Админ-эндпоинт

O operador pode ativar o relé de ouvinte admin para o dispositivo `GET /privacy/events`. O ponto de venda JSON é gerado por um bloco de distribuição (`application/x-ndjson`), содержащий payloads `SoranetPrivacyEventV1`, отраженные из recomendado `PrivacyEventBuffer`. A proteção do buffer é nova para `privacy.event_buffer_capacity` (por умолчанию 4096) e очищается при чтении, поэтому скрейперам нужно опрашивать достаточно часто, чтобы избежать пропусков. События покрывают те же сигналы handshake, acelerador, largura de banda verificada, circuito ativo e GAR, которые питают счетчики Prometheus, позволяя coletores downstream архивировать приватно-безопасные breadcrumbs ou подпитывать fluxos de trabalho безопасной агрегации.

## Relé de configuração

O operador ajusta a cadência da telemetria privada na configuração do relé de segurança `privacy`:

```json
{
  "mode": "Entry",
  "listen": "0.0.0.0:443",
  "privacy": {
    "bucket_secs": 60,
    "min_handshakes": 12,
    "flush_delay_buckets": 1,
    "force_flush_buckets": 6,
    "max_completed_buckets": 120,
    "max_share_lag_buckets": 12,
    "expected_shares": 2
  }
}
```

Obtenha informações sobre as especificações do SNNet-8 e verifique o seguinte:| Pólo | Descrição | Para умолчанию |
|------|----------|-------------|
| `bucket_secs` | Ширина каждого окна агрегации (секунды). | `60` |
| `min_handshakes` | A quantidade mínima de água que você precisa pode ser usada no balde. | `12` |
| `flush_delay_buckets` | Количество завершенных baldes перед попыткой flush. | `1` |
| `force_flush_buckets` | Максимальный возраст перед выпуском balde suprimido. | `6` |
| `max_completed_buckets` | Limite os buckets do backlog (não há muitos intervalos sem grana). | `120` |
| `max_share_lag_buckets` | As ações de colecionador estão sujeitas a ações de colecionador. | `12` |
| `expected_shares` | As ações de colecionador de Число Prio перед объединением. | `2` |
| `event_buffer_capacity` | Backlog NDJSON é transferido para administrador. | `4096` |

Установка `force_flush_buckets` não `flush_delay_buckets`, обнуление порогов или отключение guard удержания теперь проваливает validar, você está usando a chave de segurança, que está usando o telefone em seu relé.

O limite `event_buffer_capacity` também é compatível com `/admin/privacy/events`, garantia, que as telas não são seguras отставать.

## Ações de colecionador Prio

SNNet-8a разворачивает двойные coletores, которые выпускают секретно-разделенные baldes Prio. Orquestrador теперь парсит NDJSON поток `/privacy/events` para записей `SoranetPrivacyEventV1` e compartilhar `SoranetPrivacyPrioShareV1`, направляя их в `SoranetSecureAggregator::ingest_prio_share`. Baldes выпускаются, когда приходит `PrivacyBucketConfig::expected_shares` вкладов, отражая поведение relé. Ações são válidas para baldes de armazenamento e formato de registro exibido em `SoranetPrivacyBucketMetricsV1`. Esses são os apertos de mão do próximo `min_contributors`, balde de transporte como `suppressed`, повторяя поведение relé агрегатора внутри. Você pode usar o método `suppression_reason`, o operador pode usar o `insufficient_contributors`, `collector_suppressed`, `collector_window_elapsed` e `forced_flush_window_elapsed` são fornecidos para verificação de telemetria. Причина `collector_window_elapsed` также срабатывает, когда Prio share задерживаются дольше `max_share_lag_buckets`, делая зависшие coletores видимыми без хранения устаревших аккумуляторов в памяти.

## Эндпоинты приема Torii

Torii é um dispositivo público para uma rede HTTP, vários relés e coletores podem ser configurados Melhor transporte seguro:- `POST /v2/soranet/privacy/event` carrega a carga útil `RecordSoranetPrivacyEventDto`. O modelo `SoranetPrivacyEventV1` é mais novo do que o `source`. Torii fornece suporte para perfil ativo de telefonia, verifica a solicitação e abre HTTP `202 Accepted` no Norito JSON-конвертом, содержащим вычисленное окно (`bucket_start_unix`, `bucket_duration_secs`) e retransmissão.
- `POST /v2/soranet/privacy/share` carrega a carga útil `RecordSoranetPrivacyShareDto`. Eu não preciso de `SoranetPrivacyPrioShareV1` e não posso usar `forwarded_by`, meus operadores podem ser coletores de áudio. Use o HTTP `202 Accepted` com Norito JSON-Kонвертом, colecionador de resumo, balde de armazenamento e armazenamento подавления; ошибки валидации сопоставляются с телеметрическим ответом `Conversion`, чтобы сохранить детерминированную colecionadores обработку ошибок между. O orquestrador Цикл событий теперь выпускает эти compartilha при опросе relés, сохраняя синхронизацию Prio-аккумулятора Torii com baldes no relé.

Seu ponto de venda é o perfil do telefone: oni возвращают `503 Service Unavailable`, métrica métrica aberta. Os clientes podem usar Norito binário (`application/x.norito`) ou Norito JSON (`application/x.norito+json`) como; O servidor é configurado automaticamente para extrair extratores Torii.

## Métrica Prometheus

O balde de transporte não contém o tamanho `mode` (`entry`, `middle`, `exit`) e `bucket_start`. Выпускаются следующие семейства métrica:

| Métrica | Descrição |
|--------|------------|
| `soranet_privacy_circuit_events_total{kind}` | Apertos de mão de classificação em `kind={accepted,pow_rejected,downgrade,timeout,other_failure,capacity_reject}`. |
| `soranet_privacy_throttles_total{scope}` | Acelerador de pressão com `scope={congestion,cooldown,emergency,remote_quota,descriptor_quota,descriptor_replay}`. |
| `soranet_privacy_throttle_cooldown_millis_{sum,count}` | Агрегированные длительности cooldown, внесенные apertos de mão estrangulados. |
| `soranet_privacy_verified_bytes_total` | Проверенная пропускная способность от слепых доказательств измерений. |
| `soranet_privacy_active_circuits_{avg,max}` | Selecione e selecione circuitos ativos no balde. |
| `soranet_privacy_rtt_millis{percentile}` | Percentis RTT padrão (`p50`, `p90`, `p99`). |
| `soranet_privacy_gar_reports_total{category_hash}` | Veja as seções do Relatório de Ação de Governança, nas categorias de resumo. |
| `soranet_privacy_bucket_suppressed` | Os baldes são usados, mas o que você precisa não pode ser entregue. |
| `soranet_privacy_pending_collectors{mode}` | Аккумуляторы coletor de ações em ожидании объединения, сгруппированные по режиму relé. |
| `soranet_privacy_suppression_total{reason}` | Счетчики suprimido buckets с `reason={insufficient_contributors,collector_suppressed,collector_window_elapsed,forced_flush_window_elapsed}`, чтобы дашборды могли атрибутировать провалы приватности. |
| `soranet_privacy_snapshot_suppression_ratio` | Доля suprimido/слитых на последнем dreno (0-1), полезно для бюджетов алертов. |
| `soranet_privacy_last_poll_unixtime` | O sistema UNIX tem uma opção de uso diferente (execute o coletor de alerta ocioso). |
| `soranet_privacy_collector_enabled` | Medidor, que foi colocado em `0`, que possui coletor de privacidade desativado ou não ativado (coletor de alerta desativado). |
| `soranet_privacy_poll_errors_total{provider}` | Use a opção de relé de aliasu (você pode usar a decodificação de código aberto, o status HTTP ou o status negativo). |

Os baldes não são mais fáceis de usar, permitindo que os baldes sejam abertos sem problemas.

## Recomendações de operação1. **Дашборды** - строьте метрики выше, группируя по `mode` e `window_start`. Ao sugerir isso, você terá problemas com coletores ou relés. Utilize `soranet_privacy_suppression_total{reason}` para a necessidade de uso de supressão e supressão, coletores de alta potência, por meio de testes de teste. Em Grafana теперь есть панель **"Motivos de supressão (5m)"** em базе этих счетчиков и stat **"Balde suprimido %"**, вычисляющий `sum(soranet_privacy_bucket_suppressed) / count(...)` para um dia útil, este operador mostra o que está acontecendo com o usuário. Série **"Collector Share Backlog"** (`soranet_privacy_pending_collectors`) e stat **"Snapshot Suppression Ratio"** выделяют зависшие coletores e дрейф бюджета во время programação automática.
2. **Alertinг** - запускайте тревоги от приватно-безопасных счетчиков: всплески PoW rejeita, частоту cooldown, дрейф RTT e capacidade rejeita. Coloque as peças monocromáticas em um balde pré-fabricado e coloque-as no lugar certo.
3. **Impedente-ответ** - сначала полагайтесь на агрегированные данные. Se você precisar de mais recursos, use relés para obter buckets de snapshots ou fornecer suporte para download измерений вместо сбора сырых логов tráfego.
4. **Escolha** - скрейпьте достаточно часто, чтобы не превысить `max_completed_buckets`. O extrator de dados está localizado em Prometheus e pode ser usado para baldes locais.

## Supressão analítica e programas automáticos

A implementação do SNNet-8 também é demonstrada, pois os coletores automáticos são usados, e um desvio de supressão em política de segurança (≤10% buckets no relé em 30 de outubro). Nenhum instrumento é colocado no repositório; операторы должны встроить их еженедельные ритуалы. A nova supressão de painel em Grafana fornece PromQL-выражения ниже, давая дежурным командам живую видимость до того, как они прибегнут к ручным запросам.

### PromQL-рецепты para supressão de supressão

O operador está pronto para iniciar o PromQL-хелперы; foi instalado no site Grafana (`dashboards/grafana/soranet_privacy_metrics.json`) e instalado Alertmanager:

```promql
/* Suppression ratio per relay mode (30 minute window) */
(
  increase(soranet_privacy_suppression_total{reason=~"insufficient_contributors|collector_suppressed|collector_window_elapsed|forced_flush_window_elapsed"}[30m])
) /
clamp_min(
  increase(soranet_privacy_circuit_events_total{kind="accepted"}[30m]) +
  increase(soranet_privacy_suppression_total[30m]),
1
)
```

```promql
/* Detect new suppression spikes above the permitted minute budget */
increase(soranet_privacy_suppression_total{reason=~"insufficient_contributors|collector_window_elapsed|collector_suppressed"}[5m])
/
clamp_min(
  sum(increase(soranet_privacy_circuit_events_total{kind="accepted"}[5m])),
1
)
```

Используйте выходное соотношение, чтобы подтвердить, что stat **"Suppressed Bucket %"** остается ниже бюджетного poroга; Use o detector para ativar o Alertmanager para o seu dispositivo, o que significa que você não pode usar o Alertmanager.

### CLI para buckets offline

No espaço de trabalho é fornecido `cargo xtask soranet-privacy-report` para a configuração NDJSON-выгрузок. Укажите один или несколько admin-эксportов relé:

```bash
cargo xtask soranet-privacy-report \
  --input artifacts/sorafs_privacy/relay-a.ndjson \
  --input artifacts/sorafs_privacy/relay-b.ndjson \
  --json-out artifacts/sorafs_privacy/relay_summary.json
```

Ajude a programar a configuração do `SoranetSecureAggregator`, executando a supressão no stdout e na estrutura da imagem selecionada JSON é definido como `--json-out <path|->`. Ao usar este coletor, este e coletor vivo (`--bucket-secs`, `--min-contributors`, `--expected-shares` e assim por diante), operação operacional воспроизводить исторические захваты с другими порогами при разборе инцидента. Use o JSON na tela Grafana para usar o SNNet-8 para analisar a supressão de áudio.### Verifique a sessão automática

Говернанс по-прежнему требует доказать, что первая автоматизированная сессия уложилась в бюджет supressão. Хелпер теперь принимает `--max-suppression-ratio <0-1>`, чтобы CI ou операторы могли быстро завершаться ошибкой, когда baldes suprimidos Verifique a quantidade de água oxigenada (com 10%) ou baldes vazios. Recomendações:

1. Execute NDJSON do relé de endpoints de administrador e use o orquestrador `/v2/soranet/privacy/event|share` em `artifacts/sorafs_privacy/<relay>.ndjson`.
2. Verifique o suporte com base na política:

   ```bash
   cargo xtask soranet-privacy-report \
     --input artifacts/sorafs_privacy/relay-a.ndjson \
     --input artifacts/sorafs_privacy/relay-b.ndjson \
     --json-out artifacts/sorafs_privacy/relay_summary.json \
     --max-suppression-ratio 0.10
   ```

   Команда печатает наблюдаемое соотношение и завершает работу с ненулевым кодом, когда бюджет превышен **или** esses baldes não são usados, sinalizados, mas a telemetria não é fornecida para o programa. Live-метрики должны показывать, что `soranet_privacy_pending_collectors` сливается к нулю, а `soranet_privacy_snapshot_suppression_ratio` остается ниже того же бюджета no período do programa.
3. Arquive JSON-вывод e LOG CLI no pacote de transferência SNNet-8 para transferência de transporte para умолчанию, чтобы ревьюеры могли воспроизвести точные артефакты.

## Следующие шаги (SNNet-8a)

- Интегрировать двойные Coletores Prio, подключив прием compartilha к runtime, чтобы relés и coletores выпускали согласованные cargas úteis `SoranetPrivacyBucketMetricsV1`. *(Готово — см. `ingest_privacy_payload` em `crates/sorafs_orchestrator/src/lib.rs` e testes confirmados.)*
- Abra o JSON do painel Prometheus e forneça alertas, definindo lacunas de supressão, coletando coletores e anonimamente. *(Готово — см. `dashboards/grafana/soranet_privacy_metrics.json`, `dashboards/alerts/soranet_privacy_rules.yml`, `dashboards/alerts/soranet_policy_rules.yml` e prov.)*
- Подготовить артефакты калибровки дифференциальной приватности, описанные в `privacy_metrics_dp.md`, включая blocos de expansão e resumo de governança. *(Готово — notebook e артефакты генерируются `scripts/telemetry/run_privacy_dp.py`; CI wrapper `scripts/telemetry/run_privacy_dp_notebook.sh` исполняет notebook через fluxo de trabalho `.github/workflows/release-pipeline.yml`; digest сохранен в `docs/source/status/soranet_privacy_dp_digest.md`.)*

Текущий релиз доставляет основу SNNet-8: детерминированную, приватно-безопасную телеметрию, которая напрямую подключается к существующим скрейперам и дашбордам Prometheus. Артефакты калибровки дифференциальной приватности на месте, fluxo de trabalho релизного пайплайна поддерживает Na verdade, você está usando um notebook e está trabalhando para monitorar o progresso automático e расширении аналитики supressão-алертов.