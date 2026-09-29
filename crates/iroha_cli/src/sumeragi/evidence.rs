#![allow(clippy::redundant_pub_crate, clippy::needless_pass_by_value)]
use super::commands::{EvidenceCountArgs, EvidenceKindArg, EvidenceListArgs};
use crate::{CliOutputFormat, RunContext};
use eyre::Result;
use iroha::client::{SumeragiEvidenceAuditRecord, SumeragiEvidencePenaltyStatus};

pub(crate) fn list<C: RunContext>(context: &mut C, args: EvidenceListArgs) -> Result<()> {
    let client = context.client_from_config()?;
    let filter = iroha::client::SumeragiEvidenceListFilter {
        limit: args.limit,
        offset: args.offset,
        kind: args.kind.map(EvidenceKindArg::into_client),
    };
    let response = client.get_sumeragi_evidence_list(filter)?;
    if matches!(context.output_format(), CliOutputFormat::Text) {
        context.println(format!("total={}", response.total))?;
        for (idx, item) in response.items.iter().enumerate() {
            context.println(format_evidence_summary(idx, item))?;
        }
    } else {
        context.print_data(&response)?;
    }
    Ok(())
}

pub(crate) fn count<C: RunContext>(context: &mut C, _args: EvidenceCountArgs) -> Result<()> {
    let client = context.client_from_config()?;
    let response = client.get_sumeragi_evidence_count()?;
    if matches!(context.output_format(), CliOutputFormat::Text) {
        context.println(format!("count={}", response.count))?;
    } else {
        context.print_data(&response)?;
    }
    Ok(())
}

fn format_evidence_summary(idx: usize, item: &SumeragiEvidenceAuditRecord) -> String {
    let ordinal = idx + 1;
    let (penalty_status, penalty_height) = match item.penalty_status {
        SumeragiEvidencePenaltyStatus::Pending => ("pending", None),
        SumeragiEvidencePenaltyStatus::Applied { height } => ("applied", Some(height)),
        SumeragiEvidencePenaltyStatus::Cancelled { height } => ("cancelled", Some(height)),
    };
    let offenders = item
        .offenders
        .iter()
        .map(|offender| format!("{}:{}", offender.signer, offender.peer_id))
        .collect::<Vec<_>>()
        .join(",");
    let mut summary = format!(
        "{ordinal}: kind={} class={} instance={} height={} epoch={} context_id={} authority_generation={} offenders=[{offenders}] safety_violation={} native_frame_hash={} recorded_height={} recorded_view={} recorded_ms={} consensus_admitted_height={} penalty_status={penalty_status}",
        item.kind,
        item.class,
        item.instance,
        item.height,
        item.epoch,
        item.context_id,
        item.authority_generation,
        item.safety_violation,
        item.native_frame_hash,
        item.recorded_height,
        item.recorded_view,
        item.recorded_ms,
        item.consensus_admitted_height,
    );
    if let Some(height) = penalty_height {
        summary.push_str(&format!(" penalty_height={height}"));
    }
    summary
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record(penalty_status: SumeragiEvidencePenaltyStatus) -> SumeragiEvidenceAuditRecord {
        SumeragiEvidenceAuditRecord {
            kind: iroha::client::SumeragiEvidenceKind::NativeSumeragiEvidence,
            class: iroha::client::SumeragiEvidenceClass::PhaseVote,
            height: 42,
            epoch: 1,
            context_id: iroha::client::SumeragiEvidenceHash::from_bytes([0xAA; 32]),
            instance: iroha::client::SumeragiEvidenceHash::from_bytes([0xBB; 32]),
            authority_generation: iroha::client::SumeragiEvidenceHash::from_bytes([0xCC; 32]),
            native_frame_hash: iroha::client::SumeragiEvidenceHash::from_bytes([0xDD; 32]),
            offenders: vec![iroha::client::SumeragiEvidenceOffender {
                signer: 3,
                peer_id: iroha_model_base::peer::PeerId::new(
                    iroha_crypto::KeyPair::try_from_seed(
                        vec![0x31; 32],
                        iroha_crypto::Algorithm::BlsNormal,
                    )
                    .expect("evidence offender key")
                    .public_key()
                    .clone(),
                ),
            }],
            safety_violation: false,
            recorded_height: 43,
            recorded_view: 8,
            recorded_ms: 1234,
            consensus_admitted_height: 43,
            penalty_status,
        }
    }

    #[test]
    fn format_evidence_summary_includes_every_typed_field_and_pending_status() {
        let summary = format_evidence_summary(0, &record(SumeragiEvidencePenaltyStatus::Pending));
        assert!(summary.contains("1: kind=NativeSumeragiEvidence"));
        assert!(summary.contains("height=42"));
        assert!(summary.contains("recorded_view=8"));
        assert!(summary.contains("epoch=1"));
        assert!(summary.contains("offenders=[3:ea0130"));
        assert!(summary.contains("instance="));
        assert!(summary.contains("authority_generation="));
        assert!(summary.contains("safety_violation=false"));
        assert!(summary.contains("native_frame_hash="));
        assert!(summary.contains("class=phase_vote"));
        assert!(summary.contains("context_id="));
        assert!(summary.contains("consensus_admitted_height=43"));
        assert!(summary.contains("recorded_ms=1234"));
        assert!(summary.contains("penalty_status=pending"));
        assert!(!summary.contains("penalty_height="));
    }

    #[test]
    fn format_evidence_summary_uses_index_offset_and_terminal_penalty_height() {
        let summary = format_evidence_summary(
            5,
            &record(SumeragiEvidencePenaltyStatus::Applied { height: 44 }),
        );
        assert!(
            summary.starts_with("6: kind=NativeSumeragiEvidence"),
            "unexpected summary: {summary}"
        );
        assert!(summary.contains("penalty_status=applied"));
        assert!(summary.contains("penalty_height=44"));
    }
}
