---
lang: he
direction: rtl
source: docs/portal/docs/sorafs/orchestrator-config.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: orchestrator-config
כותרת: Configuracao do orquestrador SoraFS
sidebar_label: Configuracao do orquestrador
תיאור: קבע את התצורה של אחזור ריבוי מקורות, פרש את ה-falhas ו-depure a saida de telemetria.
---

:::שים לב Fonte canonica
Esta pagina espelha `docs/source/sorafs/developer/orchestrator.md`. Mantenha ambas as copias sincronizadas ate que a documentacao alternativa seja retirada.
:::

# Guia do orquestrador de להביא multi-origem

O orquestrador de fetch multi-origem do SoraFS conduz הורדות deterministas e
paralelos a partir do conjunto de provedores publicado em adverts respaldados
pela governanca. Este guia explica como configurar o orquestrador, Quais Sinais
de falha esperar durante rollouts e quais fluxos de telemetria expõem
אינדיקטורים דה סאודה.

## 1. Visao geral da configuracao

O orquestrador combina tres fontes de configuracao:

| פונטה | פרופוזיטו | Notas |
|-------|----------------|-------|
| `OrchestratorConfig.scoreboard` | נורמליזה פסו דה פרודורס, valida a frescura da telemetria e persiste o לוח התוצאות JSON usado para auditorias. | Apoiado por `crates/sorafs_car::scoreboard::ScoreboardConfig`. |
| `OrchestratorConfig.fetch` | אפליקה מגבילה את זמן הריצה (תקציבים לניסיון חוזר, מגבלות קונקורנסיה, מחליפים של אימות). | Mapeia para `FetchOptions` em `crates/sorafs_car::multi_fetch`. |
| Parametros de CLI / SDK | הגבלה של מספר עמיתים, אזורי טלמטריה ותחזוקה פוליטית של הכחשה/חיזוק. | `sorafs_cli fetch` expõe esses esses diretamente; OS SDKs OS פרופגם דרך `OrchestratorConfig`. |

Os helpers JSON em `crates/sorafs_orchestrator::bindings` serializam a
configuracao completa em Norito JSON, tornando-a portavel entre bindings de SDKs
e automacao.

### 1.1 דוגמה להגדרת JSON

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

Persista o arquivo atraves do empilhamento usual do `iroha_config` (`defaults/`,
משתמש, בפועל) para que deployments deterministas herdem os mesmos limites entre
nodos. עבור תעודת נפילה ישירה בלבד, או השקת SNNet-5a,
להתייעץ עם `docs/examples/sorafs_direct_mode_policy.json` e a orientacao
correspondente em `docs/source/sorafs/direct_mode_pack.md`.

### 1.2 ביטולי התאמה

מכשיר SNNet-9 אינטגרה קונפורמידית orientada pela governanca no orquestrador. אממ
novo objeto `compliance` עם הגדרות Norito JSON captura os carve-outs
forcam o pipeline de fetch ao modo ישירות בלבד:

```json
"compliance": {
  "operator_jurisdictions": ["US", "JP"],
  "jurisdiction_opt_outs": ["US"],
  "blinded_cid_opt_outs": [
    "C6B434E5F23ABD318F01FEDB834B34BD16B46E0CC44CD70536233A632DFA3828"
  ],
  "audit_contacts": ["mailto:compliance@example.org"]
}
```- `operator_jurisdictions` declara os codigos ISO-3166 alpha-2 onde esta
  instancia do orquestrador אופרה. Os codigos sao normalizados para maiusculas
  durante o ניתוח.
- `jurisdiction_opt_outs` espelha o registro de governanca. Quando qualquer
  משפטים לעשות מפעיל אפרטס ברשימה, או אורקסטרדור אפליקה
  `transport_policy=direct-only` emite o motivo de fallback
  `compliance_jurisdiction_opt_out`.
