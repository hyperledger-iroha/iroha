---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/dispute-revocation-runbook.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: disputa-revocación-runbook
título: Runbook de disputas e revogacoes da SoraFS
sidebar_label: Runbook de disputas y revocaciones
descripción: Flujo de gobierno para registrar disputas de capacidade da SoraFS, coordinar revogacoes e evacuar datos de forma determinística.
---

:::nota Fuente canónica
Esta página refleja `docs/source/sorafs/dispute_revocation_runbook.md`. Mantenha ambas as copias sincronizadas ate que a documentacao Sphinx herdada seja retirada.
:::

## propuesta

Este runbook guía operadores de gobierno na abertura de disputas de capacidade da SoraFS, na coordenacao de revogacoes y em garantir que a evacuacao de dados seja concluida de forma determinística.

## 1. Avaliar o incidente

- **Condicoes de gatilho:** detección de violación de SLA (uptime/falha de PoR), déficit de replicación o divergencia de cobranca.
- **Confirmar telemetría:** capturar instantáneas de `/v1/sorafs/capacity/state` e `/v1/sorafs/capacity/telemetry` del proveedor.
- **Notificar partes interesadas:** Equipo de Almacenamiento (operacoes do provedor), Consejo de Gobernanza (orgao decisor), Observabilidad (atualizacoes de Dashboards).

## 2. Preparar o paquete de evidencias1. Colete artefatos brutos (telemetría JSON, registros de CLI, notas de auditoria).
2. Normalizar un archivo determinístico (por ejemplo, un tarball); registrarse:
   - digerir BLAKE3-256 (`evidence_digest`)
   - tipo de medio (`application/zip`, `application/jsonl`, etc.)
   - URI de hospedaje (almacenamiento de objetos, pin de SoraFS o punto final acceso a través de Torii)
3. Armazene o pacote no bucket de evidencias dagobernanca com acesso write-once.

## 3. Registrar una disputa

1. Llame a la especificación JSON para `sorafs_manifest_stub capacity dispute`:

   ```json
   {
     "provider_id_hex": "<hex>",
     "complainant_id_hex": "<hex>",
     "replication_order_id_hex": "<hex or omit>",
     "kind": "replication_shortfall",
     "submitted_epoch": 1700100000,
     "description": "Provider failed to ingest order within SLA.",
     "requested_remedy": "Slash 10% stake and suspend adverts",
     "evidence": {
       "digest_hex": "<blake3-256>",
       "media_type": "application/zip",
       "uri": "https://evidence.sora.net/bundles/<id>.zip",
       "size_bytes": 1024
     }
   }
   ```

2. Ejecute una CLI:

   ```bash
   sorafs_manifest_stub capacity dispute \
     --spec=dispute.json \
     --norito-out=dispute.to \
     --base64-out=dispute.b64 \
     --json-out=dispute_summary.json \
     --request-out=dispute_request.json \
     --authority=i105... \
     --private-key=ed25519:<key>
   ```

3. Revise `dispute_summary.json` (confirme tipo, digest das evidencias e timestamps).
4. Envie o JSON da requisicao para Torii `/v1/sorafs/capacity/dispute` vía fila de transacciones de gobierno. Captura o valor de respuesta `dispute_id_hex`; ele ancora as acoes de revogacao posteriores e os relatorios de auditoria.

## 4. Evacuação e revocação1. **Janela de graca:** notifique o provedor sobre a revogacao iminente; permit a evacuacao dos dados fixados quando a politica permitir.
2. **Gere `ProviderAdmissionRevocationV1`:**
   - Utilice `sorafs_manifest_stub provider-admission revoke` con o motivo aprobado.
   - Verifique assinaturas e o digest de revogacao.
3. **Publique a revogacao:**
   - Envie a requisicao de revogacao para Torii.
   - Garanta que os adverts do proveedor estejam bloqueados (espera-se que `torii_sorafs_admission_total{result="rejected",reason="admission_missing"}` aumente).
4. **Actualizar paneles de control:** marque o provedor como revogado, referencia o ID da disputa e vincule o pacote de evidencias.

## 5. Post-mortem y acompañamiento

- Registre a linha do tempo, a causa raiz e as acoes de remediacao no tracker de incidentes degobernanza.
- Determinar una restitucao (recortes de participación, clawbacks de taxas, reembolsos aos clientes).
- Documente os aprendices; Realice los límites de SLA o alertas de monitoreo si son necesarias.

## 6. Materiales de referencia

- `sorafs_manifest_stub capacity dispute --help`
- `docs/source/sorafs/storage_capacity_marketplace.md` (secao de disputas)
- `docs/source/sorafs/provider_admission_policy.md` (flujo de revogaçao)
- Panel de observabilidad: `SoraFS / Capacity Providers`

## Lista de verificación

- [ ] Pacote de evidencias capturado e hasheado.
- [] Carga útil de la disputa validada localmente.
- [ ] Transacao de disputa no Torii aceita.
- [ ] Revogacao ejecutada (se aprovada).
- [ ] Dashboards/runbooks atualizados.
- [ ] Archivado post-mortem junto al consejo de gobierno.