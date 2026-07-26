---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/orchestrator-config.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: configuración del orquestador
título: Configuracao do orquestador SoraFS
sidebar_label: Configuración del orquestador
Descripción: Configure el orquestador de búsqueda de múltiples orígenes, interprete las fallas y depure la señal de telemetría.
---

:::nota Fuente canónica
Esta página espelha `docs/source/sorafs/developer/orchestrator.md`. Mantenha ambas as copias sincronizadas ate que a documentacao alternativa seja retirada.
:::

# Guía del orquestador de buscar origen múltiple

El orquestador de fetch multi-origem do SoraFS conduz descargas deterministas e
paralelos a partir del conjunto de proveedores publicados en anuncios respaldados
pela gobernancia. Esta guía explica cómo configurar el orquestador, quais sinais.
de falha esperar durante los lanzamientos y los flujos de telemetría expõem
indicadores de salud.

## 1. Visao general de configuración

El orquestador combina tres fuentes de configuración:| Fuente | propuesta | Notas |
|-------|-----------|-------|
| `OrchestratorConfig.scoreboard` | Normaliza pesos de proveedores, valida la frescura de la telemetría y persiste el marcador JSON usado para auditorias. | Apoiado por `crates/sorafs_car::scoreboard::ScoreboardConfig`. |
| `OrchestratorConfig.fetch` | Aplicación de límites de tiempo de ejecución (presupuestos de reintento, límites de concordancia, alternancia de verificación). | Mapas para `FetchOptions` y `crates/sorafs_car::multi_fetch`. |
| Parámetros de CLI / SDK | Limita el número de pares, regiones anexas de telemetría y exposición política de deny/boost. | `sorafs_cli fetch` exponen estas banderas directamente; Los SDK del sistema operativo se propagan a través de `OrchestratorConfig`. |

Los ayudantes JSON en `crates/sorafs_orchestrator::bindings` serializan un
Configuración completa en Norito JSON, tornando un portátil entre enlaces de SDK
e automacao.

### 1.1 Ejemplo de configuración JSON

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

Persista el archivo atraves del empilhamento habitual de `iroha_config` (`defaults/`,
usuario, real) para que los despliegues deterministas herdem os mesmos limites entre
nudos. Para un perfil de respaldo solo directo alinhado ao rollout SNNet-5a,
consultar `docs/examples/sorafs_direct_mode_policy.json` y orientación
correspondiente en `docs/source/sorafs/direct_mode_pack.md`.

### 1.2 Anulaciones de conformidad

A SNNet-9 integra conformidade orientada pela gobernadora no orquestador. Eh
Nuevo objeto `compliance` en la configuración Norito captura JSON de carve-outs que
Forcam o pipeline de fetch en modo directo-solo:

```json
"compliance": {
  "operator_jurisdictions": ["US", "JP"],
  "jurisdiction_opt_outs": ["US"],
  "blinded_cid_opt_outs": [
    "C6B434E5F23ABD318F01FEDB834B34BD16B46E0CC44CD70536233A632DFA3828"
  ],
  "audit_contacts": ["mailto:compliance@example.org"]
}
```- `operator_jurisdictions` declara los códigos ISO-3166 alfa-2 en esta
  ejemplo del orquestador de ópera. Os codigos sao normalizados para maiusculas
  durante el análisis.
- `jurisdiction_opt_outs` espelha o registro de gobierno. Cuando qualquer
  Jurisdicao do operador aparece en la lista, o orquestador aplica
  `transport_policy=direct-only` y emite o motivo de retroceso
  `compliance_jurisdiction_opt_out`.
- `blinded_cid_opt_outs` lista de resúmenes de manifiesto (CID cegados, codificados en
  mayúsculo hexadecimal). Cargas útiles corresponsales tambem forcam agendamento solo directo
  E expõem o fallback `compliance_blinded_cid_opt_out` na telemetria.
- `audit_contacts` registra como URI que un gobierno espera que los operadores
  publiquem nos playbooks GAR.
