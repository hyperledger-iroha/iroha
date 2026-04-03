<!-- Auto-generated stub for Portuguese (pt) translation. Replace this content with the full translation. -->

---
lang: pt
direction: ltr
source: docs/source/offline_qr_operator_runbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 3628c64f36c9c27ab74fab02742a0d15fd90277feb9b04ea39be374de1399ae2
source_last_modified: "2026-02-15T17:55:05.344220+00:00"
translation_last_reviewed: 2026-04-02
translator: machine-google-reviewed
---

## Runbook do operador QR off-line

Este runbook define predefinições práticas `ecc`/dimension/fps para ruídos de câmera
ambientes ao usar o transporte QR offline.

### Predefinições recomendadas

| Meio Ambiente | Estilo | ECC | Dimensão | FPS | Tamanho do pedaço | Grupo de paridade | Notas |
| --- | --- | --- | --- | --- | --- | --- | --- |
| Iluminação controlada, curto alcance | `sakura` | `M` | `360` | `12` | `360` | `0` | Maior rendimento, redundância mínima. |
| Ruído típico de câmera móvel | `sakura-storm` | `Q` | `512` | `12` | `336` | `4` | Predefinição balanceada preferida (`~3 KB/s`) para dispositivos mistos. |
| Alto brilho, desfoque de movimento, câmeras de baixo custo | `sakura-storm` | `H` | `640` | `8` | `280` | `6` | Menor rendimento, maior resiliência de decodificação. |

### Lista de verificação de codificação/decodificação

1. Codifique com botões de transporte explícitos.
2. Valide com captura de loop de scanner antes da implementação.
3. Fixe o mesmo perfil de estilo nos auxiliares de reprodução do SDK para manter a paridade de visualização.

Exemplo:

```bash
iroha offline qr encode \
  --style sakura-storm \
  --ecc Q \
  --dimension 512 \
  --fps 12 \
  --chunk-size 336 \
  --parity-group 4 \
  --in payload.bin \
  --out out_dir
```

### Validação do loop do scanner (perfil sakura-storm 3 KB/s)

Use o mesmo perfil de transporte em todos os caminhos de captura:

-`chunk_size=336`
-`parity_group=4`
-`fps=12`
-`style=sakura-storm`

Metas de validação:-iOS: `OfflineQrStreamCameraSession` + `OfflineQrStreamScanSession`
Android: `OfflineQrStreamCameraXScanner` + `OfflineQrStream.ScanSession`
- Navegador/JS: `scanQrStreamFrames(...)` + `OfflineQrStreamScanSession`

Aceitação:

- A reconstrução completa da carga útil é bem-sucedida com um quadro de dados eliminado por grupo de paridade.
- Nenhuma incompatibilidade de soma de verificação/hash de carga útil no loop de captura normal.