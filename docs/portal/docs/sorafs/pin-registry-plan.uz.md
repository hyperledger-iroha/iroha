---
lang: uz
direction: ltr
source: docs/portal/docs/sorafs/pin-registry-plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 7cc63e7549adebfe3ab539eca608e2fc88830361b3fe53b165491e36ecb83177
source_last_modified: "2026-01-22T14:35:36.748626+00:00"
translation_last_reviewed: 2026-02-07
id: pin-registry-plan
title: SoraFS Pin Registry Implementation Plan
sidebar_label: Pin Registry Plan
description: SF-4 implementation plan covering registry state machine, Torii facade, tooling, and observability.
translator: machine-google-reviewed
---

::: Eslatma Kanonik manba
:::

# SoraFS Pin registrini amalga oshirish rejasi (SF-4)

SF-4 Pin Registry shartnomasini va saqlaydigan yordamchi xizmatlarni taqdim etadi
manifest majburiyatlari, pin siyosatlarini amalga oshirish va API'larni Torii, shlyuzlar,
va orkestrlar. Ushbu hujjat tasdiqlash rejasini beton bilan kengaytiradi
zanjirdagi mantiqni, xost tomonidagi xizmatlarni, moslamalarni qamrab oluvchi amalga oshirish vazifalari,
va operatsion talablar.

## Qo'llash doirasi

1. **Registr holati mashinasi**: manifestlar, taxalluslar uchun Norito tomonidan belgilangan yozuvlar,
   voris zanjirlari, ushlab turish davrlari va boshqaruv metama'lumotlari.
