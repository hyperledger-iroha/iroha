---
lang: es
direction: ltr
source: docs/portal/docs/soranet/puzzle-service-operations.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: operaciones-de-servicio-de-rompecabezas
título: Guía de operaciones del servicio de rompecabezas
sidebar_label: Operaciones del servicio de rompecabezas
descripción: Explotación del demonio `soranet-puzzle-service` para los tickets de admisión Argon2/ML-DSA.
---

:::nota Fuente canónica
:::

# Guía de operaciones del servicio de rompecabezas.

El demonio `soranet-puzzle-service` (`tools/soranet-puzzle-service/`) emet des
entradas bases sur Argon2 qui refleja la política `pow.puzzle.*` du
Relay et, lorsque configure, orquesta de tokens de admisión ML-DSA para les
borde del relé. Expongo cinq puntos finales HTTP:

- `GET /healthz` - sonda de vida.
- `GET /v1/puzzle/config` - devolver los parámetros PoW/puzzle effectifs issus
  del relé JSON (`handshake.descriptor_commit_hex`, `pow.*`).
- `POST /v1/puzzle/mint` - emet un ticket Argon2; un cuerpo opcional JSON
  `{ "ttl_secs": <u64>, "transcript_hash_hex": "<32-byte hex>", "signed": true }`
  demande un TTL plus tribunal (abrazadera au ventana de póliza), lie le ticket a un
  transcript hash et renvoie un ticket signe par le Relay + l'empreinte de
  firma lorsque des cles de firma sont configuraes.
- `GET /v1/token/config` - cuando `pow.token.enabled = true`, regresa la póliza
  token de admisión activo (huella digital del emisor, límites TTL/desviación del reloj, ID de retransmisión,
  et el conjunto de revocación fusionne).
- `POST /v1/token/mint` - Emet un token de admisión ML-DSA en el hash del currículum
  fourni; El cuerpo acepta `{ "transcript_hash_hex": "...", "ttl_secs": <u64>, "flags": <u8> }`.Los billetes producidos por el servicio se verifican en la prueba de integración.
`volumetric_dos_soak_preserves_puzzle_and_latency_slo`, que ejerce también
Aceleradores del relé para escenarios de volumen DoS. 【tools/soranet-relay/tests/adaptive_and_puzzle.rs:337】

## Configurar la emisión de tokens

Defina los campos JSON del relé bajo `pow.token.*` (ver
`tools/soranet-relay/deploy/config/relay.entry.json` para un ejemplo) después
Active los tokens ML-DSA. Al menos, fournissez la cle publique de l'issuer
y una lista de opciones de revocación:

```json
"pow": {
  "token": {
    "enabled": true,
    "issuer_public_key_hex": "<ML-DSA-44 public key>",
    "revocation_list_hex": [],
    "revocation_list_path": "/etc/soranet/relay/token_revocations.json"
  }
}
```

El servicio de rompecabezas reutiliza estos valores y recarga automáticamente el archivo
Norito JSON de revocación en tiempo de ejecución. Utilice la CLI `soranet-admission-token`
(`cargo run -p soranet-relay --bin soranet_admission_token`) para emettre et
inspector de tokens fuera de línea, agregar entradas `token_id_hex` al archivo de
revocación, y auditar las credenciales existentes antes de poder obtener actualizaciones
en producción.

Pase la clave secreta del emisor del servicio de rompecabezas a través de la CLI de flags:

```bash
cargo run -p soranet-puzzle-service -- \
  --relay-config /etc/soranet/relay/relay.entry.json \
  --token-secret-path /etc/soranet/relay/token_issuer_secret.hex \
  --token-revocation-file /etc/soranet/relay/token_revocations.json \
  --token-revocation-refresh-secs 60
```

`--token-secret-hex` también está disponible cuando el secreto está adquirido por un
tubería fuera de banda. Le vigilante del archivo de protección de revocación `/v1/token/config`
un diario; coordonnez les misses a jour avec la commande `soranet-admission-token revoke`
pour evitarer un estado de revocación en retardo.Definir `pow.signed_ticket_public_key_hex` en el relé JSON para anunciar
la llave pública ML-DSA-44 utilizada para verificar las firmas de tickets de PoW; `/v1/puzzle/config`
repete la cle et son empreinte BLAKE3 (`signed_ticket_public_key_fingerprint_hex`) afin
Que los clientes puedan marcar el verificador. Los billetes firmados son válidos
controla el ID de retransmisión y los enlaces de transcripción y comparte el almacén de memes
revocación; Los tickets PoW bruts de 74 octetos son válidos cuando el verificador
el boleto firmado está configurado. Pase el secreto de firma a través de `--signed-ticket-secret-hex`
o `--signed-ticket-secret-path` al iniciar el servicio de rompecabezas; el desamor
Rechace los pares de claves incoherentes si el secreto no es válido contra
`pow.signed_ticket_public_key_hex`. `POST /v1/puzzle/mint` acepta `"signed": true`
(y opcional `"transcript_hash_hex"`) para enviar un billete firmado Norito es
plus des bytes du ticket brut; Las respuestas incluyen `signed_ticket_b64` y
`signed_ticket_fingerprint_hex` para seguir las huellas dactilares de reproducción. les
Las solicitudes con `signed = true` son rechazadas si el secreto de firma no se cumple.
configurar.

