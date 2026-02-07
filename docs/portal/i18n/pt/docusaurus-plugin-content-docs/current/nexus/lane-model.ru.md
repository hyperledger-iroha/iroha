---
lang: pt
direction: ltr
source: docs/portal/docs/nexus/lane-model.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: modelo nexus-lane
título: Faixas do modelo Nexus
descrição: Логическая таксономия pistas, геометрия конфигурации e правила слияния estado mundial para Sora Nexus.
---

# Faixas do modelo Nexus e partições WSV

> **Estado:** envio NX-1 - faixas de taxa, configuração geométrica e configuração de faixa de segurança.  
> **Proprietários:** Nexus GT Central, GT de Governança  
> **Consulte no roteiro:** NX-1 em `roadmap.md`

Esta página do portal contém um resumo canônico `docs/source/nexus_lanes.md`, operação de operação Sora Nexus, proprietários SDK e ревьюеры Você pode usar as pistas para serem configuradas no antigo mono-repo. Целевая архитектура сохраняет детерминизм estado mundial, при этом позволяя отдельным espaços de dados (pistas) запускать публичные или Você pode validar cargas de trabalho isoladas.

## Conceitos

- **Lane:** log shard ledger Nexus para que você possa validar e recuperar o backlog. Identifique o padrão `LaneId`.
- **Espaço de dados:** balde de governança, rotas de grupo ou não, faixas com otimização de conformidade política, roteamento e liquidação.
- **Lane Manifest:** metadados de governança-контролируемая, описывающая валидаторов, política DA, token de gás, liquidação de правила e permissões de roteamento.
- **Compromisso Global:** prova, выпускаемая pista e суммирующая новые raízes estaduais, dados de liquidação e опциональные transferências cruzadas. Глобальное кольцо NPoS упорядочивает compromissos.

## Pistas de Таксономия

Типы lanes канонически описывают видимость, superfície de governança e ganchos de liquidação. Configuração de geometria (`LaneConfig`) фиксирует эти атрибуты, чтобы узлы, SDKs e ferramentas, você pode personalizar o layout sob medida lógica.

| Pista tipo | Vídeo | Associação validada | Exposições WSV | Governança por parte do governo | Liquidação política | Pré-requisitos típicos |
|-----------|------------|-----------|-------------|--------------------|-------------------|-------------|
| `default_public` | público | Sem permissão (participação global) | Réplica completa do estado | SORA Parlamento | `xor_global` | Livro-razão público |
| `public_custom` | público | Sem permissão ou controlado por estacas | Réplica completa do estado | Módulo ponderado por estaca | `xor_lane_weighted` | Produção pública com fluxo de trabalho |
| `private_permissioned` | restrito | Фиксированный набор валидаторов (governação de одобрен) | Compromissos e provas | Conselho Federado | `xor_hosted_custody` | CBDC, cargas de trabalho de consultoria |
| `hybrid_confidential` | restrito | Associação Смешанная; provas ZK | Compromissos + divulgação seletiva | Módulo monetário programável | `xor_dual_fund` | Dinheiro programável que preserva a privacidade |

Quais são os tipos de pista que você pode usar:

- Alias do Dataspace - человекочитаемая группировка, связывающая политики conformidade.
- Identificador de governança - идентификатор, разрешаемый через `Nexus.governance.modules`.
- Identificador de liquidação - идентификатор, roteador de liquidação потребляемый para espionar buffers XOR.
- Metadados de telemetria opcionais (descrição, contato, domínio comercial), отображаемая через `/status` e painéis.

## Faixas de configuração geométrica (`LaneConfig`)

`LaneConfig` - geometria de tempo de execução, выводимая из валидированного каталога pistas. Она не заменяет manifestos de governança; вместо этого предоставляет детерминированные identificadores de armazenamento e dicas de telemetria para a pista de сконфигурированной.

```text
LaneConfigEntry {
    lane_id: LaneId,           // stable identifier
    alias: String,             // human-readable alias
    slug: String,              // sanitised alias for file/metric keys
    kura_segment: String,      // Kura segment directory: lane_{id:03}_{slug}
    merge_segment: String,     // Merge-ledger segment: lane_{id:03}_merge
    key_prefix: [u8; 4],       // Big-endian LaneId prefix for WSV key spaces
    shard_id: ShardId,         // WSV/Kura shard binding (defaults to lane_id)
    visibility: LaneVisibility,// public vs restricted lanes
    storage_profile: LaneStorageProfile,
    proof_scheme: DaProofScheme,// DA proof policy (merkle_sha256 default)
}
```

- `LaneConfig::from_catalog` altera a configuração geométrica para configuração de configuração (`State::set_nexus`).
- Aliases санитизируются в нижний registr slug; последовательности неалфавитно-цифровых символов схлопываются em `_`. Esse alias é o slug usado, usado `lane{id}`.
- `shard_id` usa a chave de metadados `da_shard_id` (por умолчанию `lane_id`) e управляет журналом shard cursor, чтобы DA replay оставался детерминированным между reinicia/resharding.
- Os prefixos de chave são garantidos, pois o WSV fornece intervalos de chaves por faixa, mas o back-end é gratuito.
- Имена Kura segmento детерминированы между hosts; Os auditores podem fornecer segmentos de gerenciamento e manifestos por meio de ferramentas personalizadas.
- Mesclar segmentos (`lane_{id:03}_merge`) хранят последние raízes de dicas de mesclagem e compromissos de estado globais para esta via.

