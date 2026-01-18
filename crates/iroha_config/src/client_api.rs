//! Functionality related to working with the configuration through client API.
//!
//! Intended usage:
//!
//! - Create [`ConfigGetDTO`] from [`base::Root`] and serialize it for the client
#![allow(clippy::too_many_lines, clippy::or_fun_call)]
//! - Deserialize [`ConfigUpdateDTO`] from the client and apply the changes

// NonZero types are imported from core::num below

use core::{
    fmt::Write as _,
    num::{NonZero, NonZeroU32, NonZeroU64},
};
use std::{collections::BTreeMap, str::FromStr};

use hex;
use iroha_crypto::PublicKey;
use iroha_data_model::{
    Level, compute::ComputePriceWeights, name::Name, soranet::vpn::VpnExitClassV1,
};
use norito::{
    Error as NoritoError,
    json::{self, Arena, FastFromJson, FastJsonWrite, JsonDeserialize, TapeWalker},
};

use crate::{
    logger::Directives,
    parameters::{actual as base, defaults},
};

fn parse_via_fast<T>(parser: &mut json::Parser<'_>) -> Result<T, json::Error>
where
    for<'a> T: FastFromJson<'a>,
{
    let input = parser.input();
    let start = parser.position();
    let mut walker = TapeWalker::new(input);
    walker.sync_to_raw(start);
    let mut arena = Arena::new();
    let value =
        T::parse(&mut walker, &mut arena).map_err(|err| json::Error::Message(err.to_string()))?;
    let end = walker.raw_pos();
    while parser.position() < end {
        let _ = parser.bump();
    }
    Ok(value)
}

// Manual Norito fast JSON writers for small DTOs used on the client path
impl FastJsonWrite for Logger {
    fn write_json(&self, out: &mut String) {
        out.push('{');
        // level
        out.push('"');
        out.push_str("level");
        out.push_str("\":\"");
        let _ = write!(out, "{}", self.level);
        out.push('"');
        // filter
        out.push(',');
        out.push('"');
        out.push_str("filter");
        out.push_str("\":");
        if let Some(f) = &self.filter {
            out.push('"');
            let _ = write!(out, "{f}");
            out.push('"');
        } else {
            out.push_str("null");
        }
        out.push('}');
    }
}

impl FastJsonWrite for Queue {
    fn write_json(&self, out: &mut String) {
        out.push('{');
        out.push('"');
        out.push_str("capacity");
        out.push_str("\":");
        let _ = write!(out, "{}", self.capacity.get());
        out.push('}');
    }
}

impl FastJsonWrite for SoranetPrivacySummary {
    fn write_json(&self, out: &mut String) {
        out.push('{');
        out.push_str("\"bucket_secs\":");
        let _ = write!(out, "{}", self.bucket_secs);
        out.push(',');
        out.push_str("\"min_handshakes\":");
        let _ = write!(out, "{}", self.min_handshakes);
        out.push(',');
        out.push_str("\"flush_delay_buckets\":");
        let _ = write!(out, "{}", self.flush_delay_buckets);
        out.push(',');
        out.push_str("\"force_flush_buckets\":");
        let _ = write!(out, "{}", self.force_flush_buckets);
        out.push(',');
        out.push_str("\"max_completed_buckets\":");
        let _ = write!(out, "{}", self.max_completed_buckets);
        out.push(',');
        out.push_str("\"max_share_lag_buckets\":");
        let _ = write!(out, "{}", self.max_share_lag_buckets);
        out.push(',');
        out.push_str("\"expected_shares\":");
        let _ = write!(out, "{}", self.expected_shares);
        out.push(',');
        out.push_str("\"event_buffer_capacity\":");
        let _ = write!(out, "{}", self.event_buffer_capacity);
        out.push('}');
    }
}

impl FastJsonWrite for SoranetVpnSummary {
    fn write_json(&self, out: &mut String) {
        out.push('{');
        out.push_str("\"enabled\":");
        out.push_str(if self.enabled { "true" } else { "false" });
        out.push(',');
        out.push_str("\"cell_size_bytes\":");
        let _ = write!(out, "{}", self.cell_size_bytes);
        out.push(',');
        out.push_str("\"flow_label_bits\":");
        let _ = write!(out, "{}", self.flow_label_bits);
        out.push(',');
        out.push_str("\"cover_to_data_per_mille\":");
        let _ = write!(out, "{}", self.cover_to_data_per_mille);
        out.push(',');
        out.push_str("\"max_cover_burst\":");
        let _ = write!(out, "{}", self.max_cover_burst);
        out.push(',');
        out.push_str("\"heartbeat_ms\":");
        let _ = write!(out, "{}", self.heartbeat_ms);
        out.push(',');
        out.push_str("\"jitter_ms\":");
        let _ = write!(out, "{}", self.jitter_ms);
        out.push(',');
        out.push_str("\"padding_budget_ms\":");
        let _ = write!(out, "{}", self.padding_budget_ms);
        out.push(',');
        out.push_str("\"guard_refresh_secs\":");
        let _ = write!(out, "{}", self.guard_refresh_secs);
        out.push(',');
        out.push_str("\"lease_secs\":");
        let _ = write!(out, "{}", self.lease_secs);
        out.push(',');
        out.push_str("\"dns_push_interval_secs\":");
        let _ = write!(out, "{}", self.dns_push_interval_secs);
        out.push(',');
        out.push_str("\"exit_class\":");
        json::write_json_string(&self.exit_class, out);
        out.push(',');
        out.push_str("\"meter_family\":");
        json::write_json_string(&self.meter_family, out);
        out.push('}');
    }
}

impl FastJsonWrite for Network {
    fn write_json(&self, out: &mut String) {
        out.push('{');
        out.push_str("\"block_gossip_size\":");
        let _ = write!(out, "{}", self.block_gossip_size.get());
        out.push(',');
        out.push_str("\"block_gossip_period_ms\":");
        let _ = write!(out, "{}", self.block_gossip_period_ms);
        out.push(',');
        out.push_str("\"peer_gossip_period_ms\":");
        let _ = write!(out, "{}", self.peer_gossip_period_ms);
        out.push(',');
        out.push_str("\"trust_decay_half_life_ms\":");
        let _ = write!(out, "{}", self.trust_decay_half_life_ms);
        out.push(',');
        out.push_str("\"trust_penalty_bad_gossip\":");
        let _ = write!(out, "{}", self.trust_penalty_bad_gossip);
        out.push(',');
        out.push_str("\"trust_penalty_unknown_peer\":");
        let _ = write!(out, "{}", self.trust_penalty_unknown_peer);
        out.push(',');
        out.push_str("\"trust_min_score\":");
        let _ = write!(out, "{}", self.trust_min_score);
        out.push(',');
        out.push_str("\"trust_gossip\":");
        out.push_str(if self.trust_gossip { "true" } else { "false" });
        out.push(',');
        out.push_str("\"relay_ttl\":");
        let _ = write!(out, "{}", self.relay_ttl);
        out.push(',');
        out.push_str("\"transaction_gossip_size\":");
        let _ = write!(out, "{}", self.transaction_gossip_size.get());
        out.push(',');
        out.push_str("\"transaction_gossip_period_ms\":");
        let _ = write!(out, "{}", self.transaction_gossip_period_ms);
        out.push(',');
        out.push_str("\"transaction_gossip_resend_ticks\":");
        let _ = write!(out, "{}", self.transaction_gossip_resend_ticks.get());
        out.push(',');
        out.push_str("\"soranet_handshake\":");
        self.soranet_handshake.write_json(out);
        out.push(',');
        out.push_str("\"soranet_privacy\":");
        self.soranet_privacy.write_json(out);
        out.push(',');
        out.push_str("\"soranet_vpn\":");
        self.soranet_vpn.write_json(out);
        out.push(',');
        out.push_str("\"lane_profile\":\"");
        out.push_str(self.lane_profile.as_label());
        out.push('"');
        out.push(',');
        out.push_str("\"require_sm_handshake_match\":");
        out.push_str(if self.require_sm_handshake_match {
            "true"
        } else {
            "false"
        });
        out.push(',');
        out.push_str("\"require_sm_openssl_preview_match\":");
        out.push_str(if self.require_sm_openssl_preview_match {
            "true"
        } else {
            "false"
        });
        out.push('}');
    }
}

/// Partial network update payload.
#[derive(Debug, Clone, Copy, Default)]
pub struct NetworkUpdate {
    /// Override whether peers must match SM helper availability.
    pub require_sm_handshake_match: Option<bool>,
    /// Override whether peers must match the OpenSSL preview flag.
    pub require_sm_openssl_preview_match: Option<bool>,
    /// Override the lane profile preset (`core` or `home`).
    pub lane_profile: Option<base::LaneProfile>,
}

impl FastJsonWrite for NetworkUpdate {
    fn write_json(&self, out: &mut String) {
        out.push('{');
        let mut first = self.lane_profile.is_none_or(|profile| {
            out.push_str("\"lane_profile\":\"");
            out.push_str(profile.as_label());
            out.push('"');
            false
        });
        if let Some(value) = self.require_sm_handshake_match {
            if !first {
                out.push(',');
            }
            first = false;
            out.push_str("\"require_sm_handshake_match\":");
            out.push_str(if value { "true" } else { "false" });
        }
        if let Some(value) = self.require_sm_openssl_preview_match {
            if !first {
                out.push(',');
            }
            out.push_str("\"require_sm_openssl_preview_match\":");
            out.push_str(if value { "true" } else { "false" });
        }
        out.push('}');
    }
}

impl<'a> FastFromJson<'a> for NetworkUpdate {
    fn parse(w: &mut TapeWalker<'a>, arena: &mut Arena) -> Result<Self, NoritoError> {
        w.expect_object_start()?;
        let kh_require = norito::json::key_hash_const("require_sm_handshake_match");
        let kh_preview = norito::json::key_hash_const("require_sm_openssl_preview_match");
        let kh_lane_profile = norito::json::key_hash_const("lane_profile");
        let mut require_sm_handshake_match = None;
        let mut require_sm_openssl_preview_match = None;
        let mut lane_profile = None;
        while !w.peek_object_end()? {
            let kh = w.read_key_hash()?;
            w.expect_colon_resync()?;
            match kh {
                x if x == kh_lane_profile && w.last_key() == "lane_profile" => {
                    let label = w.parse_string_ref_inline(arena)?.to_string();
                    lane_profile = Some(base::LaneProfile::from_label(&label));
                }
                x if x == kh_require && w.last_key() == "require_sm_handshake_match" => {
                    require_sm_handshake_match = Some(w.parse_bool_inline()?);
                }
                x if x == kh_preview && w.last_key() == "require_sm_openssl_preview_match" => {
                    require_sm_openssl_preview_match = Some(w.parse_bool_inline()?);
                }
                _ => w.skip_value()?,
            }
            let _ = w.consume_comma_if_present()?;
        }
        w.expect_object_end()?;
        Ok(Self {
            require_sm_handshake_match,
            require_sm_openssl_preview_match,
            lane_profile,
        })
    }
}

impl<'a> FastFromJson<'a> for TransportUpdate {
    fn parse(w: &mut TapeWalker<'a>, arena: &mut Arena) -> Result<Self, NoritoError> {
        w.expect_object_start()?;
        let kh_nr = norito::json::key_hash_const("norito_rpc");
        let mut norito_rpc = None;
        while !w.peek_object_end()? {
            let kh = w.read_key_hash()?;
            w.expect_colon_resync()?;
            if kh == kh_nr && w.last_key() == "norito_rpc" {
                norito_rpc = Some(NoritoRpcUpdate::parse(w, arena)?);
            } else {
                w.skip_value()?;
            }
            let _ = w.consume_comma_if_present()?;
        }
        w.expect_object_end()?;
        Ok(Self { norito_rpc })
    }
}

impl<'a> FastFromJson<'a> for NoritoRpcUpdate {
    fn parse(w: &mut TapeWalker<'a>, arena: &mut Arena) -> Result<Self, NoritoError> {
        w.expect_object_start()?;
        let kh_enabled = norito::json::key_hash_const("enabled");
        let kh_mtls = norito::json::key_hash_const("require_mtls");
        let kh_allowed = norito::json::key_hash_const("allowed_clients");
        let kh_stage = norito::json::key_hash_const("stage");
        let mut enabled = None;
        let mut require_mtls = None;
        let mut allowed_clients = None;
        let mut stage = None;
        while !w.peek_object_end()? {
            let kh = w.read_key_hash()?;
            w.expect_colon_resync()?;
            match kh {
                x if x == kh_enabled && w.last_key() == "enabled" => {
                    enabled = Some(w.parse_bool_inline()?);
                }
                x if x == kh_mtls && w.last_key() == "require_mtls" => {
                    require_mtls = Some(w.parse_bool_inline()?);
                }
                x if x == kh_allowed && w.last_key() == "allowed_clients" => {
                    let s_in = w.input();
                    let mut parser = norito::json::Parser::new_at(s_in, w.raw_pos());
                    let vec: Vec<String> =
                        norito::json::JsonDeserialize::json_deserialize(&mut parser)?;
                    w.sync_to_raw(parser.position());
                    allowed_clients = Some(vec);
                }
                x if x == kh_stage && w.last_key() == "stage" => {
                    let sref = w.parse_string_ref_inline(arena)?;
                    stage = Some(sref.to_string());
                }
                _ => w.skip_value()?,
            }
            let _ = w.consume_comma_if_present()?;
        }
        w.expect_object_end()?;
        Ok(Self {
            enabled,
            require_mtls,
            allowed_clients,
            stage,
        })
    }
}

impl FastJsonWrite for SoranetHandshakeSummary {
    fn write_json(&self, out: &mut String) {
        out.push('{');
        out.push_str("\"descriptor_commit_hex\":\"");
        out.push_str(&self.descriptor_commit_hex);
        out.push('"');
        out.push(',');
        out.push_str("\"client_capabilities_hex\":\"");
        out.push_str(&self.client_capabilities_hex);
        out.push('"');
        out.push(',');
        out.push_str("\"relay_capabilities_hex\":\"");
        out.push_str(&self.relay_capabilities_hex);
        out.push('"');
        out.push(',');
        out.push_str("\"kem_id\":");
        let _ = write!(out, "{}", self.kem_id);
        out.push(',');
        out.push_str("\"sig_id\":");
        let _ = write!(out, "{}", self.sig_id);
        out.push(',');
        out.push_str("\"resume_hash_hex\":");
        if let Some(resume) = &self.resume_hash_hex {
            out.push('"');
            out.push_str(resume);
            out.push('"');
        } else {
            out.push_str("null");
        }
        out.push(',');
        out.push_str("\"pow\":");
        self.pow.write_json(out);
        out.push('}');
    }
}

impl FastJsonWrite for SoranetHandshakeUpdate {
    fn write_json(&self, out: &mut String) {
        out.push('{');
        let mut first = true;
        let mut field = |out: &mut String| {
            if first {
                first = false;
            } else {
                out.push(',');
            }
        };
        if let Some(value) = &self.descriptor_commit_hex {
            field(out);
            out.push_str("\"descriptor_commit_hex\":\"");
            out.push_str(value);
            out.push('"');
        }
        if let Some(value) = &self.client_capabilities_hex {
            field(out);
            out.push_str("\"client_capabilities_hex\":\"");
            out.push_str(value);
            out.push('"');
        }
        if let Some(value) = &self.relay_capabilities_hex {
            field(out);
            out.push_str("\"relay_capabilities_hex\":\"");
            out.push_str(value);
            out.push('"');
        }
        if let Some(value) = self.kem_id {
            field(out);
            out.push_str("\"kem_id\":");
            let _ = write!(out, "{value}");
        }
        if let Some(value) = self.sig_id {
            field(out);
            out.push_str("\"sig_id\":");
            let _ = write!(out, "{value}");
        }
        if let Some(resume) = &self.resume_hash_hex {
            field(out);
            out.push_str("\"resume_hash_hex\":");
            match resume {
                ResumeHashDirective::Set(hex) => {
                    out.push('"');
                    out.push_str(hex);
                    out.push('"');
                }
                ResumeHashDirective::Clear => out.push_str("null"),
            }
        }
        if let Some(pow) = &self.pow {
            field(out);
            out.push_str("\"pow\":");
            pow.write_json(out);
        }
        out.push('}');
    }
}

impl FastJsonWrite for TransportUpdate {
    fn write_json(&self, out: &mut String) {
        out.push('{');
        if let Some(norito_rpc) = &self.norito_rpc {
            out.push_str("\"norito_rpc\":");
            norito_rpc.write_json(out);
        }
        out.push('}');
    }
}

impl FastJsonWrite for NoritoRpcUpdate {
    fn write_json(&self, out: &mut String) {
        out.push('{');
        let mut first = true;
        if let Some(value) = self.enabled {
            if !first {
                out.push(',');
            }
            first = false;
            out.push_str("\"enabled\":");
            out.push_str(if value { "true" } else { "false" });
        }
        if let Some(value) = self.require_mtls {
            if !first {
                out.push(',');
            }
            first = false;
            out.push_str("\"require_mtls\":");
            out.push_str(if value { "true" } else { "false" });
        }
        if let Some(list) = &self.allowed_clients {
            if !first {
                out.push(',');
            }
            first = false;
            out.push_str("\"allowed_clients\":[");
            for (idx, client) in list.iter().enumerate() {
                if idx != 0 {
                    out.push(',');
                }
                norito::json::write_json_string(client, out);
            }
            out.push(']');
        }
        if let Some(stage) = &self.stage {
            if !first {
                out.push(',');
            }
            out.push_str("\"stage\":\"");
            out.push_str(stage);
            out.push('"');
        }
        out.push('}');
    }
}

impl FastJsonWrite for SoranetHandshakePowUpdate {
    fn write_json(&self, out: &mut String) {
        out.push('{');
        let mut first = true;
        let mut field = |out: &mut String| {
            if first {
                first = false;
            } else {
                out.push(',');
            }
        };
        if let Some(value) = self.required {
            field(out);
            out.push_str("\"required\":");
            out.push_str(if value { "true" } else { "false" });
        }
        if let Some(value) = self.difficulty {
            field(out);
            out.push_str("\"difficulty\":");
            let _ = write!(out, "{value}");
        }
        if let Some(value) = self.max_future_skew_secs {
            field(out);
            out.push_str("\"max_future_skew_secs\":");
            let _ = write!(out, "{value}");
        }
        if let Some(value) = self.min_ticket_ttl_secs {
            field(out);
            out.push_str("\"min_ticket_ttl_secs\":");
            let _ = write!(out, "{value}");
        }
        if let Some(value) = self.ticket_ttl_secs {
            field(out);
            out.push_str("\"ticket_ttl_secs\":");
            let _ = write!(out, "{value}");
        }
        if let Some(key_hex) = &self.signed_ticket_public_key_hex {
            field(out);
            out.push_str("\"signed_ticket_public_key_hex\":");
            json::write_json_string(key_hex, out);
        }
        if let Some(puzzle) = &self.puzzle {
            field(out);
            out.push_str("\"puzzle\":");
            puzzle.write_json(out);
        }
        out.push('}');
    }
}

impl FastJsonWrite for SoranetHandshakePowSummary {
    fn write_json(&self, out: &mut String) {
        out.push('{');
        out.push_str("\"required\":");
        out.push_str(if self.required { "true" } else { "false" });
        out.push(',');
        out.push_str("\"difficulty\":");
        let _ = write!(out, "{}", self.difficulty);
        out.push(',');
        out.push_str("\"max_future_skew_secs\":");
        let _ = write!(out, "{}", self.max_future_skew_secs);
        out.push(',');
        out.push_str("\"min_ticket_ttl_secs\":");
        let _ = write!(out, "{}", self.min_ticket_ttl_secs);
        out.push(',');
        out.push_str("\"ticket_ttl_secs\":");
        let _ = write!(out, "{}", self.ticket_ttl_secs);
        if let Some(key_hex) = &self.signed_ticket_public_key_hex {
            out.push(',');
            out.push_str("\"signed_ticket_public_key_hex\":");
            json::write_json_string(key_hex, out);
        }
        if let Some(puzzle) = &self.puzzle {
            out.push(',');
            out.push_str("\"puzzle\":");
            puzzle.write_json(out);
        }
        out.push('}');
    }
}

impl FastJsonWrite for NetworkAcl {
    fn write_json(&self, out: &mut String) {
        out.push('{');
        let mut first = true;
        let mut comma = |out: &mut String| {
            if first {
                first = false;
            } else {
                out.push(',');
            }
        };
        if let Some(b) = self.allowlist_only {
            comma(out);
            out.push_str("\"allowlist_only\":");
            out.push_str(if b { "true" } else { "false" });
        }
        if let Some(keys) = &self.allow_keys {
            comma(out);
            out.push_str("\"allow_keys\":[");
            for (i, k) in keys.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                out.push('"');
                let _ = write!(out, "{k}");
                out.push('"');
            }
            out.push(']');
        }
        if let Some(keys) = &self.deny_keys {
            comma(out);
            out.push_str("\"deny_keys\":[");
            for (i, k) in keys.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                out.push('"');
                let _ = write!(out, "{k}");
                out.push('"');
            }
            out.push(']');
        }
        if let Some(cidrs) = &self.allow_cidrs {
            comma(out);
            out.push_str("\"allow_cidrs\":[");
            for (i, c) in cidrs.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                out.push('"');
                out.push_str(c);
                out.push('"');
            }
            out.push(']');
        }
        if let Some(cidrs) = &self.deny_cidrs {
            comma(out);
            out.push_str("\"deny_cidrs\":[");
            for (i, c) in cidrs.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                out.push('"');
                out.push_str(c);
                out.push('"');
            }
            out.push(']');
        }
        out.push('}');
    }
}