- `blinded_cid_opt_outs` רשימה של תקצירי המניפסט (CIDs cegados, codificados em
  hex maiusculo). מטענים כתבי טמבם forcam agendamento ישירות בלבד
  e expõem o fallback `compliance_blinded_cid_opt_out` עבור טלמטריה.
- `audit_contacts` נרשמים כ-URIs que a governanca espera que os operadores
  publiquem nos playbooks GAR.
- `attestations` captura os pacotes de conformidade assinados que sustentam a
  פוליטיקה. Cada entrada define uma `jurisdiction` אופציונלי (קודיגו ISO-3166
  alpha-2), um `document_uri`, o `digest_hex` canonico de 64 caracters, o
  חותמת זמן של emisso `issued_at_ms` e um `expires_at_ms` אופציונלי. Esses
  artefatos alimentam o checklist de auditoria do orquestrador para que as
  ferramentas de governanca possam vincular עוקף דוקומנטסאו אסינדה.

Forneca o bloco de conformidade via o empilhamento usual de configuracao para
que os operadores recebam מבטל את הדטרמיניסטים. O orquestrador aplica a
conformidade _depois_ רמזים למצב כתיבה: mesmo que um SDK לבקש
`upload-pq-only`, ביטולי הסכמה של משפטים או גילוי נאות או תחבורה
para ישיר בלבד e falham rapidamente quando nao existem provedores conformes.

Catalogos canonicos de opt-out vivem em
`governance/compliance/soranet_opt_outs.json`; o Conselho de Governanca publica
atualizacoes באמצעות משחרר tagueadas. אום דוגמה מלאה לקונפיגוראקאו
(incluindo attestations) esta disponivel em
`docs/examples/sorafs_compliance_policy.json`, או עיבוד תפעולי esta
קפטורדו מס
[playbook de conformidade GAR](../../../source/soranet/gar_compliance_playbook.md).

### 1.3 Ajustes de CLI e SDK| דגל / קמפו | אפייטו |
|-------------|--------|
| `--max-peers` / `OrchestratorConfig::with_max_providers` | מגבלת כמות הוכחות למידע נוסף על לוח התוצאות. Defina `None` עבור שימוש ב-todos os provedores elegiveis. |
| `--retry-budget` / `FetchOptions::per_chunk_retry_limit` | לימיטה מנסה שוב לחלק. Exceder o limite gera `MultiSourceError::ExhaustedRetries`. |
| `--telemetry-json` | אינג'טה צילומי מצב של לטציה/פלהה אין בונה לעשות לוח תוצאות. Telemetria obsoleta alem de `telemetry_grace_secs` marca provedores como inelegiveis. |
| `--scoreboard-out` | המשך לחישוב לוח התוצאות (הוכחה למשחק + חוסר) לבדיקה לאחר ריצה. |
| `--scoreboard-now` | קבע או חותמת זמן לעשות לוח תוצאות (סיגונדוס יוניקס) עבור קביעות מתקנים. |
| `--deny-provider` / הוק דה פוליטיקה דה ניקוד | Exclui provedores de forma deterministica למחוק פרסומות. השתמש ברשימה השחורה rapido. |
| `--boost-provider=name:delta` | Ajusta os creditos do round-robin ponderado de um provedor mantendo os pesos de governanca intactos. |
| `--telemetry-region` / `OrchestratorConfig::with_telemetry_region` | Rotula metricas emitidas e logs estruturados para que dashboards possam filtrar por geographa ou onda de rollout. |
| `--transport-policy` / `OrchestratorConfig::with_transport_policy` | O padrao agora e `soranet-first` או que o orquestrador multi-origem e a base. השתמש ב-`direct-only` ובו הכנת שדרוג לאחור או מדריך לתיאום, ושמור ב-`soranet-strict` עבור פיילוט ל-PQ בלבד; מבטל את דה conformidade continuam sendo o teto rigido. |

