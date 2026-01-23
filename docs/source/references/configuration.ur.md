---
lang: ur
direction: rtl
source: docs/source/references/configuration.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 74af460760b65e41904f7b9b7fed03c60bb24155464cf00b76680146a777b493
source_last_modified: "2025-12-27T07:56:10.861922+00:00"
translation_last_reviewed: 2025-12-28
---

<div dir="rtl">

# کنفیگریشن کا خلاصہ

یہ صفحہ Iroha کی کنفیگریشن فائل (TOML) کے اعلیٰ سطحی حصوں اور اُن کے کنٹرول کا خلاصہ پیش کرتا ہے۔ ایک عملی ٹیمپلیٹ کے لیے `docs/source/references/peer.template.toml` دیکھیں۔

پہلے ڈیفالٹس: کنفیگریشن اقدار عام Iroha بلاک چین ڈپلائمنٹس کے لیے منتخب کی گئی ہیں۔ زیادہ تر صورتوں میں آپ ڈیفالٹس کے ساتھ ہی چلا سکتے ہیں۔ صرف تب تبدیلی کریں جب کوئی مخصوص آپریشنل ضرورت ہو (مثلاً کسٹم پورٹس، سخت ریٹ لِمٹس، یا معلوم نیٹ ورک رکاوٹیں)۔