impl FastJsonWrite for ConfidentialGas {
    fn write_json(&self, out: &mut String) {
        out.push('{');
        out.push_str("\"proof_base\":");
        let _ = write!(out, "{}", self.proof_base);
        out.push(',');
        out.push_str("\"per_public_input\":");
        let _ = write!(out, "{}", self.per_public_input);
        out.push(',');
        out.push_str("\"per_proof_byte\":");
        let _ = write!(out, "{}", self.per_proof_byte);
        out.push(',');
        out.push_str("\"per_nullifier\":");
        let _ = write!(out, "{}", self.per_nullifier);
        out.push(',');
        out.push_str("\"per_commitment\":");
        let _ = write!(out, "{}", self.per_commitment);
        out.push('}');
    }
}

impl FastJsonWrite for ConfigUpdateDTO {
    fn write_json(&self, out: &mut String) {
        out.push('{');
        // logger
        out.push_str("\"logger\":");
        self.logger.write_json(out);
        // network_acl
        if let Some(acl) = &self.network_acl {
            out.push(',');
            out.push_str("\"network_acl\":");
            acl.write_json(out);
        }
        if let Some(network) = &self.network {
            out.push(',');
            out.push_str("\"network\":");
            network.write_json(out);
        }
        if let Some(gas) = &self.confidential_gas {
            out.push(',');
            out.push_str("\"confidential_gas\":");
            gas.write_json(out);
        }
        if let Some(handshake) = &self.soranet_handshake {
            out.push(',');
            out.push_str("\"soranet_handshake\":");
            handshake.write_json(out);
        }
        if let Some(transport) = &self.transport {
            out.push(',');
            out.push_str("\"transport\":");
            transport.write_json(out);
        }
        if let Some(compute_pricing) = &self.compute_pricing {
            out.push(',');
            out.push_str("\"compute_pricing\":");
            compute_pricing.write_json(out);
        }
        out.push('}');
    }
}

/// Subset of Iroha configuration returned to clients via the API.
#[derive(Debug, Clone)]
pub struct ConfigGetDTO {
    /// Peer public key.
    pub public_key: PublicKey,
    /// Logger configuration.
    pub logger: Logger,
    /// Network-related configuration.
    pub network: Network,
    /// Queue-related configuration.
    pub queue: Queue,
    /// Consensus-related configuration summary.
    pub consensus: Consensus,
    /// Confidential verification gas schedule.
    pub confidential_gas: ConfidentialGas,
    /// Transport-specific configuration summary (Norito-RPC, streaming).
    pub transport: Transport,
    /// Nexus-specific configuration snapshot.
    pub nexus: Nexus,
}

/// Consensus configuration summary exposed via the client API.
#[derive(Debug, Clone)]
pub struct Consensus {
    /// Currently configured consensus mode.
    pub mode: String,
    /// Whether runtime mode flips are enabled.
    pub mode_flip_enabled: bool,
}

impl From<&base::Sumeragi> for Consensus {
    fn from(value: &base::Sumeragi) -> Self {
        let mode = match value.consensus_mode {
            base::ConsensusMode::Permissioned => "permissioned",
            base::ConsensusMode::Npos => "npos",
        }
        .to_string();
        Self {
            mode,
            mode_flip_enabled: value.mode_flip_enabled,
        }
    }
}

/// Client-facing confidential gas schedule parameters.
#[derive(Debug, Clone, Copy)]
pub struct ConfidentialGas {
    /// Base verify cost for a confidential proof.
    pub proof_base: u64,
    /// Cost per public input field element.
    pub per_public_input: u64,
    /// Cost per proof byte.
    pub per_proof_byte: u64,
    /// Cost per nullifier.
    pub per_nullifier: u64,
    /// Cost per commitment.
    pub per_commitment: u64,
}

/// Transport configuration summary exposed via the client API.
#[derive(Debug, Clone)]
pub struct Transport {
    /// Norito-RPC rollout summary.
    pub norito_rpc: NoritoRpcSummary,
    /// Streaming transport summary (e.g., `SoraNet` bridge defaults).
    pub streaming: StreamingTransportSummary,
}

/// Nexus configuration summary exposed via the client API.
#[derive(Debug, Clone, Copy)]
pub struct Nexus {
    /// AXT expiry/cache/replay guardrails.
    pub axt: Axt,
}

/// AXT timing/cache configuration surfaced to SDKs.
#[derive(Debug, Clone, Copy)]
pub struct Axt {
    /// Slot length used when deriving expiry slots.
    pub slot_length_ms: NonZeroU64,
    /// Maximum tolerated wall-clock skew when enforcing expiry.
    pub max_clock_skew_ms: u64,
    /// Number of slots to retain cached proofs for reuse and replay rejection.
    pub proof_cache_ttl_slots: NonZeroU64,
    /// Number of slots to retain handle usage for replay rejection across restarts/peers.
    pub replay_retention_slots: NonZeroU64,
}

/// Norito-RPC summary (enable flag, stage, allowlist metadata).
#[derive(Debug, Clone)]
pub struct NoritoRpcSummary {
    /// Master enable switch for Norito-RPC decoding.
    pub enabled: bool,
    /// Rollout stage label (`disabled`, `canary`, `ga`).
    pub stage: String,
    /// Whether ingress requires mTLS for Norito-RPC traffic.
    pub require_mtls: bool,
    /// Number of tokens currently allowlisted for canary access.
    pub canary_allowlist_size: usize,
}

/// Streaming transport summary exposed to clients.
#[derive(Debug, Clone)]
pub struct StreamingTransportSummary {
    /// Default `SoraNet` bridge metadata applied to streaming routes.
    pub soranet: SoranetStreamingSummary,
}

/// `SoraNet` streaming bridge defaults surfaced via the configuration endpoint.
#[derive(Debug, Clone)]
pub struct SoranetStreamingSummary {
    /// Whether `SoraNet` provisioning for streaming routes is enabled.
    pub enabled: bool,
    /// Label used when opening `SoraNet` circuits for Norito streaming.
    pub stream_tag: String,
    /// Exit relay multiaddr used when manifests omit explicit routing metadata.
    pub exit_multiaddr: String,
    /// Optional padding budget (milliseconds) applied to low-latency circuits.
    pub padding_budget_ms: Option<u16>,
    /// Access posture enforced by exits (`read-only` vs `authenticated`).
    pub access_kind: String,
    /// GAR category implied by the configured access posture.
    pub gar_category: String,
    /// Domain-separated salt used to derive blinded channel identifiers.
    pub channel_salt: String,
    /// Filesystem spool where privacy-route updates are staged for exit relays.
    pub provision_spool_dir: String,
    /// Segment window (inclusive) used when provisioning privacy routes.
    pub provision_window_segments: u64,
    /// Maximum number of queued privacy-route provisioning jobs.
    pub provision_queue_capacity: u64,
}

impl From<&'_ base::Root> for ConfigGetDTO {
    fn from(value: &'_ base::Root) -> Self {
        Self {
            public_key: value.common.key_pair.public_key().clone(),
            logger: (&value.logger).into(),
            network: value.into(),
            queue: (&value.queue).into(),
            consensus: Consensus::from(&value.sumeragi),
            confidential_gas: (&value.confidential.gas).into(),
            transport: Transport::from_config(&value.torii.transport, &value.streaming.soranet),
            nexus: Nexus::from(&value.nexus),
        }
    }
}

impl Transport {
    fn from_config(
        transport: &'_ base::ToriiTransport,
        soranet: &'_ base::StreamingSoranet,
    ) -> Self {
        Self {
            norito_rpc: NoritoRpcSummary::from(&transport.norito_rpc),
            streaming: StreamingTransportSummary::from(soranet),
        }
    }
}

impl From<&'_ base::Nexus> for Nexus {
    fn from(value: &'_ base::Nexus) -> Self {
        Self {
            axt: Axt::from(&value.axt),
        }
    }
}

impl From<&'_ base::NexusAxt> for Axt {
    fn from(value: &'_ base::NexusAxt) -> Self {
        Self {
            slot_length_ms: value.slot_length_ms,
            max_clock_skew_ms: value.max_clock_skew_ms,
            proof_cache_ttl_slots: value.proof_cache_ttl_slots,
            replay_retention_slots: value.replay_retention_slots,
        }
    }
}

impl From<&'_ base::NoritoRpcTransport> for NoritoRpcSummary {
    fn from(value: &'_ base::NoritoRpcTransport) -> Self {
        Self {
            enabled: value.enabled,
            stage: value.stage.label().to_string(),
            require_mtls: value.require_mtls,
            canary_allowlist_size: value.allowed_clients.len(),
        }
    }
}

impl From<&'_ base::StreamingSoranet> for StreamingTransportSummary {
    fn from(value: &'_ base::StreamingSoranet) -> Self {
        Self {
            soranet: SoranetStreamingSummary::from(value),
        }
    }
}

impl From<&'_ base::StreamingSoranet> for SoranetStreamingSummary {
    fn from(value: &'_ base::StreamingSoranet) -> Self {
        let access_kind = value.access_kind.as_str().to_string();
        let gar_category = match value.access_kind {
            base::StreamingSoranetAccessKind::Authenticated => {
                "stream.norito.authenticated".to_string()
            }
            base::StreamingSoranetAccessKind::ReadOnly => "stream.norito.read_only".to_string(),
        };
        Self {
            enabled: value.enabled,
            stream_tag: "norito-stream".to_string(),
            exit_multiaddr: value.exit_multiaddr.clone(),
            padding_budget_ms: value.padding_budget_ms,
            access_kind,
            gar_category,
            channel_salt: value.channel_salt.clone(),
            provision_spool_dir: value.provision_spool_dir.to_string_lossy().into_owned(),
            provision_window_segments: value.provision_window_segments,
            provision_queue_capacity: value.provision_queue_capacity,
        }
    }
}

impl FastJsonWrite for ConfigGetDTO {
    fn write_json(&self, out: &mut String) {
        out.push('{');
        out.push_str("\"public_key\":\"");
        let _ = write!(out, "{}", self.public_key);
        out.push('"');
        out.push(',');
        out.push_str("\"logger\":");
        self.logger.write_json(out);
        out.push(',');
        out.push_str("\"network\":");
        self.network.write_json(out);
        out.push(',');
        out.push_str("\"queue\":");
        self.queue.write_json(out);
        out.push(',');
        out.push_str("\"consensus\":");
        self.consensus.write_json(out);
        out.push(',');
        out.push_str("\"confidential_gas\":");
        self.confidential_gas.write_json(out);
        out.push(',');
        out.push_str("\"transport\":");
        self.transport.write_json(out);
        out.push(',');
        out.push_str("\"nexus\":");
        self.nexus.write_json(out);
        out.push('}');
    }
}

impl FastJsonWrite for Consensus {
    fn write_json(&self, out: &mut String) {
        out.push('{');
        out.push_str("\"mode\":\"");
        out.push_str(&self.mode);
        out.push('"');
        out.push(',');
        out.push_str("\"mode_flip_enabled\":");
        out.push_str(if self.mode_flip_enabled {
            "true"
        } else {
            "false"
        });
        out.push('}');
    }
}

impl FastJsonWrite for Transport {
    fn write_json(&self, out: &mut String) {
        out.push('{');
        out.push_str("\"norito_rpc\":");
        self.norito_rpc.write_json(out);
        out.push(',');
        out.push_str("\"streaming\":");
        self.streaming.write_json(out);
        out.push('}');
    }
}

impl FastJsonWrite for Nexus {
    fn write_json(&self, out: &mut String) {
        out.push('{');
        out.push_str("\"axt\":");
        self.axt.write_json(out);
        out.push('}');
    }
}

impl FastJsonWrite for Axt {
    fn write_json(&self, out: &mut String) {
        out.push('{');
        out.push_str("\"slot_length_ms\":");
        let _ = write!(out, "{}", self.slot_length_ms);
        out.push(',');
        out.push_str("\"max_clock_skew_ms\":");
        let _ = write!(out, "{}", self.max_clock_skew_ms);
        out.push(',');
        out.push_str("\"proof_cache_ttl_slots\":");
        let _ = write!(out, "{}", self.proof_cache_ttl_slots);
        out.push(',');
        out.push_str("\"replay_retention_slots\":");
        let _ = write!(out, "{}", self.replay_retention_slots);
        out.push('}');
    }
}

impl FastJsonWrite for NoritoRpcSummary {
    fn write_json(&self, out: &mut String) {
        out.push('{');
        out.push_str("\"enabled\":");
        out.push_str(if self.enabled { "true" } else { "false" });
        out.push(',');
        out.push_str("\"stage\":\"");
        out.push_str(&self.stage);
        out.push('"');
        out.push(',');
        out.push_str("\"require_mtls\":");
        out.push_str(if self.require_mtls { "true" } else { "false" });
        out.push(',');
        out.push_str("\"canary_allowlist_size\":");
        let _ = write!(out, "{}", self.canary_allowlist_size);
        out.push('}');
    }
}

impl FastJsonWrite for StreamingTransportSummary {
    fn write_json(&self, out: &mut String) {
        out.push('{');
        out.push_str("\"soranet\":");
        self.soranet.write_json(out);
        out.push('}');
    }
}

impl FastJsonWrite for SoranetStreamingSummary {
    fn write_json(&self, out: &mut String) {
        out.push('{');
        out.push_str("\"enabled\":");
        let _ = write!(out, "{}", self.enabled);
        out.push(',');
        out.push_str("\"stream_tag\":\"");
        out.push_str(&self.stream_tag);
        out.push('"');
        out.push(',');
        out.push_str("\"exit_multiaddr\":\"");
        out.push_str(&self.exit_multiaddr);
        out.push('"');
        out.push(',');
        out.push_str("\"padding_budget_ms\":");
        match self.padding_budget_ms {
            Some(value) => {
                let _ = write!(out, "{value}");
            }
            None => out.push_str("null"),
        }
        out.push(',');
        out.push_str("\"access_kind\":\"");
        out.push_str(&self.access_kind);
        out.push('"');
        out.push(',');
        out.push_str("\"gar_category\":\"");
        out.push_str(&self.gar_category);
        out.push('"');
        out.push(',');
        out.push_str("\"channel_salt\":\"");
        out.push_str(&self.channel_salt);
        out.push('"');
        out.push(',');
        out.push_str("\"provision_spool_dir\":\"");
        out.push_str(&self.provision_spool_dir);
        out.push('"');
        out.push(',');
        out.push_str("\"provision_window_segments\":");
        let _ = write!(out, "{}", self.provision_window_segments);
        out.push(',');
        out.push_str("\"provision_queue_capacity\":");
        let _ = write!(out, "{}", self.provision_queue_capacity);
        out.push('}');
    }
}

