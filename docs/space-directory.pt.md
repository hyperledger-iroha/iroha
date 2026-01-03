---
lang: pt
direction: ltr
source: docs/space-directory.md
status: complete
translator: manual
source_hash: 922c3f5794c0a665150637d138f8010859d9ccfe8ea2156a1d95ea8bc7c97ac7
source_last_modified: "2025-11-12T12:40:39.146222+00:00"
translation_last_reviewed: 2025-11-14
---

<!-- Tradução para português de docs/space-directory.md (Space Directory Operator Playbook) -->

# Playbook do Operador do Space Directory

Este playbook explica como criar, publicar, auditar e rotacionar entradas do
**Space Directory** para dataspaces Nexus. Ele complementa as notas de arquitetura em
`docs/source/nexus.md` e o plano de onboarding de CBDC
(`docs/source/cbdc_lane_playbook.md`), fornecendo procedimentos práticos, fixtures e
modelos de governança.

> **Escopo.** O Space Directory funciona como o registro canônico de manifests de
> dataspace, políticas de capacidade para Universal Account ID (UAID) e do trilho de
> auditoria exigido por reguladores. Embora o contrato subjacente ainda esteja em
> desenvolvimento ativo (NX‑15), os fixtures e processos abaixo já estão prontos para
> serem integrados em ferramentas e testes de integração.

## 1. Conceitos centrais

| Termo | Descrição | Referências |
|-------|-----------|------------|
| Dataspace | Contexto de execução/lane que roda um conjunto de contratos aprovado em governança. | `docs/source/nexus.md`, `crates/iroha_data_model/src/nexus/mod.rs` |
| UAID | `UniversalAccountId` (hash blake2b‑32) usado para ancorar permissões cross‑dataspace. | `crates/iroha_data_model/src/nexus/manifest.rs` |
| Capability Manifest | `AssetPermissionManifest` descrevendo regras determinísticas de allow/deny para um par UAID/dataspace (deny vence). | Fixture `fixtures/space_directory/capability/*.manifest.json` |
| Dataspace Profile | Metadados de governança + DA publicados junto aos manifests para que operadores possam reconstruir conjuntos de validadores, whitelists de composabilidade e ganchos de auditoria. | Fixture `fixtures/space_directory/profile/cbdc_lane_profile.json` |
| SpaceDirectoryEvent | Eventos codificados em Norito emitidos quando manifests são ativados/expiram/são revogados. | `crates/iroha_data_model/src/events/data/space_directory.rs` |

## 2. Ciclo de vida do manifest

O Space Directory aplica **gestão de ciclo de vida baseada em epochs**. Cada alteração
gera um bundle de manifest assinado mais um evento:

| Evento | Disparo | Ações obrigatórias |
|--------|--------|--------------------|
| `ManifestActivated` | Novo manifest atinge `activation_epoch`. | Difundir bundle, atualizar caches, arquivar aprovação de governança. |
| `ManifestExpired` | `expiry_epoch` passa sem renovação. | Notificar operadores, limpar handles de UAID, preparar manifest de substituição. |
| `ManifestRevoked` | Decisão de deny‑wins emergencial antes do expiry. | Revogar UAID imediatamente, emitir relatório de incidente, agendar revisão de governança. |

Os assinantes devem usar `DataEventFilter::SpaceDirectory` para observar dataspaces ou
UAIDs específicos. Exemplo de filtro (Rust):

```rust
use iroha_data_model::events::data::filters::SpaceDirectoryEventFilter;

let filter = SpaceDirectoryEventFilter::new()
    .for_dataspace(11u32.into())
    .for_uaid("uaid:0f4d…ab11".parse().unwrap());
```

## 3. Workflow do operador

| Fase | Responsável(is) | Passos | Evidência |
|------|-----------------|--------|----------|
| Draft | Dono do dataspace | Clonar fixture, editar permissões/governança, rodar `cargo test -p iroha_data_model nexus::manifest`. | Diff de Git, log de testes. |
| Review | Governance WG | Validar JSON do manifest + bytes Norito, assinar log de decisão. | Ata assinada, hash do manifest (BLAKE3 + Norito `.to`). |
| Publish | Lane ops | Enviar via CLI (`iroha space-directory manifest publish`) usando payload Norito `.to` ou JSON bruto **ou** fazer POST para `/v1/space-directory/manifests` com o JSON do manifest + razão opcional, verificar a resposta do Torii e capturar `SpaceDirectoryEvent`. | Recibo CLI/Torii, log de eventos. |
| Expire | Lane ops / Governança | Rodar `iroha space-directory manifest expire` (UAID, dataspace, epoch) quando o manifest atingir o fim de vida; verificar `SpaceDirectoryEvent::ManifestExpired`, arquivar evidência de limpeza de bindings. | Saída de CLI, log de eventos. |
| Revoke | Governança + Lane ops | Rodar `iroha space-directory manifest revoke` (UAID, dataspace, epoch, reason) **ou** fazer POST em `/v1/space-directory/manifests/revoke` com o mesmo payload para o Torii, verificar `SpaceDirectoryEvent::ManifestRevoked`, atualizar o pacote de evidências. | Recibo CLI/Torii, log de eventos, anotação no ticket. |
| Monitor | SRE/Compliance | Acompanhar telemetria + logs de auditoria, configurar alertas para revogações/expiração. | Screenshot do Grafana, logs arquivados. |
| Rotate/Revoke | Lane ops + Governança | Preparar manifest de substituição (novo epoch), fazer tabletop, abrir incidente (em caso de revoke). | Ticket de rotação, post‑mortem do incidente. |