- Sora مخصوص سیکشنز (`torii.sorafs`، `sorafs.*`، ملٹی‑لین `nexus.*`) کے لیے لازمی ہے کہ ڈیمَن `irohad --sora …` کے ساتھ چلایا جائے (یا `IROHA_SORA_PROFILE=1` ایکسپورٹ کیا جائے)۔ اس فلیگ کے بغیر لوڈر ان فیچرز والی کنفیگریشنز رد کر دیتا ہے۔
- جب `nexus.enabled = false` (Iroha 2 موڈ) ہو تو کنفیگز سنگل‑لین رہتے ہیں: `/status` اور `/v1/sumeragi/status` میں لین/ڈیٹاسپیس حصے چھپ جاتے ہیں، `/metrics` لین/ڈیٹاسپیس لیبلز بالکل چھوڑ دیتا ہے، اور لین‑اسکوپڈ اسٹیٹس ٹیلز `StatusSegmentNotFound` لوٹاتے ہیں؛ لہٰذا SDKs کو لین پیرامیٹرز بھیجنے سے پہلے فلیگ پر برانچ کرنا چاہیے۔ اس موڈ میں لین/ڈیٹاسپیس/روٹنگ overrides مسترد ہوتے ہیں—`nexus.*` سیکشنز ہٹا دیں جب تک `nexus.enabled=true` یا `--sora` سے Nexus/Sora فعال نہ ہو۔
- `[common]`: چین شناخت (`chain`)، نوڈ کی key pair، trusted peers، اکاؤنٹ ڈومین کا ضمنی لیبل (`default_account_domain_label`)، اور IH58 chain discriminant (`chain_discriminant`)۔ اگر آپ کا genesis ڈومین یا نیٹ ورک پری فکس ڈیفالٹس سے مختلف ہو تو یہاں override کریں تاکہ IH58/کمپریسڈ ایڈریسز canonical رہیں۔
- ویلیڈیٹر ایڈمشن: `trusted_peers` میں BLS‑Normal ویلیڈیٹر کیز ہونی چاہئیں، اور PoP `[[trusted_peers_pop]] { public_key, pop_hex }` میں رہتے ہیں۔ PoP میپ کو پورے روسٹر کو کور کرنا چاہیے؛ کمی یا غلط PoP کنفیگ پارسنگ کے وقت مسترد ہوتے ہیں۔ کنفیگ پارسر اب پرانا `trusted_peers_bls` میپ اور غیر‑BLS ویلیڈیٹر کیز رد کرتا ہے۔ ٹرانسپورٹ شناخت اب بھی `public_key`/`private_key` سے P2P/Torii کے لیے ہوتی ہے، جبکہ کنسینسس سگنیچر صرف BLS key + PoP سے ہوتا ہے۔
- شناخت کی جدائی: `public_key`/`private_key` جوڑا ٹرانسپورٹ (P2P, Torii) کو محفوظ کرتا ہے اور `allowed_signing` سے قطع نظر Ed25519/SECP/etc رہ سکتا ہے۔ کنسینسس `trusted_peers` کے BLS‑Normal key کو PoP کے ساتھ استعمال کرتا ہے۔ ٹرانزیکشن ایڈمشن وہ واحد جگہ ہے جہاں `crypto.allowed_signing`/`allowed_curve_ids` نافذ ہوتے ہیں؛ اگر آپ BLS‑signed ٹرانزیکشنز بھیجنا چاہتے ہیں تو `allowed_signing` میں `bls_normal` اور `crypto.curves.allowed_curve_ids` میں متعلقہ curve id شامل کریں۔ کنسینسس BLS ویلیڈیٹرز کے ساتھ تب بھی چلتا رہے گا جب `allowed_signing` Ed25519/secp ڈیفالٹس پر رہے۔
- `[genesis]`: سائن شدہ genesis فائل کا مقام اور اسے verify کرنے والی پبلک key۔ referenced manifest میں `transaction.max_signatures`، `transaction.max_instructions`، `transaction.ivm_bytecode_size`، `transaction.max_tx_bytes`، `transaction.max_decompressed_bytes` جیسے پیرامیٹر فیملیز شامل ہیں؛ ایڈمشن حدود سخت کرتے وقت manifest بھی اپ ڈیٹ کریں۔ جو نوڈز خالی اسٹوریج اور بغیر لوکل `genesis.file` کے شروع ہوتے ہیں وہ trusted peers سے genesis حاصل کر سکتے ہیں، اختیاری طور پر `expected_hash` پن کر کے، `bootstrap_allowlist` (ڈیفالٹ `trusted_peers`) دے کر، اور ٹرانسفر کو `bootstrap_max_bytes` اور `bootstrap_response_throttle_ms` سے محدود کر کے۔ bootstrap کوششیں `bootstrap_request_timeout_ms`، `bootstrap_retry_interval_ms`، `bootstrap_max_attempts`، اور `bootstrap_enabled` کو مانتی ہیں؛ ریسپانڈرز ہمیشہ chain-id/پبلک key/hash میچ چیک کرتے ہیں اور بڑے payloads یا غیر‑allowlisted peers کو رد کرتے ہیں۔
- `[network]`: P2P ایڈریسز، gossip periods/sizes، DNS refresh، bounded queue capacities، اور transport options (TLS/QUIC جب فعال ہوں)۔
- `[ivm]`: VM runtime سیٹنگز۔ `ivm.memory_budget_profile` (ڈیفالٹ: `compute.default_resource_profile`) وہ compute resource profile منتخب کرتا ہے جو `max_stack_bytes` کو guest stack budget cap کے طور پر فراہم کرتا ہے۔
- `[tiered_state]`: WSV کے hot/cold tiering کنٹرولز (`enabled`, `hot_retained_keys`, `hot_retained_bytes`, `hot_retained_grace_snapshots`, `cold_store_root`, `da_store_root`, `max_snapshots`, `max_cold_bytes`). اگر `cold_store_root` سیٹ نہ ہو تو snapshots `da_store_root` میں محفوظ ہوتے ہیں؛ کسی بھی روٹ میں تبدیلی tiering metadata کو ری سیٹ کرتی ہے۔
- `[concurrency]`: thread‑pool sizing اور scheduler/prover workers اور IVM guest کے لیے stack budgets۔ `gas_to_stack_multiplier` (ڈیفالٹ `4`) guest stack space کو `min(guest_stack_bytes, gas_limit * multiplier, stack_budget)` کے طور پر محدود کرتا ہے (کم از کم 64 KiB، زیادہ سے زیادہ 1 GiB)؛ `stack_budget` `ivm.memory_budget_profile` سے آتا ہے۔ حدود سے باہر درخواستیں clamp ہوتی ہیں، لاگ ہوتی ہیں، اور `/status.stack` و `iroha_ivm_stack_*` میٹرکس میں ظاہر ہوتی ہیں (pool‑fallback/budget‑hit counters سمیت)۔ کم gas روٹس میں ریکرشَن محدود کرنے کے لیے multiplier کم کریں یا گہرے Kotodama frames کے لیے (guest stack cap کے اندر) بڑھائیں۔
- `network.peer_gossip_period_ms` (ڈیفالٹ: `250`ms) ٹوپولوجی ریفریش cadence بھی ہے۔ `IROHA_P2P_TOPOLOGY_UPDATE_MS` env override اب نظرانداز ہوتا ہے؛ کنفیگ فیلڈ ایڈجسٹ کریں۔ gossip periods (`block_gossip_period_ms`, `transaction_gossip_period_ms`, `peer_gossip_period_ms`) اور `idle_timeout_ms` کم از کم 100ms پر clamp ہوتے ہیں تاکہ tight loops نہ بنیں۔
- `network.trust_gossip` (ڈیفالٹ: `true`) TrustGossip frames کی سپورٹ advertise کرتا ہے۔ اسے `false` کرنے سے ہینڈ شیک میں صلاحیت بند ہو جاتی ہے، TrustGossip میسجز drop ہوتے ہیں جبکہ peer‑address gossip برقرار رہتا ہے۔ Drops کو `p2p_trust_gossip_skipped_total{direction,reason}` سے ٹیگ کیا جاتا ہے تاکہ opt‑outs یا capability انکار واضح ہو۔
- `[network]` کے trust scoring knobs (`trust_decay_half_life_ms`, `trust_penalty_bad_gossip`, `trust_penalty_unknown_peer`, `trust_min_score`) permissioned overlays میں لاگو ہوتے ہیں: فعال ٹوپولوجی سے باہر peers کو حوالہ دینے والے trust frames unknown‑peer penalty لیتے ہیں، `p2p_trust_penalties_total{reason="unknown_peer"}` بڑھاتے ہیں، اور `trust_min_score` سے نیچے بھیجنے والوں کو decay بحال ہونے تک نظرانداز کیا جاتا ہے۔ Public/NPoS overlays اس penalty کو چھوڑ دیتے ہیں تاکہ trust gossip کھلا رہے۔
- ٹرانزیکشن gossip tuning: `network.transaction_gossip_drop_unknown_dataspace` ان dataspaces کے لیے بیچز ڈراپ کرتا ہے جو lane catalog میں نہ ہوں؛ `network.transaction_gossip_restricted_target_cap` restricted fanout کو (ڈیفالٹ مکمل commit topology) پر محدود کرتا ہے؛ `network.transaction_gossip_public_target_cap` public lane fanout کو (ڈیفالٹ 16، `null` پر broadcast) محدود کرتا ہے؛ `network.transaction_gossip_public_target_reshuffle_ms` اور `network.transaction_gossip_restricted_target_reshuffle_ms` اہداف کے reshuffle کا وقفہ طے کرتے ہیں (ڈیفالٹ `transaction_gossip_period_ms`، clamp ≥100ms)؛ `network.transaction_gossip_resend_ticks` بتاتا ہے کہ انہی ٹرانزیکشنز کو دوبارہ بھیجنے سے پہلے کتنے gossip ادوار گزریں (زیادہ قدر تکرار کم کرتی ہے)؛ `network.transaction_gossip_restricted_fallback` (`drop` | `public_overlay`) طے کرتا ہے کہ restricted gossip commit topology خالی ہونے پر online peers کی طرف جائے یا نہ جائے؛ اور `network.transaction_gossip_restricted_public_payload` (`refuse` | `forward`, ڈیفالٹ `refuse`) فی الحال public overlay موجود ہونے پر restricted payload لیک کرنے سے انکار کرتا ہے (encrypted transport آنے تک forwarding بند ہے)۔ ٹیلی میٹری drop/forward پالیسی کو `/status.tx_gossip` اور gauges کے ذریعے ظاہر کرتی ہے۔
- `network.soranet_handshake.pow`: SoraNet ایڈمشن چیلنج کنفیگ۔ فیلڈز: `required`, `difficulty`, `max_future_skew_secs`, `min_ticket_ttl_secs`, `ticket_ttl_secs`, `revocation_store_capacity`, `revocation_max_ttl_secs`، اور `revocation_store_path`۔ revocation store ڈسک پر محفوظ ٹکٹوں کی تعداد اور مدت کنٹرول کرتا ہے۔
- `network.soranet_handshake.pow.puzzle`: Argon2 puzzle gates۔ `enabled = true` کرنے سے memory‑hard tickets لازم ہو جاتے ہیں اور `memory_kib` (>=4096)، `time_cost`، `lanes` (1–16) سیٹ کریں۔ enabled ہونے پر relay `difficulty`, `max_future_skew_secs`, `min_ticket_ttl_secs` کا احترام برقرار رکھتا ہے؛ بلاک کو disable کرنے سے کلاسک hashcash PoW واپس آتا ہے۔
- `network.soranet_handshake.kem_suite`: اختیاری textual override (`"mlkem512"`, `"mlkem768"`, یا `"mlkem1024"`; `kyber*` aliases بھی قبول) ML‑KEM پروفائل کے لیے۔ سیٹ کرنے پر `kem_id` کو supersede کرتا ہے اور منتخب suite کے لیے Kyber key lengths ویلیڈیٹ کرتا ہے۔
- `[sumeragi]`: کنسینسس رول (validator/observer) اور collector سیٹنگز (K, redundant‑send r) کے ساتھ timing slack۔ DA availability `sumeragi.da.enabled` availability evidence (availability evidence یا RBC `READY` quorum) کو ٹریک کرتا ہے مگر commit کو مؤخر نہیں کرتا؛ commit لوکل RBC deliveries کا انتظار نہیں کرتا اور legacy `da_reschedule_total` عام طور پر صفر رہتا ہے۔ per‑queue backpressure کو `sumeragi.queues.votes`, `sumeragi.queues.block_payload`, `sumeragi.queues.rbc_chunks`, `sumeragi.queues.blocks`, `sumeragi.queues.control` سے کنفیگر کیا جاتا ہے۔ `sumeragi.mode_flip.enabled` (ڈیفالٹ `true`) live permissioned↔NPoS cutovers کے لیے kill switch ہے: `false` ہونے پر نوڈ staged mode کو status/telemetry میں ظاہر کرتا ہے مگر flip نہیں کرتا جب تک فلیگ بحال نہ ہو۔ persistence failures `sumeragi.persistence.kura_retry_interval_ms`/`sumeragi.persistence.kura_retry_max_attempts` کے مطابق backoff ہوتے ہیں تاکہ بلاکس pending رہیں؛ `sumeragi.persistence.commit_inflight_timeout_ms` stuck commit jobs کو abort کرتا ہے تاکہ proposals نہ رکیں۔ missing‑block fetch `sumeragi.recovery.missing_block_signer_fallback_attempts` (ڈیفالٹ 1؛ 0 پر ترجیح ختم) تک commit certificate signers کو ترجیح دیتا ہے، پھر full commit topology پر آتا ہے۔ رکنیت کے ہیش میں انحراف `sumeragi.gating.membership_mismatch_alert_threshold` (الرٹ سے پہلے مسلسل عدم مطابقت) اور `sumeragi.gating.membership_mismatch_fail_closed` (ابھی تک عدم مطابقت رکھنے والے پئیرز کے کنسینس پیغامات ڈراپ کریں) کے ذریعے ظاہر کیا جاتا ہے۔ pacemaker proposal interval `sumeragi.npos.block_time_ms`, `sumeragi.npos.timeouts.propose_ms`, `sumeragi.pacemaker.rtt_floor_multiplier` سے اخذ ہو کر `sumeragi.pacemaker.max_backoff_ms` سے capped ہوتا ہے؛ high‑latency لنکس پر RTT floor یا block time بڑھائیں۔ INIT سے پہلے RBC stashز کو سیشنز کے درمیان `sumeragi.rbc.pending_session_limit` کے ذریعے محدود کیا جاتا ہے۔ RBC READY/DELIVER quorum ہمیشہ commit topology (`Topology::min_votes_for_commit()`) کے مطابق ہے۔
- `[torii]`: پبلک API سرور سیٹنگز جیسے listen address، request size caps، اور query store parameters۔
- `torii.events_buffer_capacity` (ڈیفالٹ: `10000`): `/v1/events/sse` اور webhook enqueue کے لیے broadcast channel سائز۔ سبسکرائبرز لیٹ ہوں تو میموری/بیک‑پریشر محدود کرنے کے لیے کم کریں۔
- `torii.ws_message_timeout_ms` (ڈیفالٹ: `10000`): Torii ایونٹ/بلاک WebSocket streams کے لیے message read/write timeout؛ سست کلائنٹس یا high‑latency لنکس کے لیے بڑھائیں۔
- `torii.app_api.*`: JSON convenience endpoints کے لیے pagination/backpressure defaults۔ `default_list_limit`، `limit` کو seed کرتا ہے، `max_list_limit` اور `max_fetch_size` page/fetch sizes کو محدود کرتے ہیں، اور `rate_limit_cost_per_row` ہر requested row کی لاگت طے کرتا ہے۔
- `torii.webhook.*`: webhook worker کے لیے delivery/backoff tuning۔ `queue_capacity` (ڈیفالٹ: `10000`) آن‑ڈسک pending deliveries کی حد، `max_attempts` (ڈیفالٹ: `12`) retries کی حد، `backoff_initial_ms`/`backoff_max_ms` (ڈیفالٹ: `1000`/`60000`) exponential window، اور `{connect,write,read}_timeout_ms` delivery HTTP timeouts۔
- `torii.push.*` (feature stub): push‑notification bridge toggles اور rate limits۔ `enabled` bridge کو فعال کرتا ہے؛ `rate_per_minute`/`burst` outbound requests پر token bucket لگاتے ہیں، `max_topics_per_device` token‑per‑device subscriptions کو محدود کرتا ہے، اور `connect_timeout_ms`/`request_timeout_ms` FCM/APNS کے لیے HTTP حدود دیتے ہیں۔ `fcm_api_key` یا APNS `apns_endpoint` + `apns_auth_token` دیں۔
- `torii.da_ingest.*`: DA pipeline کے لیے replay cache اور manifest spool کنفیگ۔
  - `telemetry_cluster_label` (اختیاری): Taikai ingest/viewer metrics پر `cluster` لیبل سیٹ کرتا ہے تاکہ dashboards ماحول الگ کر سکیں (مثلاً `region-a` بمقابلہ `staging`)۔
    `governance_metadata_key_hex` کو 32‑بائٹ ChaCha20‑Poly1305 key پر سیٹ کریں جب Torii کو governance‑only metadata seal کرنا ہو؛ `governance_metadata_key_label` (مختصر شناخت مثلاً `"primary"`) کے ساتھ جوڑیں تاکہ encrypted entries بتائیں کہ کس key نے ciphertext بنایا۔ Torii cleartext governance metadata کو خودکار طور پر encrypt کرتا ہے اور اُن manifests کو رد کرتا ہے جن کے pre‑encrypted payloads میں labels غائب یا mismatch ہوں۔ nested `replication_policy` ٹیبل ہر blob class کے لیے `RetentionPolicy` define کرتی ہے (دیکھیں `docs/source/da/replication_policy.md`)۔ `default_retention` override کر کے hot/cold ونڈوز بدلیں اور `replication_policy.overrides` میں entries (مثلاً `class = "taikai_segment"`) شامل کریں؛ Torii caller کے manifests کو انہی اقدار سے match کرنے کے لیے rewrite کرتا ہے اور mismatch envelopes رد کرتا ہے۔
  - `debug_match_filters` (ڈیفالٹ: `false`) وہ deterministic filter‑debug traces فعال کرتا ہے جو پہلے `TORII_DEBUG_MATCH` env toggle کے پیچھے تھے۔ پروڈکشن میں بند رکھیں۔