// ---------- FastFromJson (typed, tape-first) implementations ----------

impl<'a> FastFromJson<'a> for Logger {
    fn parse(w: &mut TapeWalker<'a>, arena: &mut Arena) -> Result<Self, NoritoError> {
        w.expect_object_start()?;
        let mut level: Option<Level> = None;
        let mut filter: Option<Option<Directives>> = None;
        let kh_level = norito::json::key_hash_const("level");
        let kh_filter = norito::json::key_hash_const("filter");
        while !w.peek_object_end()? {
            let kh = w.read_key_hash()?;
            w.expect_colon_resync()?;
            match kh {
                x if x == kh_level => {
                    if w.last_key() == "level" {
                        let sref = w.parse_string_ref_inline(arena)?;
                        let s = sref.to_string();
                        let lv = match s.as_str() {
                            "TRACE" => Level::TRACE,
                            "DEBUG" => Level::DEBUG,
                            "INFO" => Level::INFO,
                            "WARN" => Level::WARN,
                            "ERROR" => Level::ERROR,
                            _ => return Err(NoritoError::Message("invalid level".into())),
                        };
                        level = Some(lv);
                    } else {
                        w.skip_value()?;
                    }
                }
                x if x == kh_filter => {
                    if w.last_key() == "filter" {
                        // null => None; else parse string
                        w.skip_ws();
                        let bytes = w.input().as_bytes();
                        let pos = w.raw_pos();
                        if pos + 4 <= bytes.len() && &bytes[pos..pos + 4] == b"null" {
                            w.sync_to_raw(pos + 4);
                            filter = Some(None);
                        } else {
                            let sref = w.parse_string_ref_inline(arena)?;
                            let s = sref.to_string();
                            let d = Directives::from_str(&s)
                                .map_err(|_| NoritoError::Message("invalid directives".into()))?;
                            filter = Some(Some(d));
                        }
                    } else {
                        w.skip_value()?;
                    }
                }
                _ => w.skip_value()?,
            }
            let _ = w.consume_comma_if_present()?;
        }
        w.expect_object_end()?;
        Ok(Logger {
            level: level.ok_or_else(|| NoritoError::Message("missing level".into()))?,
            filter: filter.unwrap_or(None),
        })
    }
}

impl<'a> FastFromJson<'a> for Transport {
    fn parse(w: &mut TapeWalker<'a>, arena: &mut Arena) -> Result<Self, NoritoError> {
        w.expect_object_start()?;
        let kh_nr = norito::json::key_hash_const("norito_rpc");
        let kh_streaming = norito::json::key_hash_const("streaming");
        let mut norito_rpc: Option<NoritoRpcSummary> = None;
        let mut streaming: Option<StreamingTransportSummary> = None;
        while !w.peek_object_end()? {
            let kh = w.read_key_hash()?;
            w.expect_colon_resync()?;
            match kh {
                x if x == kh_nr && w.last_key() == "norito_rpc" => {
                    norito_rpc = Some(NoritoRpcSummary::parse(w, arena)?);
                }
                x if x == kh_streaming && w.last_key() == "streaming" => {
                    streaming = Some(StreamingTransportSummary::parse(w, arena)?);
                }
                _ => w.skip_value()?,
            }
            let _ = w.consume_comma_if_present()?;
        }
        w.expect_object_end()?;
        Ok(Self {
            norito_rpc: norito_rpc
                .ok_or_else(|| NoritoError::Message("missing norito_rpc".into()))?,
            streaming: streaming.ok_or_else(|| NoritoError::Message("missing streaming".into()))?,
        })
    }
}

impl<'a> FastFromJson<'a> for NoritoRpcSummary {
    fn parse(w: &mut TapeWalker<'a>, arena: &mut Arena) -> Result<Self, NoritoError> {
        w.expect_object_start()?;
        let kh_enabled = norito::json::key_hash_const("enabled");
        let kh_stage = norito::json::key_hash_const("stage");
        let kh_mtls = norito::json::key_hash_const("require_mtls");
        let kh_allow = norito::json::key_hash_const("canary_allowlist_size");
        let mut enabled = None;
        let mut stage = None;
        let mut require_mtls = None;
        let mut allow_size = None;
        while !w.peek_object_end()? {
            let kh = w.read_key_hash()?;
            w.expect_colon_resync()?;
            match kh {
                x if x == kh_enabled && w.last_key() == "enabled" => {
                    enabled = Some(w.parse_bool_inline()?);
                }
                x if x == kh_stage && w.last_key() == "stage" => {
                    let sref = w.parse_string_ref_inline(arena)?;
                    stage = Some(sref.to_string());
                }
                x if x == kh_mtls && w.last_key() == "require_mtls" => {
                    require_mtls = Some(w.parse_bool_inline()?);
                }
                x if x == kh_allow && w.last_key() == "canary_allowlist_size" => {
                    let raw = w.parse_u64_inline()?;
                    let converted = usize::try_from(raw).map_err(|_| {
                        NoritoError::Message(
                            "canary_allowlist_size exceeds platform usize bounds".into(),
                        )
                    })?;
                    allow_size = Some(converted);
                }
                _ => w.skip_value()?,
            }
            let _ = w.consume_comma_if_present()?;
        }
        w.expect_object_end()?;
        Ok(Self {
            enabled: enabled.ok_or_else(|| NoritoError::Message("missing enabled".into()))?,
            stage: stage.ok_or_else(|| NoritoError::Message("missing stage".into()))?,
            require_mtls: require_mtls.unwrap_or(false),
            canary_allowlist_size: allow_size.unwrap_or(0),
        })
    }
}

impl<'a> FastFromJson<'a> for StreamingTransportSummary {
    fn parse(w: &mut TapeWalker<'a>, arena: &mut Arena) -> Result<Self, NoritoError> {
        w.expect_object_start()?;
        let kh_soranet = norito::json::key_hash_const("soranet");
        let mut soranet: Option<SoranetStreamingSummary> = None;
        while !w.peek_object_end()? {
            let kh = w.read_key_hash()?;
            w.expect_colon_resync()?;
            match kh {
                x if x == kh_soranet && w.last_key() == "soranet" => {
                    soranet = Some(SoranetStreamingSummary::parse(w, arena)?);
                }
                _ => w.skip_value()?,
            }
            let _ = w.consume_comma_if_present()?;
        }
        w.expect_object_end()?;
        Ok(Self {
            soranet: soranet.ok_or_else(|| NoritoError::Message("missing soranet".into()))?,
        })
    }
}

impl<'a> FastFromJson<'a> for SoranetStreamingSummary {
    fn parse(w: &mut TapeWalker<'a>, arena: &mut Arena) -> Result<Self, NoritoError> {
        w.expect_object_start()?;
        let kh_enabled = norito::json::key_hash_const("enabled");
        let kh_stream_tag = norito::json::key_hash_const("stream_tag");
        let kh_exit = norito::json::key_hash_const("exit_multiaddr");
        let kh_padding = norito::json::key_hash_const("padding_budget_ms");
        let kh_access = norito::json::key_hash_const("access_kind");
        let kh_gar = norito::json::key_hash_const("gar_category");
        let kh_salt = norito::json::key_hash_const("channel_salt");
        let kh_spool = norito::json::key_hash_const("provision_spool_dir");
        let kh_window = norito::json::key_hash_const("provision_window_segments");
        let kh_queue = norito::json::key_hash_const("provision_queue_capacity");
        let mut enabled: Option<bool> = None;
        let mut stream_tag: Option<String> = None;
        let mut exit_multiaddr: Option<String> = None;
        let mut padding_budget_ms: Option<Option<u16>> = None;
        let mut access_kind: Option<String> = None;
        let mut gar_category: Option<String> = None;
        let mut channel_salt: Option<String> = None;
        let mut provision_spool_dir: Option<String> = None;
        let mut provision_window_segments: Option<u64> = None;
        let mut provision_queue_capacity: Option<u64> = None;
        while !w.peek_object_end()? {
            let kh = w.read_key_hash()?;
            w.expect_colon_resync()?;
            match kh {
                x if x == kh_enabled && w.last_key() == "enabled" => {
                    enabled = Some(w.parse_bool_inline()?);
                }
                x if x == kh_stream_tag && w.last_key() == "stream_tag" => {
                    let sref = w.parse_string_ref_inline(arena)?;
                    stream_tag = Some(sref.to_string());
                }
                x if x == kh_exit && w.last_key() == "exit_multiaddr" => {
                    let sref = w.parse_string_ref_inline(arena)?;
                    exit_multiaddr = Some(sref.to_string());
                }
                x if x == kh_padding && w.last_key() == "padding_budget_ms" => {
                    w.skip_ws();
                    let bytes = w.input().as_bytes();
                    let cursor = w.raw_pos();
                    if cursor + 4 <= bytes.len() && &bytes[cursor..cursor + 4] == b"null" {
                        w.sync_to_raw(cursor + 4);
                        padding_budget_ms = Some(None);
                    } else {
                        let raw = w.parse_u64_inline()?;
                        let converted = u16::try_from(raw).map_err(|_| {
                            NoritoError::Message("padding_budget_ms exceeds u16".into())
                        })?;
                        padding_budget_ms = Some(Some(converted));
                    }
                }
                x if x == kh_access && w.last_key() == "access_kind" => {
                    let sref = w.parse_string_ref_inline(arena)?;
                    access_kind = Some(sref.to_string());
                }
                x if x == kh_gar && w.last_key() == "gar_category" => {
                    let sref = w.parse_string_ref_inline(arena)?;
                    gar_category = Some(sref.to_string());
                }
                x if x == kh_salt && w.last_key() == "channel_salt" => {
                    let sref = w.parse_string_ref_inline(arena)?;
                    channel_salt = Some(sref.to_string());
                }
                x if x == kh_spool && w.last_key() == "provision_spool_dir" => {
                    let sref = w.parse_string_ref_inline(arena)?;
                    provision_spool_dir = Some(sref.to_string());
                }
                x if x == kh_window && w.last_key() == "provision_window_segments" => {
                    provision_window_segments = Some(w.parse_u64_inline()?);
                }
                x if x == kh_queue && w.last_key() == "provision_queue_capacity" => {
                    provision_queue_capacity = Some(w.parse_u64_inline()?);
                }
                _ => w.skip_value()?,
            }
            let _ = w.consume_comma_if_present()?;
        }
        w.expect_object_end()?;
        Ok(Self {
            enabled: enabled.ok_or_else(|| NoritoError::Message("missing enabled".into()))?,
            stream_tag: stream_tag
                .ok_or_else(|| NoritoError::Message("missing stream_tag".into()))?,
            exit_multiaddr: exit_multiaddr
                .ok_or_else(|| NoritoError::Message("missing exit_multiaddr".into()))?,
            padding_budget_ms: padding_budget_ms
                .ok_or_else(|| NoritoError::Message("missing padding_budget_ms".into()))?,
            access_kind: access_kind
                .ok_or_else(|| NoritoError::Message("missing access_kind".into()))?,
            gar_category: gar_category
                .ok_or_else(|| NoritoError::Message("missing gar_category".into()))?,
            channel_salt: channel_salt
                .ok_or_else(|| NoritoError::Message("missing channel_salt".into()))?,
            provision_spool_dir: provision_spool_dir
                .ok_or_else(|| NoritoError::Message("missing provision_spool_dir".into()))?,
            provision_window_segments: provision_window_segments.ok_or_else(|| {
                NoritoError::Message("missing provision_window_segments".into())
            })?,
            provision_queue_capacity: provision_queue_capacity.ok_or_else(|| {
                NoritoError::Message("missing provision_queue_capacity".into())
            })?,
        })
    }
}

impl JsonDeserialize for Logger {
    fn json_deserialize(parser: &mut json::Parser<'_>) -> Result<Self, json::Error> {
        parse_via_fast(parser)
    }
}

impl<'a> FastFromJson<'a> for Queue {
    fn parse(w: &mut TapeWalker<'a>, _arena: &mut Arena) -> Result<Self, NoritoError> {
        w.expect_object_start()?;
        let mut capacity: Option<NonZero<usize>> = None;
        let capacity_key = norito::json::key_hash_const("capacity");
        while !w.peek_object_end()? {
            let kh = w.read_key_hash()?;
            w.expect_colon_resync()?;
            match kh {
                key if key == capacity_key && w.last_key() == "capacity" => {
                    let value = usize::try_from(w.parse_u64_inline()?)
                        .map_err(|_| NoritoError::Message("usize overflow".into()))?;
                    capacity = NonZero::new(value);
                }
                _ => w.skip_value()?,
            }
            let _ = w.consume_comma_if_present()?;
        }
        w.expect_object_end()?;
        Ok(Queue {
            capacity: capacity.ok_or_else(|| NoritoError::Message("missing capacity".into()))?,
        })
    }
}

impl JsonDeserialize for Queue {
    fn json_deserialize(parser: &mut json::Parser<'_>) -> Result<Self, json::Error> {
        parse_via_fast(parser)
    }
}

#[allow(clippy::too_many_arguments)]
fn finalize_network(
    block_gossip_size: Option<NonZero<u32>>,
    block_gossip_period_ms: Option<u32>,
    peer_gossip_period_ms: Option<u32>,
    trust_decay_half_life_ms: Option<u32>,
    trust_penalty_bad_gossip: Option<i32>,
    trust_penalty_unknown_peer: Option<i32>,
    trust_min_score: Option<i32>,
    trust_gossip: Option<bool>,
    relay_ttl: Option<u8>,
    tx_gossip_size: Option<NonZero<u32>>,
    tx_gossip_period_ms: Option<u32>,
    tx_gossip_resend_ticks: Option<NonZero<u32>>,
    soranet_handshake: Option<SoranetHandshakeSummary>,
    soranet_privacy_summary: Option<SoranetPrivacySummary>,
    soranet_vpn_summary: Option<SoranetVpnSummary>,
    lane_profile: Option<base::LaneProfile>,
    require_sm_handshake_match: Option<bool>,
    require_sm_openssl_preview_match: Option<bool>,
) -> Result<Network, NoritoError> {
    Ok(Network {
        block_gossip_size: block_gossip_size
            .ok_or_else(|| NoritoError::Message("missing block_gossip_size".into()))?,
        block_gossip_period_ms: block_gossip_period_ms
            .ok_or_else(|| NoritoError::Message("missing block_gossip_period_ms".into()))?,
        peer_gossip_period_ms: peer_gossip_period_ms.unwrap_or_else(|| {
            u32::try_from(defaults::network::PEER_GOSSIP_PERIOD.as_millis())
                .expect("default peer gossip period must fit in u32")
        }),
        trust_decay_half_life_ms: trust_decay_half_life_ms.unwrap_or_else(|| {
            u32::try_from(defaults::network::TRUST_DECAY_HALF_LIFE.as_millis())
                .expect("default trust decay half-life must fit in u32")
        }),
        trust_penalty_bad_gossip: trust_penalty_bad_gossip
            .unwrap_or(defaults::network::TRUST_PENALTY_BAD_GOSSIP),
        trust_penalty_unknown_peer: trust_penalty_unknown_peer
            .unwrap_or(defaults::network::TRUST_PENALTY_UNKNOWN_PEER),
        trust_min_score: trust_min_score.unwrap_or(defaults::network::TRUST_MIN_SCORE),
        trust_gossip: trust_gossip.unwrap_or(defaults::network::TRUST_GOSSIP),
        relay_ttl: relay_ttl.unwrap_or(defaults::network::RELAY_TTL),
        transaction_gossip_size: tx_gossip_size
            .ok_or_else(|| NoritoError::Message("missing transaction_gossip_size".into()))?,
        transaction_gossip_period_ms: tx_gossip_period_ms
            .ok_or_else(|| NoritoError::Message("missing transaction_gossip_period_ms".into()))?,
        transaction_gossip_resend_ticks: tx_gossip_resend_ticks.ok_or_else(|| {
            NoritoError::Message("missing transaction_gossip_resend_ticks".into())
        })?,
        soranet_handshake: soranet_handshake.unwrap_or_default(),
        soranet_privacy: soranet_privacy_summary.unwrap_or_default(),
        soranet_vpn: soranet_vpn_summary.unwrap_or_default(),
        lane_profile: lane_profile.unwrap_or(base::LaneProfile::Core),
        require_sm_handshake_match: require_sm_handshake_match
            .unwrap_or(defaults::network::REQUIRE_SM_HANDSHAKE_MATCH),
        require_sm_openssl_preview_match: require_sm_openssl_preview_match
            .unwrap_or(defaults::network::REQUIRE_SM_OPENSSL_PREVIEW_MATCH),
    })
}

