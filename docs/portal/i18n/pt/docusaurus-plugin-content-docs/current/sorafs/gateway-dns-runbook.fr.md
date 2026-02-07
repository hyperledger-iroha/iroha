---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/gateway-dns-runbook.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Runbook de lançamento Gateway e DNS SoraFS

Esta cópia do portal reflete o runbook canônico em
[`docs/source/sorafs_gateway_dns_design_runbook.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs_gateway_dns_design_runbook.md).
Ela captura as principais operações do fluxo de trabalho DNS descentralizado e gateway
para que os responsáveis pela rede, operações e documentação possam repetir a pilha
d’automatisation avant le lancement 2025-03.

## Portée et livrables

- Coloque os bloqueios DNS (SF-4) e gateway (SF-5) para obter a derivação determinada
  hotéis, lançamentos do repertório de resolvedores, automação TLS/GAR e
  capture de preuves.
- Garder les entrées du kickoff (agenda, convite, acompanhamento de presença, instantâneo de
  télémétrie GAR) sincronizado com as últimas afetações dos proprietários.
- Produzir um pacote de artefatos auditáveis para os revisores de governança: notas
  de liberação do repertório de resolvedores, logs de gateway de sondas, sortie du chicote de
  conformidade e síntese Docs/DevRel.

## Funções e responsabilidades

| Fluxo de trabalho | Responsabilidades | Requisito de artefatos |
|------------|------------------|------------------|
| Rede TL (pilha DNS) | Mantenha o plano de determinação de hotéis, execute as liberações do repertório RAD, publique as entradas de telefone dos resolvedores. | `artifacts/soradns_directory/<ts>/`, diferenças de `docs/source/soradns/deterministic_hosts.md`, metadonnées RAD. |
| Líder de Automação de Operações (gateway) | Execute as brocas de automação TLS/ECH/GAR, lancer `sorafs-gateway-probe`, usando os ganchos PagerDuty. | `artifacts/sorafs_gateway_probe/<ts>/`, JSON de sonda, entradas `ops/drill-log.md`. |
| Grupo de controle de qualidade e ferramentas | Lancer `ci/check_sorafs_gateway_conformance.sh`, cura os equipamentos, arquiva os pacotes com autocertificação Norito. | `artifacts/sorafs_gateway_conformance/<ts>/`, `artifacts/sorafs_gateway_attest/<ts>/`. |
| Documentos/DevRel | Capture os minutos, leia a pré-leitura do design + anexos e publique a síntese das evidências neste portal. | Arquivos `docs/source/sorafs_gateway_dns_design_*.md` atualizados e notas de implementação. |

## Entradas e pré-requisitos

- Especificação de hotéis determinados (`docs/source/soradns/deterministic_hosts.md`) e
  andaime de atestado de resolução (`docs/source/soradns/resolver_attestation_directory.md`).
- Gateway de artefatos: operador manual, ajudantes de automação TLS/ECH,
  orientação modo direto, autocertificação de fluxo de trabalho sous `docs/source/sorafs_gateway_*`.
- Ferramental: `cargo xtask soradns-directory-release`,
  `cargo xtask sorafs-gateway-probe`, `scripts/telemetry/run_soradns_transparency_tail.sh`,
  `scripts/sorafs_gateway_self_cert.sh`, e ajudantes CI
  (`ci/check_sorafs_gateway_conformance.sh`, `ci/check_sorafs_gateway_probe.sh`).
- Segredos: clé de release GAR, credenciais ACME DNS/TLS, chave de roteamento PagerDuty,
  token auth Torii para buscas de resolvedores.

## Checklist pré-vol1. Confirme os participantes e a agenda do dia
   `docs/source/sorafs_gateway_dns_design_attendance.md` e difusor da agenda
   corrente (`docs/source/sorafs_gateway_dns_design_agenda.md`).
2. Prepare os racines d’artefacts que dizem que
   `artifacts/sorafs_gateway_dns/<YYYYMMDD>/` e outros
   `artifacts/soradns_directory/<YYYYMMDD>/`.
3. Rafraîchir les fixtures (manifests GAR, preuves RAD, bundles de conformité gateway) et
   certifique-se de que o estado `git submodule` corresponda à última etiqueta de ensaio.
4. Verifique os segredos (clé de release Ed25519, arquivo de conta ACME, token PagerDuty)
   e sua correspondência com as somas de verificação do cofre.
5. Faça um teste de fumaça de cabos de telefonia (endpoint Pushgateway, placa GAR Grafana)
   antes da broca.

## Étapes de ensaio de automação

### Carta de hotéis determinada e liberação do repertório RAD

1. Execute o auxiliar de derivação para determinar hotéis no conjunto de manifestos
   propôs e confirmou a ausência de deriva por relacionamento com
   `docs/source/soradns/deterministic_hosts.md`.
2. Gere um pacote de repertório de resolvedores:

```bash
cargo xtask soradns-directory-release \
  --rad-dir artifacts/soradns/rad_candidates \
  --output-root artifacts/soradns_directory \
  --release-key-path secrets/soradns/release.key \
  --car-cid bafybeigdyrdnsmanifest... \
  --note "dns-kickoff-20250303"
```

3. Registre o ID do repertório, o SHA-256 e os caminhos de saída impressos em
   `docs/source/sorafs_gateway_dns_design_gar_telemetry.md` e nos minutos do início do jogo.

### Captura de DNS de telefonia

- Tailer les logs de transparência dos resolvedores pendentes ≥10 minutos com
  `scripts/telemetry/run_soradns_transparency_tail.sh --mode staging`.
- Exporte as métricas Pushgateway e arquive os instantâneos NDJSON no local onde você está.
  repertório do ID de execução.

### Exercícios de automação do gateway

1. Execute a sonda TLS/ECH:

```bash
cargo xtask sorafs-gateway-probe \
  --config configs/sorafs_gateway/probe.staging.toml \
  --output artifacts/sorafs_gateway_probe/<run-id>.json
```

2. Lance o chicote de conformidade (`ci/check_sorafs_gateway_conformance.sh`) e
   l’helper self-cert (`scripts/sorafs_gateway_self_cert.sh`) para rafraîchir o pacote
   atestados Norito.
3. Capture eventos PagerDuty/Webhook para provar que a cadeia de automação
   função de luta em luta.

### Embalagem de pré-venda

- Apresentado `ops/drill-log.md` hoje com carimbos de data e hora, participantes e hashes de teste.
- Armazene os artefatos nos repertórios de ID de execução e publique uma síntese executiva
  nos minutos Docs/DevRel.
- Lier le bundle de preuves dans le ticket de gouvernance antes da revista de kickoff.

## Animação de sessão e remise des preuves- **Linha do tempo do moderador:**
  - T-24 h — Gestão do Programa poste le rappel + snapshot agenda/présence dans `#nexus-steering`.
  - T-2 h — Networking TL rafraîchit o snapshot telemétrie GAR e envia os deltas em `docs/source/sorafs_gateway_dns_design_gar_telemetry.md`.
  - T-15 m — Ops Automation verifica a preparação das sondas e escreve o ID de execução ativo em `artifacts/sorafs_gateway_dns/current`.
  - Pendant l’appel — O moderador compartilha este runbook e atribui um escriba diretamente; Docs/DevRel captura ações on-line.
- **Modelo de minutos:** Copie o esquema de
  `docs/source/sorafs_gateway_dns_design_minutes.md` (également miroir dans le bundle
  do portal) e confirme uma instância de resposta por sessão. Incluir a lista de
  participantes, decisões, ações, hashes de preuves et risques ouverts.
- **Upload des preuves :** Zipper le répertoire `runbook_bundle/` du rehearsal,
  junte o PDF dos minutos gerados, registre os hashes SHA-256 nos minutos +
  agenda, depois pingue o pseudônimo dos revisores de governança uma vez que os uploads
  disponível em `s3://sora-governance/sorafs/gateway_dns/<date>/`.

## Snapshot des preuves (início em março de 2025)

Les derniers artefacts ensaio/referências ao vivo no roteiro e nos minutos
estão armazenados no balde `s3://sora-governance/sorafs/gateway_dns/`. Os hashes
ci-dessous reflètent le manifest canonique (`artifacts/sorafs_gateway_dns/<run-id>/runbook_bundle/evidence_manifest_*.json`).

- **Teste — 02/03/2025 (`artifacts/sorafs_gateway_dns/20250302/`)**
  - Pacote tarball: `b13571d2822c51f771d0e471f4f66d088a78ed6c1a5adb0d4b020b04dd9a5ae0`
  - PDF da ata: `cac89ee3e6e4fa0adb9694941c7c42ffddb513f949cf1b0c9f375e14507f4f18`
- **Atelier ao vivo — 03/03/2025 (`artifacts/sorafs_gateway_dns/20250303/runbook_bundle/`)**
  -`bc83e6a014c2d223433f04ddc3c588bfeff33ee5cdcb15aad6527efeba582a1c  minutes_20250303.md`
  -`030a98fb3e3a52dbb0fcf25a6ea4365b11d9487707bb6700cb632710f7c082e4  gar_snapshot_20250303.json`
  -`5ac17e684976d6862628672627f229f7719da74235aa0a5f0ce994dad34cb3c4  sorafs_gateway_dns_design_metrics_20250303.prom`
  -`5c6163d0ae9032c2d52ca2ecca4037dfaddcc503eb56239b53c5e9c4000997cf  probe_20250303.json`
  -`87f6341896bfb830966a4a5d0fc9158fabcc135ba16ef0d53882e558de77ba49  probe_20250303_webhook.jsonl`
  -`9b968b0bf4ca654d466ec2be5291936f1441908354e9d2da4d0a52f1568bbe03  probe.staging.toml`
  - _(Upload attente : `gateway_dns_minutes_20250303.pdf` — Docs/DevRel adiciona o SHA-256 para que o PDF seja exibido no pacote.)_

## Material conectado

- [Playbook d'opérations gateway](./operations-playbook.md)
- [Plano de observabilidade SoraFS](./observability-plan.md)
- [Rastreador DNS descentralizado e gateway](https://github.com/hyperledger-iroha/iroha/blob/master/roadmap.md#core-workstreams)