## Партиционирование estado mundial- Логический estado mundial Nexus - это объединение espaços de estado na pista. As vias públicas сохраняют estado completo; pistas privadas/confidenciais экспортируют raízes Merkle/de compromisso no livro razão de mesclagem.
- Armazenamento MV префиксирует каждый ключ 4-байтовым префиксом `LaneConfigEntry::key_prefix`, получая ключи вида `[00 00 00 01] ++ PackedKey`.
- Tabelas compartilhadas (contas, ativos, gatilhos, registros de governança) хранят записи, сгруппированные по lane prefix, сохраняя детерминизм varreduras de intervalo.
- Os metadados do livro-razão de mesclagem são definidos para o layout: каждая lane пишет raízes de dica de mesclagem e raízes de estado globais reduzidas em `lane_{id:03}_merge`, что позволяет делать retenção/despejo direcionado при выводе lane.
- Índices de faixa cruzada (aliases de contas, registros de ativos, manifestos de governança) хранят явные prefixos de faixa, чтобы операторы могли быстро выполнять reconciliação.
- **Política de retenção** - vias públicas сохраняют полные órgãos de bloqueio; faixas somente para compromissos podem ser compactadas por corpos старые после pontos de verificação, потому что compromissos авторитетны. As faixas confidenciais controlam diários de texto cifrado em segmentos ilimitados, mas não bloqueiam cargas de trabalho.
- **Ferramentas** - утилиты обслуживания (`kagami`, comandos de administração CLI) são usados no namespace slugged para métricas de экспонировании, rótulos Prometheus ou архивировании Segmentos Kura.

## Roteamento e APIs

- Torii endpoints REST/gRPC принимают опциональный `lane_id`; отсутствие означает `lane_default`.
- SDKs oferecem seletores de pista e mapeiam aliases fáceis de usar em `LaneId` para pistas de catálogo.
- As regras de roteamento são usadas para validar o catálogo e podem ser usadas na pista e no espaço de dados. `LaneConfig` fornece aliases compatíveis com telemetria para painéis e logs.

## Liquidação e taxas

- Каждая lane платит taxas XOR глобальному набору валидаторов. As pistas podem gerar tokens de gás nativos, mas não oferecem garantia de equivalentes XOR em compromissos.
- Provas de liquidação mostram valor, metadados de conversão e garantia de prova (por exemplo, transferência para cofre de taxas globais).
- O roteador de liquidação Единый (NX-3) fornece buffers para os prefixos de pista, e a telemetria de liquidação é fornecida com a geometria de armazenamento.

## Governança

- Lanes объявляют свой módulo de governança через каталог. `LaneConfigEntry` não contém alias e slug originais, contém telemetria e trilhas de auditoria.
- Nexus registro распространяет подписанные lane manifests, включающие `LaneId`, vinculação de espaço de dados, identificador de governança, identificador de liquidação e metadados.
- Ganchos de atualização de tempo de execução fornecem políticas de governança primárias (`gov_upgrade_id` para uso) e логируют diffs para ponte de telemetria (eventos `nexus.config.diff`).

## Telemetria e status

- `/status` contém aliases de pista, ligações de espaço de dados, identificadores de governança e perfis de liquidação, proibindo o catálogo e `LaneConfig`.
- Métricas do agendador (`nexus_scheduler_lane_teu_*`) отображают aliases/slugs, чтобы операторы быстро связывали backlog e pressão TEU.
- `nexus_lane_configured_total` считает количество выведенных entradas de pista e пересчитывается при изменении конфигурации. A telemetria é usada para diferenciar as diferenças da geometria da pista.
- Os medidores de backlog do Dataspace contêm metadados de alias/descrição, que podem ser usados ​​para monitorar a pressão da fila em negócios.

## Configuração e tipos Norito

- `LaneCatalog`, `LaneConfig`, e `DataSpaceCatalog` são usados em `iroha_data_model::nexus` e Norito-совместимые estruturas para manifestos e SDKs.
- `LaneConfig` é inserido em `iroha_config::parameters::actual::Nexus` e reiniciado automaticamente no catálogo; A codificação Norito não é necessária, assim como este auxiliar de tempo de execução útil.
- Пользовательская конфигурация (`iroha_config::parameters::user::Nexus`) продолжает принимать декларативные descritores lane и dataspace; Ao analisar a velocidade, você precisará de um número geométrico e de obter novos aliases ou IDs de pista.

## Оставшаяся работа

- Интегрировать atualizações do roteador de liquidação (NX-3) com nova geometria, чтобы débitos e recibos de buffer XOR помечались slug lane.
- Use ferramentas administrativas para especificar famílias de colunas, compactar pistas aposentadas e verificar logs de blocos por pista para o namespace slugged.
- Finalize o algoritmo de mesclagem (ordenação, poda, detecção de conflitos) e crie acessórios de regressão para reprodução entre pistas.
- Crie ganchos de conformidade para listas brancas/listas negras e políticas de dinheiro programável (трекится под NX-12).

---*Esta página fornece acompanhamentos de NX-1 por apenas uma vez de NX-2 a NX-18. Por favor, você pode abrir seus contatos em `roadmap.md` ou rastreador de governança, usar o portal de sincronização com documentos canônicos.*