impl<'a> FastFromJson<'a> for Network {
    fn parse(w: &mut TapeWalker<'a>, arena: &mut Arena) -> Result<Self, NoritoError> {
        w.expect_object_start()?;
        let mut block_gossip_size: Option<NonZero<u32>> = None;
        let mut block_gossip_period_ms: Option<u32> = None;
        let mut peer_gossip_period_ms: Option<u32> = None;
        let mut trust_decay_half_life_ms: Option<u32> = None;
        let mut trust_penalty_bad_gossip: Option<i32> = None;
        let mut trust_penalty_unknown_peer: Option<i32> = None;
        let mut trust_min_score: Option<i32> = None;
        let mut trust_gossip: Option<bool> = None;
        let mut relay_ttl: Option<u8> = None;
        let mut tx_gossip_size: Option<NonZero<u32>> = None;
        let mut tx_gossip_period_ms: Option<u32> = None;
        let mut tx_gossip_resend_ticks: Option<NonZero<u32>> = None;
        let mut soranet_handshake: Option<SoranetHandshakeSummary> = None;
        let mut soranet_privacy_summary: Option<SoranetPrivacySummary> = None;
        let mut soranet_vpn_summary: Option<SoranetVpnSummary> = None;
        let mut lane_profile: Option<base::LaneProfile> = None;
        let mut require_sm_handshake_match: Option<bool> = None;
        let mut require_sm_openssl_preview_match: Option<bool> = None;
        let kh_block_gossip_size = norito::json::key_hash_const("block_gossip_size");
        let kh_block_gossip_period = norito::json::key_hash_const("block_gossip_period_ms");
        let kh_peer_gossip_period = norito::json::key_hash_const("peer_gossip_period_ms");
        let kh_trust_decay_half_life = norito::json::key_hash_const("trust_decay_half_life_ms");
        let kh_trust_penalty = norito::json::key_hash_const("trust_penalty_bad_gossip");
        let kh_trust_penalty_unknown = norito::json::key_hash_const("trust_penalty_unknown_peer");
        let kh_trust_min_score = norito::json::key_hash_const("trust_min_score");
        let kh_trust_gossip = norito::json::key_hash_const("trust_gossip");
        let kh_relay_ttl = norito::json::key_hash_const("relay_ttl");
        let kh_tx_gossip_size = norito::json::key_hash_const("transaction_gossip_size");
        let kh_tx_gossip_period = norito::json::key_hash_const("transaction_gossip_period_ms");
        let kh_tx_gossip_resend_ticks =
            norito::json::key_hash_const("transaction_gossip_resend_ticks");
        let kh_handshake = norito::json::key_hash_const("soranet_handshake");
        let kh_privacy = norito::json::key_hash_const("soranet_privacy");
        let kh_vpn = norito::json::key_hash_const("soranet_vpn");
        let kh_lane_profile = norito::json::key_hash_const("lane_profile");
        let kh_require_sm_match = norito::json::key_hash_const("require_sm_handshake_match");
        let kh_require_sm_openssl_match =
            norito::json::key_hash_const("require_sm_openssl_preview_match");
        while !w.peek_object_end()? {
            let kh = w.read_key_hash()?;
            w.expect_colon_resync()?;
            match kh {
                x if x == kh_block_gossip_size && w.last_key() == "block_gossip_size" => {
                    block_gossip_size = NonZeroU32::new(
                        u32::try_from(w.parse_u64_inline()?)
                            .map_err(|_| NoritoError::Message("u32 overflow".into()))?,
                    );
                }
                x if x == kh_block_gossip_period && w.last_key() == "block_gossip_period_ms" => {
                    block_gossip_period_ms = Some(
                        u32::try_from(w.parse_u64_inline()?)
                            .map_err(|_| NoritoError::Message("u32 overflow".into()))?,
                    );
                }
                x if x == kh_peer_gossip_period && w.last_key() == "peer_gossip_period_ms" => {
                    peer_gossip_period_ms = Some(
                        u32::try_from(w.parse_u64_inline()?)
                            .map_err(|_| NoritoError::Message("u32 overflow".into()))?,
                    );
                }
                x if x == kh_trust_decay_half_life
                    && w.last_key() == "trust_decay_half_life_ms" =>
                {
                    trust_decay_half_life_ms = Some(
                        u32::try_from(w.parse_u64_inline()?)
                            .map_err(|_| NoritoError::Message("u32 overflow".into()))?,
                    );
                }
                x if x == kh_trust_penalty && w.last_key() == "trust_penalty_bad_gossip" => {
                    trust_penalty_bad_gossip = Some(
                        i32::try_from(w.parse_i64_inline()?)
                            .map_err(|_| NoritoError::Message("i32 overflow".into()))?,
                    );
                }
                x if x == kh_trust_penalty_unknown
                    && w.last_key() == "trust_penalty_unknown_peer" =>
                {
                    trust_penalty_unknown_peer = Some(
                        i32::try_from(w.parse_i64_inline()?)
                            .map_err(|_| NoritoError::Message("i32 overflow".into()))?,
                    );
                }
                x if x == kh_trust_min_score && w.last_key() == "trust_min_score" => {
                    trust_min_score = Some(
                        i32::try_from(w.parse_i64_inline()?)
                            .map_err(|_| NoritoError::Message("i32 overflow".into()))?,
                    );
                }
                x if x == kh_trust_gossip && w.last_key() == "trust_gossip" => {
                    trust_gossip = Some(w.parse_bool_inline()?);
                }
                x if x == kh_relay_ttl && w.last_key() == "relay_ttl" => {
                    relay_ttl = Some(
                        u8::try_from(w.parse_u64_inline()?)
                            .map_err(|_| NoritoError::Message("u8 overflow".into()))?,
                    );
                }
                x if x == kh_tx_gossip_size && w.last_key() == "transaction_gossip_size" => {
                    tx_gossip_size = NonZeroU32::new(
                        u32::try_from(w.parse_u64_inline()?)
                            .map_err(|_| NoritoError::Message("u32 overflow".into()))?,
                    );
                }
                x if x == kh_tx_gossip_period && w.last_key() == "transaction_gossip_period_ms" => {
                    tx_gossip_period_ms = Some(
                        u32::try_from(w.parse_u64_inline()?)
                            .map_err(|_| NoritoError::Message("u32 overflow".into()))?,
                    );
                }
                x if x == kh_tx_gossip_resend_ticks
                    && w.last_key() == "transaction_gossip_resend_ticks" =>
                {
                    tx_gossip_resend_ticks = NonZeroU32::new(
                        u32::try_from(w.parse_u64_inline()?)
                            .map_err(|_| NoritoError::Message("u32 overflow".into()))?,
                    );
                }
                x if x == kh_handshake && w.last_key() == "soranet_handshake" => {
                    soranet_handshake = Some(SoranetHandshakeSummary::parse(w, arena)?);
                }
                x if x == kh_privacy && w.last_key() == "soranet_privacy" => {
                    soranet_privacy_summary = Some(SoranetPrivacySummary::parse(w, arena)?);
                }
                x if x == kh_vpn && w.last_key() == "soranet_vpn" => {
                    soranet_vpn_summary = Some(SoranetVpnSummary::parse(w, arena)?);
                }
                x if x == kh_lane_profile && w.last_key() == "lane_profile" => {
                    let label = w.parse_string_ref_inline(arena)?.to_string();
                    lane_profile = Some(base::LaneProfile::from_label(&label));
                }
                x if x == kh_require_sm_match && w.last_key() == "require_sm_handshake_match" => {
                    require_sm_handshake_match = Some(w.parse_bool_inline()?);
                }
                x if x == kh_require_sm_openssl_match
                    && w.last_key() == "require_sm_openssl_preview_match" =>
                {
                    require_sm_openssl_preview_match = Some(w.parse_bool_inline()?);
                }
                _ => w.skip_value()?,
            }
            let _ = w.consume_comma_if_present()?;
        }
        w.expect_object_end()?;
        finalize_network(
            block_gossip_size,
            block_gossip_period_ms,
            peer_gossip_period_ms,
            trust_decay_half_life_ms,
            trust_penalty_bad_gossip,
            trust_penalty_unknown_peer,
            trust_min_score,
            trust_gossip,
            relay_ttl,
            tx_gossip_size,
            tx_gossip_period_ms,
            tx_gossip_resend_ticks,
            soranet_handshake,
            soranet_privacy_summary,
            soranet_vpn_summary,
            lane_profile,
            require_sm_handshake_match,
            require_sm_openssl_preview_match,
        )
    }
}

impl JsonDeserialize for Network {
    fn json_deserialize(parser: &mut json::Parser<'_>) -> Result<Self, json::Error> {
        parse_via_fast(parser)
    }
}

impl<'a> FastFromJson<'a> for SoranetPrivacySummary {
    fn parse(w: &mut TapeWalker<'a>, _arena: &mut Arena) -> Result<Self, NoritoError> {
        w.expect_object_start()?;
        let mut bucket_secs = None;
        let mut min_handshakes = None;
        let mut flush_delay_buckets = None;
        let mut force_flush_buckets = None;
        let mut max_completed_buckets = None;
        let mut max_share_lag_buckets = None;
        let mut expected_shares = None;
        let mut event_buffer_capacity = None;
        let kh_bucket = norito::json::key_hash_const("bucket_secs");
        let kh_min = norito::json::key_hash_const("min_handshakes");
        let kh_delay = norito::json::key_hash_const("flush_delay_buckets");
        let kh_force = norito::json::key_hash_const("force_flush_buckets");
        let kh_max = norito::json::key_hash_const("max_completed_buckets");
        let kh_share_lag = norito::json::key_hash_const("max_share_lag_buckets");
        let kh_shares = norito::json::key_hash_const("expected_shares");
        let kh_capacity = norito::json::key_hash_const("event_buffer_capacity");
        while !w.peek_object_end()? {
            let kh = w.read_key_hash()?;
            w.expect_colon_resync()?;
            match kh {
                x if x == kh_bucket && w.last_key() == "bucket_secs" => {
                    bucket_secs = Some(w.parse_u64_inline()?);
                }
                x if x == kh_min && w.last_key() == "min_handshakes" => {
                    min_handshakes = Some(w.parse_u64_inline()?);
                }
                x if x == kh_delay && w.last_key() == "flush_delay_buckets" => {
                    flush_delay_buckets = Some(w.parse_u64_inline()?);
                }
                x if x == kh_force && w.last_key() == "force_flush_buckets" => {
                    force_flush_buckets = Some(w.parse_u64_inline()?);
                }
                x if x == kh_max && w.last_key() == "max_completed_buckets" => {
                    max_completed_buckets = Some(
                        usize::try_from(w.parse_u64_inline()?)
                            .map_err(|_| NoritoError::Message("usize overflow".into()))?,
                    );
                }
                x if x == kh_share_lag && w.last_key() == "max_share_lag_buckets" => {
                    max_share_lag_buckets = Some(w.parse_u64_inline()?);
                }
                x if x == kh_shares && w.last_key() == "expected_shares" => {
                    expected_shares = Some(
                        u16::try_from(w.parse_u64_inline()?)
                            .map_err(|_| NoritoError::Message("u16 overflow".into()))?,
                    );
                }
                x if x == kh_capacity && w.last_key() == "event_buffer_capacity" => {
                    event_buffer_capacity = Some(
                        usize::try_from(w.parse_u64_inline()?)
                            .map_err(|_| NoritoError::Message("usize overflow".into()))?,
                    );
                }
                _ => w.skip_value()?,
            }
            let _ = w.consume_comma_if_present()?;
        }
        w.expect_object_end()?;
        Ok(Self {
            bucket_secs: bucket_secs.unwrap_or_default(),
            min_handshakes: min_handshakes.unwrap_or_default(),
            flush_delay_buckets: flush_delay_buckets.unwrap_or_default(),
            force_flush_buckets: force_flush_buckets.unwrap_or_default(),
            max_completed_buckets: max_completed_buckets.unwrap_or_default(),
            max_share_lag_buckets: max_share_lag_buckets.unwrap_or_default(),
            expected_shares: expected_shares.unwrap_or_default(),
            event_buffer_capacity: event_buffer_capacity.unwrap_or_default(),
        })
    }
}

impl<'a> FastFromJson<'a> for SoranetVpnSummary {
    fn parse(w: &mut TapeWalker<'a>, arena: &mut Arena) -> Result<Self, NoritoError> {
        w.expect_object_start()?;
        let mut enabled = None;
        let mut cell_size_bytes = None;
        let mut flow_label_bits = None;
        let mut cover_to_data_per_mille = None;
        let mut max_cover_burst = None;
        let mut heartbeat_ms = None;
        let mut jitter_ms = None;
        let mut padding_budget_ms = None;
        let mut guard_refresh_secs = None;
        let mut lease_secs = None;
        let mut dns_push_interval_secs = None;
        let mut exit_class = None;
        let mut meter_family = None;

        let kh_enabled = norito::json::key_hash_const("enabled");
        let kh_cell = norito::json::key_hash_const("cell_size_bytes");
        let kh_flow = norito::json::key_hash_const("flow_label_bits");
        let kh_cover = norito::json::key_hash_const("cover_to_data_per_mille");
        let kh_burst = norito::json::key_hash_const("max_cover_burst");
        let kh_heartbeat = norito::json::key_hash_const("heartbeat_ms");
        let kh_jitter = norito::json::key_hash_const("jitter_ms");
        let kh_padding = norito::json::key_hash_const("padding_budget_ms");
        let kh_guard = norito::json::key_hash_const("guard_refresh_secs");
        let kh_lease = norito::json::key_hash_const("lease_secs");
        let kh_dns = norito::json::key_hash_const("dns_push_interval_secs");
        let kh_exit_class = norito::json::key_hash_const("exit_class");
        let kh_meter_family = norito::json::key_hash_const("meter_family");

        while !w.peek_object_end()? {
            let kh = w.read_key_hash()?;
            w.expect_colon_resync()?;
            match kh {
                x if x == kh_enabled && w.last_key() == "enabled" => {
                    enabled = Some(w.parse_bool_inline()?);
                }
                x if x == kh_cell && w.last_key() == "cell_size_bytes" => {
                    cell_size_bytes = Some(
                        u16::try_from(w.parse_u64_inline()?)
                            .map_err(|_| NoritoError::Message("u16 overflow".into()))?,
                    );
                }
                x if x == kh_flow && w.last_key() == "flow_label_bits" => {
                    flow_label_bits = Some(
                        u8::try_from(w.parse_u64_inline()?)
                            .map_err(|_| NoritoError::Message("u8 overflow".into()))?,
                    );
                }
                x if x == kh_cover && w.last_key() == "cover_to_data_per_mille" => {
                    cover_to_data_per_mille = Some(
                        u16::try_from(w.parse_u64_inline()?)
                            .map_err(|_| NoritoError::Message("u16 overflow".into()))?,
                    );
                }
                x if x == kh_burst && w.last_key() == "max_cover_burst" => {
                    max_cover_burst = Some(
                        u16::try_from(w.parse_u64_inline()?)
                            .map_err(|_| NoritoError::Message("u16 overflow".into()))?,
                    );
                }
                x if x == kh_heartbeat && w.last_key() == "heartbeat_ms" => {
                    heartbeat_ms = Some(
                        u16::try_from(w.parse_u64_inline()?)
                            .map_err(|_| NoritoError::Message("u16 overflow".into()))?,
                    );
                }
                x if x == kh_jitter && w.last_key() == "jitter_ms" => {
                    jitter_ms = Some(
                        u16::try_from(w.parse_u64_inline()?)
                            .map_err(|_| NoritoError::Message("u16 overflow".into()))?,
                    );
                }
                x if x == kh_padding && w.last_key() == "padding_budget_ms" => {
                    padding_budget_ms = Some(
                        u16::try_from(w.parse_u64_inline()?)
                            .map_err(|_| NoritoError::Message("u16 overflow".into()))?,
                    );
                }
                x if x == kh_guard && w.last_key() == "guard_refresh_secs" => {
                    guard_refresh_secs = Some(w.parse_u64_inline()?);
                }
                x if x == kh_lease && w.last_key() == "lease_secs" => {
                    lease_secs = Some(w.parse_u64_inline()?);
                }
                x if x == kh_dns && w.last_key() == "dns_push_interval_secs" => {
                    dns_push_interval_secs = Some(w.parse_u64_inline()?);
                }
                x if x == kh_exit_class && w.last_key() == "exit_class" => {
                    exit_class = Some(w.parse_string_ref_inline(arena)?.to_string());
                }
                x if x == kh_meter_family && w.last_key() == "meter_family" => {
                    meter_family = Some(w.parse_string_ref_inline(arena)?.to_string());
                }
                _ => w.skip_value()?,
            }
            let _ = w.consume_comma_if_present()?;
        }
        w.expect_object_end()?;

        let flow_label_bits = flow_label_bits.unwrap_or(defaults::soranet::vpn::FLOW_LABEL_BITS);
        if let Err(err) =
            iroha_data_model::soranet::vpn::VpnFlowLabelV1::max_value_for_bits(flow_label_bits)
        {
            return Err(NoritoError::Message(err.to_string()));
        }

        let lease_secs = lease_secs.unwrap_or(defaults::soranet::vpn::lease_secs_u64());
        let lease_secs = u32::try_from(lease_secs)
            .map_err(|_| NoritoError::Message("lease_secs exceeds u32::MAX".into()))?;
        let exit_class =
            exit_class.unwrap_or_else(|| defaults::soranet::vpn::EXIT_CLASS.to_string());
        let exit_class = VpnExitClassV1::try_from_label(&exit_class)
            .map_err(|err| NoritoError::Message(err.to_string()))?
            .as_label()
            .to_string();

        Ok(SoranetVpnSummary {
            enabled: enabled.unwrap_or(defaults::soranet::vpn::ENABLED),
            cell_size_bytes: cell_size_bytes.unwrap_or(defaults::soranet::vpn::CELL_SIZE_BYTES),
            flow_label_bits,
            cover_to_data_per_mille: cover_to_data_per_mille
                .unwrap_or(defaults::soranet::vpn::COVER_TO_DATA_PER_MILLE),
            max_cover_burst: max_cover_burst.unwrap_or(defaults::soranet::vpn::MAX_COVER_BURST),
            heartbeat_ms: heartbeat_ms.unwrap_or(defaults::soranet::vpn::HEARTBEAT_MS),
            jitter_ms: jitter_ms.unwrap_or(defaults::soranet::vpn::JITTER_MS),
            padding_budget_ms: padding_budget_ms
                .unwrap_or(defaults::soranet::vpn::PADDING_BUDGET_MS),
            guard_refresh_secs: guard_refresh_secs
                .unwrap_or(defaults::soranet::vpn::guard_refresh_secs_u64()),
            lease_secs: u64::from(lease_secs),
            dns_push_interval_secs: dns_push_interval_secs
                .unwrap_or(defaults::soranet::vpn::dns_push_interval_secs_u64()),
            exit_class,
            meter_family: meter_family
                .unwrap_or_else(|| defaults::soranet::vpn::METER_FAMILY.to_string()),
        })
    }
}

