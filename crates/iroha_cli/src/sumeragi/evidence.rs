#![allow(clippy::redundant_pub_crate, clippy::needless_pass_by_value)]
use super::commands::{EvidenceCountArgs, EvidenceKindArg, EvidenceListArgs};
use crate::{CliOutputFormat, RunContext};
use eyre::Result;
use iroha::client::{SumeragiEvidenceAuditRecord, SumeragiEvidencePenaltyStatus};

pub(crate) fn list<C: RunContext>(context: &mut C, args: EvidenceListArgs) -> Result<()> {
    let client = context.client_from_config();
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
    let client = context.client_from_config();
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
    let mut summary = format!(
        "{ordinal}: kind={} class={} height={} view={} epoch={} signer={} context_id={} artifact_hash_1={} artifact_hash_2={} recorded_height={} recorded_view={} recorded_ms={} consensus_admitted_height={} penalty_status={penalty_status}",
        item.kind,
        item.class,
        item.height,
        item.view,
        item.epoch,
        item.signer,
        item.context_id,
        item.artifact_hash_1,
        item.artifact_hash_2,
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
            kind: iroha::client::SumeragiEvidenceKind::SumeragiV2Equivocation,
            class: iroha::client::SumeragiEvidenceClass::PhaseVote,
            height: 42,
            view: 7,
            epoch: 1,
            signer: 3,
            context_id: iroha::client::SumeragiEvidenceHash::from_bytes([0xAA; 32]),
            artifact_hash_1: iroha::client::SumeragiEvidenceHash::from_bytes([0xBB; 32]),
            artifact_hash_2: iroha::client::SumeragiEvidenceHash::from_bytes([0xCC; 32]),
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
        assert!(summary.contains("1: kind=SumeragiV2Equivocation"));
        assert!(summary.contains("height=42"));
        assert!(summary.contains("view=7"));
        assert!(summary.contains("epoch=1"));
        assert!(summary.contains("signer=3"));
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
            summary.starts_with("6: kind=SumeragiV2Equivocation"),
            "unexpected summary: {summary}"
        );
        assert!(summary.contains("penalty_status=applied"));
        assert!(summary.contains("penalty_height=44"));
    }
}