Todos os artefatos de um rollout vão para
`artifacts/nexus/<dataspace>/<timestamp>/`, com um manifest de checksums para atender
pedidos de evidência de reguladores.

## 4. Template de manifest e fixtures

Use os fixtures curados como referência canônica de esquema. O exemplo de CBDC atacadista
(`fixtures/space_directory/capability/cbdc_wholesale.manifest.json`) tem entradas de allow
e deny:

```json
{
  "version": 1,
  "uaid": "uaid:0f4d86b20839a8ddbe8a1a3d21cf1c502d49f3f79f0fa1cd88d5f24c56c0ab11",
  "dataspace": 11,
  "issued_ms": 1762723200000,
  "activation_epoch": 4097,
  "expiry_epoch": 4600,
  "entries": [
    {
      "scope": {
        "dataspace": 11,
        "program": "cbdc.transfer",
        "method": "transfer",
        "asset": "CBDC#centralbank",
        "role": "Initiator"
      },
      "effect": {
        "Allow": {
          "max_amount": "500000000",
          "window": "PerDay"
        }
      },
      "notes": "Wholesale transfer allowance (per UAID, per day)."
    },
    {
      "scope": {
        "dataspace": 11,
        "program": "cbdc.kit",
        "method": "withdraw"
      },
      "effect": {
        "Deny": {
          "reason": "Withdrawals disabled for this UAID."
        }
      },
      "notes": "Deny wins over any preceding allowance."
    }
  ]
}
```

Uma regra deny‑wins típica de onboarding de CBDC poderia ser:

```json
{
  "scope": {
    "dataspace": 11,
    "program": "cbdc.transfer",
    "method": "transfer",
    "asset": "CBDC#centralbank"
  },
  "effect": {
    "Deny": { "reason": "sanctions/watchlist match" }
  }
}
```

Quando um manifest é publicado, a combinação `uaid` + `dataspace` passa a ser a chave
principal do registro de permissões. Manifests posteriores são encadeados por
`activation_epoch`, e o registro garante que a última versão ativa preserve sempre a
semântica “deny vence”.

## 5. API Torii

### Publicar manifest

Operadores podem publicar manifests diretamente no Torii, sem depender do CLI.

```
POST /v1/space-directory/manifests
```

| Campo | Tipo | Descrição |
|-------|------|-----------|
| `authority` | `AccountId` | Conta que assina a transação de publicação. |
| `private_key` | `ExposedPrivateKey` | Chave privada em base64 que o Torii usa para assinar em nome de `authority`. |
| `manifest` | `SpaceDirectoryManifest` | Manifest completo (JSON) que será codificado em Norito internamente. |
| `reason` | `Option<String>` | Mensagem opcional para trilha de auditoria, armazenada junto aos dados de ciclo de vida. |

Exemplo de corpo JSON:

```jsonc
{
  "authority": "ops@cbdc",
  "private_key": "ed25519:CiC7…",
  "manifest": {
    "version": 1,
    "uaid": "uaid:0f4d86b20839a8ddbe8a1a3d21cf1c502d49f3f79f0fa1cd88d5f24c56c0ab11",
    "dataspace": 11,
    "issued_ms": 1762723200000,
    "activation_epoch": 4097,
    "entries": [
      {
        "scope": {
          "dataspace": 11,
          "program": "cbdc.transfer",
          "method": "transfer",
          "asset": "CBDC#centralbank"
        },
        "effect": {
          "Allow": { "max_amount": "500000000", "window": "PerDay" }
        }
      }
    ]
  },
  "reason": "CBDC onboarding wave 4"
}
```

O Torii responde com `202 Accepted` assim que a transação entra na fila. Quando o bloco
é executado, `SpaceDirectoryEvent::ManifestActivated` é emitido (sujeito a
`activation_epoch`), os bindings são reconstruídos automaticamente e o endpoint de
inventário de manifests passa a refletir o novo payload. Os controles de acesso
espelham as demais APIs de escrita do Space Directory (restrições por CIDR/token de API
e política de tarifas).

### API de revogação de manifest