2. **Shartnomani amalga oshirish**: pinning hayot aylanishi uchun deterministik CRUD operatsiyalari
   (`ReplicationOrder`, `Precommit`, `Completion`, ko'chirish).
3. **Xizmat fasad**: Torii registr tomonidan quvvatlangan gRPC/REST oxirgi nuqtalari
   va SDK'lar, jumladan, sahifalash va attestatsiyadan foydalanadi.
4. **Asboblar va jihozlar**: CLI yordamchilari, sinov vektorlari va saqlanishi kerak bo'lgan hujjatlar
   manifestlar, taxalluslar va boshqaruv konvertlari sinxronlashtiriladi.
5. **Telemetriya va operatsiyalar**: registr salomatligi uchun ko'rsatkichlar, ogohlantirishlar va ish kitoblari.

## Ma'lumotlar modeli

### Asosiy yozuvlar (Norito)

| Struktura | Tavsif | Maydonlar |
|--------|-------------|--------|
| `PinManifestRecord` | Chain-authoritative manifest lifecycle entry. The envelope digest and exact 36-byte CIDv1/dag-cbor/BLAKE3-256 content root are distinct commitments. | `digest`, `root_cid`, `chunker`, `chunk_digest_sha3_256`, `por_root`, `content_length`, `policy`, `submitted_by`, `submitted_epoch`, `alias`, `successor_of`, `metadata`, `status`, `retirement_reason`, `council_envelope_digest`, `pin_fee_payment`. |
| `PinManifestFinalizedRecordV1` | Immutable read result binding one native manifest record to the finalized block used for the query. | `finalized_cursor` (`height`, `block_hash`), `manifest`. |
| `AliasBindingV1` | Xaritalar taxallus -> manifest CID. | `alias`, `manifest_cid`, `bound_at`, `expiry_epoch`. |
| `ReplicationOrderV1` | Manifestni pin qilish bo'yicha provayderlar uchun ko'rsatma. | `order_id`, `manifest_cid`, `providers`, `redundancy`, `deadline`, `policy_hash`. |
| `ReplicationReceiptV1` | Provayderni tasdiqlash. | `order_id`, `provider_id`, `status`, `timestamp`, `por_sample_digest`. |
| `ManifestPolicyV1` | Boshqaruv siyosatining surati. | `min_replicas`, `max_retention_epochs`, `allowed_profiles`, `pin_fee_basis_points`. |

Implementation reference: the authoritative manifest lifecycle and finalized
read schemas live in `crates/iroha_data_model/src/sorafs/pin_registry.rs`.
Supporting alias, replication, and policy envelopes live in
`crates/sorafs_manifest/src/pin_registry.rs`. Consensus admission derives and
validates the stored commitments; Torii and operator tooling consume the exact
native finalized record rather than maintaining a second pin-record format.

Status:
- The native `PinManifestRecord` and `PinManifestFinalizedRecordV1` are the V1
  manifest-registry surface used by core, Torii, fixtures, and reference
  validators.
- Rust code generation uses Norito derives; SDK parity follows the normal guard
  lanes whenever the native schema changes.
- Architecture, manifest-pipeline, CLI, OpenAPI, status, and roadmap documents
  describe the shared validation path and endpoint behavior.

## Contract Implementation

| Task | Owner(s) | Notes |
|------|----------|-------|
| Registry storage and smart-contract state. | Core Infra / Smart Contract Team | Implemented in Iroha world state (`pin_manifests`, `manifest_aliases`, `replication_orders`) with deterministic Norito payload hashing and integer-only policy arithmetic. |
| Entry points: `RegisterPinManifest`, `ApprovePinManifest`, `RetirePinManifest`, `BindManifestAlias`, `IssueReplicationOrder`, `CompleteReplicationOrder`, `ExpireReplicationOrder`. | Core Infra | Registration carries the complete canonical manifest, resource-bounds and validates it in consensus, and derives all stored commitments. Core execution also validates aliases, council envelopes, governance permissions, canonical replication payloads, completion, and deadline-bound expiration. |
| State transitions: enforce succession (manifest A -> B), retention epochs, alias uniqueness, and replication status changes. | Governance Council / Core Infra | `ensure_successor_chain` enforces approved, non-retired, acyclic multi-hop lineage; alias uniqueness, retention, and replication issue/complete bookkeeping are covered by unit tests. |
| Governed parameters: load `ManifestPolicyV1` from config/governance state. | Governance Council | Runtime config maps pin-policy constraints into the shared validator. Live policy-change ceremonies are rollout governance evidence, not missing local contract code. |
| Registry telemetry and audit surface. | Observability | Torii exports registry metrics and attested REST snapshots. Additional signed event archives can be layered over those snapshots if governance requires them. |

Coverage:
- Unit tests cover registration, approval, retirement, alias binding, replication
  order issue/complete, permissions, duplicate rejection, and side-effect-free
  failure paths.
- Successor tests cover self references, unknown/pending/retired predecessors,
  cycle closure, and malformed existing predecessor cycles.
- `ci/check_sorafs_fixtures.sh` regenerates chunker, provider-admission, and pin
  registry fixtures and runs the parity checks that keep the canonical schema
  surface stable.

## Service Facade (Torii/SDK Integration)

| Component | Task | Owner(s) |
|-----------|------|----------|
| Torii Service | Ships `/v1/sorafs/pin`, `/v1/sorafs/pin/{digest_hex}`, `/v1/sorafs/aliases`, and `/v1/sorafs/replication`. The manifest-detail route returns exact native `PinManifestFinalizedRecordV1` JSON and accepts only the optional paired expected finalized height/hash precondition; pagination and filters remain on list routes. | Networking TL / Core Infra |
| Finality binding | Listing responses retain their listing attestation. A manifest-detail response carries the native `finalized_cursor` beside the authoritative `PinManifestRecord`; a stale requested cursor fails with HTTP 409. | Core Infra |
| CLI | `iroha app sorafs pin register`, `pin list`, `pin show`, `alias list`, and `replication list` wrap the REST and ISI surfaces for operator audits. | Tooling WG |
| SDK | Rust request builders and the JavaScript, Python, Swift, and C# guard lanes mirror the manifest payload and pin-register validation surface. | SDK Teams |

Operations:
- List endpoints use attested snapshots, deterministic pagination, and the cache
  behavior documented in the alias policy where alias proofs are involved.
- `GET /v1/sorafs/pin/{digest_hex}` returns only `finalized_cursor` and the
  native `manifest`. The retired `limit`, attestation, embedded alias/order
  arrays, counts, and truncation fields are absent; callers use
  `/v1/sorafs/aliases` and `/v1/sorafs/replication` for bounded list queries.
- Mutating operations go through ISI/governance permissions; REST handling keeps
  the same Torii auth and resource-guard model as the surrounding SoraFS APIs.

## Armatura va CI

- Armatura katalogi: `crates/iroha_core/tests/fixtures/sorafs_pin_registry/` imzolangan manifest/taxallus/buyurtma oniy tasvirlarini `cargo run -p iroha_core --example gen_pin_snapshot` tomonidan qayta tiklangan holda saqlaydi.
- CI qadami: `ci/check_sorafs_fixtures.sh` suratni qayta tiklaydi va agar farqlar paydo bo'lsa, CI moslamalarini bir xilda ushlab turgan holda ishlamay qoladi.
- Integratsiya testlari (`crates/iroha_core/tests/pin_registry.rs`) baxtli yo'lni qo'llaydi, shuningdek, takroriy taxallusni rad etish, taxallusni tasdiqlash/saqlash himoyasi, mos kelmaydigan chunker tutqichlari, replikatsiyalar sonini tekshirish va voris qo'riqlash xatosi (noma'lum/oldindan tasdiqlangan/nafaqaga chiqqan/o'z-o'zidan ko'rsatkichlar); qamrov tafsilotlari uchun `register_manifest_rejects_*` holatlariga qarang.
- Birlik testlari endi `crates/iroha_core/src/smartcontracts/isi/sorafs.rs` da taxallusni tekshirish, saqlash himoyasi va voris tekshiruvlarini qamrab oladi; davlat mashinasi erga tushgandan so'ng multi-hop ketma-ketligini aniqlash.
- Kuzatuv quvurlari tomonidan ishlatiladigan hodisalar uchun oltin JSON.

## Telemetriya va kuzatuvchanlik

Ko'rsatkichlar (Prometheus):
- `torii_sorafs_registry_manifests_total{status="pending|approved|retired"}`
- `torii_sorafs_registry_aliases_total`
- `torii_sorafs_registry_orders_total{status="pending|completed|expired"}`
- `torii_sorafs_replication_sla_total{outcome="met|missed|pending"}`
- `torii_sorafs_replication_completion_latency_epochs{stat="avg|p95|max|count"}`
- `torii_sorafs_replication_deadline_slack_epochs{stat="avg|p95|max|count"}`
- Mavjud provayder telemetriyasi (`torii_sorafs_capacity_*`, `torii_sorafs_fee_projection_nanos`) end-to-end asboblar paneli uchun qo'llaniladi.

Jurnallar:
- Boshqaruv auditlari uchun tuzilgan Norito hodisalar oqimi (imzolanganmi?).

Ogohlantirishlar:
- SLA dan ortiq kutilayotgan replikatsiya buyurtmalari.
- taxallusning amal qilish muddati < pol.
- saqlash qoidalarini buzish (manifest muddati tugashidan oldin yangilanmagan).

Boshqaruv paneli:
- Grafana JSON `docs/source/grafana_sorafs_pin_registry.json` manifest hayotiy siklining yakunlari, taxalluslar qamrovi, toʻyinganlik toʻyinganligi, SLA nisbati, kechikish va boʻshashmaslik va qoʻngʻiroq boʻyicha koʻrib chiqish uchun oʻtkazib yuborilgan buyurtma stavkalarini kuzatadi.

## Runbooks va hujjatlar

- Ro'yxatga olish kitobi holatini yangilash uchun `docs/source/sorafs/migration_ledger.md` ni yangilang.
- Operator uchun qo'llanma: `docs/source/sorafs/runbooks/pin_registry_ops.md` (hozir nashr etilgan) ko'rsatkichlar, ogohlantirishlar, joylashtirish, zaxiralash va tiklash oqimlarini qamrab oladi.
- Boshqaruv bo'yicha qo'llanma: siyosat parametrlarini, tasdiqlash ish jarayonini, nizolarni ko'rib chiqishni tavsiflang.
- Har bir so'nggi nuqta uchun API mos yozuvlar sahifalari (Docusaurus docs).

## Bog'liqlar va ketma-ketlik

1. To'liq tekshirish rejasi vazifalari (ManifestValidator integratsiyasi).
2. Norito sxemasi + standart parametrlarini yakunlang.
3. Shartnoma + xizmat ko'rsatish, simli telemetriyani amalga oshirish.
4. Armaturalarni qayta tiklash, integratsiya to'plamlarini ishga tushirish.
5. Docs/runbook-larni yangilang va yo'l xaritasi elementlarini tugallangan deb belgilang.

SF-4 ostidagi yo'l xaritasi nazorat ro'yxatining har bir bandi taraqqiyotga erishilganda ushbu rejaga havola qilishi kerak.
REST jabhasi endi tasdiqlangan ro'yxatning so'nggi nuqtalari bilan jo'natiladi:

- `GET /v1/sorafs/pin` returns the attested manifest catalogue.
- `GET /v1/sorafs/pin/{digest_hex}` returns exact `PinManifestFinalizedRecordV1` JSON with `finalized_cursor.height`, `finalized_cursor.block_hash`, and native `manifest`.
- `GET /v1/sorafs/aliases` va `GET /v1/sorafs/replication` faol moddalarni ochib beradi.
  taxallus katalogi va izchil sahifalash bilan replikatsiya tartibi to'plami va
  holat filtrlari.

CLI ushbu qo'ng'iroqlarni o'radi (`iroha app sorafs pin list`, `pin show`, `alias list`,
`replication list`) shuning uchun operatorlar ro'yxatga olish kitobi tekshiruvlarini teginmasdan yozishi mumkin
quyi darajadagi API.