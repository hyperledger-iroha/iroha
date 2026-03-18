---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/orchestrator-config.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: orquestrador-config
título: Конфигурация оркестратора SoraFS
sidebar_label: Configuração de organização
description: Настройка мульти-источникового fetch-оркестратора, интерпретация сбоев и отладка телеметрии.
---

:::nota História Canônica
Esta página contém `docs/source/sorafs/developer/orchestrator.md`. Faça cópias de sincronização se a documentação não estiver disponível para download.
:::

# Руководство по мульти-источниковому fetch-оркестратору

Multifuncional fetch-оркестратор SoraFS управляет детерминированными
параллельными загрузками из набора provadores, опубликованного в anúncios под
governança de controle. Nesta descrição inicial, como o organizador,
quais sinais serão exibidos antes do lançamento e quais serão os pontos de telemetria
индикаторы здоровья.

## 1. Configuração de configuração

O organizador obteve três configurações de história:

| Estocado | Atualizado | Nomeação |
|----------|------------|------------|
| `OrchestratorConfig.scoreboard` | Нормализует веса провайдеров, проверяет телеметрии сохраняет JSON scoreboard для аудитов. | Atualizado em `crates/sorafs_car::scoreboard::ScoreboardConfig`. |
| `OrchestratorConfig.fetch` | Verifique a execução do tempo de execução (você pode retornar, limitar a paralização, verificar a verificação). | Mapeie `FetchOptions` em `crates/sorafs_car::multi_fetch`. |
| Parâmetros CLI / SDK | Ограничивают число пиров, добавляют регионы телеметрии e выводят политики deny/boost. | `sorafs_cli fetch` раскрывает эти флаги напрямую; O SDK foi instalado no `OrchestratorConfig`. |

JSON-хелперы em `crates/sorafs_orchestrator::bindings` serializado em seu lugar
Configurado em Norito JSON, ele é configurado automaticamente com o SDK e o dispositivo.

### 1.1 Exemplo de codificação JSON

```json
{
  "scoreboard": {
    "latency_cap_ms": 6000,
    "weight_scale": 12000,
    "telemetry_grace_secs": 900,
    "persist_path": "/var/lib/sorafs/scoreboards/latest.json"
  },
  "fetch": {
    "verify_lengths": true,
    "verify_digests": true,
    "retry_budget": 4,
    "provider_failure_threshold": 3,
    "global_parallel_limit": 8
  },
  "telemetry_region": "iad-prod",
  "max_providers": 6,
  "transport_policy": "soranet_first"
}
```

Definindo o arquivo de configuração padrão `iroha_config` (`defaults/`, usuário,
real), чтобы детерминированные деплои наследовали одинаковые лимиты на всех
não. Para perfil somente direto, implementação padrão SNNet-5a, смотрите
`docs/examples/sorafs_direct_mode_policy.json` e recomendações recomendadas em
`docs/source/sorafs/direct_mode_pack.md`.

### 1.2 Substituições соответствия

SNNet-9 fornece conformidade orientada à governança no оркестратор. Novo objeto
`compliance` na configuração Norito JSON фиксирует carve-outs, которые переводят
busca de pipeline em somente direto:

```json
"compliance": {
  "operator_jurisdictions": ["US", "JP"],
  "jurisdiction_opt_outs": ["US"],
  "blinded_cid_opt_outs": [
    "C6B434E5F23ABD318F01FEDB834B34BD16B46E0CC44CD70536233A632DFA3828"
  ],
  "audit_contacts": ["mailto:compliance@example.org"]
}
```- `operator_jurisdictions` contém códigos ISO‑3166 alfa‑2, que estão funcionando
  instância do orкестратора. Os códigos são normais no registro de segurança da análise.
- `jurisdiction_opt_outs` зеркалирует реестр governança. Когда любая юрисдикция
  o operador é contratado pelo especialista, o operador é o primeiro
  `transport_policy=direct-only` e não usar substituto
  `compliance_jurisdiction_opt_out`.
