---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/direct-mode-pack.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: pacote de modo direto
título: SoraFS کا pacote substituto de modo direto (SNNet-5a)
sidebar_label: pacote de modo direto
description: SNNet-5a منتقلی کے دوران SoraFS کو Torii/QUIC modo direto میں چلانے کے لیے ضروری config, verificações de conformidade e etapas de implementação۔
---

:::nota مستند ماخذ
:::

Circuitos SoraNet SoraFS کے لیے transporte padrão ہیں, مگر item de roteiro **SNNet-5a** ایک substituto regulamentado کا تقاضا کرتا ہے تاکہ implementação de anonimato مکمل ہونے تک operadores determinísticos de acesso de leitura برقرار رکھ سکیں۔ یہ pack botões CLI/SDK, perfis de configuração, testes de conformidade e lista de verificação de implantação کو اکٹھا کرتا ہے جو transportes de privacidade کو چھیڑے بغیر SoraFS کو direto Modo Torii/QUIC میں چلانے کے لیے درکار ہیں۔

یہ preparação de fallback em ambientes de produção regulamentados پر لاگو ہوتا ہے جب تک SNNet-5 سے SNNet-9 اپنے portas de prontidão پاس نہ کر لیں۔ نیچے والے artefatos کو معمول کے SoraFS material de implantação کے ساتھ رکھیں تاکہ operadores ضرورت پڑنے پر anônimo اور modos diretos کے درمیان سوئچ کر سکیں۔

## 1. Sinalizadores CLI e SDK

- Desativação de agendamento de relé `sorafs_cli fetch --transport-policy=direct-only ...` کرتا ہے اور Transportes Torii/QUIC impor کرتا ہے۔ Ajuda CLI اب `direct-only` کو valor aceito کے طور پر دکھاتی ہے۔
- SDKs کو `OrchestratorConfig::with_transport_policy(TransportPolicy::DirectOnly)` set کرنا چاہیے جب بھی وہ "modo direto" alternar expor کریں۔ `iroha::ClientOptions` e `iroha_android` میں ligações geradas e enum forward کرتے ہیں۔
- Chicotes de gateway (`sorafs_fetch`, ligações Python) ajudantes JSON Norito compartilhados کے ذریعے análise de alternância somente direta کر سکتے ہیں تاکہ automação کو ایک جیسا comportamento ملے۔

Runbooks voltados para parceiros میں documento de sinalização کریں اور alternadores de recursos کو variáveis ​​de ambiente کے بجائے `iroha_config` کے ذریعے wire کریں۔

## 2. Perfis de política de gateway

A configuração do orquestrador determinístico persiste کرنے کے لیے Norito JSON استعمال کریں۔ `docs/examples/sorafs_direct_mode_policy.json` میں perfil de exemplo یہ codificação کرتا ہے:

- `transport_policy: "direct_only"` — ان provedores کو rejeitar کرتا ہے جو صرف Transportes de retransmissão SoraNet anunciam کرتے ہیں۔
- `max_providers: 2` — peers diretos کو سب سے endpoints Torii/QUIC confiáveis ​​تک محدود کرتا ہے۔ Subsídios de conformidade regional کے مطابق ajustar کریں۔
- `telemetry_region: "regulated-eu"` — métricas emitidas کو rótulo کرتا ہے تاکہ execuções de fallback de painéis/auditorias کو الگ پہچان سکیں۔
- Orçamentos de repetição conservadores (`retry_budget: 2`, `provider_failure_threshold: 3`) تاکہ máscara de gateways mal configurada نہ ہوں۔

JSON کو `sorafs_cli fetch --config` (automação) یا Ligações SDK (`config_from_json`) کے ذریعے carregar کریں, پھر operadores کے سامنے política expor کریں۔ Trilhas de auditoria کے لیے saída do placar (`persist_path`) محفوظ کریں۔

Botões de aplicação do lado do gateway `docs/examples/sorafs_gateway_direct_mode.toml` میں درج ہیں۔ یہ modelo `iroha app sorafs gateway direct-mode enable` کی saída کو espelho کرتا ہے، desativação de verificações de envelope/admissão کرتا ہے, fio de padrões de limite de taxa کرتا ہے, اور tabela `direct_mode` کو nomes de host derivados de plano اور manifest digests سے populate کرتا ہے۔ Gerenciamento de configuração میں snippet commit کرنے سے پہلے valores de espaço reservado کو اپنے plano de implementação سے substituir کریں۔

## 3. Conjunto de testes de conformidadeProntidão de modo direto اب orquestrador اور caixas CLI دونوں میں cobertura شامل کرتی ہے:

- `direct_only_policy_rejects_soranet_only_providers` اس بات کو یقینی بناتا ہے کہ `TransportPolicy::DirectOnly` تیزی سے falhar کرے جب ہر anúncio candidato صرف Suporte a relés SoraNet کرتا ہو۔【crates/sorafs_orchestrator/src/lib.rs:7238】
- `direct_only_policy_prefers_direct_transports_when_available` یہ یقینی بناتا ہے کہ Torii/QUIC transports موجود ہوں تو انہی کو استعمال کیا جائے اور SoraNet relés کو sessão سے خارج رکھا جائے۔【crates/sorafs_orchestrator/src/lib.rs:7285】
- `direct_mode_policy_example_is_valid` `docs/examples/sorafs_direct_mode_policy.json` analisar کرتا ہے تاکہ auxiliares de documentos کے ساتھ alinhados رہیں۔【crates/sorafs_orchestrator/src/lib.rs:7509】【docs/examples/sorafs_direct_mode_policy.json:1】
- `fetch_command_respects_direct_transports` `sorafs_cli fetch --transport-policy=direct-only` کو mocked Torii gateway کے خلاف چلتا ہے, جو ambientes regulamentados کے لیے teste de fumaça فراہم کرتا ہے جہاں pino de transporte direto ہوتے ہیں۔【crates/sorafs_car/tests/sorafs_cli.rs:2733】
- `scripts/sorafs_direct_mode_smoke.sh` comando کو política JSON اور persistência do placar کے ساتھ wrap کرتا ہے تاکہ automação de implementação ہو سکے۔

Atualizações publicadas کرنے سے پہلے suíte focada چلائیں:

```bash
cargo test -p sorafs_orchestrator direct_only_policy
cargo test -p sorafs_car --features cli fetch_command_respects_direct_transports
```

اگر alterações upstream de compilação do espaço de trabalho کی وجہ سے falha ہو تو erro de bloqueio کو `status.md` میں registro کریں اور dependência catch up ہونے پر دوبارہ چلائیں۔

## 4. Fugas automatizadas

As regressões específicas do ambiente de cobertura CLI são importantes (como desvios de política de gateway e incompatibilidades de manifesto)۔ ایک auxiliar de fumaça dedicado `scripts/sorafs_direct_mode_smoke.sh` میں ہے جو `sorafs_cli fetch` کو política de orquestrador de modo direto, persistência de placar, e captura de resumo کے ساتھ wrap کرتا ہے۔

Exemplo de uso:

```bash
./scripts/sorafs_direct_mode_smoke.sh \
  --config docs/examples/sorafs_direct_mode_smoke.conf \
  --provider name=gw-regulated,provider-id=001122...,base-url=https://gw.example/direct/,stream-token=BASE64
```- یہ sinalizadores CLI de script e arquivos de configuração key=value دونوں کو respeito کرتا ہے (دیکھیں `docs/examples/sorafs_direct_mode_smoke.conf`)۔ چلانے سے پہلے resumo do manifesto e entradas de anúncio do provedor کو valores de produção سے preencher کریں۔
- `--policy` padrão para `docs/examples/sorafs_direct_mode_policy.json` ہے، مگر `sorafs_orchestrator::bindings::config_to_json` سے بننے والا کوئی بھی orquestrador JSON دیا جا سکتا ہے۔ Política de CLI کو `--orchestrator-config=PATH` کے ذریعے قبول کرتا ہے, جس سے execuções reproduzíveis ممکن ہوتی ہیں بغیر sinalizadores ہاتھ سے melodia کیے۔
- جب `sorafs_cli` `PATH` میں نہ ہو تو ajudante اسے `sorafs_orchestrator` caixa سے (perfil de liberação) construir کرتا ہے تاکہ fumaça é enviada encanamento de modo direto کو exercício کریں۔
- Saídas:
  - Carga útil montada (`--output`, padrão `artifacts/sorafs_direct_mode/payload.bin`).
  - Resumo de busca (`--summary`, carga útil padrão کے ساتھ) جس میں região de telemetria اور relatórios do provedor شامل ہوتے ہیں جو evidência de implementação بنتے ہیں۔
  - Política de instantâneo do placar JSON میں دیے گئے caminho پر persist ہوتا ہے (مثلاً `fetch_state/direct_mode_scoreboard.json`)۔ اسے resumo کے ساتھ alterar tickets میں arquivo کریں۔
- Automação de portão de adoção: buscar مکمل ہونے کے بعد helper `cargo xtask sorafs-adoption-check` چلاتا ہے جس میں placar persistente اور caminhos de resumo استعمال ہوتے ہیں۔ Padrão de quórum obrigatório na linha de comando پر دیے گئے provedores کی تعداد ہے؛ Exemplo de amostra e substituição `--min-providers=<n>` Resumo dos relatórios de adoção مطابق) اور `--require-telemetry` جب متعلقہ flag دیا جائے, pass کرتا ہے۔ اضافی xtask args کے لیے `XTASK_SORAFS_ADOPTION_FLAGS` استعمال کریں (مثلاً downgrade aprovado کے دوران `--allow-single-source` تاکہ gate fallback کو tolerar بھی کرے اور impor بھی)۔ صرف diagnóstico local میں `--skip-adoption-check` استعمال کریں؛ roteiro کے مطابق ہر execução regulamentada em modo direto میں pacote de relatório de adoção لازمی ہے۔