- `[torii.iso_bridge.reference_data]`: ISO reference datasets کے راستے (`isin_crosswalk_path`, `bic_lei_path`, `mic_directory_path`)، ingest cadence (`refresh_interval_secs`)، اور optional `cache_dir` (snapshot copies + provenance metadata: version/source/checksum)۔
- `[torii.soranet_privacy_ingest]`: `/v1/soranet/privacy/{event,share}` کے guards۔ `enabled` لازمی `true`، `require_token` + `tokens` `X-SoraNet-Privacy-Token` (یا `X-API-Token`) نافذ کرتے ہیں، `allow_cidrs` CIDR کے ذریعے submitters محدود کرتا ہے (خالی فہرست default deny)، اور `rate_per_sec`/`burst` token‑bucket limiter لگاتے ہیں۔ ریجیکشنز 401/403/429 کے ساتھ retry hints اور ٹیگڈ telemetry counters دیتے ہیں؛ rollout checklist کے لیے `docs/source/sorafs_authz_runbook.md` دیکھیں۔
- `[torii.sorafs]`: Torii side پر SoraFS discovery cache controls۔ values canonical `[sorafs.*]` سے copy ہوتی ہیں، لہٰذا root sections پر سیٹ کریں؛ Torii view runtime snapshots/telemetry کے لیے ہے۔
- `[sorafs.discovery]`: canonical discovery toggle اور capability allow‑list۔ `discovery_enabled` ڈیفالٹ `false` (Sora پروفائلز جیسے `tests/fixtures/base.toml` اسے explicitly enable کرتے ہیں)، `known_capabilities` کے defaults `["torii_gateway", "chunk_range_fetch", "vendor_reserved"]` ہیں، اور اختیاری `[sorafs.discovery.admission]` جدول governance envelopes directory (`envelopes_dir`) کی طرف اشارہ کرتا ہے۔
- `[sorafs.storage]`: ایمبیڈڈ SoraFS worker کنفیگ۔ اہم فیلڈز: `enabled`, `data_dir` (storage root), `max_capacity_bytes`, `max_parallel_fetches`, `max_pins`, `por_sample_interval_secs`, optional `alias`, اور جب settlements ڈسک پر لکھنے ہوں تو `governance_dag_dir`۔ `[sorafs.storage.adverts]` advert metadata (`stake_pointer`, `availability`, `max_latency_ms`, `topics`) expose کرتا ہے؛ runtime snapshots میں `torii.sorafs_storage` کے تحت یہی ڈھانچہ ہوتا ہے۔
- `[sorafs.storage.pin]`: `/v1/sorafs/storage/pin` کے لیے auth/rate limits۔ `require_token=true` bearer token لگاتا ہے (`Authorization: Bearer ...` یا `X-SoraFS-Pin-Token`)، `tokens` allow‑list دیتا ہے، `allow_cidrs` clients محدود کرتا ہے، اور `rate_limit` token‑bucket (اختیاری ban کے ساتھ) لگاتا ہے۔ guards fail ہوں تو Torii pin سے پہلے ہی 401/403/429 واپس کرتا ہے، throttled responses میں `Retry-After` شامل ہوتا ہے۔
- `[sorafs.repair]`: مرمت شیڈولر کی ترتیبات۔ کلیدیں: `enabled` (ڈیفالٹ `false`)، اختیاری `state_dir` (غیر مقرر ہو تو `<sorafs.storage.data_dir>/repair`)، `claim_ttl_secs` (ڈیفالٹ `900`)، `heartbeat_interval_secs` (ڈیفالٹ `60`)، `max_attempts` (ڈیفالٹ `3`)، `worker_concurrency` (ڈیفالٹ `4`)، `backoff_initial_secs` (ڈیفالٹ `5`)، `backoff_max_secs` (ڈیفالٹ `60`)، اور `default_slash_penalty_nano` (ڈیفالٹ `1000000000`)۔
- `[sorafs.gc]`: GC شیڈولر کی ترتیبات۔ کلیدیں: `enabled` (ڈیفالٹ `false`)، اختیاری `state_dir` (غیر مقرر ہو تو `<sorafs.storage.data_dir>/gc`)، `interval_secs` (ڈیفالٹ `900`)، `max_deletions_per_run` (ڈیفالٹ `500`)، `retention_grace_secs` (ڈیفالٹ `86400`)، اور `pre_admission_sweep` (ڈیفالٹ `true`)۔
- `[sorafs.quota]`: control‑plane quota windows (capacity declaration، storage pin، telemetry، deal telemetry، dispute، اور PoR submission)۔ `torii.sorafs_quota` میں Torii view انہی values کو verbatim رکھتا ہے۔
- `[sorafs.alias_cache]`: alias‑proof cache policy (`positive_ttl`, `refresh_window`, `hard_expiry`, `negative_ttl`, `revocation_ttl`, `rotation_max_age`)۔ defaults 10 منٹ / 2 منٹ / 15 منٹ / 60 سیکنڈ / 5 منٹ / 6 گھنٹے ہیں؛ سختی صرف کم کر کے کریں۔ runtime snapshot یہی keys `torii.sorafs_alias_cache` میں دکھاتا ہے۔
- `sorafs.rollout_phase`: اعلیٰ سطحی PQ rollout selector (`canary`, `ramp`, `default`)۔ clients متعلقہ anonymity stage inherit کرتے ہیں جب تک `sorafs.anonymity_policy` واضح نہ ہو۔
- `sorafs.anonymity_policy`: SoraNet PQ rollout flag (`anon-guard-pq`, `anon-majority-pq`, `anon-strict-pq`)۔ ڈیفالٹ Stage A (`anon-guard-pq`)؛ PQ coverage کافی ہونے پر Stage B/C تک بڑھائیں۔ صرف تب override کریں جب کسی جزو کو phase سے ہٹنا ہو۔
- `[nts]`: Network Time Service sampling cadence اور smoothing۔ `sample_interval_ms` کم از کم 100ms پر clamp ہے؛ صرف advisory، consensus اس پر منحصر نہیں۔
- `[kura]`: بلاک اسٹوریج پاتھ اور میموری ونڈو؛ snapshots/replay کی initialization policy۔ `block_sync_roster_retention` (ڈیفالٹ 7200) commit‑roster snapshots کے ذخیرہ کو محدود کرتا ہے، `roster_sidecar_retention` (ڈیفالٹ 512) `pipeline/roster_sidecars.*` میں sidecars کی تعداد محدود کرتا ہے۔
- `[logger]`: لاگ لیول اور فارمیٹ۔
- `[queue]`: ٹرانزیکشن queue limits اور TTL۔
- `[nexus]`: multi‑lane routing catalog، manifest registry، اور governance modules۔ lane entries `visibility`, `lane_type`, `governance`, `settlement`, `proof_scheme` (ڈیفالٹ `merkle_sha256`; `kzg_bls12_381` lanes کو deterministic KZG commitments ملتے ہیں) اور آزاد `metadata = { key = "value" }` لیتے ہیں۔ `registry` بلاک `manifest_directory`, `cache_directory`, `poll_interval_ms` (ڈیفالٹ 60s) کنٹرول کرتا ہے۔ `governance` بلاک `default_module` اور `modules` mapping فراہم کرتا ہے؛ ہر module `module_type` اور `params` overrides دیتا ہے۔
  - lane metadata key `da_manifest_policy` DA manifest enforcement کو lane پر سیٹ کرتا ہے: `strict` (ڈیفالٹ) missing/unreadable manifests اور hash mismatch پر بلاک کرتا ہے، جبکہ `audit`/`audit_only`/`warn` missing manifests کو warning کے ساتھ allow کرتے ہیں (hash mismatch پھر بھی fatal ہے)۔
  - `nexus.staking.public_validator_mode` / `nexus.staking.restricted_validator_mode` lane‑level validator lifecycle منتخب کرتے ہیں: `stake_elected` (public lanes کے لیے ڈیفالٹ) staking/NPoS راستہ فعال رکھتا ہے، جبکہ `admin_managed` (restricted lanes کے لیے ڈیفالٹ) staking کو bypass کر کے governance/admin flows سے admission چاہتا ہے۔ stake‑elected lanes کو commit topology میں زندہ consensus key والے registered peer کی ضرورت ہوتی ہے اور epoch boundaries پر activate ہوتے ہیں؛ admin‑managed lanes genesis roster برقرار رکھتے ہیں تاکہ permissioned deployments بغیر staking کے چل سکیں۔
  - `[nexus.axt]`: timing/expiry guardrails۔ `slot_length_ms` کو `1..=600_000` میں ہونا چاہیے (ڈیفالٹ `1`)؛ hosts `current_slot = block.creation_time_ms / slot_length_ms` نکالتے ہیں۔ `max_clock_skew_ms` ڈیفالٹ `0` ہے، `60_000` تک محدود ہے، اور `slot_length_ms` سے بڑا نہیں ہو سکتا؛ proofs/handles کے expiry checks اس allowance کو شامل کرتے ہیں۔ `proof_cache_ttl_slots` (ڈیفالٹ `1`, ویلیڈیشن `1..=64`) قبول/رد proofs کو کتنے slots تک cache میں رکھتا ہے۔ `replay_retention_slots` (ڈیفالٹ `128`, ویلیڈیشن `1..=4_096`) handles کے replay ledger کو کتنے عرصے تک محفوظ رکھتا ہے؛ صرف تب بڑھائیں جب handles طویل مدت تک valid رہیں۔ مثال کے لیے `crates/iroha_config/tests/fixtures/nexus_axt_full.toml` دیکھیں۔