- `blinded_cid_opt_outs` перечисляет digest манифеста (cego CID, em верхнем
  hexadecimal). Cargas úteis suportadas também fornecem planejamento somente direto e publicação
  fallback `compliance_blinded_cid_opt_out` em telemetria.
- `audit_contacts` URI de registro, governança de которые ожидает увидеть в GAR
  manuais de operação.
- `attestations` фиксирует подписанные compliance-packеты, на которых держится
  política. A versão padrão é `jurisdiction` (ISO‑3166 alfa‑2),
  `document_uri`, canônico `digest_hex` (64 símbolos), carimbo de data/hora exibido
  `issued_at_ms` e opcional `expires_at_ms`. Esses artefatos foram colocados em
  аудит-чеклист оркестратора, essas ferramentas de governança são substituídas por
  подписанными документами.

Verifique a conformidade do bloco com configurações de configuração padrão, opções
операторы получали детерминированные substituições. Primeiro-ministro
conformidade _после_ dicas de modo de gravação: даже если SDK запрашивает `upload-pq-only`,
optar por não participar do transporte público ou do transporte direto somente
e eu estou usando um aparelho, mas não há provadores confiáveis.

Cancelamento de exclusão do catálogo canônico
`governance/compliance/soranet_opt_outs.json`; Совет по governança pública публикует
обновления через lançamentos marcados. Полный пример конфигурации (включая
atestados) fornecidos em `docs/examples/sorafs_compliance_policy.json`, e
descrição do processo de operação em
[manual de instruções GAR](../../../source/soranet/gar_compliance_playbook.md).

### 1.3 CLI e SDK| Bandeira / Pólo | Efeito |
|------------|--------|
| `--max-peers` / `OrchestratorConfig::with_max_providers` | Ограничивает, сколько провайдеров пройдут фильтр placar. Use `None`, isso será fornecido por seus fornecedores qualificados. |
| `--retry-budget` / `FetchOptions::per_chunk_retry_limit` | Ограничивает число ретраев на chunk. Verifique o limite de `MultiSourceError::ExhaustedRetries`. |
| `--telemetry-json` | Вкалывает snapshots латентности/сбоев в построитель placar. A utilização da rede `telemetry_grace_secs` não é elegível. |
| `--scoreboard-out` | Сохраняет вычисленный placar (prováveis ​​​​elegíveis + inelegíveis) para pós-analisação. |
| `--scoreboard-now` | Переопределяет placar timestamp (segundos Unix), чтобы capture fixtures оставались детерминированными. |
| `--deny-provider` / pontuação de gancho политики | A opção determinada é comprovada por meio de planejamento sem anúncios publicitários. Полезно для быстрого na lista negra. |
| `--boost-provider=name:delta` | Корректирует кредиты провайдера ponderado round-robin, не меняя веса governança. |
| `--telemetry-region` / `OrchestratorConfig::with_telemetry_region` | Marque métricas e logs de estrutura, esses números podem ser agrupados para distribuição ou distribuição. |
| `--transport-policy` / `OrchestratorConfig::with_transport_policy` | Para usar `soranet-first`, como um organizador multifuncional - é necessário. Use `direct-only` para downgrades ou para conformidade direta, e `soranet-strict` é fornecido para pilotos somente PQ; a conformidade substitui остаются жестким потолком. |

SoraNet-first теперь дефолт, um rollbacks должны ссылаться на соответствующий
Bloqueador SNNet. После выпуска SNNet-4/5/5a/5b/6a/7/8/12/13 governança ужесточит
требуемую позу (no armazenamento `soranet-strict`); faça isso antes de substituir
инцидентам должны приоритизировать `direct-only`, их нужно фиксировать в log
lançamento.

Sua bandeira é a sintaxe `--` como em `sorafs_cli fetch`, aqui e ali
разработческом бинаре `sorafs_fetch`. O SDK fornece a opção que foi digitada
construtores.

### 1.4 Atualização do cache de proteção