- `attestations` captura os pacotes de conformidade assinados que sustentam a
  política. Cada entrada define una `jurisdiction` opcional (código ISO-3166
  alpha-2), um `document_uri`, o `digest_hex` canonico de 64 caracteres, o
  marca de tiempo de emisión `issued_at_ms` y un `expires_at_ms` opcional. eses
  artefatos alimentam o checklist de auditoria do orquestador para que as
  ferramentas degobernanza possam vincular anula una documentacao assinada.Forneca o bloco de conformidade via o empilhamento usual de configuracao para
que os operadores recebam anulan a los deterministas. El orquestador aplica a
conformidad _depois_ dos sugerencias de modo de escritura: mesmo que un SDK solicita
`upload-pq-only`, opt-outs de jurisdicao ou manifest ainda forcam o transporte
para direct-only e falham rapidamente cuando nao existen proveedores conformes.

Catálogos canónicos de opt-out vivem em
`governance/compliance/soranet_opt_outs.json`; o Consejo de Gobernanza Pública
atualizacoes vía lanzamientos tagueadas. Un ejemplo completo de configuración
(incluindo atestados) esta disponible em
`docs/examples/sorafs_compliance_policy.json`, y el proceso operativo esta
capturado no
[playbook de conformidade GAR](../../../source/soranet/gar_compliance_playbook.md).

### 1.3 Ajustes de CLI y SDK| Bandera / Campo | Efeito |
|----------------------|--------|
| `--max-peers` / `OrchestratorConfig::with_max_providers` | Limita la cantidad de proveedores que sobreviven al filtro del marcador. Defina `None` para usar todos los proveedores elegidos. |
| `--retry-budget` / `FetchOptions::per_chunk_retry_limit` | Limita los reintentos por fragmento. Se excede el límite gera `MultiSourceError::ExhaustedRetries`. |
| `--telemetry-json` | Injeta instantáneas de latencia/falha no constructor del marcador. Telemetria obsoleta alem de `telemetry_grace_secs` marca provedores como inelegiveis. |
| `--scoreboard-out` | Persiste o scoreboard calculado (provedores elegiveis + inelegiveis) para inspección pos-ejecución. |
| `--scoreboard-now` | Sobrescreve la marca de tiempo del marcador (segundos Unix) para mantener capturas de partidos deterministas. |
| `--deny-provider` / gancho de política de puntuación | Exclui proveedores de forma determinística sin eliminar anuncios. Utilidad para incluir rápidamente en listas negras. |
| `--boost-provider=name:delta` | Ajusta los créditos al round robin ponderado de un proveedor manteniendo los pesos de gobierno intactos. |
| `--telemetry-region` / `OrchestratorConfig::with_telemetry_region` | Rotulas métricas emitidas y registros estructurados para que los paneles de control puedan filtrar por geografía o onda de despliegue. || `--transport-policy` / `OrchestratorConfig::with_transport_policy` | El padrao agora e `soranet-first` ja que o orquestador multi-origem e a base. Utilice `direct-only` para preparar la degradación o seguir una dirección de conformidad y reserve `soranet-strict` para pilotos PQ-only; invalida la conformidade continuam sendo o teto rígido. |

SoraNet-first y ahora o padrao de envio, y rollbacks deben citar o bloquear
SNNet relevante. Después de SNNet-4/5/5a/5b/6a/7/8/12/13 antes graduadas, a
gobernadora endurecera a postura requerida (rumo a `soranet-strict`); comió la,
apenas overrides motivados por incidentes devem priorizar `direct-only`, e eles
Deben estar registrados en el registro de implementación.

Todos los flags acima aceitam a sintaxe `--` tanto em `sorafs_cli fetch` cuanto no
binario `sorafs_fetch` voltado a desenvolvedores. Los SDK se consideran como otras operaciones
por meio de constructores tipados.

### 1.4 Gestao de caché de guardias