- `[governance]`: verifying keys، conviction parameters، approval thresholds، turnout floor، اور council sizing/eligibility۔
- `[norito]`: Norito serialization tuning (compression sizes/levels اور adaptive layout toggles)۔ design کے لحاظ سے deterministic۔ `max_archive_len` ڈیفالٹ 512 MiB ہے اور اسے کم از کم `sumeragi.rbc.store_max_bytes` اور `network.max_frame_bytes` تک بڑھایا جاتا ہے تاکہ consensus payloads decodable رہیں۔
- `[streaming]`: Norito streaming کے لیے control‑plane key material (HPKE + control signatures)۔
  - `identity_public_key` / `identity_private_key` (اختیاری): Ed25519 multihash strings جو streaming signatures کے لیے node کی main key pair کو override کرتے ہیں—جب validator key GOST یا غیر‑Ed25519 ہو تو مفید۔
  - `kyber_public_key` / `kyber_secret_key` (اختیاری): HPKE کے لیے hex‑encoded Kyber‑768 keys؛ دونوں لازم ہیں۔
  - `session_store_dir` (ڈیفالٹ: `./storage/streaming`): encrypted session snapshots کے لیے ڈائریکٹری۔
  - `feature_bits` (ڈیفالٹ: `0b11`): QUIC capability negotiation میں اعلان شدہ mask۔ بٹ 0 feedback hints، بٹ 1 privacy‑overlay provider سپورٹ، باقی reserved۔ غیر‑سپورٹڈ bits والے viewers مسترد ہوتے ہیں۔
  - `soranet` (ڈیفالٹ: enabled): جب manifests میں explicit metadata نہ ہو تو privacy routes کو خودکار طور پر SoraNet پر bridge کرتا ہے۔
    - `enabled` (ڈیفالٹ: `true`): auto‑provisioning logic gate؛ disable کرنے سے existing manifests جوں کے توں رہیں گے۔
    - `exit_multiaddr` (ڈیفالٹ: `/dns/torii/udp/9443/quic`): exit relay multi‑address۔
    - `padding_budget_ms` (ڈیفالٹ: `25`): low‑latency padding budget (ms)؛ `null` اضافی padding بند کرتا ہے۔
    - `access_kind` (ڈیفالٹ: `authenticated`): exit posture (`authenticated` یا `read-only`)۔
    - `channel_salt` (ڈیفالٹ: `iroha.soranet.channel.seed.v1`): domain string جو stream/route IDs کے ساتھ hash ہو کر blinded channel IDs دیتی ہے۔
    - `provision_window_segments` (ڈیفالٹ: `4`): segment window (inclusive) جو privacy routes کی خودکار provision کے لئے استعمال ہوتا ہے؛ windows اسی سائز کے multiples پر align ہوتی ہیں۔
    - `provision_queue_capacity` (ڈیفالٹ: `256`): privacy routes کی provisioning jobs کی زیادہ سے زیادہ تعداد جو queue میں رکھی جا سکتی ہیں؛ اس کے بعد backpressure apply ہوتا ہے۔
  - `soravpn`: streaming routes کے لئے SoraVPN local provisioning spool settings۔
    - `provision_spool_dir` (ڈیفالٹ: `./storage/streaming/soravpn_routes`): مقامی VPN nodes کیلئے SoraVPN route updates stage کرنے کا spool directory۔
    - `provision_spool_max_bytes` (ڈیفالٹ: `0`, unlimited): SoraVPN provision spool کی زیادہ سے زیادہ disk footprint۔
  - `sync` (ڈیفالٹ: observe‑only): audio/video sync enforcement policy۔
    - `enabled` (ڈیفالٹ: `false`): drift reports evaluate کرنے کے لیے `true` کریں۔
    - `observe_only` (ڈیفالٹ: `true`): violations لاگ ہوں مگر segments قبول رہیں؛ سخت enforcement کے لیے `false`۔
    - `min_window_ms` (ڈیفالٹ: `5000`): کم از کم diagnostic window۔
    - `ewma_threshold_ms` (ڈیفالٹ: `10`): sustained EWMA drift threshold (ms)۔
    - `hard_cap_ms` (ڈیفالٹ: `12`): فی sample زیادہ سے زیادہ drift (ms)۔