## 5. Lista de verificação de implementação1. **Congelamento de configuração:** perfil JSON de modo direto کو `iroha_config` repo میں store کریں اور hash کو change ticket میں درج کریں۔
2. **Auditoria de gateway:** modo direto پر switch کرنے سے پہلے تصدیق کریں کہ Torii endpoints TLS, capacidade TLVs e aplicação de log de auditoria کر رہے ہیں۔ Perfil de política de gateway کو operadores کے لیے publicar کریں۔
3. **Aprovação de conformidade:** manual atualizado کو revisores de conformidade/regulamentação کے ساتھ compartilhar کریں اور sobreposição de anonimato سے باہر چلانے کی captura de aprovações کریں۔
4. **Teste:** conjunto de testes de conformidade چلائیں اور staging fetch provedores Torii confiáveis ​​کے خلاف کریں۔ Saídas do placar e arquivo de resumos CLI
5. **Transferência de produção:** anúncio da janela de mudança کریں، `transport_policy` کو `direct_only` پر flip کریں (اگر `soranet-first` منتخب کیا تھا) اور monitor de painéis de modo direto کریں (latência `sorafs_fetch`, contadores de falha do provedor)۔ documento do plano de reversão کریں تاکہ SNNet-4/5/5a/5b/6a/7/8/12/13 `roadmap.md:532` میں graduado ہونے پر SoraNet-first واپس جا سکیں۔
6. **Revisão pós-alteração:** instantâneos do placar, buscar resumos e monitorar resultados کو alterar ticket کے ساتھ anexar کریں۔ `status.md` Data de vigência e atualização de anomalias کریں۔

Lista de verificação کو `sorafs_node_ops` runbook کے ساتھ رکھیں تاکہ comutação ao vivo dos operadores سے پہلے ensaio de fluxo de trabalho کر سکیں۔ جب SNNet-5 GA تک پہنچے تو telemetria de produção میں confirmação de paridade کرنے کے بعد fallback retirar کریں۔

## 6. Evidências e requisitos de adoção

Capturas em modo direto کو ابھی بھی Portão de adoção SF-6c satisfazem کرنا ہوتا ہے۔ ہر executar کے لیے placar, resumo, envelope de manifesto اور pacote de relatório de adoção کریں تاکہ `cargo xtask sorafs-adoption-check` validação de postura de fallback کر سکے۔ Campos ausentes gate کو fail کرا دیتے ہیں, اس لیے alterar tickets میں registro de metadados esperado کریں۔

- **Metadados de transporte:** `scoreboard.json` کو `transport_policy="direct_only"` declare کرنا چاہیے (اور جب downgrade force ہو تو `transport_policy_override=true` flip کریں)۔ Campos de política de anonimato کو emparelhados رکھیں چاہے وہ padrões herdam کریں, تاکہ revisores دیکھ سکیں کہ plano de anonimato encenado سے desvio ہوا یا نہیں۔
- **Contadores de provedores:** Sessões somente de gateway کو `provider_count=0` persistir کرنا چاہیے اور `gateway_provider_count=<n>` میں Provedores Torii کی تعداد preencher ہونی چاہیے۔ JSON کو ہاتھ سے editar نہ کریں: Contagens CLI/SDK derivam کرتا ہے اور portão de adoção e capturas rejeitadas کرتا ہے جن میں divisão ausente ہو۔
- **Evidência manifesta:** جب Torii gateways شامل ہوں تو assinado `--gateway-manifest-envelope <path>` (یا SDK equivalente) passe کریں تاکہ `gateway_manifest_provided` کے ساتھ `gateway_manifest_id`/`gateway_manifest_cid` `scoreboard.json` registro de registro ہوں۔ یقینی بنائیں کہ `summary.json` میں وہی `manifest_id`/`manifest_cid` موجود ہوں؛ کسی بھی فائل میں par نہ ہو تو falha na verificação de adoção ہو جائے گا۔
- **Expectativas de telemetria:** جب captura de telemetria کے ساتھ ہو تو gate کو `--require-telemetry` کے ساتھ چلائیں تاکہ métricas de relatório de adoção emitem ہونے کا ثبوت sim Ensaios aéreos میں bandeira omitir کیا جا سکتا ہے، مگر CI اور alterar ingressos کو documento de ausência کرنا چاہیے۔

Exemplo:

```bash
cargo xtask sorafs-adoption-check \
  --scoreboard fetch_state/direct_mode_scoreboard.json \
  --summary fetch_state/direct_mode_summary.json \
  --allow-single-source \
  --require-direct-only \
  --json-out artifacts/sorafs_direct_mode/adoption_report.json \
  --require-telemetry
````adoption_report.json` کو placar, resumo, envelope de manifesto e pacote de registro de fumaça کے ساتھ anexar کریں۔ یہ trabalho de adoção de CI de artefatos (`ci/check_sorafs_orchestrator_adoption.sh`) کی aplicação کو espelho کرتے ہیں اور downgrades de modo direto کو رکھتے ہیں۔