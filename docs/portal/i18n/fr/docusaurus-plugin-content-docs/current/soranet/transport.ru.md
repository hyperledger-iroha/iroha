---
lang: fr
direction: ltr
source: docs/portal/docs/soranet/transport.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

---
id: transport
title: Обзор транспорта SoraNet
sidebar_label: Обзор транспорта
description: Handshake, ротация salts и руководство по возможностям для анонимного оверлея SoraNet.
---

:::note Канонический источник
Эта страница зеркалирует спецификацию транспорта SNNet-1 в `docs/source/soranet/spec.md`. Держите обе копии синхронизированными, пока набор устаревшей документации не будет выведен из эксплуатации.
:::

SoraNet - это анонимный оверлей, который поддерживает range fetches SoraFS, Norito RPC streaming и будущие Nexus data lanes. Программа транспорта (roadmap items **SNNet-1**, **SNNet-1a** и **SNNet-1b**) определила детерминированный handshake, переговоры по post-quantum (PQ) возможностям и план ротации salts, чтобы каждый relay, client и gateway наблюдал одинаковую security posture.

## Цели и модель сети

- Строить треххоповые circuits (entry -> middle -> exit) поверх QUIC v1, чтобы злоупотребляющие peers никогда не достигали Torii напрямую.
- Наложить handshake Noise XX *hybrid* (Curve25519 + Kyber768) поверх QUIC/TLS, чтобы привязать session keys к TLS transcript.
- Требовать capability TLVs, которые объявляют поддержку PQ KEM/подписи, роль relay и версию протокола; GREASE неизвестные типы, чтобы будущие расширения оставались развертываемыми.
- Ежедневно ротировать blinded-content salts и фиксировать guard relays на 30 дней, чтобы churn в directory не мог деанонимизировать клиентов.
- Держать cells фиксированными на 1024 B, вводить padding/dummy cells и экспортировать детерминированную telemetry, чтобы быстро ловить попытки downgrade.

## Pipeline handshake (SNNet-1a)

1. **QUIC/TLS envelope** - clients подключаются к relays по QUIC v1 и завершают handshake TLS 1.3, используя сертификаты Ed25519, подписанные governance CA. TLS exporter (`tls-exporter("soranet handshake", 64)`) засевает слой Noise, чтобы transcripts были неразделимы.
2. **Noise XX hybrid** - строка протокола `Noise_XXhybrid_25519+Kyber768_AESGCM_SHA256` с prologue = TLS exporter. Поток сообщений:

   ```
   -> e, s
   <- e, ee, se, s, pq_ciphertext
   -> ee, se, pq_ciphertext
   ```

   Выход Curve25519 DH и обе Kyber encapsulations смешиваются в финальных симметричных ключах. Провал переговоров по PQ материалу полностью обрывает handshake - fallback только на классике запрещен.

3. **Puzzle tickets и tokens** - relays могут требовать Argon2id proof-of-work ticket до `ClientHello`. Tickets - это length-prefixed frames, которые несут захешированное решение Argon2 и истекают в пределах политики:

   ```norito
   struct PowTicketV1 {
       version: u8,
       difficulty: u8,
       expires_at: u64,
       client_nonce: [u8; 32],
       solution: [u8; 32],
   }
   ```

   Admission tokens с префиксом `SNTK` обходят puzzles, когда подпись ML-DSA-44 от эмитента валидируется относительно активной политики и списка отзыва.

4. **Capability TLV exchange** - финальный Noise payload переносит capability TLVs, описанные ниже. Clients прерывают соединение, если любая обязательная capability (PQ KEM/подпись, роль или версия) отсутствует или не совпадает с записью directory.

5. **Transcript logging** - relays логируют hash transcript, TLS fingerprint и TLV contents, чтобы подпитывать downgrade detectors и compliance pipelines.

## Capability TLVs (SNNet-1c)

Capabilities используют фиксированную TLV-оболочку `typ/length/value`:

```norito
struct CapabilityTLV {
    typ: u16,
    length: u16,
    value: Vec<u8>,
}
```

Типы, определенные сегодня:

- `snnet.pqkem` - уровень Kyber (`kyber768` для текущего rollout).
- `snnet.pqsig` - suite PQ подписей (`ml-dsa-44`).
- `snnet.role` - роль relay (`entry`, `middle`, `exit`, `gateway`).
- `snnet.version` - идентификатор версии протокола.
- `snnet.grease` - случайные filler элементы в зарезервированном диапазоне, чтобы обеспечить терпимость к будущим TLVs.

Clients поддерживают allow-list обязательных TLVs и валят handshake при пропуске или downgrade. Relays публикуют тот же set в directory microdescriptor, чтобы валидация была детерминированной.

## Ротация salts и CID blinding (SNNet-1b)

- Governance публикует запись `SaltRotationScheduleV1` со значениями `(epoch_id, salt, valid_after, valid_until)`. Relays и gateways получают подписанный график у directory publisher.
- Clients применяют новый salt в `valid_after`, сохраняют предыдущий salt на 12 часов grace периода и держат историю из 7 epochs для переносимости задержанных обновлений.
- Канонические blinded identifiers используют:

  ```
  cache_key = BLAKE3("soranet.blinding.canonical.v1" ∥ salt ∥ cid)
  ```

  Gateways принимают blinded key через `Sora-Req-Blinded-CID` и эхом возвращают его в `Sora-Content-CID`. Circuit/request blinding (`CircuitBlindingKey::derive`) находится в `iroha_crypto::soranet::blinding`.
- Если relay пропускает epoch, он останавливает новые circuits до загрузки графика и выпускает `SaltRecoveryEventV1`, который on-call dashboards трактуют как paging сигнал.

## Directory data и guard policy

- Microdescriptors несут identity relay (Ed25519 + ML-DSA-65), PQ keys, capability TLVs, region tags, guard eligibility и текущий advertised salt epoch.
- Clients фиксируют guard sets на 30 дней и сохраняют `guard_set` caches вместе с подписанным directory snapshot. CLI и SDK wrappers выводят cache fingerprint, чтобы evidence rollout можно было прикреплять к change review.

## Telemetry и rollout checklist

- Метрики для экспорта перед production:
  - `soranet_handshake_success_total{role}`
  - `soranet_handshake_failure_total{reason}`
  - `soranet_handshake_latency_seconds`
  - `soranet_capability_mismatch_total`
  - `soranet_salt_rotation_lag_seconds`
- Пороговые значения алертов живут рядом с матрицей SLO для SOP ротации salts (`docs/source/soranet_salt_plan.md#slo--alert-matrix`) и должны быть отражены в Alertmanager до продвижения сети.
- Alerts: >5% failure rate за 5 минут, salt lag >15 минут или capability mismatches в production.
- Шаги rollout:
  1. Прогнать interoperability tests relay/client на staging с включенными hybrid handshake и PQ stack.
  2. Отрепетировать SOP ротации salts (`docs/source/soranet_salt_plan.md`) и приложить drill artefacts к change record.
  3. Включить capability negotiation в directory, затем раскатить на entry relays, middle relays, exit relays и в конце clients.
  4. Зафиксировать guard cache fingerprints, salt schedules и telemetry dashboards для каждой фазы; приложить evidence bundle к `status.md`.

Следование этому checklist позволяет командам операторов, clients и SDK внедрять SoraNet transports синхронно и при этом выполнять требования детерминизма и аудита, отраженные в SNNet roadmap.