- `[relay]` (tools/soranet-relay): SoraNet entry/exit relays کے لیے reference daemon۔
  - `mode` (`"Entry" | "Middle" | "Exit"`): privacy role، telemetry labels، اور padding rules منتخب کرتا ہے۔
  - `listen` / `admin_listen`: QUIC listener اور اختیاری admin socket (metrics + control)۔
  - `padding`: stream padding profile۔ `cell_size` (bytes) اور `max_idle_millis` interval بیان کرتے ہیں۔ `global_rate_limit_bytes_per_sec` (ڈیفالٹ: `0`) token‑bucket throttle فعال کرتا ہے، اور `burst_bytes` shared bucket capacity کو محدود کرتا ہے۔
- `exit_routing.norito_stream`: Torii Norito streaming adapter کا اختیاری bridge؛ فعال ہونے پر relay فائل سسٹم spool (`exit-<relay-id>/norito-stream/*.norito`) اسکین کرتا ہے۔
- `exit_routing.kaigi_stream`: Kaigi hubs کا اختیاری bridge؛ فعال ہونے پر relay `exit-<relay-id>/kaigi-stream/*.norito` سے routes لوڈ کرتا ہے، `hub_ws_url` پر connect کرتا ہے، اور channel metadata سے blinded room IDs نکالتا ہے۔ knobs میں `connect_timeout_millis`, GAR categories, filesystem spool override، اور `route_refresh_secs` شامل ہیں۔
    - `torii_ws_url`: Norito streaming کے لیے `ws://` یا `wss://` endpoint۔
    - `connect_timeout_millis` (ڈیفالٹ: `5000`): حد سے زیادہ کنکشن کوششیں abort ہوتی ہیں۔
    - `padding_target_millis` (ڈیفالٹ: `35`): اگر `padding_budget_ms` نہ ہو تو idle padding cadence؛ `0` پر بند۔
    - `gar_category_read_only` / `gar_category_authenticated`: Norito stream کے لیے GAR labels؛ ڈیفالٹ `stream.norito.read_only` / `stream.norito.authenticated`۔
    - `spool_dir`: filesystem catalog root؛ ہر relay id کے لیے `exit-<relay-id>/norito-stream` subdir۔
    - `route_refresh_secs` (ڈیفالٹ: `5`): spool scan cache refresh cadence۔