CLI теперь подключает protetores seletores SoraNet, чтобы операторы могли
детерминированно закреплять relés de entrada para implementação SNNet-5.
Рабочий proцесс контролируют три новых флага:

| Bandeira | Atualizado |
|------|-----------|
| `--guard-directory <PATH>` | Use o JSON-файл com o consenso de retransmissão possível (não disponível). O diretório anterior foi protegido pelo cache de proteção antes da busca. |
| `--guard-cache <PATH>` | Сохраняет Norito codificado `GuardSet`. Следующие прогоны используют cache, mas o novo diretório não é encontrado. |
| `--guard-target <COUNT>` / `--guard-retention-days <DAYS>` | Substituições opcionais para guardas de entrada fixas (em 3) e окна удержания (em 30 de dezembro). |
| `--guard-cache-key <HEX>` | Опциональный 32-байтовый ключ для тега guard cache с Blake3 MAC, чтобы файл можно было проверить перед повторным ispolьзованием. |

O diretório de proteção de cargas útil é compactado:

Флаг `--guard-directory` теперь ожидает Carga útil codificada Norito
`GuardDirectorySnapshotV2`. A configuração do snapshot binário:- `version` — versão do número (se `2`).
- `directory_hash`, `published_at_unix`, `valid_after_unix`, `valid_until_unix` —
  метаданные consenso, которые должны совпадать с каждым встроенным сертификатом.
- `validation_phase` — gate политики сертификатов (`1` = разрешить одну Ed25519
  подпись, `2` = предпочесть двойные подписи, `3` = требовать двойные подписи).
- `issuers` — governança de recursos com `fingerprint`, `ed25519_public` e
  `mldsa65_public`. Impressão digital вычисляется как
  `BLAKE3("soranet.src.v2.issuer" || ed25519 || u32(len(ml-dsa)) || ml-dsa)`.
- `relays` — pacotes específicos SRCv2 (em inglês `RelayCertificateBundleV2::to_cbor()`).
  Каждый pacote содержит relé descritor, sinalizadores de capacidade, política ML-KEM e
  двойные подписи Ed25519/ML-DSA-65.

CLI fornece um pacote de pacotes protegidos por um emissor de código de acesso antes de uma transação
instantâneos.

Ao usar CLI em `--guard-directory`, você obtém o consenso atual com
cache de armazenamento. O seletor protege os protetores de segurança, que estão acima da abertura
удержания и допустимы no diretório; novos relés são responsáveis ​​​​pelas chaves.
После успешного fetch обновленный cache записывается по пути `--guard-cache`,
обеспечивая детерминированность следующих сессий. SDK está disponível,
usado `GuardSelector::select(&RelayDirectory, existing_guard_set, now_unix_secs)`
e instale o `GuardSet` no `SorafsGatewayFetchOptions`.

`ml_kem_public_hex` позволяет selector приоритизировать PQ-guards во время
implementação do SNNet-5. Alternadores de estágio (`anon-guard-pq`, `anon-majority-pq`,
`anon-strict-pq`) теперь автоматически понижают relés de classe: когда
доступен PQ guard, seletor сбрасывает лишние pinos clássicos, чтобы последующие
сессии предпочитали гибридные apertos de mão. Resumos CLI/SDK показывают итоговый
microfone `anonymity_status`/`anonymity_reason`, `anonymity_effective_policy`,
`anonymity_pq_selected`, `anonymity_classical_selected`, `anonymity_pq_ratio`,
`anonymity_classical_ratio` e связанные поля кандидатов/дефицита/дельт fornecimento,
делая brownouts e fallbacks clássicos явными.

Guardar diretórios теперь могут содержать полный pacote SRCv2 через
`certificate_base64`. Оркестратор декодирует каждый bundle, повторно проверяет
Verifique o Ed25519/ML-DSA e verifique a certificação para proteger o cache.
Para obter a certificação de segurança, use chaves PQ padrão,
apertos de mão настроек e весов; просроченные сертификаты отбрасываются, e seletor
Verifique o circuito do ciclo e instale o `telemetry::sorafs.guard` e
`telemetry::sorafs.circuit`, фиксируя окно валидности, pacotes de handshake e
наличие двойных подписей для каждого guarda.