impl<'a> FastFromJson<'a> for NetworkAcl {
    fn parse(w: &mut TapeWalker<'a>, _arena: &mut Arena) -> Result<Self, NoritoError> {
        w.expect_object_start()?;
        let mut allowlist_only: Option<bool> = None;
        let mut allow_keys: Option<Vec<iroha_crypto::PublicKey>> = None;
        let mut deny_keys: Option<Vec<iroha_crypto::PublicKey>> = None;
        let mut allow_cidrs: Option<Vec<String>> = None;
        let mut deny_cidrs: Option<Vec<String>> = None;
        let key_allowlist_only = norito::json::key_hash_const("allowlist_only");
        let key_allow_keys = norito::json::key_hash_const("allow_keys");
        let key_deny_keys = norito::json::key_hash_const("deny_keys");
        let key_allow_cidrs = norito::json::key_hash_const("allow_cidrs");
        let key_deny_cidrs = norito::json::key_hash_const("deny_cidrs");
        while !w.peek_object_end()? {
            let kh = w.read_key_hash()?;
            w.expect_colon_resync()?;
            match kh {
                x if x == key_allowlist_only && w.last_key() == "allowlist_only" => {
                    allowlist_only = Some(w.parse_bool_inline()?);
                }
                x if x == key_allow_keys && w.last_key() == "allow_keys" => {
                    let s_in = w.input();
                    let mut p = norito::json::Parser::new_at(s_in, w.raw_pos());
                    let arr: Vec<String> = norito::json::JsonDeserialize::json_deserialize(&mut p)?;
                    w.sync_to_raw(p.position());
                    let mut parsed = Vec::with_capacity(arr.len());
                    for s in arr {
                        let pk = iroha_crypto::PublicKey::from_str(&s)
                            .map_err(|_| NoritoError::Message("invalid public key".into()))?;
                        parsed.push(pk);
                    }
                    allow_keys = Some(parsed);
                }
                x if x == key_deny_keys && w.last_key() == "deny_keys" => {
                    let s_in = w.input();
                    let mut p = norito::json::Parser::new_at(s_in, w.raw_pos());
                    let arr: Vec<String> = norito::json::JsonDeserialize::json_deserialize(&mut p)?;
                    w.sync_to_raw(p.position());
                    let mut parsed = Vec::with_capacity(arr.len());
                    for s in arr {
                        let pk = iroha_crypto::PublicKey::from_str(&s)
                            .map_err(|_| NoritoError::Message("invalid public key".into()))?;
                        parsed.push(pk);
                    }
                    deny_keys = Some(parsed);
                }
                x if x == key_allow_cidrs && w.last_key() == "allow_cidrs" => {
                    let s_in = w.input();
                    let mut p = norito::json::Parser::new_at(s_in, w.raw_pos());
                    let arr: Vec<String> = norito::json::JsonDeserialize::json_deserialize(&mut p)?;
                    w.sync_to_raw(p.position());
                    allow_cidrs = Some(arr);
                }
                x if x == key_deny_cidrs && w.last_key() == "deny_cidrs" => {
                    let s_in = w.input();
                    let mut p = norito::json::Parser::new_at(s_in, w.raw_pos());
                    let arr: Vec<String> = norito::json::JsonDeserialize::json_deserialize(&mut p)?;
                    w.sync_to_raw(p.position());
                    deny_cidrs = Some(arr);
                }
                _ => w.skip_value()?,
            }
            let _ = w.consume_comma_if_present()?;
        }
        w.expect_object_end()?;
        Ok(Self {
            allowlist_only,
            allow_keys,
            deny_keys,
            allow_cidrs,
            deny_cidrs,
        })
    }
}

impl JsonDeserialize for NetworkAcl {
    fn json_deserialize(parser: &mut json::Parser<'_>) -> Result<Self, json::Error> {
        parse_via_fast(parser)
    }
}

impl<'a> FastFromJson<'a> for ConfigUpdateDTO {
    fn parse(w: &mut TapeWalker<'a>, arena: &mut Arena) -> Result<Self, NoritoError> {
        w.expect_object_start()?;
        let kh_logger = norito::json::key_hash_const("logger");
        let kh_acl = norito::json::key_hash_const("network_acl");
        let kh_network = norito::json::key_hash_const("network");
        let kh_gas = norito::json::key_hash_const("confidential_gas");
        let kh_handshake = norito::json::key_hash_const("soranet_handshake");
        let kh_transport = norito::json::key_hash_const("transport");
        let kh_compute = norito::json::key_hash_const("compute_pricing");
        let mut logger: Option<Logger> = None;
        let mut network_acl: Option<NetworkAcl> = None;
        let mut network: Option<NetworkUpdate> = None;
        let mut confidential_gas: Option<ConfidentialGas> = None;
        let mut soranet_handshake: Option<SoranetHandshakeUpdate> = None;
        let mut transport: Option<TransportUpdate> = None;
        let mut compute_pricing: Option<ComputePricingUpdate> = None;
        while !w.peek_object_end()? {
            let kh = w.read_key_hash()?;
            w.expect_colon_resync()?;
            match kh {
                x if x == kh_logger && w.last_key() == "logger" => {
                    logger = Some(Logger::parse(w, arena)?);
                }
                x if x == kh_acl && w.last_key() == "network_acl" => {
                    network_acl = Some(NetworkAcl::parse(w, arena)?);
                }
                x if x == kh_network && w.last_key() == "network" => {
                    network = Some(<NetworkUpdate as FastFromJson>::parse(w, arena)?);
                }
                x if x == kh_gas && w.last_key() == "confidential_gas" => {
                    confidential_gas = Some(ConfidentialGas::parse(w, arena)?);
                }
                x if x == kh_handshake && w.last_key() == "soranet_handshake" => {
                    soranet_handshake = Some(SoranetHandshakeUpdate::parse(w, arena)?);
                }
                x if x == kh_transport && w.last_key() == "transport" => {
                    transport = Some(TransportUpdate::parse(w, arena)?);
                }
                x if x == kh_compute && w.last_key() == "compute_pricing" => {
                    compute_pricing = Some(ComputePricingUpdate::parse(w, arena)?);
                }
                _ => w.skip_value()?,
            }
            let _ = w.consume_comma_if_present()?;
        }
        w.expect_object_end()?;
        Ok(Self {
            logger: logger.ok_or_else(|| NoritoError::Message("missing logger".into()))?,
            network_acl,
            network,
            confidential_gas,
            soranet_handshake,
            transport,
            compute_pricing,
        })
    }
}

impl JsonDeserialize for ConfigUpdateDTO {
    fn json_deserialize(parser: &mut json::Parser<'_>) -> Result<Self, json::Error> {
        parse_via_fast(parser)
    }
}

impl<'a> FastFromJson<'a> for ConfigGetDTO {
    fn parse(w: &mut TapeWalker<'a>, arena: &mut Arena) -> Result<Self, NoritoError> {
        use iroha_crypto::PublicKey;
        w.expect_object_start()?;
        let mut public_key: Option<PublicKey> = None;
        let mut logger: Option<Logger> = None;
        let mut network: Option<Network> = None;
        let mut queue: Option<Queue> = None;
        let mut consensus: Option<Consensus> = None;
        let mut confidential_gas: Option<ConfidentialGas> = None;
        let mut transport: Option<Transport> = None;
        let mut nexus: Option<Nexus> = None;
        let kh_pk = norito::json::key_hash_const("public_key");
        let kh_log = norito::json::key_hash_const("logger");
        let kh_net = norito::json::key_hash_const("network");
        let kh_q = norito::json::key_hash_const("queue");
        let kh_consensus = norito::json::key_hash_const("consensus");
        let kh_gas = norito::json::key_hash_const("confidential_gas");
        let kh_transport = norito::json::key_hash_const("transport");
        let kh_nexus = norito::json::key_hash_const("nexus");
        while !w.peek_object_end()? {
            let kh = w.read_key_hash()?;
            w.expect_colon_resync()?;
            match kh {
                x if x == kh_pk && w.last_key() == "public_key" => {
                    let sref = w.parse_string_ref_inline(arena)?;
                    let s = sref.to_string();
                    let pk = PublicKey::from_str(&s)
                        .map_err(|_| NoritoError::Message("invalid public key".into()))?;
                    public_key = Some(pk);
                }
                x if x == kh_log && w.last_key() == "logger" => {
                    logger = Some(Logger::parse(w, arena)?);
                }
                x if x == kh_net && w.last_key() == "network" => {
                    network = Some(Network::parse(w, arena)?);
                }
                x if x == kh_q && w.last_key() == "queue" => {
                    queue = Some(Queue::parse(w, arena)?);
                }
                x if x == kh_consensus && w.last_key() == "consensus" => {
                    consensus = Some(Consensus::parse(w, arena)?);
                }
                x if x == kh_gas && w.last_key() == "confidential_gas" => {
                    confidential_gas = Some(ConfidentialGas::parse(w, arena)?);
                }
                x if x == kh_transport && w.last_key() == "transport" => {
                    transport = Some(Transport::parse(w, arena)?);
                }
                x if x == kh_nexus && w.last_key() == "nexus" => {
                    nexus = Some(Nexus::parse(w, arena)?);
                }
                _ => w.skip_value()?,
            }
            let _ = w.consume_comma_if_present()?;
        }
        w.expect_object_end()?;
        Ok(Self {
            public_key: public_key
                .ok_or_else(|| NoritoError::Message("missing public_key".into()))?,
            logger: logger.ok_or_else(|| NoritoError::Message("missing logger".into()))?,
            network: network.ok_or_else(|| NoritoError::Message("missing network".into()))?,
            queue: queue.ok_or_else(|| NoritoError::Message("missing queue".into()))?,
            consensus: consensus.ok_or_else(|| NoritoError::Message("missing consensus".into()))?,
            confidential_gas: confidential_gas
                .ok_or_else(|| NoritoError::Message("missing confidential_gas".into()))?,
            transport: transport.ok_or_else(|| NoritoError::Message("missing transport".into()))?,
            nexus: nexus.ok_or_else(|| NoritoError::Message("missing nexus".into()))?,
        })
    }
}

// Bridge for typed JSON parser path used by `from_json_fast_smart` on small inputs.
impl JsonDeserialize for ConfigGetDTO {
    fn json_deserialize(parser: &mut json::Parser<'_>) -> Result<Self, json::Error> {
        parse_via_fast(parser)
    }
}

impl<'a> FastFromJson<'a> for Consensus {
    fn parse(w: &mut TapeWalker<'a>, arena: &mut Arena) -> Result<Self, NoritoError> {
        w.expect_object_start()?;
        let mut mode: Option<String> = None;
        let mut mode_flip_enabled: Option<bool> = None;
        let kh_mode = norito::json::key_hash_const("mode");
        let kh_flip = norito::json::key_hash_const("mode_flip_enabled");
        while !w.peek_object_end()? {
            let kh = w.read_key_hash()?;
            w.expect_colon_resync()?;
            match kh {
                x if x == kh_mode && w.last_key() == "mode" => {
                    let sref = w.parse_string_ref_inline(arena)?;
                    mode = Some(sref.to_string());
                }
                x if x == kh_flip && w.last_key() == "mode_flip_enabled" => {
                    mode_flip_enabled = Some(w.parse_bool_inline()?);
                }
                _ => w.skip_value()?,
            }
            let _ = w.consume_comma_if_present()?;
        }
        w.expect_object_end()?;
        Ok(Self {
            mode: mode.ok_or_else(|| NoritoError::Message("missing mode".into()))?,
            mode_flip_enabled: mode_flip_enabled.unwrap_or(defaults::sumeragi::MODE_FLIP_ENABLED),
        })
    }
}

impl JsonDeserialize for Consensus {
    fn json_deserialize(parser: &mut json::Parser<'_>) -> Result<Self, json::Error> {
        parse_via_fast(parser)
    }
}

impl<'a> FastFromJson<'a> for Nexus {
    fn parse(w: &mut TapeWalker<'a>, arena: &mut Arena) -> Result<Self, NoritoError> {
        w.expect_object_start()?;
        let mut axt: Option<Axt> = None;
        let kh_axt = norito::json::key_hash_const("axt");
        while !w.peek_object_end()? {
            let kh = w.read_key_hash()?;
            w.expect_colon_resync()?;
            match kh {
                x if x == kh_axt && w.last_key() == "axt" => {
                    axt = Some(Axt::parse(w, arena)?);
                }
                _ => w.skip_value()?,
            }
            let _ = w.consume_comma_if_present()?;
        }
        w.expect_object_end()?;
        Ok(Self {
            axt: axt.ok_or_else(|| NoritoError::Message("missing axt".into()))?,
        })
    }
}

impl JsonDeserialize for Nexus {
    fn json_deserialize(parser: &mut json::Parser<'_>) -> Result<Self, json::Error> {
        parse_via_fast(parser)
    }
}

impl<'a> FastFromJson<'a> for Axt {
    fn parse(w: &mut TapeWalker<'a>, _arena: &mut Arena) -> Result<Self, NoritoError> {
        w.expect_object_start()?;
        let mut slot_length_ms: Option<NonZeroU64> = None;
        let mut max_clock_skew_ms: Option<u64> = None;
        let mut proof_cache_ttl_slots: Option<NonZeroU64> = None;
        let mut replay_retention_slots: Option<NonZeroU64> = None;
        let kh_slot_length = norito::json::key_hash_const("slot_length_ms");
        let kh_max_skew = norito::json::key_hash_const("max_clock_skew_ms");
        let kh_proof_ttl = norito::json::key_hash_const("proof_cache_ttl_slots");
        let kh_replay_retention = norito::json::key_hash_const("replay_retention_slots");
        while !w.peek_object_end()? {
            let kh = w.read_key_hash()?;
            w.expect_colon_resync()?;
            match kh {
                x if x == kh_slot_length && w.last_key() == "slot_length_ms" => {
                    slot_length_ms = NonZeroU64::new(w.parse_u64_inline()?);
                }
                x if x == kh_max_skew && w.last_key() == "max_clock_skew_ms" => {
                    max_clock_skew_ms = Some(w.parse_u64_inline()?);
                }
                x if x == kh_proof_ttl && w.last_key() == "proof_cache_ttl_slots" => {
                    proof_cache_ttl_slots = NonZeroU64::new(w.parse_u64_inline()?);
                }
                x if x == kh_replay_retention && w.last_key() == "replay_retention_slots" => {
                    replay_retention_slots = NonZeroU64::new(w.parse_u64_inline()?);
                }
                _ => w.skip_value()?,
            }
            let _ = w.consume_comma_if_present()?;
        }
        w.expect_object_end()?;
        Ok(Self {
            slot_length_ms: slot_length_ms
                .ok_or_else(|| NoritoError::Message("missing slot_length_ms".into()))?,
            max_clock_skew_ms: max_clock_skew_ms
                .ok_or_else(|| NoritoError::Message("missing max_clock_skew_ms".into()))?,
            proof_cache_ttl_slots: proof_cache_ttl_slots
                .ok_or_else(|| NoritoError::Message("missing proof_cache_ttl_slots".into()))?,
            replay_retention_slots: replay_retention_slots
                .ok_or_else(|| NoritoError::Message("missing replay_retention_slots".into()))?,
        })
    }
}

impl JsonDeserialize for Axt {
    fn json_deserialize(parser: &mut json::Parser<'_>) -> Result<Self, json::Error> {
        parse_via_fast(parser)
    }
}

impl From<&'_ base::ConfidentialGas> for ConfidentialGas {
    fn from(value: &'_ base::ConfidentialGas) -> Self {
        Self {
            proof_base: value.proof_base,
            per_public_input: value.per_public_input,
            per_proof_byte: value.per_proof_byte,
            per_nullifier: value.per_nullifier,
            per_commitment: value.per_commitment,
        }
    }
}

impl<'a> FastFromJson<'a> for ConfidentialGas {
    fn parse(w: &mut TapeWalker<'a>, _arena: &mut Arena) -> Result<Self, NoritoError> {
        w.expect_object_start()?;
        let mut proof_base: Option<u64> = None;
        let mut per_public_input: Option<u64> = None;
        let mut per_proof_byte: Option<u64> = None;
        let mut per_nullifier: Option<u64> = None;
        let mut per_commitment: Option<u64> = None;
        let kh_proof_base = norito::json::key_hash_const("proof_base");
        let kh_per_public_input = norito::json::key_hash_const("per_public_input");
        let kh_per_proof_byte = norito::json::key_hash_const("per_proof_byte");
        let kh_per_nullifier = norito::json::key_hash_const("per_nullifier");
        let kh_per_commitment = norito::json::key_hash_const("per_commitment");
        while !w.peek_object_end()? {
            let kh = w.read_key_hash()?;
            w.expect_colon_resync()?;
            match kh {
                x if x == kh_proof_base && w.last_key() == "proof_base" => {
                    proof_base = Some(w.parse_u64_inline()?);
                }
                x if x == kh_per_public_input && w.last_key() == "per_public_input" => {
                    per_public_input = Some(w.parse_u64_inline()?);
                }
                x if x == kh_per_proof_byte && w.last_key() == "per_proof_byte" => {
                    per_proof_byte = Some(w.parse_u64_inline()?);
                }
                x if x == kh_per_nullifier && w.last_key() == "per_nullifier" => {
                    per_nullifier = Some(w.parse_u64_inline()?);
                }
                x if x == kh_per_commitment && w.last_key() == "per_commitment" => {
                    per_commitment = Some(w.parse_u64_inline()?);
                }
                _ => w.skip_value()?,
            }
            let _ = w.consume_comma_if_present()?;
        }
        w.expect_object_end()?;
        Ok(Self {
            proof_base: proof_base
                .ok_or_else(|| NoritoError::Message("missing proof_base".into()))?,
            per_public_input: per_public_input
                .ok_or_else(|| NoritoError::Message("missing per_public_input".into()))?,
            per_proof_byte: per_proof_byte
                .ok_or_else(|| NoritoError::Message("missing per_proof_byte".into()))?,
            per_nullifier: per_nullifier
                .ok_or_else(|| NoritoError::Message("missing per_nullifier".into()))?,
            per_commitment: per_commitment
                .ok_or_else(|| NoritoError::Message("missing per_commitment".into()))?,
        })
    }
}