- `[crypto]`: `iroha_config` کے ذریعے سامنے آنے والی cryptography defaults۔
  - `sm_intrinsics` (ڈیفالٹ: `auto`): SM intrinsic dispatch policy (`auto`, `force-enable`, یا `force-disable`)۔
  - `enable_sm_openssl_preview` (ڈیفالٹ: `false`): OpenSSL‑backed SM preview path (SM3 hashing, SM2 verification, SM4‑GCM AEAD) کے لیے opt‑in؛ `--features "sm sm-ffi-openssl"` درکار۔
  - `default_hash` (ڈیفالٹ: `blake2b-256`): اگر caller hash نہ دے تو یہی استعمال ہوتا ہے۔
  - `allowed_signing` (ڈیفالٹ: `["ed25519"]`): قبول شدہ الگورتھمز کی sorted فہرست۔
    - `allowed_curve_ids` (ڈیفالٹ: `allowed_signing` سے اخذ)۔
    - `sm2_distid_default` (ڈیفالٹ: `1234567812345678`)۔
    - *Preview (feature `sm-ffi-openssl`)*: `crypto.enable_sm_openssl_preview` سے کنٹرول۔

  **SM rollout checklist.** `iroha_config` layering (`defaults` → `user` → `actual`) مرحلہ وار rollout کی اجازت دیتا ہے:
  1. **Build validation.** `--features sm` کے ساتھ کمپائل کریں۔
  2. **Configuration propagation.** `crypto.allowed_signing` میں `sm2` شامل کریں اور `crypto.default_hash = "sm3-256"` سیٹ کریں۔
  3. **Genesis alignment.** `kagami genesis generate --consensus-mode <mode>` سے genesis manifests regenerate کریں۔
  4. **Telemetry & admission smoke tests.** `cargo test -p iroha_crypto --features sm` چلائیں اور `/status` میں `crypto.sm_helpers_available = true` چیک کریں۔
  5. **Rollback plan.** `sm2` نکال کر `default_hash = "blake2b-256"` پر واپس جائیں۔

