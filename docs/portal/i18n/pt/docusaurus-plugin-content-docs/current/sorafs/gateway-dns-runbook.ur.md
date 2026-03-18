---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/gateway-dns-runbook.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# SoraFS گیٹ وے اور DNS کک آف رن بک

یہ پورٹل کاپی کینونیکل رن بک کو منعکس کرتی ہے جو
[`docs/source/sorafs_gateway_dns_design_runbook.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs_gateway_dns_design_runbook.md) میں ہے۔
یہ DNS e gateway descentralizado
نیٹ ورکنگ, آپس، اور ڈاکیومنٹیشن لیڈز 2025-03 کے کک آف سے پہلے آٹومیشن اسٹیک کی
مشقیق کر سکیں۔

## اسکوپ اور ڈلیوریبلز

- Marcos de DNS (SF-4) ou gateway (SF-5) کو جوڑنا, جس میں derivação determinística de host,
  liberações de diretório resolvedor, automação TLS/GAR, e captura de evidências
- کک آف ان پٹس (agenda, convite, rastreador de presença, instantâneo de telemetria GAR) کو
  تازہ ترین atribuições de proprietário کے ساتھ ہم آہنگ رکھنا۔
- گورننس ریویورز کے لیے قابلِ آڈٹ pacote de artefatos تیار کرنا: diretório resolvedor
  notas de versão, logs de sonda de gateway, saída de chicote de conformidade, e resumo do Docs/DevRel۔

## رولز اور ذمہ داریاں

| ورک اسٹریم | ذمہ داریاں | artefactos industriais |
|------------|------------|------------------|
| TL de rede (pilha DNS) | plano de host determinístico برقرار رکھنا، liberações de diretório RAD چلانا، entradas de telemetria do resolvedor شائع کرنا۔ | `artifacts/soradns_directory/<ts>/`, `docs/source/soradns/deterministic_hosts.md` کے diffs, e metadados RAD۔ |
| Líder de Automação de Operações (gateway) | Brocas de automação TLS/ECH/GAR چلانا, `sorafs-gateway-probe` چلانا, ganchos PagerDuty اپڈیٹ کرنا۔ | `artifacts/sorafs_gateway_probe/<ts>/`, sonda JSON, entradas `ops/drill-log.md`۔ |
| Grupo de controle de qualidade e ferramentas | `ci/check_sorafs_gateway_conformance.sh` چلانا، fixtures کیوریٹ کرنا, Norito pacotes de autocertificação آرکائیو کرنا۔ | `artifacts/sorafs_gateway_conformance/<ts>/`, `artifacts/sorafs_gateway_attest/<ts>/`. |
| Documentos/DevRel | minutos ریکارڈ کرنا، design pré-leitura + apêndices اپڈیٹ کرنا، اور اسی پورٹل میں resumo de evidências شائع کرنا۔ | Notas de lançamento `docs/source/sorafs_gateway_dns_design_*.md` فائلز اور |

## ان پٹس اور پری ریکوائرمنٹس

- especificação de host determinística (`docs/source/soradns/deterministic_hosts.md`) e resolvedor
  andaime de atestação (`docs/source/soradns/resolver_attestation_directory.md`).
- artefatos de gateway: manual do operador, auxiliares de automação TLS/ECH, orientação em modo direto,
  O fluxo de trabalho de autocertificação é `docs/source/sorafs_gateway_*`.
- Ferramental: `cargo xtask soradns-directory-release`,
  `cargo xtask sorafs-gateway-probe`, `scripts/telemetry/run_soradns_transparency_tail.sh`,
  `scripts/sorafs_gateway_self_cert.sh`, e ajudantes de CI
  (`ci/check_sorafs_gateway_conformance.sh`, `ci/check_sorafs_gateway_probe.sh`).
- Segredos: chave de liberação GAR, credenciais DNS/TLS ACME, chave de roteamento PagerDuty,
  O resolvedor busca o token de autenticação Torii۔

## پری فلائٹ چیک لسٹ

1. `docs/source/sorafs_gateway_dns_design_attendance.md` اپڈیٹ کر کے شرکاء اور agenda
   کنفرم کریں اور موجودہ agenda (`docs/source/sorafs_gateway_dns_design_agenda.md`) شیئر کریں۔
2. `artifacts/sorafs_gateway_dns/<YYYYMMDD>/` Cabo
   `artifacts/soradns_directory/<YYYYMMDD>/` جیسے raízes de artefato تیار کریں۔
3. fixtures (manifestos GAR, provas RAD, pacotes de conformidade de gateway) ریفریش کریں اور
   یقینی بنائیں کہ `git submodule` کی حالت تازہ ترین tag de ensaio سے میچ کرتی ہے۔
4. segredos (chave de liberação Ed25519, arquivo de conta ACME, token PagerDuty) ویریفائی کریں اور
   somas de verificação do cofre
5. perfurar alvos de telemetria (endpoint Pushgateway, placa GAR Grafana) e teste de fumaça

## آٹومیشن etapas do ensaio

### mapa de host determinístico e liberação do diretório RAD1. تجویز کردہ manifestos سیٹ کے خلاف auxiliar de derivação de host determinístico چلائیں اور
   تصدیق کریں کہ `docs/source/soradns/deterministic_hosts.md` کے مقابلے میں کوئی drift نہیں۔
2. pacote de diretório resolvedor تیار کریں:

```bash
cargo xtask soradns-directory-release \
  --rad-dir artifacts/soradns/rad_candidates \
  --output-root artifacts/soradns_directory \
  --release-key-path secrets/soradns/release.key \
  --car-cid bafybeigdyrdnsmanifest... \
  --note "dns-kickoff-20250303"
```

3. Defina o ID do diretório, SHA-256, e os caminhos de saída e
   `docs/source/sorafs_gateway_dns_design_gar_telemetry.md` Os minutos iniciais do jogo

### Captura de telemetria DNS

- logs de transparência do resolvedor کو ≥10 منٹ تک tail کریں:
  `scripts/telemetry/run_soradns_transparency_tail.sh --mode staging`.
- Exportação de métricas Pushgateway کریں اور instantâneos NDJSON کو diretório de ID de execução کے ساتھ آرکائیو کریں۔

### Exercícios de automação de gateway

1. Teste de sonda TLS/ECH:

```bash
cargo xtask sorafs-gateway-probe \
  --config configs/sorafs_gateway/probe.staging.toml \
  --output artifacts/sorafs_gateway_probe/<run-id>.json
```

2. Chicote de conformidade (`ci/check_sorafs_gateway_conformance.sh`) e auxiliar de autocertificação
   (`scripts/sorafs_gateway_self_cert.sh`) چلائیں تاکہ Pacote de atestado Norito ریفریش ہو۔
3. Captura de eventos PagerDuty/Webhook کریں تاکہ آٹومیشن پاتھ ponta a ponta

### Embalagem de evidências

- `ops/drill-log.md` میں timestamps, participantes e hashes de sonda اپڈیٹ کریں۔
- executar diretórios de ID میں artefatos محفوظ کریں اور Docs/DevRel minutos میں resumo executivo شائع کریں۔
- revisão inicial سے پہلے ticket de governança میں pacote de evidências لنک کریں۔

## سیشن فیسلیٹیشن اور entrega de evidências

- **Cronograma do moderador:**
  - T-24 h — Gerenciamento de programa `#nexus-steering` میں lembrete + instantâneo de agenda/presença پوسٹ کرے۔
  - T-2 h — Instantâneo de telemetria TL GAR de rede
  - T-15 m — Prontidão da sonda de Automação de Operações
  - کال کے دوران — Moderador یہ رن بک شیئر کرے اور escriba ao vivo atribuir کرے؛ Itens de ação inline do Docs/DevRel ریکارڈ کرے۔
- **Modelo de minuta:**
  `docs/source/sorafs_gateway_dns_design_minutes.md` سے esqueleto کاپی کریں (pacote de portal میں بھی ہے) اور ہر سیشن کے لیے ایک مکمل commit de instância کریں۔ lista de participantes, decisões, itens de ação, hashes de evidências e riscos pendentes
- **Upload de evidências:** ensaio کے Diretório `runbook_bundle/` کو zip کریں، minutos renderizados PDF anexar کریں، minutos + agenda میں hashes SHA-256 لکھیں, اور uploads کے بعد alias do revisor de governança کو ping کریں جب فائلز `s3://sora-governance/sorafs/gateway_dns/<date>/` میں پہنچ جائیں۔

## Instantâneo de evidências (início de março de 2025)

roteiro اور minutos میں حوالہ دیے گئے تازہ ترین ensaio/artefatos ao vivo
`s3://sora-governance/sorafs/gateway_dns/` balde میں ہیں۔ نیچے دیے گئے hashes
manifesto canônico (`artifacts/sorafs_gateway_dns/<run-id>/runbook_bundle/evidence_manifest_*.json`) کو refletir کرتے ہیں۔

- **Teste — 02/03/2025 (`artifacts/sorafs_gateway_dns/20250302/`)**
  - Pacote tarball: `b13571d2822c51f771d0e471f4f66d088a78ed6c1a5adb0d4b020b04dd9a5ae0`
  - Minutos PDF: `cac89ee3e6e4fa0adb9694941c7c42ffddb513f949cf1b0c9f375e14507f4f18`
- **Workshop ao vivo — 03/03/2025 (`artifacts/sorafs_gateway_dns/20250303/runbook_bundle/`)**
  -`bc83e6a014c2d223433f04ddc3c588bfeff33ee5cdcb15aad6527efeba582a1c  minutes_20250303.md`
  -`030a98fb3e3a52dbb0fcf25a6ea4365b11d9487707bb6700cb632710f7c082e4  gar_snapshot_20250303.json`
  -`5ac17e684976d6862628672627f229f7719da74235aa0a5f0ce994dad34cb3c4  sorafs_gateway_dns_design_metrics_20250303.prom`
  -`5c6163d0ae9032c2d52ca2ecca4037dfaddcc503eb56239b53c5e9c4000997cf  probe_20250303.json`
  -`87f6341896bfb830966a4a5d0fc9158fabcc135ba16ef0d53882e558de77ba49  probe_20250303_webhook.jsonl`
  -`9b968b0bf4ca654d466ec2be5291936f1441908354e9d2da4d0a52f1568bbe03  probe.staging.toml`
  - _(Upload pendente: `gateway_dns_minutes_20250303.pdf` — Docs/DevRel PDF آنے پر SHA-256 شامل کرے گا۔)_

## Material relacionado

- [Manual de operações de gateway](./operations-playbook.md)
- [Plano de observabilidade SoraFS](./observability-plan.md)
- [DNS descentralizado e rastreador de gateway] (https://github.com/hyperledger-iroha/iroha/blob/master/roadmap.md#core-workstreams)