SoraNet-first e agora o padrao de envio, e rollbacks devem citar o bloqueador
SNNet רלוונטי. Depois que SNNet-4/5/5a/5b/6a/7/8/12/13 forem graduadas, a
governanca endurecera a postura requerida (rumo a `soranet-strict`); אכל לה,
apenas מבטל את המוטיבציה לאירועים שהתפתחו מראש `direct-only`, e eles
devem ser registrados ללא יומן השקה.

Todos os flags acima aceitam a sintaxe `--` tanto em `sorafs_cli fetch` quanto no
binario `sorafs_fetch` voltado a desenvolvedores. ערכות SDK של מערכות הפעלה מוצגות כאופקו מזימות
por meio de builders tipados.

### 1.4 Gestao de cache de guards

אינטגרת אגורה של CLI או בורר שומרים דה SoraNet עבור מפעילי פוסאם
ממסרים קבועים של אנטרדה דה פורמה דטרמיניסטיקה אנטים עושים השקה מלאה
transporte SNNet-5. Tres Novos flags controlam o fluxo:

| דגל | פרופוזיטו |
|------|--------|
| `--guard-directory <PATH>` | Aponta para um arquivo JSON que decreve o consenso de relays mais recente (subset abaixo). העברת ספרייה או מטמון שומרים לפני הביצוע או אחזור. |
| `--guard-cache <PATH>` | Persiste o `GuardSet` קוד קוד עם Norito. מבצעים מחדש את הפעולות הבאות או המטמון של ספרייה חדשה. |
| `--guard-target <COUNT>` / `--guard-retention-days <DAYS>` | עוקף את האופציות על מספר השומרים דה אנטרדה א פיקסאר (פאדראו 3) e a janela de retencao (padrao 30 dias). |
| `--guard-cache-key <HEX>` | Chave optional de 32 bytes usada para marcar caches de guard com um MAC Blake3 para que o arquivo seja verificado antes da reutilizacao. |

כמו מדריך המשמר של שומרי המטען:דגל O `--guard-directory` agora espera um מטען `GuardDirectorySnapshotV2`
codificado em Norito. הודעת בינאריו של תמונת מצב:

- `version` - versao do esquema (atualmente `2`).
- `directory_hash`, `published_at_unix`, `valid_after_unix`, `valid_until_unix` —
  metadados de consenso que devem מתכתב עם אישור אישור אמבוטידו.
- `validation_phase` — שער פוליטיקה דה תעודות (`1` = permitir uma
  assinatura Ed25519, `2` = preferir assinaturas duplas, `3` = exigir assinaturas
  duplas).
- `issuers` — emissores de governanca com `fingerprint`, `ed25519_public` e
  `mldsa65_public`. Os טביעות אצבע סאו calculados como
  `BLAKE3("soranet.src.v2.issuer" || ed25519 || u32(len(ml-dsa)) || ml-dsa)`.
- `relays` — uma list de bundles SRCv2 (saida de
  `RelayCertificateBundleV2::to_cbor()`). קאדה צרור קרגה או מתאר לעשות
  ממסר, flags de capacidade, פוליטיקה ML-KEM ו-assinaturas duplas Ed25519/ML-DSA-65.

A CLI verifica cada צרור contra as chaves de emissor declaradas antes de
mesclar o directory com o cache de guards. Esbocos JSON אלטרנטיבות לאחר מכן
aceitos; צילומי מצב SRCv2 סאו אובrigatorios.

Invoque a CLI com `--guard-directory` para mesclar o consenso mais recente com o
cache existente. O seletor preserva guards fixados que ainda estao dentro da
janela de retencao e sao elegiveis אין ספרייה; Novos Relays substituem entradas
expiradas. Depois de um להביא bem-sucedido, o cache atualizado e escrito de volta
ללא קמינהו פורנסידו דרך `--guard-cache`, סדנאות מנטנדו לאחר מכן
קריטריונים. Os SDKs podem reproduzir o mesmo comportamento chamando
`GuardSelector::select(&RelayDirectory, existing_guard_set, now_unix_secs)` ה
passando o `GuardSet` resultante para `SorafsGatewayFetchOptions`.