- `[fraud_monitoring]`: PSP‑provided fraud assessments کے لیے admission guardrails۔
  - `enabled` (ڈیفالٹ: false) master toggle۔
  - `service_endpoints` (ڈیفالٹ: `[]`) verifier URLs۔
  - `connect_timeout_ms` / `request_timeout_ms` (ڈیفالٹ: 500 / 1500)۔
  - `missing_assessment_grace_secs` (ڈیفالٹ: 0)۔
  - `required_minimum_band` (ڈیفالٹ: `null`)۔
- `[zk]`: zero‑knowledge verification settings۔
  - `fastpq.execution_mode` (ڈیفالٹ: `"auto"`)۔
  - `fastpq.device_class` (ڈیفالٹ: unset ⇒ `device_class="unknown"`)۔
  - `fastpq.chip_family` (ڈیفالٹ: unset ⇒ `chip_family="unknown"`)۔
  - `fastpq.gpu_kind` (ڈیفالٹ: unset ⇒ `gpu_kind="unknown"`)۔
  - `fastpq.metal_queue_fanout` / `fastpq.metal_queue_column_threshold`۔
  - `fastpq.metal_max_in_flight` / `fastpq.metal_threadgroup_width` / `fastpq.metal_trace` / `fastpq.metal_debug_enum` / `fastpq.metal_debug_fused`۔
