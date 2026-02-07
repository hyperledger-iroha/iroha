---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/provider-advert-rollout.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
título: "Plano de implantação de anúncios de provedores SoraFS"
---

> Adaptado de [`docs/source/sorafs/provider_advert_rollout.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/provider_advert_rollout.md).

# Plano de implantação de anúncios de provedores SoraFS

Este plano coordena o bascule de anúncios permitidos de fornecedores na superfície
totalmente governamental `ProviderAdvertV1` necessário para a recuperação de pedaços
multi-fonte. Il se concentre sur trois livrables :

- **Guia do operador.** Ações que os fornecedores de armazenamento devem realizar
  terminal avant chaque gate.
- **Couverture de télémétrie.** Dashboards e alertas de observabilidade e operações
  usado para confirmar se a rede não aceita anúncios em conformidade.
- **Calendário de implantação.** Datas explícitas para rejeitar envelopes

A implantação está alinhada com os jalons SF-2b/2c do
[roteiro de migração SoraFS](./migration-roadmap) e suponha que a política
admissão du [política de admissão do provedor](./provider-admission-policy) foi interrompida em
vigor.

## Cronologia das fases

| Fase | Fenêtre (cible) | Comportamento | Operador de ações | Foco observabilidade |
|-------|-----------------|-----------|------------------|-------------------|

## Lista de verificação do operador1. **Inventariar anúncios.** Listar todos os anúncios publicados e registrados:
   - Caminho do envelope governamental (`defaults/nexus/sorafs_admission/...` ou equivalente em produção).
   - `profile_id` e `profile_aliases` do anúncio.
   - Lista de capacidades (pelo menos `torii_gateway` e `chunk_range_fetch`).
   - Sinalizador `allow_unknown_capabilities` (reques quand des TLVs réservés vendor sont présents).
2. **Registre-se com o fornecedor de ferramentas.**
   - Reconstrua a carga útil com seu anúncio do editor do provedor, com garantia:
     -`profile_id=sorafs.sf1@1.0.0`
     - `capability=chunk_range_fetch` com um `max_span` definido
     - `allow_unknown_capabilities=<true|false>` quando os TLVs GREASE são apresentados
   - Validador via `/v1/sorafs/providers` e `sorafs_fetch`; os avisos sobre des
     capacidades inconnues doivent être triés.
3. **Validar a prontidão multifonte.**
   - Execute `sorafs_fetch` com `--provider-advert=<path>`; a CLI ecoou
     desorvido quando `chunk_range_fetch` manque e exiba avisos para o
     capacidades inconnues ignorados. Capturar o relacionamento JSON e arquivar com
     os registros de operações.
4. **Prepare as renovações.**
   - Soumettre des envelopes `ProviderAdmissionRenewalV1` pelo menos 30 dias antes
     o gateway de aplicação (R2). Les renovallements devem conservar o identificador
     canônico e conjunto de capacidades; seusls le stake, les endpoints ou
     os metadados não devem ser alterados.
5. **Comunique-se com as equipes dependentes.**
   - Os proprietários do SDK devem publicar versões que expõem avisos adicionais
     operadores quando os anúncios são rejeitados.
   - DevRel anuncia cada transição de fase; incluir garantias de painéis
     et la logique de seuil ci-dessous.
6. **Painéis e alertas do instalador.**
   - Importe a exportação Grafana e coloque-a sob **SoraFS / Provedor
     Implementação** com UID `sorafs-provider-admission`.
   - Certifique-se de que as regras de alerta apontam para o canal compartilhado
     `sorafs-advert-rollout` em preparação e produção.

## Telemetria e painéis

As métricas a seguir foram expostas via `iroha_telemetry` :

- `torii_sorafs_admission_total{result,reason}` — considera aceitos e rejeitados
  e avisos. As razões incluem `missing_envelope`, `unknown_capability`,
  `stale`, e `policy_violation`.

Exportar Grafana: [`docs/source/grafana_sorafs_admission.json`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/grafana_sorafs_admission.json).
Importe o arquivo para o repositório compartilhado de painéis (`observability/dashboards`)
e atualize apenas o UID da fonte de dados antes da publicação.

A placa foi publicada no dossiê Grafana **SoraFS / Provider Rollout** com
UID estável `sorafs-provider-admission`. As regras de alerta
`sorafs-admission-warn` (aviso) e `sorafs-admission-reject` (crítico) são
pré-configurado para utilizar a política de notificação `sorafs-advert-rollout` ;
ajuste este ponto de contato se a lista de destinos mudar tanto que você pode editar
o JSON do painel.

Panneaux Grafana recomendado:| Painel | Consulta | Notas |
|-------|-------|-------|
| **Taxa de resultado de admissão** | `sum by(result)(rate(torii_sorafs_admission_total[5m]))` | Gráfico de pilha para visualizador aceitar vs avisar vs rejeitar. Alerte quando avisar > 0,05 * total (aviso) ou rejeitar > 0 (crítico). |
| **Taxa de alerta** | `sum(rate(torii_sorafs_admission_total{result="warn"}[5m])) / sum(rate(torii_sorafs_admission_total[5m]))` | Série temporal à linha exclusiva que alimenta seu pager (aviso de 5% em 15 minutos). |
| **Motivos de rejeição** | `sum by(reason)(rate(torii_sorafs_admission_total{result="reject"}[5m]))` | Guia de triagem do runbook; anexar garantias às etapas de mitigação. |
| **Atualizar dívida** | `sum(rate(torii_sorafs_admission_total{reason="stale"}[1h]))` | Indica os provedores que avaliam o prazo de atualização; cruze com os logs do cache de descoberta. |

CLI de artefatos para manuais de painéis:

- `sorafs_fetch --provider-metrics-out` escrito dos computadores `failures`, `successes` e
  `disabled` por provedor. Importar painéis ad-hoc para vigilância
  as execuções do orquestrador antes de limitar os fornecedores na produção.
- Os campos `chunk_retry_rate` e `provider_failure_rate` do relacionamento JSON
  observado antes de estrangulamentos ou sintomas de cargas obsoletas anteriores
  souvent les rejets d'admission.

### Mise en page du dashboard Grafana

Observabilidade pública de um conselho dédié — **SoraFS Admissão do Provedor
Lançamento** (`sorafs-provider-admission`) — sous **SoraFS / Lançamento do Provedor**
com os seguintes IDs de painel canônicos:

- Painel 1 — *Taxa de resultado de admissão* (área empilhada, unité "ops/min").
- Painel 2 — *Proporção de advertência* (série única), émettant l'expression
  `sum(taxa(torii_sorafs_admission_total{result="warn"}[5m])) /
   soma(taxa(torii_sorafs_admission_total[5m]))`.
- Painel 3 — *Motivos de rejeição* (séries temporais reagrupadas par `reason`), triées par
  `rate(...[5m])`.
- Painel 4 — *Atualizar dívida* (stat), representando a consulta do quadro ci-dessus et
  anotado com os prazos de atualização dos anúncios extraídos do registro de migração.

Copie (ou crie) o formato JSON no repositório de painéis de infraestrutura para
`observability/dashboards/sorafs_provider_admission.json`, depois mete-o hoje
exclua o UID da fonte de dados; os IDs do painel e as regras de alerta são
referenciado pelos runbooks ci-dessous, evite o dinheiro dos renumeradores sem
leia esta documentação.

Por conveniência, o repositório fornece uma definição de painel de referência para
`docs/source/grafana_sorafs_admission.json`; copie-o em seu dossiê Grafana
se você tiver um ponto de partida para testes locais.

### Regras de alerta Prometheus

Adicione o grupo de regras a seguir
`observability/prometheus/sorafs_admission.rules.yml` (crie o arquivo se for
o primeiro grupo de regras SoraFS) e inclui-lo em sua configuração
Prometheus. Substitua `<pagerduty>` pela etiqueta de rota para você
rodízio de plantão.

```yaml
groups:
  - name: torii_sorafs_admission
    rules:
      - alert: SorafsProviderAdvertWarnFlood
        expr: sum(rate(torii_sorafs_admission_total{result="warn"}[5m])) /
              sum(rate(torii_sorafs_admission_total[5m])) > 0.05
        for: 15m
        labels:
          severity: warning
          route: <pagerduty>
        annotations:
          summary: "SoraFS provider adverts generating warnings"
          description: |
            Warn outcomes exceeded 5% of all admissions for 15 minutes.
            Inspect panel 3 on the sorafs/provider-admission dashboard and
            coordinate advert rotation with the affected operator.
      - alert: SorafsProviderAdvertReject
        expr: increase(torii_sorafs_admission_total{result="reject"}[5m]) > 0
        for: 5m
        labels:
          severity: critical
          route: <pagerduty>
        annotations:
          summary: "SoraFS provider adverts rejected"
          description: |
            Provider adverts have been rejected for the last five minutes.
            Check panel 4 (rejection reasons) and rotate envelopes before
            the refresh deadline elapses.
```

Execute `scripts/check_prometheus_rules.sh observability/prometheus/sorafs_admission.rules.yml`
antes de fazer alterações para verificar se a sintaxe está correta
`promtool check rules`.

## Matriz de implantação| Características do anúncio | R0 | R1 | R2 | R3 |
|-----|----|----|----|----|
| `profile_id = sorafs.sf1@1.0.0`, `chunk_range_fetch` presente, aliases canônicos, `signature_strict=true` | ✅ | ✅ | ✅ | ✅ |
| Ausência de capacidade `chunk_range_fetch` | ⚠️ Avisar (ingestão + telemetria) | ⚠️Avisar | ❌ Rejeitar (`reason="missing_capability"`) | ❌ Rejeitar |
| TLVs de capacidade inconnue sans `allow_unknown_capabilities=true` | ✅ | ⚠️ Avisar (`reason="unknown_capability"`) | ❌ Rejeitar | ❌ Rejeitar |
| `refresh_deadline` expirado | ❌ Rejeitar | ❌ Rejeitar | ❌ Rejeitar | ❌ Rejeitar |
| `signature_strict=false` (diagnóstico de luminárias) | ✅ (desenvolvimento único) | ⚠️Avisar | ⚠️Avisar | ❌ Rejeitar |

Todas as horas utilizam UTC. As datas de aplicação são refletidas no
registro de migração e ne bougeront pas sans vote du Council ; mudança de anúncio
requer a atualização diária deste arquivo e do livro-razão no mesmo PR.

> **Nota de implementação:** R1 introduz a série `result="warn"` em
> `torii_sorafs_admission_total`. O patch de ingestão Torii que adicionou o novo
> o rótulo é suivi com as placas de telefonia SF-2; jusque-là, use o

## Comunicação e gerenciamento de incidentes

- **Mailer hebdomadaire de statut.** DevRel divulga um breve currículo de métricas
  de admissão, avisos durante o curso e prazos a vencer.
- **Resposta ao incidente.** Se os alertas `reject` forem desativados, ligue para:
  1. Recupere o anúncio errado por meio da descoberta Torii (`/v1/sorafs/providers`).
  2. Verifique a validação do anúncio no provedor de pipeline e compare com
     `/v1/sorafs/providers` para reproduzir erros.
  3. Coordonne com o fornecedor para fazer o tourner do anúncio antes da prochaine
     prazo de atualização.
- **Gel des changements.** Nenhuma modificação do esquema de recursos pendente
  R1/R2 menos que o comitê de implementação não seja válido; les essais GREASE doivent
  são planejados durante a janela de manutenção hebdomadaire et jornalisados
  no registro de migração.

## Referências

- [Protocolo de nó/cliente SoraFS] (https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/sorafs_node_client_protocol.md)
- [Política de Admissão de Provedor](./provider-admission-policy)
- [Roteiro de migração](./migration-roadmap)
- [Extensões de múltiplas fontes de anúncio do provedor] (https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/provider_advert_multisource.md)