impl JsonDeserialize for ConfidentialGas {
    fn json_deserialize(parser: &mut json::Parser<'_>) -> Result<Self, json::Error> {
        parse_via_fast(parser)
    }
}

fn parse_price_weights(
    w: &mut TapeWalker<'_>,
    arena: &mut Arena,
) -> Result<ComputePriceWeights, NoritoError> {
    w.expect_object_start()?;
    let kh_cycles = norito::json::key_hash_const("cycles_per_unit");
    let kh_egress = norito::json::key_hash_const("egress_bytes_per_unit");
    let kh_unit = norito::json::key_hash_const("unit_label");
    let mut cycles = None;
    let mut egress = None;
    let mut unit_label = None;
    while !w.peek_object_end()? {
        let kh = w.read_key_hash()?;
        w.expect_colon_resync()?;
        match kh {
            x if x == kh_cycles && w.last_key() == "cycles_per_unit" => {
                cycles = Some(w.parse_u64_inline()?);
            }
            x if x == kh_egress && w.last_key() == "egress_bytes_per_unit" => {
                egress = Some(w.parse_u64_inline()?);
            }
            x if x == kh_unit && w.last_key() == "unit_label" => {
                unit_label = Some(w.parse_string_ref_inline(arena)?.to_string());
            }
            _ => w.skip_value()?,
        }
        let _ = w.consume_comma_if_present()?;
    }
    w.expect_object_end()?;
    Ok(ComputePriceWeights {
        cycles_per_unit: NonZeroU64::new(
            cycles.ok_or_else(|| NoritoError::Message("missing cycles_per_unit".into()))?,
        )
        .ok_or_else(|| NoritoError::Message("cycles_per_unit must be > 0".into()))?,
        egress_bytes_per_unit: NonZeroU64::new(
            egress.ok_or_else(|| NoritoError::Message("missing egress_bytes_per_unit".into()))?,
        )
        .ok_or_else(|| NoritoError::Message("egress_bytes_per_unit must be > 0".into()))?,
        unit_label: unit_label.ok_or_else(|| NoritoError::Message("missing unit_label".into()))?,
    })
}

fn parse_price_families(
    w: &mut TapeWalker<'_>,
    arena: &mut Arena,
) -> Result<BTreeMap<Name, ComputePriceWeights>, NoritoError> {
    w.expect_object_start()?;
    let mut families = BTreeMap::new();
    while !w.peek_object_end()? {
        let family_label = w.parse_string_ref_inline(arena)?.to_string();
        w.expect_colon_resync()?;
        let name = Name::from_str(&family_label)
            .map_err(|_| NoritoError::Message("invalid price family name".into()))?;
        let weights = parse_price_weights(w, arena)?;
        families.insert(name, weights);
        let _ = w.consume_comma_if_present()?;
    }
    w.expect_object_end()?;
    Ok(families)
}

/// Compute pricing update payload (price families + default family).
#[derive(Debug, Clone, Default)]
pub struct ComputePricingUpdate {
    /// Replacement price family weights (governance bounded).
    pub price_families: BTreeMap<Name, ComputePriceWeights>,
    /// Optional default price family override.
    pub default_price_family: Option<Name>,
}

impl FastJsonWrite for ComputePricingUpdate {
    fn write_json(&self, out: &mut String) {
        out.push('{');
        let mut first = true;
        if !self.price_families.is_empty() {
            first = false;
            out.push_str("\"price_families\":{");
            let mut iter = self.price_families.iter().peekable();
            while let Some((name, weights)) = iter.next() {
                out.push('"');
                let _ = write!(out, "{name}");
                out.push_str("\":");
                let encoded = json::to_string(
                    &json::to_value(weights).expect("compute price weights serialization"),
                )
                .expect("compute price weights serialization");
                out.push_str(&encoded);
                if iter.peek().is_some() {
                    out.push(',');
                }
            }
            out.push('}');
        }
        if let Some(default_family) = &self.default_price_family {
            if !first {
                out.push(',');
            }
            out.push_str("\"default_price_family\":\"");
            let _ = write!(out, "{default_family}");
            out.push('"');
        }
        out.push('}');
    }
}

impl<'a> FastFromJson<'a> for ComputePricingUpdate {
    fn parse(w: &mut TapeWalker<'a>, arena: &mut Arena) -> Result<Self, NoritoError> {
        w.expect_object_start()?;
        let kh_price_families = norito::json::key_hash_const("price_families");
        let kh_default_family = norito::json::key_hash_const("default_price_family");
        let mut price_families = None;
        let mut default_price_family = None;
        while !w.peek_object_end()? {
            let kh = w.read_key_hash()?;
            w.expect_colon_resync()?;
            match kh {
                x if x == kh_price_families && w.last_key() == "price_families" => {
                    price_families = Some(parse_price_families(w, arena)?);
                }
                x if x == kh_default_family && w.last_key() == "default_price_family" => {
                    let label = w.parse_string_ref_inline(arena)?.to_string();
                    let name = Name::from_str(&label)
                        .map_err(|_| NoritoError::Message("invalid default_price_family".into()))?;
                    default_price_family = Some(name);
                }
                _ => w.skip_value()?,
            }
            let _ = w.consume_comma_if_present()?;
        }
        w.expect_object_end()?;
        Ok(Self {
            price_families: price_families.unwrap_or_default(),
            default_price_family,
        })
    }
}

/// Subset of the configuration that clients are allowed to update.
#[derive(Debug, Clone)]
pub struct ConfigUpdateDTO {
    /// Logger settings to update.
    pub logger: Logger,
    /// Optional P2P ACL update.
    pub network_acl: Option<NetworkAcl>,
    /// Optional network update.
    pub network: Option<NetworkUpdate>,
    /// Optional confidential gas schedule override.
    pub confidential_gas: Option<ConfidentialGas>,
    /// Optional `SoraNet` handshake override.
    pub soranet_handshake: Option<SoranetHandshakeUpdate>,
    /// Optional transport configuration override.
    pub transport: Option<TransportUpdate>,
    /// Optional compute pricing override (governance bounded).
    pub compute_pricing: Option<ComputePricingUpdate>,
}

/// Partial transport update payload.
#[derive(Debug, Clone, Default)]
pub struct TransportUpdate {
    /// Optional Norito-RPC update.
    pub norito_rpc: Option<NoritoRpcUpdate>,
}

/// Norito-RPC transport update payload.
#[derive(Debug, Clone, Default)]
pub struct NoritoRpcUpdate {
    /// Optional enable toggle override.
    pub enabled: Option<bool>,
    /// Optional mTLS requirement flag override.
    pub require_mtls: Option<bool>,
    /// Optional replacement allowlist for canary access (empty vector clears the list).
    pub allowed_clients: Option<Vec<String>>,
    /// Optional stage label override (`disabled`, `canary`, `ga`).
    pub stage: Option<String>,
}

/// P2P ACL update DTO.
#[derive(Debug, Clone, Default)]
pub struct NetworkAcl {
    /// When true, only allow peers listed in `allow_keys`.
    pub allowlist_only: Option<bool>,
    /// Allowlist of peer public keys.
    pub allow_keys: Option<Vec<iroha_crypto::PublicKey>>,
    /// Denylist of peer public keys.
    pub deny_keys: Option<Vec<iroha_crypto::PublicKey>>,
    /// CIDR allowlist.
    pub allow_cidrs: Option<Vec<String>>,
    /// CIDR denylist.
    pub deny_cidrs: Option<Vec<String>>,
}

/// Logger configuration exposed to clients.
#[derive(Debug, Clone)]
pub struct Logger {
    /// Global log level threshold.
    pub level: Level,
    /// Optional module-specific filters.
    pub filter: Option<Directives>,
}

impl From<&'_ base::Logger> for Logger {
    fn from(value: &'_ base::Logger) -> Self {
        Self {
            level: value.level,
            filter: value.filter.clone(),
        }
    }
}

/// Selected queue configuration exposed to clients.
#[derive(Debug, Clone, Copy)]
pub struct Queue {
    /// Maximum number of transactions kept in the queue.
    pub capacity: NonZero<usize>,
}

impl From<&'_ base::Queue> for Queue {
    fn from(value: &'_ base::Queue) -> Self {
        Self {
            capacity: value.capacity,
        }
    }
}

/// Selected network configuration exposed to clients.
#[derive(Debug, Clone)]
pub struct Network {
    /// Number of block hashes per gossip message.
    pub block_gossip_size: NonZero<u32>,
    /// Block gossip interval in milliseconds.
    pub block_gossip_period_ms: u32,
    /// Peer gossip interval in milliseconds.
    pub peer_gossip_period_ms: u32,
    /// Relay hop limit.
    pub relay_ttl: u8,
    /// Peer trust decay half-life in milliseconds.
    pub trust_decay_half_life_ms: u32,
    /// Penalty applied for invalid/bad trust gossip.
    pub trust_penalty_bad_gossip: i32,
    /// Penalty applied when gossip mentions peers outside the topology.
    pub trust_penalty_unknown_peer: i32,
    /// Minimum trust score before gossip is ignored.
    pub trust_min_score: i32,
    /// Whether trust gossip capability is enabled.
    pub trust_gossip: bool,
    /// Number of transactions per gossip message.
    pub transaction_gossip_size: NonZero<u32>,
    /// Transaction gossip interval in milliseconds.
    pub transaction_gossip_period_ms: u32,
    /// Number of gossip periods to wait before re-sending the same transactions.
    pub transaction_gossip_resend_ticks: NonZero<u32>,
    /// `SoraNet` handshake configuration summary.
    pub soranet_handshake: SoranetHandshakeSummary,
    /// `SoraNet` privacy telemetry configuration summary.
    pub soranet_privacy: SoranetPrivacySummary,
    /// `SoraNet` VPN control-plane configuration summary.
    pub soranet_vpn: SoranetVpnSummary,
    /// Lane profile preset applied to the node.
    pub lane_profile: base::LaneProfile,
    /// Whether peers must match SM helper availability during handshake.
    pub require_sm_handshake_match: bool,
    /// Whether peers must match the OpenSSL preview toggle during handshake.
    pub require_sm_openssl_preview_match: bool,
}

impl From<&'_ base::Root> for Network {
    fn from(value: &'_ base::Root) -> Self {
        let handshake = SoranetHandshakeSummary::from(&value.network.soranet_handshake);
        let privacy = SoranetPrivacySummary::from(&value.network.soranet_privacy);
        let vpn = SoranetVpnSummary::from(&value.network.soranet_vpn);
        Self {
            block_gossip_size: value.block_sync.gossip_size,
            block_gossip_period_ms: u32::try_from(value.block_sync.gossip_period.as_millis())
                .expect("block gossip period should fit into a u32"),
            peer_gossip_period_ms: u32::try_from(value.network.peer_gossip_period.as_millis())
                .expect("peer gossip period should fit into a u32"),
            relay_ttl: value.network.relay_ttl,
            trust_decay_half_life_ms: u32::try_from(
                value.network.trust_decay_half_life.as_millis(),
            )
            .expect("trust decay half-life should fit into a u32"),
            trust_penalty_bad_gossip: value.network.trust_penalty_bad_gossip,
            trust_penalty_unknown_peer: value.network.trust_penalty_unknown_peer,
            trust_min_score: value.network.trust_min_score,
            trust_gossip: value.network.trust_gossip,
            transaction_gossip_size: value.transaction_gossiper.gossip_size,
            transaction_gossip_period_ms: u32::try_from(
                value.transaction_gossiper.gossip_period.as_millis(),
            )
            .expect("transaction gossip period should fit into a u32"),
            transaction_gossip_resend_ticks: value.transaction_gossiper.gossip_resend_ticks,
            soranet_handshake: handshake,
            soranet_privacy: privacy,
            soranet_vpn: vpn,
            lane_profile: value.network.lane_profile,
            require_sm_handshake_match: value.network.require_sm_handshake_match,
            require_sm_openssl_preview_match: value.network.require_sm_openssl_preview_match,
        }
    }
}

/// Client-facing view of the `SoraNet` handshake settings.
#[derive(Debug, Clone, Default)]
pub struct SoranetHandshakeSummary {
    /// Descriptor commitment advertised by the relay (hex).
    pub descriptor_commit_hex: String,
    /// Client capability TLVs encoded as hex string.
    pub client_capabilities_hex: String,
    /// Relay capability TLVs encoded as hex string.
    pub relay_capabilities_hex: String,
    /// Negotiated ML-KEM identifier.
    pub kem_id: u8,
    /// Negotiated signature suite identifier.
    pub sig_id: u8,
    /// Optional resume hash advertised to peers (hex).
    pub resume_hash_hex: Option<String>,
    /// Proof-of-work admission parameters.
    pub pow: SoranetHandshakePowSummary,
}

impl From<&'_ base::SoranetHandshake> for SoranetHandshakeSummary {
    fn from(value: &'_ base::SoranetHandshake) -> Self {
        Self {
            descriptor_commit_hex: hex::encode(value.descriptor_commit.value()),
            client_capabilities_hex: hex::encode(value.client_capabilities.value()),
            relay_capabilities_hex: hex::encode(value.relay_capabilities.value()),
            kem_id: value.kem_id,
            sig_id: value.sig_id,
            resume_hash_hex: value
                .resume_hash
                .as_ref()
                .map(|hash| hex::encode(hash.value())),
            pow: SoranetHandshakePowSummary::from(&value.pow),
        }
    }
}

/// Summary of the privacy telemetry configuration advertised by the relay.
#[derive(Debug, Clone, Copy, Default)]
pub struct SoranetPrivacySummary {
    /// Duration of each telemetry bucket in seconds.
    pub bucket_secs: u64,
    /// Minimum contributors required before publishing a bucket.
    pub min_handshakes: u64,
    /// Buckets to wait after the first contribution before attempting a flush.
    pub flush_delay_buckets: u64,
    /// Forced flush interval in buckets.
    pub force_flush_buckets: u64,
    /// Maximum completed buckets retained at once.
    pub max_completed_buckets: usize,
    /// Maximum bucket lag tolerated for collector shares before suppression.
    pub max_share_lag_buckets: u64,
    /// Expected number of collector shares per bucket.
    pub expected_shares: u16,
    /// Capacity of the in-memory privacy event buffer.
    pub event_buffer_capacity: usize,
}

impl From<&'_ base::SoranetPrivacy> for SoranetPrivacySummary {
    fn from(value: &'_ base::SoranetPrivacy) -> Self {
        Self {
            bucket_secs: value.bucket_secs,
            min_handshakes: value.min_handshakes,
            flush_delay_buckets: value.flush_delay_buckets,
            force_flush_buckets: value.force_flush_buckets,
            max_completed_buckets: value.max_completed_buckets,
            max_share_lag_buckets: value.max_share_lag_buckets,
            expected_shares: value.expected_shares,
            event_buffer_capacity: value.event_buffer_capacity,
        }
    }
}

/// Summary of the VPN control-plane configuration.
#[derive(Debug, Clone, Default)]
pub struct SoranetVpnSummary {
    /// Whether the VPN surface is enabled.
    pub enabled: bool,
    /// Fixed cell size (bytes).
    pub cell_size_bytes: u16,
    /// Flow label width (bits).
    pub flow_label_bits: u8,
    /// Cover-to-data ratio (permille).
    pub cover_to_data_per_mille: u16,
    /// Maximum burst of cover cells.
    pub max_cover_burst: u16,
    /// Heartbeat cadence (milliseconds).
    pub heartbeat_ms: u16,
    /// Maximum jitter (milliseconds).
    pub jitter_ms: u16,
    /// Padding budget (milliseconds).
    pub padding_budget_ms: u16,
    /// Guard/exit refresh cadence (seconds).
    pub guard_refresh_secs: u64,
    /// Lease duration (seconds).
    pub lease_secs: u64,
    /// DNS push interval (seconds).
    pub dns_push_interval_secs: u64,
    /// Exit class label for billing/telemetry.
    pub exit_class: String,
    /// Meter family identifier.
    pub meter_family: String,
}

impl From<&'_ base::SoranetVpn> for SoranetVpnSummary {
    fn from(value: &'_ base::SoranetVpn) -> Self {
        let lease_secs = u32::try_from(value.lease.as_secs())
            .expect("soranet_vpn.lease exceeds u32::MAX seconds");
        let exit_class = VpnExitClassV1::try_from_label(&value.exit_class)
            .expect("soranet_vpn.exit_class must be standard|low-latency|high-security");
        Self {
            enabled: value.enabled,
            cell_size_bytes: value.cell_size_bytes,
            flow_label_bits: value.flow_label_bits,
            cover_to_data_per_mille: value.cover_to_data_per_mille,
            max_cover_burst: value.max_cover_burst,
            heartbeat_ms: value.heartbeat_ms,
            jitter_ms: value.jitter_ms,
            padding_budget_ms: value.padding_budget_ms,
            guard_refresh_secs: value.guard_refresh.as_secs(),
            lease_secs: u64::from(lease_secs),
            dns_push_interval_secs: value.dns_push_interval.as_secs(),
            exit_class: exit_class.as_label().to_string(),
            meter_family: value.meter_family.clone(),
        }
    }
}