La CLI ahora integra el selector de guardias de SoraNet para que los operadores puedan
arreglar los relés de entrada de forma determinística antes del despliegue completo de
transporte SNNet-5. Tres novos flags controlam o fluxo:| Bandera | propuesta |
|------|-----------|
| `--guard-directory <PATH>` | Aponta para un archivo JSON que describe el consenso de relés más recientes (subconjunto abajo). Pasar el directorio actualiza el caché de guardias antes de ejecutar o recuperar. |
| `--guard-cache <PATH>` | Persiste el `GuardSet` codificado en Norito. Las ejecuciones posteriores reutilizan el caché también en el nuevo directorio. |
| `--guard-target <COUNT>` / `--guard-retention-days <DAYS>` | Anula las opciones para el número de guardias de entrada a fixar (padrao 3) y a janela de retencao (padrao 30 días). |
| `--guard-cache-key <HEX>` | Chave opcional de 32 bytes usados ​​para marcar cachés de guardia con un MAC Blake3 para que el archivo seja verificado antes de la reutilización. |

Como cargas de directorio de guardias usan un esquema compacto:

O flag `--guard-directory` ahora espera una carga útil `GuardDirectorySnapshotV2`
codificado en Norito. O instantánea binaria contem:- `version` — versao do esquema (actualmente `2`).
- `directory_hash`, `published_at_unix`, `valid_after_unix`, `valid_until_unix` —
  metadados de consenso que devem corresponden a cada certificado embutido.
- `validation_phase` — puerta de política de certificados (`1` = permitir uma
  assinatura Ed25519, `2` = preferir assinaturas duplas, `3` = exigir assinaturas
  duplas).
- `issuers` — emisores de gobierno con `fingerprint`, `ed25519_public` e
  `mldsa65_public`. Os huellas dactilares sao calculadas como
  `BLAKE3("soranet.src.v2.issuer" || ed25519 || u32(len(ml-dsa)) || ml-dsa)`.
- `relays` — una lista de paquetes SRCv2 (saida de
  `RelayCertificateBundleV2::to_cbor()`). Cada paquete carga o descriptor hacer
  relevo, banderas de capacidad, política ML-KEM e assinaturas duplas Ed25519/ML-DSA-65.

A CLI verifica cada paquete contra as chaves de emissor declaradas antes de
mesclar o directorio com o caché de guardias. Esbocos JSON alternativos nao sao mais
aceitos; instantáneas SRCv2 sao obrigatorios.Invoque a CLI com `--guard-directory` para aclarar el consenso más reciente con o
caché existente. O seletor preserva guards fixados que ainda estao dentro da
janela de retencao e sao elegiveis no hay directorio; novos relevos sustituyeem entradas
expiradas. Después de un fetch bem-sucedido, el caché actualizado y escrito de volta
no caminho fornecido via `--guard-cache`, manteniendo sesiones posteriores
criteriosas. Los SDK pueden reproducir el mismo comportamiento del usuario
`GuardSelector::select(&RelayDirectory, existing_guard_set, now_unix_secs)` y
pasando el `GuardSet` resultante para `SorafsGatewayFetchOptions`.

`ml_kem_public_hex` permite que el selector priorice guardias con capacidad PQ
Cuando el lanzamiento de SNNet-5 está en curso. Os alterna de etapa (`anon-guard-pq`,
`anon-majority-pq`, `anon-strict-pq`) ágora rebaixam relés clásicos
automáticamente: cuando se guarda PQ, este nivel está disponible en el selector de eliminación de pines
clásicos excedentes para que sesiones posteriores favorecam handshakes hibridos.
Los resúmenes CLI/SDK exponen la mezcla resultante vía `anonymity_status`/
`anonymity_reason`, `anonymity_effective_policy`, `anonymity_pq_selected`,
`anonymity_classical_selected`, `anonymity_pq_ratio`, `anonymity_classical_ratio`
e campos complementarios de candidatos/deficit/delta de oferta, tornando claros
apagones y retrocesos clásicos.Directorios de guardias ahora pueden embutir un paquete SRCv2 completo vía
`certificate_base64`. O orquestador decodifica cada paquete, revalida como
assinaturas Ed25519/ML-DSA y retém o certificado analizado junto al caché de
guardias. Quando um certificado esta presente ele se torna a fonte canonica para
chaves PQ, preferencias de handshake y ponderacao; certificados vencidos sao
descartados y el selector retorna a los campos alternativos del descriptor. Certificados
propagam-se pela gestao do ciclo de vida de circuitos e sao expostos via
`telemetry::sorafs.guard` e `telemetry::sorafs.circuit`, que registraron a janela
de validade, suites de handshake e se assinaturas duplas foram observadas para
cada guardia.