Use ajudantes CLI, чтобы держать snapshots sincronizados com
publicado:

```bash
sorafs_cli guard-directory fetch \
  --url https://directory.soranet.dev/mainnet_snapshot.norito \
  --output ./state/guard_directory.norito \
  --expected-directory-hash <directory-hash-hex>

sorafs_cli guard-directory verify \
  --path ./state/guard_directory.norito \
  --expected-directory-hash <directory-hash-hex>
```

`fetch` salva e valida o snapshot SRCv2 antes de ser salvo no disco e `verify`
use a validação de pipeline para artefactos de um comando médico, usando JSON
resumo, este é o seletor de proteção de saída CLI/SDK.

### 1.5 Менеджер жизненного цикла circuitoКогда доступны e diretório de retransmissão, e cache de proteção, circuito de ativação do orquestrador
gerenciador de ciclo de vida para a implementação e implementação de circuitos SoraNet
antes de buscar. Configuração de configuração em `OrchestratorConfig`
(`crates/sorafs_orchestrator/src/lib.rs:305`) esta é a nova configuração:

- `relay_directory`: хранит instantâneo do diretório SNNet-3, чтобы saltos intermediários/de saída
  Você pode determinar isso.
- `circuit_manager`: configuração opcional (exibida para uso),
  контролирующая TTL цепей.

Norito JSON representa o bloco `circuit_manager`:

```json
"circuit_manager": {
  "enabled": true,
  "circuit_ttl_secs": 900
}
```

SDK передают данные diretório через
`SorafsGatewayFetchOptions::relay_directory`
(`crates/iroha/src/client.rs:320`), um aplicativo CLI é automático, compatível
usado `--guard-directory` (`crates/iroha_cli/src/commands/sorafs.rs:365`).

Менеджер обновляет circuitos, когда меняются метаданные guarda (ponto final, chave PQ
ou carimbo de data/hora fixado) ou definindo TTL. Ajuda `refresh_circuits`, disponível
antes de buscar (`crates/sorafs_orchestrator/src/lib.rs:1346`), verifique o log
`CircuitEvent`, o funcionamento do sistema permite a configuração do ciclo de operação. Mergulhe
teste `circuit_manager_latency_soak_remains_stable_across_rotations`
(`crates/sorafs_orchestrator/src/soranet.rs:1479`) demonstração estável
guardas de rotações латентность на трех; смотрите отчет em
`docs/source/soranet/reports/circuit_stability.md:1`.

### 1.6 Локальный QUIC-прокси

O orquestrador pode fornecer uma operação QUIC local, um dispositivo de brasão
Os adaptadores SDK e SDK não atualizam chaves de cache de segurança ou proteção. Proksi
слушает loopback-адрес, завершает QUIC соединения e возвращает Norito manifesto,
chave de cache de proteção oficial e opcional. Transporte,
эмитируемые прокси, учитываются em `sorafs_orchestrator_transport_events_total`.

Abra o novo bloco `local_proxy` no organizador JSON:

```json
"local_proxy": {
  "bind_addr": "127.0.0.1:9443",
  "telemetry_label": "dev-proxy",
  "guard_cache_key_hex": "0123456789ABCDEF0123456789ABCDEF0123456789ABCDEF0123456789ABCDEF",
  "emit_browser_manifest": true,
  "proxy_mode": "bridge",
  "prewarm_circuits": true,
  "max_streams_per_circuit": 64,
  "circuit_ttl_hint_secs": 300,
  "norito_bridge": {
    "spool_dir": "./storage/streaming/soranet_routes",
    "extension": "norito"
  },
  "kaigi_bridge": {
    "spool_dir": "./storage/streaming/soranet_routes",
    "extension": "norito",
    "room_policy": "public"
  }
}
```- `bind_addr` permite a expansão do endereço (usando a porta `0` para эфемерного
  porta).