## Libro de jugadas de rotación de cles1. **Colecciona el nuevo compromiso de descriptor.** Gobernanza pública del relé
   descriptor commit en el paquete de directorio. Copie la cadena hexadecimal en
   `handshake.descriptor_commit_hex` en la configuración del relé JSON participante
   con servicio de rompecabezas.
2. **Verifier les bornes de Policy Puzzle.** Confirmez que les valeurs
   `pow.puzzle.{memory_kib,time_cost,lanes}` pierde un día alineado con el plan
   de liberación. Los operadores deben cuidar la configuración determinada por Argon2
   entre relés (mínimo 4 MiB de memoria, 1 <= carriles <= 16).
3. **Preparar le redemarrage.** Recargar l'unite systemd ou le contenedor une
   fois que gobernancia anuncia el corte de rotación. El servicio no es compatible
   pas le hot-reload; un redemarage est requis pour prendre le nouveau descriptor
   comprometerse.
4. **Valider.** Emite un ticket vía `POST /v1/puzzle/mint` y confirma que
   `difficulty` e `expires_at` corresponsal de la nueva política. La relación
   remojo (`docs/source/soranet/reports/pow_resilience.md`) capturar des bornes de
   asistentes de latencia para referencia. Lorsque les tokens sont actives, lisez
   `/v1/token/config` para verificar que el emisor de huellas dactilares anuncia y le
   cuenta de revocación corresponsal aux valeurs asistentes.

## Procedimiento de desactivación de urgencia1. Defina `pow.puzzle.enabled = false` en la configuración del relé participante.
   Gardez `pow.required = true` si los tickets de respaldo de hashcash deben restaurarse
   obligatorios.
2. Opcionalmente, imponga las entradas `pow.emergency` para rechazarlas.
   Descriptores obsoletos que indican que la puerta Argon2 está fuera de línea.
3. Redemarrez a la fois le Relay et le puzzle service pour appliquer le
   cambio.
4. Vigile `soranet_handshake_pow_difficulty` para verificar que la dificultad
   tombe a la valeur hashcash atiende, y validez que `/v1/puzzle/config`
   informe `puzzle = null`.

## Monitoreo y alertas- **Latencia SLO:** Suivez `soranet_handshake_latency_seconds` y guarde el P95
  sous 300 ms. Las compensaciones de la prueba de remojo se realizan mediante donnees de calibración
  para los aceleradores de guardia.【docs/source/soranet/reports/pow_resilience.md:1】
- **Presión de cuota:** Utilisez `soranet_guard_capacity_report.py` avec les
  Relé de métricas para ajustar los tiempos de reutilización `pow.quotas` (`soranet_abuse_remote_cooldowns`,
  `soranet_handshake_throttled_remote_quota_total`).【docs/source/soranet/relay_audit_pipeline.md:68】
- **Alineación del rompecabezas:** `soranet_handshake_pow_difficulty` corresponde a la
  Difícil retorno par `/v1/puzzle/config`. Una divergencia indica una configuración
  Retransmisión obsoleta o tasa de reestructuración.
- **Preparación del token:** Alertez si `/v1/token/config` canaliza a `enabled = false`
  de maniere inattendue o si `revocation_source` informa las marcas de tiempo obsoletas.
  Los operadores no deben girar el archivo de revocación Norito a través de la CLI
  Este token se retira para garantizar la precisión del punto final.
- **Salud del servicio:** Probez `/healthz` avec la cadence de liveness habituelle et
  alerte si `/v1/puzzle/mint` envía respuestas HTTP 500 (indica una falta de coincidencia)
  des parámetros Argon2 o des echecs RNG). Los errores de acuñación de tokens
  manifiesto a través de las respuestas HTTP 4xx/5xx sur `/v1/token/mint`; traitez les
  echecs repetes como una condición de paginación.

## Registro de cumplimiento y auditoríaLes retransmisiones activadas de eventos `handshake` estructuras que incluyen las razones
acelerador y las dificultades de enfriamiento. Asegúrese de que el tubo de cumplimiento
Descripción en `docs/source/soranet/relay_audit_pipeline.md` ingere estos registros afin
Que los cambios de políticas siguen siendo auditables. Rompecabezas de la puerta
est active, archivez des echantillons de tickets mintes et le snapshot de
configuración Norito con el ticket de implementación para auditorías futuras. les
fichas de admisión mintes avant les fenetres de mantenimiento doivent etre suivis
con leurs valeurs `token_id_hex` e inserte en el archivo de revocación une
fois expira o revoca.