Utilice los ayudantes de CLI para mantener instantáneas en sincronización con publicadores:

```bash
sorafs_cli guard-directory fetch \
  --url https://directory.soranet.dev/mainnet_snapshot.norito \
  --output ./state/guard_directory.norito \
  --expected-directory-hash <directory-hash-hex>

sorafs_cli guard-directory verify \
  --path ./state/guard_directory.norito \
  --expected-directory-hash <directory-hash-hex>
```

`fetch` baja y verifica la instantánea SRCv2 antes de grabarlo en disco, mientras
`verify` reproduz o pipeline de validacao para artefatos vindos de outras equipes,
Emitiendo un resumen JSON que se escribe en el selector de guardias CLI/SDK.

### 1.5 Gestor de ciclo de vida de circuitos

Cuando un directorio de retransmisión y un caché de guardias sao fornecidos, o orquestador
activa o gestor de ciclo de vida de circuitos para preconstruir y renovar
circuitos SoraNet antes de cada recuperación. La configuración vive en `OrchestratorConfig`
(`crates/sorafs_orchestrator/src/lib.rs:305`) vía dos novos campos:- `relay_directory`: cargue la instantánea del directorio SNNet-3 para que salte
  middle/exit sejam selecionados de forma determinística.
- `circuit_manager`: configuracao opcional (habilitada por padrao) que controla o
  TTL hacer circuito.

Norito JSON ahora aceita un bloque `circuit_manager`:

```json
"circuit_manager": {
  "enabled": true,
  "circuit_ttl_secs": 900
}
```

Los SDK encaminan los datos del directorio a través de
`SorafsGatewayFetchOptions::relay_directory`
(`crates/iroha/src/client.rs:320`), y una CLI o se conecta automáticamente siempre que
`--guard-directory` y fornecido (`crates/iroha_cli/src/commands/sorafs.rs:365`).

O gestor renova circuitos siempre que metadados do guard mudam (endpoint, chave
PQ o marca de tiempo fijada) o cuando expira el TTL. Oh ayudante `refresh_circuits`
invocado antes de cada fetch (`crates/sorafs_orchestrator/src/lib.rs:1346`)
emite registros `CircuitEvent` para que los operadores puedan rastrear decisiones de ciclo
de vida. O prueba de inmersión `circuit_manager_latency_soak_remains_stable_across_rotations`
(`crates/sorafs_orchestrator/src/soranet.rs:1479`) demuestra latencia estavel
atraves de tres rotacoes de guardias; veja o relatorio em
`docs/source/soranet/reports/circuit_stability.md:1`.

### 1.6 Proxy QUIC local

