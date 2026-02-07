---
lang: pt
direction: ltr
source: docs/portal/docs/devportal/incident-runbooks.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Ранбуки инцидентов и тренировки rollback

## Abençoado

Пункт дорожной карты **DOCS-9** contém um manual de instruções detalhado e um plano de repetição, itens
O operador do portal pode ser enviado sem problemas. Eta заметка
охватывает три высокосигнальных инцидента - неудачные деплои, деградацию репликации и
сбои аналитики - и документирует квартальные тренировки, доказывающие, что rollback alias
e a validação de síntese produz o trabalho de ponta a ponta.

### Materiais úteis

- [`devportal/deploy-guide`](./deploy-guide) — fluxo de trabalho упаковки, подписи и alias de promoção.
- [`devportal/observability`](./observability) — tags de lançamento, análises e sondagens, упомянутые ниже.
-`docs/source/sorafs_node_client_protocol.md`
  e [`sorafs/pin-registry-ops`](../sorafs/pin-registry-ops)
  — телеметрия реестра и пороги эскалации.
- `docs/portal/scripts/sorafs-pin-release.sh` e ajudantes `npm run probe:*`
  упомянуты в чеклистах.

### Obtenção de telefonia e instrumentos

| Sinal / Instrumento | Atualizado |
| ------------- | ------- |
| `torii_sorafs_replication_sla_total` (atendido/perdido/pendente) | Verifique as réplicas e o SLA. |
| `torii_sorafs_replication_backlog_total`, `torii_sorafs_replication_completion_latency_epochs` | Limpe o backlog de pendências e garanta a segurança para triagem. |
| `torii_sorafs_gateway_refusals_total`, `torii_sorafs_manifest_submit_total{status="error"}` | Coloque-o no gateway de armazenamento, o que significa implantá-lo. |
| `npm run probe:portal` / `npm run probe:tryit-proxy` | Testes de inteligência, testes de porta de controle e reversões de teste. |
| `npm run check:links` | Portão битых ссылок; используется после каждой mitigação. |
| `sorafs_cli manifest submit ... --alias-*` (rede `scripts/sorafs-pin-release.sh`) | Механизм promoção/reversão de alias. |
| Placa `Docs Portal Publishing` Grafana (`dashboards/grafana/docs_portal.json`) | Агрегирует recusas/alias/TLS/replicação телеметрию. Os alertas do PagerDuty são exibidos neste painel quando você os envia. |

## Ранбук - Неудачный деплой или плохой артефакт

### Условия срабатывания

- Padrões de pré-visualização/produção (`npm run probe:portal -- --expect-release=...`).
- Alertas Grafana em `torii_sorafs_gateway_refusals_total` ou
  `torii_sorafs_manifest_submit_total{status="error"}` após implementação.
- QA вручную замечает сломанные маршруты или сбои proxy Experimente сразу после
  apelido de promoção.

### Немедленное сдерживание

1. **Aplicar implementação:** crie o pipeline de CI `DEPLOY_FREEZE=1` (fluxo de trabalho do GitHub de entrada)
   Se você for o proprietário do trabalho Jenkins, esses novos artefatos não serão usados.
2. **Artigos de atualização:** Número `build/checksums.sha256`,
   `portal.manifest*.{json,to,bundle,sig}`, e você testa se há falha na compilação,
   чтобы rollback ссылался на точные resumos.
3. **Уведомить стейкхолдеров:** armazenamento SRE, líder Docs/DevRel e diretor de governança
   (é um erro `docs.sora`).

### Reversão do processo

1. Manifesto último conhecido em bom estado (LKG). Fluxo de trabalho de produção хранит их в
   `artifacts/devportal/<release>/sorafs/portal.manifest.to`.
2. Crie um alias para este manifesto com o ajudante de envio:

```bash
cd docs/portal
./scripts/sorafs-pin-release.sh \
  --build-dir build \
  --artifact-dir artifacts/revert-$(date +%Y%m%d%H%M) \
  --sorafs-dir artifacts/revert-$(date +%Y%m%d%H%M)/sorafs \
  --pin-min-replicas 5 \
  --alias "docs-prod-revert" \
  --alias-namespace "${PIN_ALIAS_NAMESPACE}" \
  --alias-name "${PIN_ALIAS_NAME}" \
  --alias-proof "${PIN_ALIAS_PROOF_PATH}" \
  --torii-url "${TORII_URL}" \
  --submitted-epoch "$(date +%Y%m%d)" \
  --authority "${AUTHORITY}" \
  --private-key "${PRIVATE_KEY}" \
  --skip-submit

# swap in the LKG artefacts before submission
cp /secure/archive/lkg/portal.manifest.to artifacts/.../sorafs/portal.manifest.to
cp /secure/archive/lkg/portal.manifest.bundle.json artifacts/.../sorafs/

cargo run -p sorafs_orchestrator --bin sorafs_cli -- \
  manifest submit \
  --manifest artifacts/.../sorafs/portal.manifest.to \
  --chunk-plan artifacts/.../sorafs/portal.plan.json \
  --torii-url "${TORII_URL}" \
  --authority "${AUTHORITY}" \
  --private-key "${PRIVATE_KEY}" \
  --alias-namespace "${PIN_ALIAS_NAMESPACE}" \
  --alias-name "${PIN_ALIAS_NAME}" \
  --alias-proof "${PIN_ALIAS_PROOF_PATH}" \
  --metadata rollback_from="${FAILED_RELEASE}" \
  --summary-out artifacts/.../sorafs/rollback.submit.json
```

3. Registre a reversão resumida no ticket de incidente com o resumo do LKG e o novo manifesto.

### Validação

1.`npm run probe:portal -- --expect-release=${LKG_TAG}`.
2.`npm run check:links`.
3. `sorafs_cli manifest verify-signature ...` e `sorafs_cli proof verify ...`
   (como guia de implantação), чтобы убедиться, что manifesto repromovido совпадает с архивным CAR.
4. `npm run probe:tryit-proxy` чтобы убедиться, что Try-It staging proxy вернулся.

### Após o incidente

1. Verifique a causa raiz do pipeline.
2. Adicione a seção "Lições aprendidas" em [`devportal/deploy-guide`](./deploy-guide)
   новыми выводами, если есть.
3. Verifique defeitos para testes de prova (sonda, verificador de link, etc.).

## Ранбук - Деградация репликации

### Условия срабатывания

- Alerta: `sum(torii_sorafs_replication_sla_total{outcome="met"}) /
  clamp_min(sum(torii_sorafs_replication_sla_total{outcome=~"met|missed"}), 1) <
  0,95` na técnica 10 minutos.
- `torii_sorafs_replication_backlog_total > 10` na técnica 10 minutos (com.
  `pin-registry-ops.md`).
- Governança сообщает о медленной доступности alias после release.

### Triagem

1. Painéis de controle [`sorafs/pin-registry-ops`](../sorafs/pin-registry-ops), чтобы
   por exemplo, localize o backlog na classe de armazenamento ou nos fornecedores da frota.
2. Transfira o log Torii para `sorafs_registry::submit_manifest`, instale-o,
   não падают ли submissões.
3. Você pode fornecer a réplica do código `sorafs_cli manifest status --manifest ...`
   (показывает исходы репликации по провайдерам).

### Mitigação1. Manifesto Перевыпустить com a maior réplica do seu arquivo (`--pin-min-replicas 7`) через
   `scripts/sorafs-pin-release.sh`, agendador de itens configurado para maior número de tarefas
   provador. Adicione um novo resumo no log de incidentes.
2. Se o backlog for fornecido para uma prova adequada, você deve excluir o agendador de replicação
   (descrito em `pin-registry-ops.md`) e abra um novo manifesto, criado anteriormente
   провайдеров обновить alias.
3. Quando você usa o alias com replicações de paridade, religa o alias no manifesto quente no teste
   (`docs-preview`), затем опубликовать manifesto de acompanhamento после очистки backlog SRE.

### Recuperação e encerramento

1. Monitore `torii_sorafs_replication_sla_total{outcome="missed"}` e instale-o, aqui
   счетчик стабилизировался.