- FASTPQ Metal/bring‑up overrides (env‑only): `FASTPQ_GPU`, `FASTPQ_METAL_LIB`, `FASTPQ_SKIP_GPU_BUILD`, `FASTPQ_METAL_TRACE`, `FASTPQ_DEBUG_METAL_ENUM`, `FASTPQ_METAL_THREADGROUP`, `FASTPQ_METAL_FFT_LANES`, `FASTPQ_METAL_FFT_TILE_STAGES`, `FASTPQ_METAL_LDE_TILE_STAGES`, `FASTPQ_METAL_POSEIDON_LANES`, `FASTPQ_METAL_POSEIDON_BATCH`, `FASTPQ_POSEIDON_PIPE_COLUMNS`, `FASTPQ_POSEIDON_PIPE_DEPTH`, `FASTPQ_METAL_FFT_COLUMNS`, `FASTPQ_METAL_LDE_COLUMNS`, `FASTPQ_METAL_QUEUE_FANOUT`, `FASTPQ_METAL_COLUMN_THRESHOLD`, `FASTPQ_METAL_MAX_IN_FLIGHT`۔
- `[accel]`: IVM اور helpers کے لیے hardware acceleration settings۔
  - `enable_simd` (ڈیفالٹ: true)۔
  - `enable_cuda` (ڈیفالٹ: true)۔
  - `enable_metal` (ڈیفالٹ: true)۔
  - `max_gpus` (ڈیفالٹ: 0)۔
  - `merkle_min_leaves_gpu` (ڈیفالٹ: 8192)۔

نوٹس
- ایکسیلیریشن ڈیفالٹ میں خودکار ہے؛ backends پہلی بار golden self‑tests چلاتے ہیں اور mismatch پر خود کو بند کر دیتے ہیں؛ VM scalar/SIMD پر واپس جاتی ہے اور مختلف ہارڈویئر پر یکساں رہتی ہے۔
- `peer.template.toml` طرز کی فائلیں ایڈٹ کرنا اور ڈپلائمنٹ میں extend کرنا بہتر ہے۔ runtime رویہ `iroha_config` سے کنفیگر ہوتا ہے؛ env overrides صرف dev tooling کے لیے ہیں۔

</div>