El orquestador puede iniciar opcionalmente un proxy QUIC local para que se extienda
de navegador y adaptadores SDK nao precisam gerenciar certificados o chaves de
caché de guardias. O proxy liga a um endereco loopback, encerra conexoes QUIC e
retorna um manifest Norito descrevendo o certificado e a chave de cache de guard
opcional al cliente. Eventos de transporte emitidos pelo proxy sao contados via
`sorafs_orchestrator_transport_events_total`.Habilite el proxy por medio del nuevo bloque `local_proxy` sin JSON del orquestador:

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
```- `bind_addr` controla onde o proxy cortado (use porta `0` para solicitar porta
  efímera).
- `telemetry_label` propaga-se para as métricas para que los paneles distingan
  proxy de sesiones de búsqueda.
- `guard_cache_key_hex` (opcional) permite que el proxy muestre o mesmo cache de
  Guards con chave que CLI/SDK usa, manteniendo extenso el navegador conectado.
- `emit_browser_manifest` alterna se o handshake devuelve un manifiesto que
  extensoes podem armazenar e validar.
- `proxy_mode` selecciona se o proxy faz bridge local (`bridge`) o apenas emite
  metadados para que SDK abram circuitos SoraNet por cuenta propia
  (`metadata-only`). O proxy padrao e `bridge`; use `metadata-only` cuando um
  La estación de trabajo debe exportar el manifiesto sin retransmitir flujos.
- `prewarm_circuits`, `max_streams_per_circuit` e `circuit_ttl_hint_secs`
  expõem sugerencias adicionales al navegador para que possa orcar streams paralelos e
  entender la cantidad de circuitos de reutilización de proxy.
- `car_bridge` (opcional) aponta para un caché local de archivos CAR. El campo
  `extension` controla el sufijo anexado cuando el alvo de stream omite `*.car`;
  defina `allow_zst = true` para servir payloads `*.car.zst` precomprimidos.
- `kaigi_bridge` (opcional) muestra rotaciones Kaigi en spool ao proxy. El campo
  `room_policy` anuncia se o bridge opera em modo `public` o `authenticated`para que los clientes hagan navegador preselecionem os etiquetas GAR corretos.
- `sorafs_cli fetch` anula la exposición `--local-proxy-mode=bridge|metadata-only` e
  `--local-proxy-norito-spool=PATH`, lo que permite alternar el modo de tiempo de ejecución o
  Apontar para spools alternativas sin modificar a política JSON.
- `downgrade_remediation` (opcional) configura el gancho de degradación automática.
  Cuando habilitado, el orquestador observa a telemetria de relevos para rajadas
  de downgrade e, apos o `threshold` configurado dentro de `window_secs`, força o
  proxy local para o `target_mode` (padrao `metadata-only`). Cuando el sistema operativo baja de categoría
  cessam, o proxy retorna ao `resume_mode` apos `cooldown_secs`. Usar o matriz
  `modes` para limitar o gatilho a funciones de relé específicas (padrao relés
  de entrada).

Cuando el proxy roda en modo puente sirve dos servicios de aplicación:- **`norito`** – o alvo de stream do cliente e resuelto a relativo
  `norito_bridge.spool_dir`. Os alvos sao sanitizados (sem traversal, sem caminhos
  absolutos), y cuando el archivo nao tem extensao, o sufixo configurado y aplicado
  Antes de que la carga útil se transmita al navegador.
- **`car`** – algunos de los flujos se resuelven dentro de `car_bridge.cache_dir`, herdam
  a extensao padrao configurado y rejeitam payloads comprimidos a menos que
  `allow_zst` esteja habilitada. Puentes bem-sucedidos respondem com `STREAM_ACK_OK`
  antes de transferir los bytes del archivo para que los clientes puedan realizar la canalización
  da verificaçao.

En ambos casos el proxy fornece o HMAC do cache-tag (cuando tiene una chave de
cache de guard durante o handshake) e registra códigos de razao de telemetría
`norito_*` / `car_*` para que los paneles de control difieran en éxito, archivos ausentes
e falhas de sanitizacao rapidamente.

`Orchestrator::local_proxy().await` Expõe o handle em execucao para que chamadas
Puede leer el certificado PEM, buscar el manifiesto del navegador o solicitarlo.
encerramento gracioso cuando la aplicación finaliza.

Cuando se habilita, el proxy ahora sirve registros **manifest v2**. alem hacer
certificado existente y chave de cache de guard, a v2 adiciona:- `alpn` (`"sorafs-proxy/1"`) y una matriz `capabilities` para que clientes
  confirmem o protocolo de stream que devem falar.
- Un `session_id` para apretón de manos y un bloque de sal `cache_tagging` para derivar
  afinidades de guardia por sesión y etiquetas HMAC.
- Consejos de circuito y selección de guardia (`circuit`, `guard_selection`,
  `route_hints`) para que integracoes do navegador exponham uma UI mais rica antes
  de abrir arroyos.
- `telemetry_v2` con botones de protección y privacidad para instrumentos locales.
- Cada `STREAM_ACK_OK` incluido `cache_tag_hex`. Clientes espelham o valor no header
  `x-sorafs-cache-tag` ao emite requisitos HTTP o TCP para que selecciones de guardia
  em cache permaneçam criptografadas em repouso.

Esses campos fazem parte do esquema actual; los clientes deben utilizar el conjunto
completo ao negociar transmisiones.

## 2. Semántica de falhas

El orquestador aplica verificacoes estritas de capacidade y presupuestos antes que um
unico byte seja transferido. As falhas se enquadram em tres categorias:1. **Falhas de elegibilidade (pre-vuelo).** Proveedores sin capacidad de alcance,
   anuncios caducados o telemetria obsoleta sao registrados no artefato do
   marcador e omitidos do agendamento. Resumen del preenchem o array de CLI
   `ineligible_providers` con razoes para que operadores inspeccionem drift de
   gobernanza sem raspar troncos.
2. **Esgotamento em runtime.** Cada proveedor rastreia falhas consecutivas. cuando
   `provider_failure_threshold` y atingido, o proveedor y marcado como `disabled`
   pelo restante da sessao. Se todos los proveedores transicionarem para `disabled`,
   o orquestador retorna
   `MultiSourceError::NoHealthyProviders { last_error, chunk_index }`.
3. **Abortos deterministas.** Limites rígidos surgen como errores estruturados:
   - `MultiSourceError::NoCompatibleProviders` — o manifiesta exige um span de
     trozos o alinhamento que los proveedores restantes nao conseguem honrar.
   - `MultiSourceError::ExhaustedRetries` — o presupuesto de reintentos por fragmento de foi
     consumido.
   - `MultiSourceError::ObserverFailed` — observadores aguas abajo (ganchos de
     streaming) rejeitaram um trozo verificado.

Cada error incorpora el índice del trozo problemático y, cuando disponivel, a razao
final de falha do provedor. Trate esses errores como bloqueadores de liberación —
reintries com a mesma entrada reproduzirao a falha ate que o advert, a telemetria
ou a saude do provedor subyacente mudem.

### 2.1 Persistencia del marcadorCuando `persist_path` y configurado, o orquestador escreve o marcador final
apos cada run. El documento JSON relacionado:

- `eligibility` (`eligible` o `ineligible::<reason>`).
- `weight` (peso normalizado atribuido para este run).
- metadados do `provider` (identificador, endpoints, presupuesto de concorrencia).

Archive snapshots do scoreboard junto aos artefatos de release para que decisoes
de banimento e rollout permaneçam auditaveis.

## 3. Telemetría y depuracao

### 3.1 Métricas Prometheus

El orquestador emite las siguientes métricas vía `iroha_telemetry`:| Métrica | Etiquetas | Descripción |
|---------|--------|-----------|
| `sorafs_orchestrator_active_fetches` | `manifest_id`, `region` | Gauge de fetches orquestrados em voo. |
| `sorafs_orchestrator_fetch_duration_ms` | `manifest_id`, `region` | Histograma registrando latencia de fetch de ponta a ponta. |
| `sorafs_orchestrator_fetch_failures_total` | `manifest_id`, `region`, `reason` | Contador de falhas terminais (retries esgotados, sem provedores, falha de observador). |
| `sorafs_orchestrator_retries_total` | `manifest_id`, `provider`, `reason` | Contador de tentativas de reintento por proveedor. |
| `sorafs_orchestrator_provider_failures_total` | `manifest_id`, `provider`, `reason` | Contador de falhas de provedor na sessao que levam a desabilitacao. |
| `sorafs_orchestrator_policy_events_total` | `region`, `stage`, `outcome`, `reason` | Contagem de decisiones de política de anonimato (cumprida vs brownout) agrupadas por estagio de rollout y motivo de fallback. |
| `sorafs_orchestrator_pq_ratio` | `region`, `stage` | Histograma de participación de retransmisiones PQ en el conjunto seleccionado por SoraNet. |
| `sorafs_orchestrator_pq_candidate_ratio` | `region`, `stage` | Histograma de ratios de oferta de relevos PQ sin instantánea del marcador. |
| `sorafs_orchestrator_pq_deficit_ratio` | `region`, `stage` | Histograma do deficit de politica (brecha entre alvo e a participaçao PQ real). |
| `sorafs_orchestrator_classical_ratio` | `region`, `stage` | Histograma da participación de relevos clásicos usados ​​en cada sesión. || `sorafs_orchestrator_classical_selected` | `region`, `stage` | Histograma de contagios de relés clásicos seleccionados por sesión. |

Integre las métricas en los paneles de preparación antes de habilitar las perillas de
producao. O diseño recomendado espelha o plano de observabilidade SF-6:

1. **Fetches ativos** — alerta se o calibre sobe sem completes correspondientes.
2. **Razao de reintentos** — avisa cuando contadores `retry` exceden las líneas base
   históricas.
3. **Falhas de provedor** — dispara alertas no pager quando qualquer provedor
   cruza `session_failure > 0` dentro de 15 minutos.

### 3.2 Objetivos de registro estructurados

El orquestador público de eventos estructurados para objetivos deterministas:

- `telemetry::sorafs.fetch.lifecycle` — marcadores `start` e `complete` com
  Contagem de trozos, reintentos y duración total.
- `telemetry::sorafs.fetch.retry` — eventos de reintento (`provider`, `reason`,
  `attempts`) manual de triaje para alimentación.
- `telemetry::sorafs.fetch.provider_failure` — proveedores desabilitados devido a
  errores repetidos.
- `telemetry::sorafs.fetch.error` — Falhas terminais resumidas con `reason` e
  metadados opcionalis do provedor.Encaminhe esses fluxos para o pipeline de logs Norito existente para que a
resposta a incidentes tenha uma unica fonte de verdade. Eventos del ciclo de vida.
exponente de mistura PQ/classica vía `anonymity_effective_policy`,
`anonymity_pq_ratio`, `anonymity_classical_ratio` y sus contadores asociados,
tornando simples integrar paneles sin raspar métricas. Durante los lanzamientos de
GA, corrige el nivel de registro en `info` para eventos de ciclo de vida/retry e use
`warn` para falhas terminais.

### 3.3 Currículums JSON

Tanto `sorafs_cli fetch` como el SDK Rust retorna un resumen estructurado que contiene:

- `provider_reports` com contagens de sucesso/falha e se o provedor foi
  debilitado.
- `chunk_receipts` detalhando qual provedor atendeu cada trozo.
- matrices `retry_stats` e `ineligible_providers`.

Arquive o arquivo de resumo ao depurar provedores problemáticos — os recibos
mapeiam directamente para los metadados de log acima.

## 4. Lista de verificación operativa1. **Preparar configuración sin CI.** Ejecute `sorafs_fetch` con configuración
   alvo, pase `--scoreboard-out` para capturar una visa de elegibilidad e
   comparar com o liberación anterior. Qualquer provedor inelegvel inesperado
   interrumpir una promoción.
2. **Validar telemetria.** Garantía de implementación de métricas exportables `sorafs.fetch.*`
   Los registros estructurados antes de habilitar la recuperación de múltiples orígenes para los usuarios. un
   ausencia de métricas normalmente indica que a fachada do orquestador nao foi
   invocada.
3. **Anulaciones de documentación.** Ao aplicar `--deny-provider` o `--boost-provider`
   de emergencia, comité el JSON (o una llamada CLI) sin registro de cambios. Reversiones en desarrollo
   revertir o anular y capturar una nueva instantánea del marcador.
4. **Reejecutar pruebas de humo.** Después de modificar los presupuestos de reintento o los límites de
   provedores, refaça o fetch do fixo canónico
   (`fixtures/sorafs_manifest/ci_sample/`) y verifique que os recibos de trozos
   permaneceremos deterministas.

Seguir os passos acima mantém o comportamento do orquestrador reproduzivel em
implementaciones por fase y fornece a telemetria necesaria para responder a incidentes.

### 4.1 Anulaciones de políticaOperadores pueden arreglar o estación activa de transporte/anonimato sin editar a
configuracao base definindo `policy_override.transport_policy` e
`policy_override.anonymity_policy` en su JSON de `orchestrator` (o fornecendo
`--transport-policy-override=` / `--anonymity-policy-override=` año
`sorafs_cli fetch`). Cuando qualquer anule este presente, o orquestador pula
o caída de tensión de respaldo habitual: se o nivel PQ solicitado nao puder ser satisfeito, o
fetch falha com `no providers` em vez de degradar silenciosamente. oh retroceso
Para o comportamento padrao e tao simple quanto limpar os campos de override.