2. Verifique `sorafs_cli manifest status` como evidência, esta é uma réplica nova em condições normais.
3. Resolva ou обновить backlog de replicação post-mortem com a data final
   (масштабирование провайдеров, tuning chunker, etc.).

## Ранбук - Análise de análise ou telemetria

### Условия срабатывания

- Teste `npm run probe:portal`, sem painéis de controle para garantir a segurança
  `AnalyticsTracker` durou 15 minutos.
- Revisão de privacidade фиксирует неожиданный рост eventos descartados.
- `npm run probe:tryit-proxy` é compatível com `/probe/analytics`.

### Resposta

1. Verifique as entradas em tempo de construção: `DOCS_ANALYTICS_ENDPOINT` e
   `DOCS_ANALYTICS_SAMPLE_RATE` – artefato de liberação (`build/release.json`).
2. Coloque `npm run probe:portal` em `DOCS_ANALYTICS_ENDPOINT`, configurado
   no coletor de teste, você pode usar o rastreador para fornecer cargas úteis.
3. Coletores não necessários, instalar `DOCS_ANALYTICS_ENDPOINT=""` e reconstruir,
   чтобы curto-circuito do rastreador; зафиксировать окно interrupção no cronograma do incidente.
4. Verifique se `scripts/check-links.mjs` produz impressão digital `checksums.sha256`
   (аналитические сбои *не* должны блокировать проверку mapa do site).
5. Depois de instalar o coletor, coloque-o `npm run test:widgets`, progнать
   auxiliar de análise de testes de unidade antes de republicar.

### Após o incidente

1. Resolver [`devportal/observability`](./observability) com nova configuração
   coletor или требованиями amostragem.
2. Acesse o aviso de governança, exceto análises de dados ou de terceiros
   não é político.

## Квартальные учения по устойчивости

Запускайте оба drill в **первый вторник каждого квартала** (janeiro/abril/julho/outubro)
ou talvez seja uma boa infraestrutura de configuração. Храните артефакты в
`artifacts/devportal/drills/<YYYYMMDD>/`.

| Uчение | Шаги | Documentar |
| ----- | ----- | -------- |
| Reversão de alias de repetição | 1. Повторить rollback "Failed implantation" самым свежим manifesto de produção.<br/>2. Religue as sondas de produção após a produção.<br/>3. Сохранить `portal.manifest.submit.summary.json` e sondas lógicas em uma broca de pacote. | `rollback.submit.json`, você usa sondas e repetições de tags de liberação. |
| Validação de auditoria sintética | 1. Selecione `npm run probe:portal` e `npm run probe:tryit-proxy` para proteger a produção e o preparo.<br/>2. Запустить `npm run check:links` e архивировать `build/link-report.json`.<br/>3. Selecione capturas de tela/exportações no painel Grafana, verifique as sondas. | Логи probes + `link-report.json` со ссылкой на manifesto de impressão digital. |

Эскалируйте пропущенные drills менеджеру Docs/DevRel e na revisão de governança SRE,
так как roadmap требует детерминированных квартальных доказательств, что alias rollback
e as sondas de portal são detectadas.

## PagerDuty e coordenação de plantão

- Сервис PagerDuty **Docs Portal Publishing** está disponível em alertas
  `dashboards/grafana/docs_portal.json`. Palavra `DocsPortal/GatewayRefusals`,
  `DocsPortal/AliasCache` e `DocsPortal/TLSExpiry` usam Docs/DevRel primário
  с Armazenamento SRE как secundário.
- Ao usar o `DOCS_RELEASE_TAG`, veja as capturas de tela capturadas
  painel Grafana e instale seu probe/link-check no ponto de partida
  mitigação начала.
- Possível mitigação (reversão ou reimplantação) através do `npm run probe:portal`,
  `npm run check:links` e fornecer instantâneos Grafana, показывающие возврат метрик
  por favor. Verifique todas as evidências de que o PagerDuty está sendo criptografado.
- Seu alerta será alterado (por exemplo, expiração de TLS e backlog), сначала
  recusas de triagem (publicação de остановить), reversão de выполнить, затем закрыть TLS/backlog
  с Armazenamento SRE em bridge.