`ml_kem_public_hex` מתיר que o selector priorize guards com capacidade PQ
כיצד להשקיע את SNNet-5 במקום. חילופי מערכות הפעלה de etapa (`anon-guard-pq`,
`anon-majority-pq`, `anon-strict-pq`) agora rebaixam relays classicos
אוטומטי: quando um guard PQ esta disponivel o selector להסיר פינים
classicos excedentes para que sessoes sequentes favorecam לחיצות ידיים hibridos.
קורות החיים של CLI/SDK חושפים את התוצאה דרך `anonymity_status`/
`anonymity_reason`, `anonymity_effective_policy`, `anonymity_pq_selected`,
`anonymity_classical_selected`, `anonymity_pq_ratio`, `anonymity_classical_ratio`
e campos complementares de candidatos/deficit/delta de supply, טורננדו קלרוס
brownouts e fallbacks classicos.

מדריכים של שומרים אגור פודם אמבוטיר על צרור SRCv2 השלם באמצעות
`certificate_base64`. O orquestrador decodifica cada צרור, revalida as
assinaturas Ed25519/ML-DSA e retém o certificado analisado junto ao cache de
שומרים. Quando um certificado esta presente ele se torna a fonte canonica para
chaves PQ, preferences de לחיצת יד e ponderacao; certificados expirados sao
descartados e o seletor retorna aos campos alternativos do descriptor. תעודות
propagam-se pela gestao do ciclo de vida de circuitos e sao expostos via
`telemetry::sorafs.guard` ו-`telemetry::sorafs.circuit`, אני רושם את ג'נלה
de validade, suites de handshake e se assinaturas duplas foram observadas para
שומר קאדה.השתמש ב-OS helpers da CLI לצילומי מצב ב-Sincronia com המפרסמים:

```bash
sorafs_cli guard-directory fetch \
  --url https://directory.soranet.dev/mainnet_snapshot.norito \
  --output ./state/guard_directory.norito \
  --expected-directory-hash <directory-hash-hex>

sorafs_cli guard-directory verify \
  --path ./state/guard_directory.norito \
  --expected-directory-hash <directory-hash-hex>
```

`fetch` הבאיקסה e verifica o תמונת מצב SRCv2 antes de grava-lo em disco, enquanto
`verify` reproduz o pipeline de validacao para artefatos vindos de outras equipes,
emitindo um resumo JSON que espelha a saida do selector de guards CLI/SDK.

### 1.5 Gestor de ciclo de vida de circuitos

ספריית ממסר Quando um e um cache de guards sao fornecidos, o orquestrador
ativa o gestor de ciclo de vida de circuitos para preconstruir e renovar
circuitos SoraNet אחזור מוקדם. A configuracao vive em `OrchestratorConfig`
(`crates/sorafs_orchestrator/src/lib.rs:305`) דרך dois novos campos:

- `relay_directory`: carrega o snapshot do directory SNNet-3 para que hops
  mid/exit sejam selecionados de forma deterministica.
- `circuit_manager`: configuracao אופציונלי (habilitada por padrao) que controla o
  TTL לעשות מעגל.

Norito JSON agora aceita um bloco `circuit_manager`:

```json
"circuit_manager": {
  "enabled": true,
  "circuit_ttl_secs": 900
}
```

OS SDKs encaminham dados לעשות directory via
`SorafsGatewayFetchOptions::relay_directory`
(`crates/iroha/src/client.rs:320`), ו-CLI o conecta automaticamente semper que
`--guard-directory` e fornecido (`crates/iroha_cli/src/commands/sorafs.rs:365`).