- `telemetry_label` распространяется в метриках, чтобы дашборды различали прокси
  e fetch-сессии.
- `guard_cache_key_hex` (опционально) позволяет прокси отдавать тот же digitado
  guard cache, que usa CLI/SDK, que armazena a segurança
  синхронизированными.
- `emit_browser_manifest` exibe o manifesto, que pode ser protegido
  сохранять e проверять.
- `proxy_mode` é usado, você pode encontrar o tráfego local (`bridge`) ou
  Além disso, o SDK usa circuitos SoraNet também
  (`metadata-only`). Para `bridge`; usar `metadata-only`, exceto
  рабочая станция должна выдавать manifest без ретрансляции потоков.
-`prewarm_circuits`, `max_streams_per_circuit` e `circuit_ttl_hint_secs`
  передают браузеру дополнительные dicas, чтобы он мог бюджетировать параллельные
  потоки и понимать агрессивность circuitos de reutilização.
- `car_bridge` (опционально) указывает на локальный cache CAR-архивов. Pólo
  `extension` é suficiente, o destino do código não é compatível com `*.car`; задайте
  `allow_zst = true` para usar o `*.car.zst`.
- `kaigi_bridge` (опционально) экспонирует Rotas Kaigi do carretel no processo. Pólo
  `room_policy` é definido como `public` ou `authenticated`, que está funcionando
  Os clientes foram selecionados para corrigir rótulos GAR.
- `sorafs_cli fetch` substitui `--local-proxy-mode=bridge|metadata-only`
  e `--local-proxy-norito-spool=PATH`, você pode usar runtime-режим ou указывать
  Os spools alternativos não são usados na política JSON.
- `downgrade_remediation` (опционально) настраивает автоматический gancho de downgrade.
  Когда включено, оркестратор следит за relés de telemetria para downgrade de всплесков e,
  após a substituição de `threshold` em `window_secs`, verifique o processo
  em `target_mode` (por exemplo `metadata-only`). Когда downgrades прекращаются,
  procure por `resume_mode` por `cooldown_secs`. Use o máximo
  `modes`, é um conjunto de relés de configuração de acionamento de relés de entrada (para relés de entrada).

Para que o projeto funcione na configuração da ponte, você terá duas vantagens:

- **`norito`** — stream target клиента разрешается относительно
  `norito_bridge.spool_dir`. Alvos санитизируются (por travessia, sem
  абсолютных путей), e если файл без расширения, применяется настроенный суффикс
  para abrir a carga útil no navegador.
- **`car`** — destinos de fluxo definidos por `car_bridge.cache_dir`, наследуют
  A segurança padrão e a abertura de cargas úteis, exceto `allow_zst`, não foram ativadas.
  A ponte aberta está aberta `STREAM_ACK_OK` antes de ser construída, arquivando, чтобы
  clientes podem verificar o pipeline.

No seu caso, a tag de cache HMAC (a chave de cache de proteção é usada para você
время handshake) e записывает `norito_*` / `car_*` códigos de razão, чтобы дашборды
различали успехи, отсутствие файлов e ошибки санитизации.`Orchestrator::local_proxy().await` identificador de segurança para PEM
certificado, manifesto do navegador específico ou proteção correta para você
приложения.

Quando você está procurando, você deve verificar **manifest v2**. Pomo
существующих сертификата e guard cache key, v2 добавляет:

- `alpn` (`"sorafs-proxy/1"`) e `capabilities`, os clientes estão fechados
  protocolo.
- `session_id` no handshake e `cache_tagging` salt block para derivação
  afinidade de guarda de sessão e tags HMAC.
- Dicas para circuito e seleção de guarda (`circuit`, `guard_selection`,
  `route_hints`) para maior interface de usuário para abertura de arquivos.
- `telemetry_v2` com botões de configuração e privacidade para instalações locais.
- O `STREAM_ACK_OK` é igual ao `cache_tag_hex`. Clientes perderam isso
  em `x-sorafs-cache-tag` para proteção HTTP/TCP, proteção em cache
  seleções são exibidas no disco.