/// Summary of the proof-of-work admission settings.
#[derive(Debug, Clone, Default)]
pub struct SoranetHandshakePowSummary {
    /// Indicates whether `PoW` tickets are required.
    pub required: bool,
    /// Required difficulty in leading zero bits.
    pub difficulty: u8,
    /// Maximum allowed ticket future skew in seconds.
    pub max_future_skew_secs: u64,
    /// Minimum ticket time-to-live in seconds.
    pub min_ticket_ttl_secs: u64,
    /// Target ticket time-to-live in seconds.
    pub ticket_ttl_secs: u64,
    /// Optional Argon2 puzzle parameters.
    pub puzzle: Option<SoranetHandshakePuzzleSummary>,
    /// Optional ML-DSA-44 public key for verifying signed tickets (hex).
    pub signed_ticket_public_key_hex: Option<String>,
}

impl From<&'_ base::SoranetPow> for SoranetHandshakePowSummary {
    fn from(value: &'_ base::SoranetPow) -> Self {
        Self {
            required: value.required,
            difficulty: value.difficulty,
            max_future_skew_secs: value.max_future_skew.as_secs(),
            min_ticket_ttl_secs: value.min_ticket_ttl.as_secs(),
            ticket_ttl_secs: value.ticket_ttl.as_secs(),
            puzzle: value.puzzle.map(SoranetHandshakePuzzleSummary::from),
            signed_ticket_public_key_hex: value.signed_ticket_public_key.as_ref().map(hex::encode),
        }
    }
}

/// Summary of the Argon2 puzzle gate.
#[derive(Debug, Clone, Copy, Default)]
pub struct SoranetHandshakePuzzleSummary {
    /// Memory cost in kibibytes.
    pub memory_kib: u32,
    /// Time cost (iterations).
    pub time_cost: u32,
    /// Parallel lanes.
    pub lanes: u32,
}

impl From<base::SoranetPuzzle> for SoranetHandshakePuzzleSummary {
    fn from(value: base::SoranetPuzzle) -> Self {
        Self {
            memory_kib: value.memory_kib.get(),
            time_cost: value.time_cost.get(),
            lanes: value.lanes.get(),
        }
    }
}

impl<'a> FastFromJson<'a> for SoranetHandshakePuzzleSummary {
    fn parse(w: &mut TapeWalker<'a>, _arena: &mut Arena) -> Result<Self, NoritoError> {
        let mut summary = Self::default();
        w.expect_object_start()?;
        let kh_memory = norito::json::key_hash_const("memory_kib");
        let kh_time = norito::json::key_hash_const("time_cost");
        let kh_lanes = norito::json::key_hash_const("lanes");
        while !w.peek_object_end()? {
            let kh = w.read_key_hash()?;
            w.expect_colon_resync()?;
            match kh {
                x if x == kh_memory && w.last_key() == "memory_kib" => {
                    summary.memory_kib = u32::try_from(w.parse_u64_inline()?)
                        .map_err(|_| NoritoError::Message("u32 overflow".into()))?;
                }
                x if x == kh_time && w.last_key() == "time_cost" => {
                    summary.time_cost = u32::try_from(w.parse_u64_inline()?)
                        .map_err(|_| NoritoError::Message("u32 overflow".into()))?;
                }
                x if x == kh_lanes && w.last_key() == "lanes" => {
                    summary.lanes = u32::try_from(w.parse_u64_inline()?)
                        .map_err(|_| NoritoError::Message("u32 overflow".into()))?;
                }
                _ => w.skip_value()?,
            }
            let _ = w.consume_comma_if_present()?;
        }
        w.expect_object_end()?;
        Ok(summary)
    }
}

impl JsonDeserialize for SoranetHandshakePuzzleSummary {
    fn json_deserialize(parser: &mut json::Parser<'_>) -> Result<Self, json::Error> {
        parse_via_fast(parser)
    }
}

impl SoranetHandshakePuzzleSummary {
    fn write_json(&self, out: &mut String) {
        out.push('{');
        out.push_str("\"memory_kib\":");
        let _ = write!(out, "{}", self.memory_kib);
        out.push(',');
        out.push_str("\"time_cost\":");
        let _ = write!(out, "{}", self.time_cost);
        out.push(',');
        out.push_str("\"lanes\":");
        let _ = write!(out, "{}", self.lanes);
        out.push('}');
    }
}

/// Partial update directive for Argon2 puzzle parameters.
#[derive(Debug, Clone, Copy, Default)]
pub struct SoranetHandshakePuzzleUpdate {
    /// Toggle the puzzle requirement.
    pub enabled: Option<bool>,
    /// Override memory cost (KiB).
    pub memory_kib: Option<u32>,
    /// Override time cost (iterations).
    pub time_cost: Option<u32>,
    /// Override Argon2 lanes.
    pub lanes: Option<u32>,
}

impl SoranetHandshakePuzzleUpdate {
    fn write_json(&self, out: &mut String) {
        out.push('{');
        let mut first = true;
        let mut emit = |out: &mut String| {
            if first {
                first = false;
            } else {
                out.push(',');
            }
        };
        if let Some(enabled) = self.enabled {
            emit(out);
            out.push_str("\"enabled\":");
            out.push_str(if enabled { "true" } else { "false" });
        }
        if let Some(memory) = self.memory_kib {
            emit(out);
            out.push_str("\"memory_kib\":");
            let _ = write!(out, "{memory}");
        }
        if let Some(time) = self.time_cost {
            emit(out);
            out.push_str("\"time_cost\":");
            let _ = write!(out, "{time}");
        }
        if let Some(lanes) = self.lanes {
            emit(out);
            out.push_str("\"lanes\":");
            let _ = write!(out, "{lanes}");
        }
        out.push('}');
    }
}

fn parse_soranet_puzzle_update(
    w: &mut TapeWalker<'_>,
) -> Result<SoranetHandshakePuzzleUpdate, NoritoError> {
    w.expect_object_start()?;
    let mut enabled = None;
    let mut memory_kib = None;
    let mut time_cost = None;
    let mut lanes = None;
    let kh_enabled = norito::json::key_hash_const("enabled");
    let kh_memory = norito::json::key_hash_const("memory_kib");
    let kh_time = norito::json::key_hash_const("time_cost");
    let kh_lanes = norito::json::key_hash_const("lanes");
    while !w.peek_object_end()? {
        let kh = w.read_key_hash()?;
        w.expect_colon_resync()?;
        match kh {
            x if x == kh_enabled && w.last_key() == "enabled" => {
                enabled = Some(w.parse_bool_inline()?);
            }
            x if x == kh_memory && w.last_key() == "memory_kib" => {
                memory_kib = Some(
                    u32::try_from(w.parse_u64_inline()?)
                        .map_err(|_| NoritoError::Message("u32 overflow".into()))?,
                );
            }
            x if x == kh_time && w.last_key() == "time_cost" => {
                time_cost = Some(
                    u32::try_from(w.parse_u64_inline()?)
                        .map_err(|_| NoritoError::Message("u32 overflow".into()))?,
                );
            }
            x if x == kh_lanes && w.last_key() == "lanes" => {
                lanes = Some(
                    u32::try_from(w.parse_u64_inline()?)
                        .map_err(|_| NoritoError::Message("u32 overflow".into()))?,
                );
            }
            _ => w.skip_value()?,
        }
        let _ = w.consume_comma_if_present()?;
    }
    w.expect_object_end()?;
    Ok(SoranetHandshakePuzzleUpdate {
        enabled,
        memory_kib,
        time_cost,
        lanes,
    })
}

/// Directive describing how to update the handshake resume hash.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResumeHashDirective {
    /// Remove the existing resume hash.
    Clear,
    /// Set the resume hash to the provided hex string.
    Set(String),
}

/// Partial update DTO for the `SoraNet` handshake settings.
#[derive(Debug, Clone, Default)]
pub struct SoranetHandshakeUpdate {
    /// Optional descriptor commitment override (hex).
    pub descriptor_commit_hex: Option<String>,
    /// Optional client capabilities override (hex).
    pub client_capabilities_hex: Option<String>,
    /// Optional relay capabilities override (hex).
    pub relay_capabilities_hex: Option<String>,
    /// Optional ML-KEM identifier override.
    pub kem_id: Option<u8>,
    /// Optional signature suite identifier override.
    pub sig_id: Option<u8>,
    /// Resume hash directive. `None` leaves the current value unchanged.
    pub resume_hash_hex: Option<ResumeHashDirective>,
    /// Optional `PoW` override.
    pub pow: Option<SoranetHandshakePowUpdate>,
}

/// Partial update DTO for `PoW` admission configuration.
#[derive(Debug, Clone, Default)]
pub struct SoranetHandshakePowUpdate {
    /// Override the required flag.
    pub required: Option<bool>,
    /// Override difficulty.
    pub difficulty: Option<u8>,
    /// Override future skew.
    pub max_future_skew_secs: Option<u64>,
    /// Override minimum ticket TTL.
    pub min_ticket_ttl_secs: Option<u64>,
    /// Override ticket TTL.
    pub ticket_ttl_secs: Option<u64>,
    /// Override puzzle parameters.
    pub puzzle: Option<SoranetHandshakePuzzleUpdate>,
    /// Override the signed-ticket verification key (hex).
    pub signed_ticket_public_key_hex: Option<String>,
}

impl<'a> FastFromJson<'a> for SoranetHandshakePowSummary {
    fn parse(w: &mut TapeWalker<'a>, arena: &mut Arena) -> Result<Self, NoritoError> {
        w.expect_object_start()?;
        let mut required = None;
        let mut difficulty = None;
        let mut max_future_skew = None;
        let mut min_ticket_ttl = None;
        let mut ticket_ttl = None;
        let mut puzzle = None;
        let mut signed_ticket_public_key_hex = None;
        let kh_required = norito::json::key_hash_const("required");
        let kh_difficulty = norito::json::key_hash_const("difficulty");
        let kh_max_future_skew = norito::json::key_hash_const("max_future_skew_secs");
        let kh_min_ticket_ttl = norito::json::key_hash_const("min_ticket_ttl_secs");
        let kh_ticket_ttl = norito::json::key_hash_const("ticket_ttl_secs");
        let kh_signed_ticket_public_key =
            norito::json::key_hash_const("signed_ticket_public_key_hex");
        let kh_puzzle = norito::json::key_hash_const("puzzle");
        while !w.peek_object_end()? {
            let kh = w.read_key_hash()?;
            w.expect_colon_resync()?;
            match kh {
                x if x == kh_required && w.last_key() == "required" => {
                    required = Some(w.parse_bool_inline()?);
                }
                x if x == kh_difficulty && w.last_key() == "difficulty" => {
                    difficulty = Some(
                        u8::try_from(w.parse_u64_inline()?)
                            .map_err(|_| NoritoError::Message("u8 overflow".into()))?,
                    );
                }
                x if x == kh_max_future_skew && w.last_key() == "max_future_skew_secs" => {
                    max_future_skew = Some(w.parse_u64_inline()?);
                }
                x if x == kh_min_ticket_ttl && w.last_key() == "min_ticket_ttl_secs" => {
                    min_ticket_ttl = Some(w.parse_u64_inline()?);
                }
                x if x == kh_ticket_ttl && w.last_key() == "ticket_ttl_secs" => {
                    ticket_ttl = Some(w.parse_u64_inline()?);
                }
                x if x == kh_signed_ticket_public_key
                    && w.last_key() == "signed_ticket_public_key_hex" =>
                {
                    signed_ticket_public_key_hex =
                        Some(w.parse_string_ref_inline(arena)?.to_string());
                }
                x if x == kh_puzzle && w.last_key() == "puzzle" => {
                    puzzle = Some(SoranetHandshakePuzzleSummary::parse(w, arena)?);
                }
                _ => w.skip_value()?,
            }
            let _ = w.consume_comma_if_present()?;
        }
        w.expect_object_end()?;
        Ok(Self {
            required: required.unwrap_or(false),
            difficulty: difficulty.unwrap_or(0),
            max_future_skew_secs: max_future_skew.unwrap_or(0),
            min_ticket_ttl_secs: min_ticket_ttl.unwrap_or(0),
            ticket_ttl_secs: ticket_ttl.unwrap_or(0),
            puzzle,
            signed_ticket_public_key_hex,
        })
    }
}

impl JsonDeserialize for SoranetHandshakePowSummary {
    fn json_deserialize(parser: &mut json::Parser<'_>) -> Result<Self, json::Error> {
        parse_via_fast(parser)
    }
}

impl<'a> FastFromJson<'a> for SoranetHandshakePowUpdate {
    fn parse(w: &mut TapeWalker<'a>, arena: &mut Arena) -> Result<Self, NoritoError> {
        let _ = arena;
        w.expect_object_start()?;
        let mut required = None;
        let mut difficulty = None;
        let mut max_future_skew = None;
        let mut min_ticket_ttl = None;
        let mut ticket_ttl = None;
        let mut puzzle = None;
        let mut signed_ticket_public_key_hex = None;
        let kh_required = norito::json::key_hash_const("required");
        let kh_difficulty = norito::json::key_hash_const("difficulty");
        let kh_max_future_skew = norito::json::key_hash_const("max_future_skew_secs");
        let kh_min_ticket_ttl = norito::json::key_hash_const("min_ticket_ttl_secs");
        let kh_ticket_ttl = norito::json::key_hash_const("ticket_ttl_secs");
        let kh_signed_ticket_public_key =
            norito::json::key_hash_const("signed_ticket_public_key_hex");
        let kh_puzzle = norito::json::key_hash_const("puzzle");
        while !w.peek_object_end()? {
            let kh = w.read_key_hash()?;
            w.expect_colon_resync()?;
            match kh {
                x if x == kh_required && w.last_key() == "required" => {
                    required = Some(w.parse_bool_inline()?);
                }
                x if x == kh_difficulty && w.last_key() == "difficulty" => {
                    difficulty = Some(
                        u8::try_from(w.parse_u64_inline()?)
                            .map_err(|_| NoritoError::Message("u8 overflow".into()))?,
                    );
                }
                x if x == kh_max_future_skew && w.last_key() == "max_future_skew_secs" => {
                    max_future_skew = Some(w.parse_u64_inline()?);
                }
                x if x == kh_min_ticket_ttl && w.last_key() == "min_ticket_ttl_secs" => {
                    min_ticket_ttl = Some(w.parse_u64_inline()?);
                }
                x if x == kh_ticket_ttl && w.last_key() == "ticket_ttl_secs" => {
                    ticket_ttl = Some(w.parse_u64_inline()?);
                }
                x if x == kh_signed_ticket_public_key
                    && w.last_key() == "signed_ticket_public_key_hex" =>
                {
                    signed_ticket_public_key_hex =
                        Some(w.parse_string_ref_inline(arena)?.to_string());
                }
                x if x == kh_puzzle && w.last_key() == "puzzle" => {
                    puzzle = Some(parse_soranet_puzzle_update(w)?);
                }
                _ => w.skip_value()?,
            }
            let _ = w.consume_comma_if_present()?;
        }
        w.expect_object_end()?;
        Ok(Self {
            required,
            difficulty,
            max_future_skew_secs: max_future_skew,
            min_ticket_ttl_secs: min_ticket_ttl,
            ticket_ttl_secs: ticket_ttl,
            puzzle,
            signed_ticket_public_key_hex,
        })
    }
}

impl JsonDeserialize for SoranetHandshakePowUpdate {
    fn json_deserialize(parser: &mut json::Parser<'_>) -> Result<Self, json::Error> {
        parse_via_fast(parser)
    }
}

impl JsonDeserialize for SoranetHandshakePuzzleUpdate {
    fn json_deserialize(parser: &mut json::Parser<'_>) -> Result<Self, json::Error> {
        let input = parser.input();
        let start = parser.position();
        let mut walker = TapeWalker::new(input);
        walker.sync_to_raw(start);
        let value = parse_soranet_puzzle_update(&mut walker)
            .map_err(|err| json::Error::Message(err.to_string()))?;
        let end = walker.raw_pos();
        while parser.position() < end {
            let _ = parser.bump();
        }
        Ok(value)
    }
}