O gestor renova circuitos semper que metadados do guard mudam (נקודת קצה, chave
PQ ou fixado חותמת זמן) או quando o TTL expira. O עוזר `refresh_circuits`
אחזור אינבוקדו אנטה דה קאדה (`crates/sorafs_orchestrator/src/lib.rs:1346`)
emite logs `CircuitEvent` para que operadores possam rastrear decisoes de ciclo
דה וידה. O בדיקת השרייה `circuit_manager_latency_soak_remains_stable_across_rotations`
(`crates/sorafs_orchestrator/src/soranet.rs:1479`) הדגמה לטציה אסטבל
atraves de tres rotacoes de guards; veja o relatorio em
`docs/source/soranet/reports/circuit_stability.md:1`.

### 1.6 Proxy QUIC מקומי

O orquestrador pode opcionalmente iniciar um proxy QUIC local para que extensoes
de navegador e adaptadores SDK נאו מדוייק תעודות גרנציה או chaves de
מטמון השומרים. O proxy liga a um endereco loopback, encerra conexoes QUIC e
retorna um manifest Norito descrevendo o certificado e a chave de cache de guard
אופציונלי למשל לקוחות. Eventos de transporte emitidos pelo proxy sao contados via
`sorafs_orchestrator_transport_events_total`.