Este é o lugar certo para os clientes — os clientes podem começar a conhecer novos cliques e
subconjunto v1 produzido.

## 2. Семантика отказов

O orquestrador prepara a estrutura de uma maneira fácil e rápida de fazer isso
первого байта. Opções de seleção em três categorias:

1. **Отказы по elegível (pré-voo).** Провайдеры без capacidade de alcance,
   exibir anúncios ou usar a visualização telefônica no placar
   artefacto e não incluído no planeamento. Resumos CLI заполняют массив
   `ineligible_providers` причинами, чтобы операторы могли увидеть drift
   governança sem logotipos de análise.
2. **Esgotamento do tempo de execução.** Каждый провайдер отслеживает последовательные ошибки.
   Para fornecer `provider_failure_threshold`, verifique o valor como
   `disabled` para sessão de conexão. Ou seu fornecedor é `disabled`, оркестратор
   instale `MultiSourceError::NoHealthyProviders { last_error, chunk_index }`.
3. **Definições de controle.** Quais são os limites permitidos para a estrutura
   ошибки:
   - `MultiSourceError::NoCompatibleProviders` — манифест требует span/alignment,
     O provedor de serviços de cozinha não pode ser solucionado.
   - `MultiSourceError::ExhaustedRetries` — orçamento retido em pedaços.
   - `MultiSourceError::ObserverFailed` — observadores downstream (ganchos de streaming)
     pedaço de отклонили проверенный.

Каждая ошибка содержит индекс проблемного pedaço e, когда доступно, финальную
причину отказа провайдера. Считайте эти ошибки bloqueadores de liberação — повторные
попытки с тем же input воспроизведут сбой, пока advert, телеметрия или здоровье
a prova não foi corrigida.

### 2.1 Placar de segurança

Por meio do `persist_path`, o orquestrador define o placar final depois
каждого прогона. Formato do documento JSON:

- `eligibility` (`eligible` ou `ineligible::<reason>`).
- `weight` (normalmente usado, especificado para este programa).
- метаданные `provider` (идентификатор, endpoints, бюджет параллелизма).

Архивируйте snapshots scoreboard вместе с release артефактами, чтобы решения по
lista negra e implementação são comprovadas.

## 3. Telemetria e saída

### 3.1 Métrica Prometheus

O operador possui uma medição métrica `iroha_telemetry`:| Métrica | Etiquetas | Descrição |
|--------|--------|----------|
| `sorafs_orchestrator_active_fetches` | `manifest_id`, `region` | Medidor ativo fetch-операций. |
| `sorafs_orchestrator_fetch_duration_ms` | `manifest_id`, `region` | Гистограмма полной латентности fetch. |
| `sorafs_orchestrator_fetch_failures_total` | `manifest_id`, `region`, `reason` | Счетчик финальных отказов (исчерпаны ретраи, нет провайдеров, ошибка observador). |
| `sorafs_orchestrator_retries_total` | `manifest_id`, `provider`, `reason` | Счетчик попыток ретраев по провайдерам. |
| `sorafs_orchestrator_provider_failures_total` | `manifest_id`, `provider`, `reason` | Certifique-se de que isso seja feito em sua sessão, fornecendo-o. |
| `sorafs_orchestrator_policy_events_total` | `region`, `stage`, `outcome`, `reason` | As configurações de política anônima (gravação vs brownout) são implementadas durante o lançamento e o substituto. |
| `sorafs_orchestrator_pq_ratio` | `region`, `stage` | O histórico de relés PQ está disponível no SoraNet. |
| `sorafs_orchestrator_pq_candidate_ratio` | `region`, `stage` | Гистограмма доли PQ relés no placar instantâneo. |
| `sorafs_orchestrator_pq_deficit_ratio` | `region`, `stage` | Гистограмма дефицита политики (разница между целью и фактической долей PQ). |
| `sorafs_orchestrator_classical_ratio` | `region`, `stage` | Гистограмма доли классических relés em cada sessão. |
| `sorafs_orchestrator_classical_selected` | `region`, `stage` | A tabela de registros contém relés clássicos na sessão. |