impl<'a> FastFromJson<'a> for SoranetHandshakeUpdate {
    fn parse(w: &mut TapeWalker<'a>, arena: &mut Arena) -> Result<Self, NoritoError> {
        w.expect_object_start()?;
        let mut descriptor_commit_hex = None;
        let mut client_capabilities_hex = None;
        let mut relay_capabilities_hex = None;
        let mut kem_id = None;
        let mut sig_id = None;
        let mut resume_hash_hex: Option<ResumeHashDirective> = None;
        let mut pow = None;
        let kh_descriptor = norito::json::key_hash_const("descriptor_commit_hex");
        let kh_client = norito::json::key_hash_const("client_capabilities_hex");
        let kh_relay = norito::json::key_hash_const("relay_capabilities_hex");
        let kh_kem = norito::json::key_hash_const("kem_id");
        let kh_sig = norito::json::key_hash_const("sig_id");
        let kh_resume = norito::json::key_hash_const("resume_hash_hex");
        let kh_pow = norito::json::key_hash_const("pow");
        while !w.peek_object_end()? {
            let kh = w.read_key_hash()?;
            w.expect_colon_resync()?;
            match kh {
                x if x == kh_descriptor && w.last_key() == "descriptor_commit_hex" => {
                    let value = w.parse_string_ref_inline(arena)?.to_string();
                    descriptor_commit_hex = Some(value);
                }
                x if x == kh_client && w.last_key() == "client_capabilities_hex" => {
                    let value = w.parse_string_ref_inline(arena)?.to_string();
                    client_capabilities_hex = Some(value);
                }
                x if x == kh_relay && w.last_key() == "relay_capabilities_hex" => {
                    let value = w.parse_string_ref_inline(arena)?.to_string();
                    relay_capabilities_hex = Some(value);
                }
                x if x == kh_kem && w.last_key() == "kem_id" => {
                    kem_id = Some(
                        u8::try_from(w.parse_u64_inline()?)
                            .map_err(|_| NoritoError::Message("u8 overflow".into()))?,
                    );
                }
                x if x == kh_sig && w.last_key() == "sig_id" => {
                    sig_id = Some(
                        u8::try_from(w.parse_u64_inline()?)
                            .map_err(|_| NoritoError::Message("u8 overflow".into()))?,
                    );
                }
                x if x == kh_resume && w.last_key() == "resume_hash_hex" => {
                    w.skip_ws();
                    let bytes = w.input().as_bytes();
                    let cursor = w.raw_pos();
                    if cursor + 4 <= bytes.len() && &bytes[cursor..cursor + 4] == b"null" {
                        w.sync_to_raw(cursor + 4);
                        resume_hash_hex = Some(ResumeHashDirective::Clear);
                    } else {
                        let value = w.parse_string_ref_inline(arena)?.to_string();
                        resume_hash_hex = Some(ResumeHashDirective::Set(value));
                    }
                }
                x if x == kh_pow && w.last_key() == "pow" => {
                    pow = Some(SoranetHandshakePowUpdate::parse(w, arena)?);
                }
                _ => w.skip_value()?,
            }
            let _ = w.consume_comma_if_present()?;
        }
        w.expect_object_end()?;
        Ok(Self {
            descriptor_commit_hex,
            client_capabilities_hex,
            relay_capabilities_hex,
            kem_id,
            sig_id,
            resume_hash_hex,
            pow,
        })
    }
}

impl JsonDeserialize for SoranetHandshakeUpdate {
    fn json_deserialize(parser: &mut json::Parser<'_>) -> Result<Self, json::Error> {
        parse_via_fast(parser)
    }
}

impl<'a> FastFromJson<'a> for SoranetHandshakeSummary {
    fn parse(w: &mut TapeWalker<'a>, arena: &mut Arena) -> Result<Self, NoritoError> {
        w.expect_object_start()?;
        let mut descriptor_commit_hex: Option<String> = None;
        let mut client_capabilities_hex: Option<String> = None;
        let mut relay_capabilities_hex: Option<String> = None;
        let mut kem_id: Option<u8> = None;
        let mut sig_id: Option<u8> = None;
        let mut resume_hash_hex: Option<String> = None;
        let mut pow = None;
        let kh_descriptor = norito::json::key_hash_const("descriptor_commit_hex");
        let kh_client = norito::json::key_hash_const("client_capabilities_hex");
        let kh_relay = norito::json::key_hash_const("relay_capabilities_hex");
        let kh_kem = norito::json::key_hash_const("kem_id");
        let kh_sig = norito::json::key_hash_const("sig_id");
        let kh_resume = norito::json::key_hash_const("resume_hash_hex");
        let kh_pow = norito::json::key_hash_const("pow");
        while !w.peek_object_end()? {
            let kh = w.read_key_hash()?;
            w.expect_colon_resync()?;
            match kh {
                x if x == kh_descriptor && w.last_key() == "descriptor_commit_hex" => {
                    let value = w.parse_string_ref_inline(arena)?.to_string();
                    descriptor_commit_hex = Some(value);
                }
                x if x == kh_client && w.last_key() == "client_capabilities_hex" => {
                    let value = w.parse_string_ref_inline(arena)?.to_string();
                    client_capabilities_hex = Some(value);
                }
                x if x == kh_relay && w.last_key() == "relay_capabilities_hex" => {
                    let value = w.parse_string_ref_inline(arena)?.to_string();
                    relay_capabilities_hex = Some(value);
                }
                x if x == kh_kem && w.last_key() == "kem_id" => {
                    kem_id = Some(
                        u8::try_from(w.parse_u64_inline()?)
                            .map_err(|_| NoritoError::Message("u8 overflow".into()))?,
                    );
                }
                x if x == kh_sig && w.last_key() == "sig_id" => {
                    sig_id = Some(
                        u8::try_from(w.parse_u64_inline()?)
                            .map_err(|_| NoritoError::Message("u8 overflow".into()))?,
                    );
                }
                x if x == kh_resume && w.last_key() == "resume_hash_hex" => {
                    w.skip_ws();
                    let bytes = w.input().as_bytes();
                    let cursor = w.raw_pos();
                    if cursor + 4 <= bytes.len() && &bytes[cursor..cursor + 4] == b"null" {
                        w.sync_to_raw(cursor + 4);
                        resume_hash_hex = None;
                    } else {
                        let value = w.parse_string_ref_inline(arena)?.to_string();
                        resume_hash_hex = Some(value);
                    }
                }
                x if x == kh_pow && w.last_key() == "pow" => {
                    pow = Some(SoranetHandshakePowSummary::parse(w, arena)?);
                }
                _ => w.skip_value()?,
            }
            let _ = w.consume_comma_if_present()?;
        }
        w.expect_object_end()?;
        Ok(Self {
            descriptor_commit_hex: descriptor_commit_hex.unwrap_or_default(),
            client_capabilities_hex: client_capabilities_hex.unwrap_or_default(),
            relay_capabilities_hex: relay_capabilities_hex.unwrap_or_default(),
            kem_id: kem_id.unwrap_or(0),
            sig_id: sig_id.unwrap_or(0),
            resume_hash_hex,
            pow: pow.unwrap_or_default(),
        })
    }
}

impl JsonDeserialize for SoranetHandshakeSummary {
    fn json_deserialize(parser: &mut json::Parser<'_>) -> Result<Self, json::Error> {
        parse_via_fast(parser)
    }
}

#[cfg(test)]
mod test {
    use iroha_crypto::{
        KeyPair,
        soranet::handshake::{
            DEFAULT_CLIENT_CAPABILITIES, DEFAULT_DESCRIPTOR_COMMIT, DEFAULT_RELAY_CAPABILITIES,
        },
    };
    use iroha_data_model::Level;
    use nonzero_ext::nonzero;

    use super::*;

    #[test]
    #[allow(clippy::too_many_lines)]
    fn snapshot_serialized_form() {
        let soranet_streaming =
            StreamingTransportSummary::from(&base::StreamingSoranet::from_defaults());
        let soranet_vpn = SoranetVpnSummary::from(&base::SoranetVpn::default());
        let value = ConfigGetDTO {
            public_key: KeyPair::from_seed(vec![1, 2, 3], <_>::default())
                .public_key()
                .clone(),
            logger: Logger {
                level: Level::TRACE,
                filter: None,
            },
            network: Network {
                block_gossip_size: nonzero!(10u32),
                block_gossip_period_ms: 5000,
                peer_gossip_period_ms: 1_000,
                trust_decay_half_life_ms: u32::try_from(
                    defaults::network::TRUST_DECAY_HALF_LIFE.as_millis(),
                )
                .expect("trust decay half-life should fit into a u32"),
                trust_penalty_bad_gossip: defaults::network::TRUST_PENALTY_BAD_GOSSIP,
                trust_penalty_unknown_peer: defaults::network::TRUST_PENALTY_UNKNOWN_PEER,
                trust_min_score: defaults::network::TRUST_MIN_SCORE,
                trust_gossip: defaults::network::TRUST_GOSSIP,
                relay_ttl: 8,
                transaction_gossip_size: nonzero!(512u32),
                transaction_gossip_period_ms: 1000,
                transaction_gossip_resend_ticks: defaults::network::TRANSACTION_GOSSIP_RESEND_TICKS,
                soranet_handshake: SoranetHandshakeSummary {
                    descriptor_commit_hex: hex::encode(DEFAULT_DESCRIPTOR_COMMIT),
                    client_capabilities_hex: hex::encode(DEFAULT_CLIENT_CAPABILITIES),
                    relay_capabilities_hex: hex::encode(DEFAULT_RELAY_CAPABILITIES),
                    kem_id: 1,
                    sig_id: 1,
                    resume_hash_hex: None,
                    pow: SoranetHandshakePowSummary {
                        required: false,
                        difficulty: 0,
                        max_future_skew_secs: 300,
                        min_ticket_ttl_secs: 30,
                        ticket_ttl_secs: 60,
                        puzzle: Some(SoranetHandshakePuzzleSummary {
                            memory_kib: 64 * 1024,
                            time_cost: 2,
                            lanes: 1,
                        }),
                        signed_ticket_public_key_hex: None,
                    },
                },
                soranet_privacy: SoranetPrivacySummary {
                    bucket_secs: 60,
                    min_handshakes: 12,
                    flush_delay_buckets: 1,
                    force_flush_buckets: 6,
                    max_completed_buckets: 120,
                    max_share_lag_buckets: 12,
                    expected_shares: 2,
                    event_buffer_capacity: 4_096,
                },
                soranet_vpn,
                lane_profile: base::LaneProfile::Core,
                require_sm_handshake_match: true,
                require_sm_openssl_preview_match: true,
            },
            queue: Queue {
                capacity: nonzero!(656_565_usize),
            },
            consensus: Consensus {
                mode: "permissioned".to_string(),
                mode_flip_enabled: defaults::sumeragi::MODE_FLIP_ENABLED,
            },
            confidential_gas: ConfidentialGas {
                proof_base: 777_777,
                per_public_input: 3_333,
                per_proof_byte: 42,
                per_nullifier: 123,
                per_commitment: 321,
            },
            transport: Transport {
                norito_rpc: NoritoRpcSummary {
                    enabled: true,
                    stage: "ga".to_string(),
                    require_mtls: true,
                    canary_allowlist_size: 2,
                },
                streaming: soranet_streaming,
            },
            nexus: Nexus {
                axt: Axt {
                    slot_length_ms: nonzero!(10_u64),
                    max_clock_skew_ms: 5,
                    proof_cache_ttl_slots: nonzero!(4_u64),
                    replay_retention_slots: nonzero!(256_u64),
                },
            },
        };

        let actual = norito::json::to_json_pretty(&value).expect("The value is a valid JSON");

        // NOTE: whenever this is updated, make sure to update the documentation accordingly:
        //       https://docs.iroha.tech/reference/torii-endpoints.html
        //       -> Configuration endpoints
        let expected = expect_test::expect![[r#"
            {
              "public_key": "ed0120E159467BC273205BB250D41686FCA6CB85F163D5354B06CB3BA8FD42290EDF2A",
              "logger": {
                "level": "TRACE",
                "filter": null
              },
              "network": {
                "block_gossip_size": 10,
                "block_gossip_period_ms": 5000,
                "peer_gossip_period_ms": 1000,
                "trust_decay_half_life_ms": 300000,
                "trust_penalty_bad_gossip": 5,
                "trust_penalty_unknown_peer": 3,
                "trust_min_score": -20,
                "trust_gossip": true,
                "relay_ttl": 8,
                "transaction_gossip_size": 512,
                "transaction_gossip_period_ms": 1000,
                "transaction_gossip_resend_ticks": 3,
                "soranet_handshake": {
                  "descriptor_commit_hex": "76d0f4f511391e6548e6f9c80f30ed61c4cbbb98b5ecec922d8af67233f21f1f",
                  "client_capabilities_hex": "010100020101010200020101010400030203010202000200047f100004deadbeef7f110004cafebabe",
                  "relay_capabilities_hex": "0101000201010102000201010103002076d0f4f511391e6548e6f9c80f30ed61c4cbbb98b5ecec922d8af67233f21f1f0104000302030102010001010202000200047f12000412345678",
                  "kem_id": 1,
                  "sig_id": 1,
                  "resume_hash_hex": null,
                  "pow": {
                    "required": false,
                    "difficulty": 0,
                    "max_future_skew_secs": 300,
                    "min_ticket_ttl_secs": 30,
                    "ticket_ttl_secs": 60,
                    "puzzle": {
                      "memory_kib": 65536,
                      "time_cost": 2,
                      "lanes": 1
                    }
                  }
                },
                "soranet_privacy": {
                  "bucket_secs": 60,
                  "min_handshakes": 12,
                  "flush_delay_buckets": 1,
                  "force_flush_buckets": 6,
                  "max_completed_buckets": 120,
                  "max_share_lag_buckets": 12,
                  "expected_shares": 2,
                  "event_buffer_capacity": 4096
                },
                "soranet_vpn": {
                  "enabled": true,
                  "cell_size_bytes": 1024,
                  "flow_label_bits": 24,
                  "cover_to_data_per_mille": 250,
                  "max_cover_burst": 3,
                  "heartbeat_ms": 500,
                  "jitter_ms": 10,
                  "padding_budget_ms": 15,
                  "guard_refresh_secs": 3600,
                  "lease_secs": 600,
                  "dns_push_interval_secs": 90,
                  "exit_class": "standard",
                  "meter_family": "soranet.vpn.standard"
                },
                "lane_profile": "core",
                "require_sm_handshake_match": true,
                "require_sm_openssl_preview_match": true
              },
              "queue": {
                "capacity": 656565
              },
              "consensus": {
                "mode": "permissioned",
                "mode_flip_enabled": true
              },
              "confidential_gas": {
                "proof_base": 777777,
                "per_public_input": 3333,
                "per_proof_byte": 42,
                "per_nullifier": 123,
                "per_commitment": 321
              },
              "transport": {
                "norito_rpc": {
                  "enabled": true,
                  "stage": "ga",
                  "require_mtls": true,
                  "canary_allowlist_size": 2
                },
                "streaming": {
                  "soranet": {
                    "enabled": true,
                    "stream_tag": "norito-stream",
                    "exit_multiaddr": "/dns/torii/udp/9443/quic",
                    "padding_budget_ms": 25,
                    "access_kind": "authenticated",
                    "gar_category": "stream.norito.authenticated",
                    "channel_salt": "iroha.soranet.channel.seed.v1",
                    "provision_spool_dir": "./storage/streaming/soranet_routes",
                    "provision_window_segments": 4,
                    "provision_queue_capacity": 256
                  }
                }
              },
              "nexus": {
                "axt": {
                  "slot_length_ms": 10,
                  "max_clock_skew_ms": 5,
                  "proof_cache_ttl_slots": 4,
                  "replay_retention_slots": 256
                }
              }
            }"#]];
        expected.assert_eq(&actual);
    }

    #[test]
    fn config_update_with_handshake_roundtrip() {
        let update = ConfigUpdateDTO {
            logger: Logger {
                level: Level::INFO,
                filter: None,
            },
            network_acl: None,
            network: Some(NetworkUpdate {
                require_sm_handshake_match: Some(false),
                require_sm_openssl_preview_match: Some(true),
                lane_profile: None,
            }),
            confidential_gas: None,
            soranet_handshake: Some(SoranetHandshakeUpdate {
                descriptor_commit_hex: Some("0a0b0c0d".to_string()),
                client_capabilities_hex: Some("010203".to_string()),
                relay_capabilities_hex: None,
                kem_id: Some(7),
                sig_id: Some(11),
                resume_hash_hex: Some(ResumeHashDirective::Set("deadbeef".to_string())),
                pow: Some(SoranetHandshakePowUpdate {
                    required: Some(true),
                    difficulty: Some(4),
                    max_future_skew_secs: Some(900),
                    min_ticket_ttl_secs: Some(45),
                    ticket_ttl_secs: Some(120),
                    puzzle: None,
                    signed_ticket_public_key_hex: None,
                }),
            }),
            transport: None,
            compute_pricing: None,
        };

        let json = norito::json::to_json(&update).expect("serialize update");
        let parsed: ConfigUpdateDTO = norito::json::from_json(&json).expect("deserialize update");

        assert_eq!(parsed.logger.level, Level::INFO);
        let handshake = parsed
            .soranet_handshake
            .expect("handshake update should roundtrip");
        assert_eq!(handshake.descriptor_commit_hex.as_deref(), Some("0a0b0c0d"));
        assert_eq!(handshake.client_capabilities_hex.as_deref(), Some("010203"));
        assert_eq!(handshake.kem_id, Some(7));
        assert_eq!(handshake.sig_id, Some(11));
        assert_eq!(
            handshake.resume_hash_hex,
            Some(ResumeHashDirective::Set("deadbeef".to_string()))
        );
        let pow = handshake.pow.expect("pow update should be present");
        assert_eq!(pow.required, Some(true));
        assert_eq!(pow.difficulty, Some(4));
        assert_eq!(pow.max_future_skew_secs, Some(900));
        assert_eq!(pow.min_ticket_ttl_secs, Some(45));
        assert_eq!(pow.ticket_ttl_secs, Some(120));
        let network = parsed.network.expect("network update should roundtrip");
        assert_eq!(network.require_sm_handshake_match, Some(false));
        assert_eq!(network.require_sm_openssl_preview_match, Some(true));
    }

    #[test]
    fn vpn_summary_parse_rejects_unknown_exit_class() {
        let mut parser = norito::json::Parser::new(r#"{"exit_class":"ultra-fast"}"#);
        let err = parse_via_fast::<SoranetVpnSummary>(&mut parser).expect_err("unknown class");
        assert!(
            err.to_string().contains("unknown VPN exit class label"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn vpn_summary_parse_rejects_overflowing_lease() {
        let overflowing = u64::from(u32::MAX) + 10;
        let payload = format!(r#"{{"lease_secs": {overflowing}}}"#);
        let mut parser = norito::json::Parser::new(&payload);
        let err = parse_via_fast::<SoranetVpnSummary>(&mut parser).expect_err("lease overflow");
        assert!(
            err.to_string().contains("exceeds u32::MAX"),
            "unexpected error: {err}"
        );
    }
}