אפשרות פרוקסי ל-Meido do Novo Bloco `local_proxy` ללא JSON ל-Orquestrador:

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
```- שליטה ב-`bind_addr` ב-Proxy escuta (השתמש בפורטה `0` לפורטה שיפוטית
  אפמרה).
- `telemetry_label` propaga-se para as metricas para que dashboards distingam
  proxies de sessoes de fetch.
- `guard_cache_key_hex` (אופציונלי) מתיר que o proxy exponha o mesmo cache de
  Guards com chave que CLI/SDKs usam, mantendo extensoes do navegador alinhadas.
- `emit_browser_manifest` אלטרנה לס או לחיצת יד devolve um manifest que
  extensoes podem armazenar e validar.
- `proxy_mode` בחר ב-proxy faz bridge local (`bridge`) או apenas emite
  metadados para que SDKs abram circuitos SoraNet por conta propria
  (`metadata-only`). O proxy padrao e `bridge`; השתמש ב-`metadata-only` quando um
  תחנת עבודה מפתחת אקספור או מניפסט שמעבירים זרמים מחדש.
- `prewarm_circuits`, `max_streams_per_circuit` ו-`circuit_ttl_hint_secs`
  expõem hints adicionais ao navegador para que possa orcar streams paralelos e
  או קוונטו או פרוקסי ניצול מעגלים מחדש.
- `car_bridge` (אופציונלי) aponta para um cache local de arquivos CAR. או קמפו
  `extension` controla o sufixo anexado quando o alvo de stream omite `*.car`;
  defina `allow_zst = true` עבור עומסי שירות `*.car.zst` precomprimidos.
- `kaigi_bridge` (אופציונלי) הצגת מחזורי Kaigi עם סליל ו-proxy. או קמפו
  `room_policy` anuncia se o bridge opera em modo `public` ou `authenticated`
  עבור לקוחות לעשות navegador preselecionem או תוויות GAR corretos.
- `sorafs_cli fetch` expõe עוקף את `--local-proxy-mode=bridge|metadata-only` e
  `--local-proxy-norito-spool=PATH`, מאפשר חלופי או מצב ריצה או
  אפונטר עבור סלילים אלטרנטיביים לשינוי ב-JSON פוליטי.
- `downgrade_remediation` (אופציונלי) תצורה או הוק לשדרוג לאחור אוטומטי.
  קונדיטוריה, או אורקסטרדור תצפית על טלמטריה של ממסרים עבור רג'אדס
  degrade down e, apos o `threshold` configurado dentro de `window_secs`, força o
  proxy local para o `target_mode` (padrao `metadata-only`). שדרוג לאחור של מערכת ההפעלה Quando
  cessam, o proxy retorna ao `resume_mode` apos `cooldown_secs`. השתמש במערך o
  `modes` להגביל או להגביל את הפונקציות של ממסר (Padrao ממסרים
  de entrada).

Quando o proxy roda em modo bridge ele serve dois servicos de aplicacao:

- **`norito`** - o alvo de stream do cliente e resolvido relativo a
  `norito_bridge.spool_dir`. Os alvos sao sanitizados (סם מעבר, sem caminhos
  absolutos), e quando o arquivo nao tem extensao, o sufixo configurado e aplicado
  ante do payload ser transmitido para o navegador.
- **`car`** - alvos de stream se resolvem dentro de `car_bridge.cache_dir`, herdam
  a extensao padrao configurada e rejeitam payloads comprimidos a menos que
  `allow_zst` esteja habilitado. Bridges bem-sucedidos respondem com `STREAM_ACK_OK`
  antes de transferir os bytes do arquivo para que clientes possam fazer pipeline
  da verificacao.Em ambos os casos o proxy fornece o HMAC do cache-tag (quando havia uma chave de
cache de guard durante o לחיצת יד) e registra codigos de razao de telemetria
`norito_*` / `car_*` עבור לוחות מחוונים שונים הצלחה, arquivos ausentes
e falhas de sanitizacao rapidamente.

`Orchestrator::local_proxy().await` expõe o handle em execucao para que chamadas
possam ler o PEM לעשות אישורים, buscar או manifest do navegador או עורך דין
encerramento gracioso quando a aplicacao finaliza.

ללא שם: Quando habilitado, או proxy agora serve registros **manifest v2**. עלם כן
אישור קיומו של שומר מטמון, אדיקציה v2:

- `alpn` (`"sorafs-proxy/1"`) ומערך `capabilities` עבור לקוחות
  confirmem o protocolo de stream que devem falar.
- Um `session_id` por לחיצת יד e um bloco de sal `cache_tagging` para נגזרת
  afinidades de guard por sessao e tags HMAC.
- רמזים למעגל ובחירה (`circuit`, `guard_selection`,
  `route_hints`) para que integracoes do navegador exponham uma UI mais rica antes
  זרמי דה abrir.
- `telemetry_v2` כפתורי com de amostragem e פרטיות למכשירים מקומיים.
- Cada `STREAM_ACK_OK` כולל `cache_tag_hex`. Clientes espelham o valor ללא כותרת
  `x-sorafs-cache-tag` ao emitir requisicoes HTTP ou TCP para que selecoes de guard
  em cache permaneçam criptografadas em repouso.

Esses campos fazem parte do schema atual; לקוחות מפתחים שימוש בחיבור
השלמה בין השאר זרמים נגוציארים.

## 2. Semantica de falhas

O orquestrador aplica verificacoes estritas de capacidade e budgets antes que um
unico byte seja transferido. כפי שפאלהס לראות את הקטגוריות:

1. **Falhas de elegibilidade (לפני טיסה).** Provedores sem capacidade de range,
   תוקף פרסומות או טלמטריה מיושן סאו registrados אין artefato לעשות
   לוח התוצאות וחסרונות לעשות סדר היום. רזומה של מערך קדם CLI
   `ineligible_providers` com razoes para que operadores inspecionem drift de
   governanca sem raspar יומני.
2. **Esgotamento em runtime.** Cada provedor rastreia falhas consecutivas. קואנדו
   `provider_failure_threshold` e atingido, o provedor e marcado como `disabled`
   pelo restante da sessao. בדוק את כל הפעולות של טרנסיציונרים עבור `disabled`,
   o orquestrador retorna
   `MultiSourceError::NoHealthyProviders { last_error, chunk_index }`.
3. **Abortos deterministas.** Limites rigidos surgem como erros estruturados:
   - `MultiSourceError::NoCompatibleProviders` — o manifest exige um span de
     chunks ou alinhamento que os provedores restantes nao conseguem honrar.
   - `MultiSourceError::ExhaustedRetries` — o budget de retries por chunk foi
     consumido.
   - `MultiSourceError::ObserverFailed` - צופים במורד הזרם (הווים דה
     סטרימינג) rejeitaram um chunk verificado.

Cada erro incorpora o indice do chunk problemático e, quando disponivel, a razao
final de falha do provedor. Trate esses erros como bloqueadores de release -
ניסיונות חוזרים com a mesma entrada reproduzirao a falha ate que o advert, a telemetria
ou a saude do provedor subjacente mudem.

### 2.1 Persistencia do לוח התוצאותQuando `persist_path` e configurado, o orquestrador escreve או גמר לוח התוצאות
ריצת אפוס קאדה. על תגובת JSON:

- `eligibility` (`eligible` או `ineligible::<reason>`).
- `weight` (peso normalizado atribuido para este run).
- metadados do `provider` (זיהוי, נקודות קצה, budget de concorrencia).

תצלומי מצב של Arquive עושים את לוח התוצאות junto aos artefatos de release para que decisoes
de banimento e rollout permaneçam auditavis.

## 3. Telemetria e depuracao

### 3.1 מדדים Prometheus

O orquestrador emite as seguintes metricas דרך `iroha_telemetry`:

| מטריקה | תוויות | תיאור |
|--------|--------|--------|
| `sorafs_orchestrator_active_fetches` | `manifest_id`, `region` | Gauge de fetches orquestrados em voo. |
| `sorafs_orchestrator_fetch_duration_ms` | `manifest_id`, `region` | Histograma registrando latencia de fetch de ponta a ponta. |
| `sorafs_orchestrator_fetch_failures_total` | `manifest_id`, `region`, `reason` | Contador de falhas terminais (מחדש esgotados, sem provedores, falha de observador). |
| `sorafs_orchestrator_retries_total` | `manifest_id`, `provider`, `reason` | Contador de tentativas de rery for provedor. |
| `sorafs_orchestrator_provider_failures_total` | `manifest_id`, `provider`, `reason` | Contador de falhas de provedor na sessao que levam a desabilitacao. |
| `sorafs_orchestrator_policy_events_total` | `region`, `stage`, `outcome`, `reason` | Contagem de decisoes de politica de anonimato (cumprida vs brownout) agrupadas por estagio de rollout e motivo de fallback. |
| `sorafs_orchestrator_pq_ratio` | `region`, `stage` | היסטוריית משתתפים ממסרים PQ ללא בחירה SoraNet. |
| `sorafs_orchestrator_pq_candidate_ratio` | `region`, `stage` | היסטוריית היחסים של ממסרים PQ אין תמונת מצב לעשות לוח תוצאות. |
| `sorafs_orchestrator_pq_deficit_ratio` | `region`, `stage` | Histograma do deficit de politica (פער entre alvo e a participacao PQ real). |
| `sorafs_orchestrator_classical_ratio` | `region`, `stage` | Histograma da participacao de relays classicos usada em cada sessao. |
| `sorafs_orchestrator_classical_selected` | `region`, `stage` | Histograma de contagens de relays classicos selecionados por sessao. |

אינטגר כמו מדדים עם לוחות המחוונים של בימוי antes de habilitar knobs de
producao. O precomendado espelha o plano de observabilidade SF-6:

1. **מביא ativos** - alerta se o gauge sobe sem completions correspondentes.
2. **Razao de retries** — avisa quando contadores `retry` חריג קווי הבסיס
   היסטוריקות.
3. **Falhas de provedor** - dispara alertas no pager quando qualquer provedor
   cruza `session_failure > 0` dentro de 15 דקות.

### 3.2 יעדי יומן estruturados

O orquestrador publica eventos estruturados para targets deterministas:- `telemetry::sorafs.fetch.lifecycle` — marcadores `start` e `complete` com
  contagem de chunks, ניסיונות חוזרים e duracao סך הכל.
- `telemetry::sorafs.fetch.retry` - אירועים חוזרים (`provider`, `reason`,
  `attempts`) מדריך לטריאג' לתזונה.
- `telemetry::sorafs.fetch.provider_failure` - provedores desabilitados devido a
  שגיאות חוזרות.
- `telemetry::sorafs.fetch.error` — falhas terminais resumidas com `reason` e
  metadados opcionais do provedor.

Encaminhe esses fluxos para o pipeline de logs Norito existente para que a
resposta a incidentes tenha uma unica fonte de verdade. Eventos de ciclo de vida
לחשוף את מיסטורה PQ/classica דרך `anonymity_effective_policy`,
`anonymity_pq_ratio`, `anonymity_classical_ratio` ו-seus contadores associados,
טורננדו פשוטים אינטגררים לוחות מחוונים סמים מדדים רשומים. Durante rollouts de
GA, תיקון או nivel de log em `info` עבור אירועי ciclo de vida/נסה להשתמש מחדש
`warn` ל-Falhas Terminais.

### 3.3 קורות חיים JSON

Tanto `sorafs_cli fetch` quanto o SDK Retornam Rust resumo estruturado contendo:

- `provider_reports` com contagens de sucesso/falha e se o provedor foi
  desabilitado.
- `chunk_receipts` detalhando qual provedor atendeu cada chunk.
- מערכים `retry_stats` ו-`ineligible_providers`.

ארכיון או ארכיוו דה רזומה אאו דפוראר הוכחות בעיות - קבלות
mapeiam diretamente para os metadados de log acima.

## 4. רשימת רשימת פעולות

1. **הכן תצורה ללא CI.** בצע את `sorafs_fetch` com a configuracao
   alvo, pass `--scoreboard-out` para capturar a visao de elegibilidade e
   השווה com o release anterior. Qualquer provedor inelegivel inesperado
   interrompe a promocao.
2. **Telemetria Validar.** Garanta que o deploy exporta metricas `sorafs.fetch.*`
   estruturados estruturados antes de habilitar מביאה ריבוי אוריגמים עבור שימושים. א
   ausencia de metricas normalmente indica que a fachada do orquestrador nao foi
   invocada.
3. **עקיפות דוקומנטריות.** Ao aplicar `--deny-provider` או `--boost-provider`
   emergenciais, comite o JSON (או invocacao CLI) ללא יומן שינויים. החזרות לאחור מפתחות
   Reverter או לעקוף e capturar um novo תמונת מצב לעשות לוח תוצאות.
4. **בצעו מחדש בדיקות עשן.** Depois de modificar budgets de rery ou limites de
   provedores, refaça o fetch do fixture canonico
   (`fixtures/sorafs_manifest/ci_sample/`) ואימות לגבי קבלות של נתחים
   permanecem deterministas.

Seguir os passos acima mantém o comportamento do orquestrador reproduzivel em
השקות פור שלב e fornece a telemetria necessaria para resposta a incidentes.

### 4.1 עוקף את הפוליטיקה

Operadores podem fixar o estagio ativo de transporte/anonimato sem editar a
configuracao base definindo `policy_override.transport_policy` ה
`policy_override.anonymity_policy` em seu JSON de `orchestrator` (ou fornecendo
`--transport-policy-override=` / `--anonymity-policy-override=` ao
`sorafs_cli fetch`). Quando qualquer override esta presente, o orquestrador pula
o fallback brownout רגיל: se o nivel PQ solicitado nao puder ser satisfeito, o
אחזר את falha com `no providers` אם יש לך תקלה. הו החזרה לאחור
para o comportamento padrao e tao simples quanto limpar os campos de override.