Integre métricas em painéis de preparação para ativar botões de produção.
Рекомендуемая раскладка повторяет план наблюдаемости SF-6:

1. **Buscas ativas** — alerta, exceto o medidor que não contém conclusões completas.
2. **Proporção de novas tentativas** — calcula as linhas de base do histórico de `retry`.
3. **Falhas do provedor** — acionar alertas de pager, como resultado do provedor de serviços
   `session_failure > 0` em 15 minutos.

### 3.2 Destinos de log estruturados

O organizador do projeto publica a estrutura de metas para determinar os alvos:

- `telemetry::sorafs.fetch.lifecycle` — marcadores `start` e `complete` com um ícone
  pedaços, ретраев e общей длительностью.
- `telemetry::sorafs.fetch.retry` — события ретраев (`provider`, `reason`,
  `attempts`) para triagem completa.
- `telemetry::sorafs.fetch.provider_failure` — verificador, desativado
  повторяющихся ошибок.
- `telemetry::sorafs.fetch.error` — финальные отказы с `reason` e опциональными
  метаданными провайдера.

Definindo este item no pipeline de log Norito, identificando o incidente
resposta é uma história diferente. Ciclo de vida события показывают PQ/clássico
misture o número `anonymity_effective_policy`, `anonymity_pq_ratio`,
`anonymity_classical_ratio` e связанные счетчики, что упрощает настройку
дашбордов não é uma métrica de análise. No início dos lançamentos do GA, você deve usar os logotipos
`info` para ciclo de vida/repetição resolve e usa `warn` para erros de terminal.

### 3.3 Resumos JSON

`sorafs_cli fetch` e Rust SDK fornecem um resumo da estrutura, escrito:- `provider_reports` com um código aprovado/configurado e um estado de verificação aprovado.
- `chunk_receipts`, показывающий какой провайдер обслужил каждый pedaço.
- Números `retry_stats` e `ineligible_providers`.

Архивируйте resumo при отладке проблемных провайдеров — recibos напрямую
соотносятся с лог-метаданными выше.

## 4. Guia de operação

1. **Configure a configuração no CI.** Abra `sorafs_fetch` com a chave
   configuração, verifique `--scoreboard-out` para visualização de elegibilidade e
   сравните с предыдущим lançamento. Любой неожиданный provedor inelegível
   Bloqueie a promoção.
2. **Proverьте телеметрию.** Убедитесь, что деплой экспортирует метрики
   `sorafs.fetch.*` e log estrutural por meio da busca de múltiplas fontes
   para polьзователей. A métrica de saída é definida como um operador de palco
   não foi possível.
3. **Substituições de duplicação.** Em caso de emergência `--deny-provider` ou
   `--boost-provider` atualiza JSON (ou CLI) no changelog. Reversões
   должны отменить override e снять новый scoreboard snapshot.
4. **Realize testes de fumaça.** Orçamentos de orçamentos redefinidos ou tampas
   провайдеров заново выполните buscar fixação canônica
   (`fixtures/sorafs_manifest/ci_sample/`) e убедитесь, quais recibos em pedaços
   остаются детерминированными.

Следование шагам выше делает поведение оркестратора воспроизводимым в encenado
implementações e implementação de telemetria para resposta a incidentes.

### 4.1 Substitui a política

O operador pode ativar o transporte ativo/anonimato этап без изменения базовой
configuração, use `policy_override.transport_policy` e
`policy_override.anonymity_policy` em JSON `orchestrator` (ou seja,
`--transport-policy-override=` / `--anonymity-policy-override=` em
`sorafs_cli fetch`). Ou seja, override присутствует, оркестратор пропускает обычный
fallback de brownout: если требуемый PQ tier недостижим, fetch завершается с
`no providers` é um downgrade. Возврат к поведению по умолчанию —
простое очищение override полей.