Revogações de emergência não exigem mais usar o CLI: os operadores podem enviar POST
diretamente ao Torii para enfileirar a instrução canônica
`RevokeSpaceDirectoryManifest`. A conta de envio deve possuir
`CanPublishSpaceDirectoryManifest { dataspace }`, da mesma forma que no workflow via CLI.

```
POST /v1/space-directory/manifests/revoke
```

| Campo | Tipo | Descrição |
|-------|------|-----------|
| `authority` | `AccountId` | Conta que assina a transação de revogação. |
| `private_key` | `ExposedPrivateKey` | Chave privada em base64 que o Torii usa para assinar em nome de `authority`. |
| `uaid` | `String` | Literal UAID (`uaid:<hex>` ou digest hex de 64 caracteres). |
| `dataspace` | `u64` | Identificador do dataspace que hospeda o manifest. |
| `revoked_epoch` | `u64` | Epoch (inclusivo) em que a revogação deve surtir efeito. |
| `reason` | `Option<String>` | Mensagem opcional de auditoria gravada junto aos dados de ciclo de vida. |

Exemplo de corpo JSON:

```jsonc
{
  "authority": "ops@cbdc",
  "private_key": "ed25519:CiC7…",
  "uaid": "uaid:0f4d86b20839a8ddbe8a1a3d21cf1c502d49f3f79f0fa1cd88d5f24c56c0ab11",
  "dataspace": 11,
  "revoked_epoch": 9216,
  "reason": "Fraud investigation #NX-16-R05"
}
```

O Torii retorna `202 Accepted` quando a transação entra na fila. Ao executar o bloco,
`SpaceDirectoryEvent::ManifestRevoked` é emitido, `uaid_dataspaces` é reconstruído
automaticamente e tanto `/portfolio` quanto o inventário de manifests passam a relatar
o estado revogado imediatamente. As regras de CIDR e de política de tarifas são
idênticas às dos endpoints de leitura.

## 6. Template de perfil de dataspace

Perfis capturam tudo o que um novo validador precisa saber antes de se conectar. O
fixture `profile/cbdc_lane_profile.json` documenta:

- Emissor/quórum de governança (`parliament@cbdc` + ID do ticket de evidência).
- Conjunto de validadores + quórum e namespaces protegidos (`cbdc`, `gov`).
- Perfil de DA (classe A, lista de attestadores, cadência de rotação).
- ID de grupo de composabilidade e whitelist que liga UAIDs a manifests de capacidade.
- Hooks de auditoria (lista de eventos, esquema de logs, serviço PagerDuty).

Reaproveite o JSON como ponto de partida para novos dataspaces e ajuste os caminhos de
whitelist para apontar para os manifests de capacidade apropriados.

## 7. Publicação e rotação

1. **Codificar o UAID.** Derive o digest blake2b‑32 e prefixe com `uaid:`:

   ```bash
   python3 - <<'PY'
   import hashlib, binascii
   seed = bytes.fromhex("0f4d86b20839a8ddbe8a1a3d21cf1c502d49f3f79f0fa1cd88d5f24c56c0ab11")
   print("uaid:" + hashlib.blake2b(seed, digest_size=32).hexdigest())
   PY
   ```

2. **Codificar payload Norito.**

   ```bash
   cargo xtask space-directory encode \
     --json fixtures/space_directory/capability/cbdc_wholesale.manifest.json \
     --out artifacts/nexus/cbdc/manifest/cbdc_wholesale.manifest.to
   ```

   Ou execute o helper de CLI (que grava tanto o arquivo `.to` quanto um `.hash` com o
   digest BLAKE3‑256):

   ```bash
   iroha space-directory manifest encode \
     --json fixtures/space_directory/capability/cbdc_wholesale.manifest.json \
     --out artifacts/nexus/cbdc/manifest/cbdc_wholesale.manifest.to
   ```

3. **Publicar via Torii.**

   ```bash
   # Se você já codificou o manifest em Norito:
   iroha space-directory manifest publish \
     --uaid uaid:0f4d…ab11 \
     --dataspace 11 \
     --payload artifacts/nexus/cbdc/manifest/cbdc_wholesale.manifest.to
   ```

   Ou use a API HTTP descrita na seção anterior. Em ambos os casos, certifique‑se de
   arquivar:
   - o JSON original do manifest;
   - o payload Norito `.to` e o hash BLAKE3;
   - a resposta do Torii e o evento `SpaceDirectoryEvent`.

4. **Rotacionar ou revogar.**

   - Para rotação planejada: prepare um manifest de substituição com `activation_epoch`
     futuro, realize exercícios de tabletop e coordene a ativação com operadores de
     todos os dataspaces afetados.
   - Para revogação emergencial: siga o playbook de revogação (seção “Revoke” da tabela
     de workflow), inclua o identificador do incidente em `reason` e preserve logs e
     snapshots relevantes.

Seguindo este playbook, o Space Directory fornece um registro canônico, verificável e
orientado a reguladores sobre como capacidades são concedidas, limitadas e revogadas em
todos os dataspaces Nexus.

