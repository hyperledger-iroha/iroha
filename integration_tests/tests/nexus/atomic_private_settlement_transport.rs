//! Source-bound local transport observations for the real-process happy-day experiment.
//! Native finality separately authenticates the original manifest and result. These logs
//! establish execution by the retained test binary, not Byzantine remote-transport proofs.
use eyre::{Result, ensure, eyre};
use norito::json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

pub(super) struct Expected {
    pub process_id: u32,
    pub instance: String,
    pub height: u64,
    pub block: String,
    pub result: String,
    pub availability: String,
    pub payload: String,
    pub bytes: u64,
    pub epoch: u64,
    pub context: String,
    pub proposer: u64,
    pub local_is_author: bool,
    pub local: String,
    pub peers: BTreeSet<String>,
    pub k: usize,
    pub width: usize,
    pub stripes: usize,
}

/// Exact audit bytes and their offsets in the source-bound process log prefix.
#[derive(Debug, norito::JsonSerialize)]
pub(super) struct AuditLine {
    pub offset: usize,
    pub line: String,
}

pub(super) struct Verified {
    pub admitted_rows: usize,
    pub audit_lines: Vec<AuditLine>,
}

/// A filesystem maximum run number is not the owned process's current run.
pub(super) fn current_run_log(
    peer_dir: &Path,
    live_run: Option<usize>,
    stdout: &Path,
    stderr: &Path,
) -> Result<usize> {
    let run = live_run
        .filter(|run| *run > 0)
        .ok_or_else(|| eyre!("transport source lacks a current process run"))?;
    ensure!(
        stdout == peer_dir.join(format!("run-{run}-stdout.log"))
            && stderr == peer_dir.join(format!("run-{run}-stderr.log")),
        "transport log does not belong to the owned current process run"
    );
    Ok(run)
}
fn text<'a>(fields: &'a Value, key: &str) -> Result<&'a str> {
    fields
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| eyre!("transport observation lacks {key}"))
}
fn number(fields: &Value, key: &str) -> Result<u64> {
    fields
        .get(key)
        .and_then(Value::as_u64)
        .ok_or_else(|| eyre!("transport observation lacks {key}"))
}
#[derive(Default)]
struct Rows {
    indices: BTreeSet<usize>,
    bad: bool,
}

/// Require actual distinct authenticated row admission, then successful reconstruction,
/// then the same process applying the independently certified decision. An author may use
/// its own actual authored codeword. Storage restoration supplies neither observation.
pub(super) fn verify(log: &[u8], expected: &Expected) -> Result<Verified> {
    ensure!(log.len() <= 64 * 1024 * 1024, "transport log exceeds bound");
    ensure!(
        expected.process_id > 0
            && expected.k > 0
            && expected.width > expected.k
            && expected.stripes > 0,
        "invalid independently resolved transport geometry"
    );
    let total_rows = expected
        .stripes
        .checked_mul(expected.width)
        .ok_or_else(|| eyre!("transport geometry overflows"))?;
    let raw = std::str::from_utf8(log)?;
    ensure!(
        raw.ends_with('\n'),
        "transport prefix lacks a complete line"
    );
    let mut rows: BTreeMap<u64, Rows> = BTreeMap::new();
    let mut custody = false;
    let mut applied = 0;
    let mut count = 0;
    let mut offset = 0;
    let mut audit_lines = Vec::new();
    for raw_line in raw.split_inclusive('\n') {
        let line_offset = offset;
        offset += raw_line.len();
        let line = raw_line.trim_end_matches('\n');
        if !line.contains("sumeragi payload") && !line.contains("sumeragi block applied") {
            continue;
        }
        ensure!(
            line.len() <= 1 << 20,
            "transport observation exceeds line bound"
        );
        let value: Value = norito::json::from_str(line)?;
        let fields = value
            .get("fields")
            .ok_or_else(|| eyre!("transport log lacks fields"))?;
        let message = text(fields, "message")?;
        if !matches!(
            message,
            "sumeragi payload row admitted"
                | "sumeragi payload custody verified"
                | "sumeragi block applied"
        ) {
            continue;
        }
        if text(fields, "instance")? != expected.instance
            || number(fields, "height")? != expected.height
        {
            continue;
        }
        if text(fields, "block")? != expected.block {
            continue;
        }
        ensure!(
            number(fields, "process_id")? == u64::from(expected.process_id),
            "transport observation belongs to another process"
        );
        audit_lines.push(AuditLine {
            offset: line_offset,
            line: raw_line.to_owned(),
        });
        if message == "sumeragi block applied" {
            ensure!(
                custody && text(fields, "result")? == expected.result,
                "application differs from certified result or lacks transport custody"
            );
            applied += 1;
            continue;
        }
        ensure!(applied == 0, "transport custody appeared after application");
        ensure!(
            text(fields, "availability_digest")? == expected.availability,
            "transport table substituted"
        );
        let id = number(fields, "acquisition")?;
        if message == "sumeragi payload row admitted" {
            ensure!(
                id != 0 && (rows.contains_key(&id) || rows.len() < 4096),
                "invalid or excessive acquisition identities"
            );
            let row = rows.entry(id).or_default();
            let index = usize::try_from(number(fields, "index")?)?;
            let from = text(fields, "from")?;
            row.bad |= index >= total_rows
                || from == expected.local
                || !expected.peers.contains(from)
                || !row.indices.insert(index);
            continue;
        }
        ensure!(
            number(fields, "epoch")? == expected.epoch
                && text(fields, "context")? == expected.context
                && number(fields, "proposer")? == expected.proposer
                && number(fields, "payload_bytes")? == expected.bytes
                && text(fields, "payload_hash")? == expected.payload,
            "transport custody differs from certified original source"
        );
        let accepted = usize::try_from(number(fields, "accepted_rows")?)?;
        match text(fields, "origin")? {
            "author" => ensure!(
                expected.local_is_author && id == 0 && accepted == 0,
                "non-author substituted authored custody"
            ),
            "network_rows" => {
                let row = rows
                    .remove(&id)
                    .ok_or_else(|| eyre!("network custody lacks actual rows"))?;
                ensure!(
                    !row.bad && row.indices.len() == accepted,
                    "row evidence duplicates or substitutes sender/index"
                );
                for stripe in 0..expected.stripes {
                    ensure!(
                        row.indices
                            .range(stripe * expected.width..(stripe + 1) * expected.width)
                            .count()
                            >= expected.k,
                        "network custody lacks distinct rows in stripe {stripe}"
                    );
                }
                count = accepted;
            }
            _ => return Err(eyre!("storage or unknown custody cannot qualify transport")),
        }
        ensure!(!custody, "duplicate custody completion");
        custody = true;
    }
    ensure!(
        custody && applied == 1,
        "missing or repeated transport/application observation"
    );
    Ok(Verified {
        admitted_rows: count,
        audit_lines,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn expected() -> Expected {
        Expected {
            process_id: 42,
            instance: "i".into(),
            height: 9,
            block: "b".into(),
            result: "r".into(),
            availability: "a".into(),
            payload: "p".into(),
            bytes: 16,
            epoch: 1,
            context: "c".into(),
            proposer: 0,
            local_is_author: false,
            local: "local".into(),
            peers: BTreeSet::from(["relay".into()]),
            k: 2,
            width: 3,
            stripes: 2,
        }
    }
    fn line(extra: &str) -> String {
        format!(
            "{{\"fields\":{{\"process_id\":42,\"instance\":\"i\",\"height\":9,\"block\":\"b\",{extra}}}}}\n"
        )
    }
    fn valid() -> String {
        let mut s = String::new();
        for index in [0, 1, 3, 4] {
            s.push_str(&line(&format!("\"message\":\"sumeragi payload row admitted\",\"availability_digest\":\"a\",\"acquisition\":1,\"index\":{index},\"from\":\"relay\"")));
        }
        s.push_str(&line("\"message\":\"sumeragi payload custody verified\",\"availability_digest\":\"a\",\"acquisition\":1,\"epoch\":1,\"context\":\"c\",\"proposer\":0,\"payload_bytes\":16,\"payload_hash\":\"p\",\"accepted_rows\":4,\"origin\":\"network_rows\""));
        s.push_str(&line(
            "\"message\":\"sumeragi block applied\",\"result\":\"r\"",
        ));
        s
    }
    #[test]
    fn actual_distinct_rows_then_custody_then_application_qualify() {
        assert_eq!(
            verify(valid().as_bytes(), &expected())
                .unwrap()
                .admitted_rows,
            4
        );
    }
    #[test]
    fn substituted_or_incomplete_transport_never_qualifies() {
        let s = valid();
        let cases = [
            s.replace("\"process_id\":42", "\"process_id\":43"),
            s.replace("\"process_id\":42,", ""),
            s.replace("\"result\":\"r\"", "\"result\":\"other\""),
            s.replace("network_rows", "author"),
            s.replace("network_rows", "stored"),
            s.replace("\"index\":1", "\"index\":0"),
            s.replace("\"index\":4", "\"index\":2"),
            s.replace("\"from\":\"relay\"", "\"from\":\"unknown\""),
            s.replace(
                "\"availability_digest\":\"a\"",
                "\"availability_digest\":\"other\"",
            ),
            s.replace("\"context\":\"c\"", "\"context\":\"other\""),
            s.replace("\"height\":9", "\"height\":8"),
            s.lines().skip(4).collect::<Vec<_>>().join("\n") + "\n",
            s.lines().take(4).collect::<Vec<_>>().join("\n") + "\n",
            s.lines().rev().collect::<Vec<_>>().join("\n") + "\n",
            format!("{s}{s}"),
            s[..s.len() - 5].to_owned(),
        ];
        for (i, bad) in cases.iter().enumerate() {
            assert!(verify(bad.as_bytes(), &expected()).is_err(), "case {i}");
        }
    }
    #[test]
    fn original_author_only_qualifies_on_the_author_process() {
        let s = valid()
            .lines()
            .skip(4)
            .collect::<Vec<_>>()
            .join("\n")
            .replace("network_rows", "author")
            .replace("\"acquisition\":1", "\"acquisition\":0")
            .replace("\"accepted_rows\":4", "\"accepted_rows\":0")
            + "\n";
        assert!(verify(s.as_bytes(), &expected()).is_err());
        let mut e = expected();
        e.local_is_author = true;
        assert_eq!(verify(s.as_bytes(), &e).unwrap().admitted_rows, 0);
    }

    #[test]
    fn stale_future_foreign_and_missing_run_logs_are_rejected() {
        let dir = Path::new("network/peer-a");
        let stdout = dir.join("run-2-stdout.log");
        let stderr = dir.join("run-2-stderr.log");
        assert_eq!(current_run_log(dir, Some(2), &stdout, &stderr).unwrap(), 2);
        for run in [None, Some(0), Some(1), Some(3)] {
            assert!(current_run_log(dir, run, &stdout, &stderr).is_err());
        }
        for wrong in [
            dir.join("run-1-stdout.log"),
            dir.join("run-3-stdout.log"),
            Path::new("network/peer-b/run-2-stdout.log").to_owned(),
        ] {
            assert!(current_run_log(dir, Some(2), &wrong, &stderr).is_err());
        }
        assert!(current_run_log(dir, Some(2), &stdout, &dir.join("run-1-stderr.log")).is_err());
    }

    #[test]
    fn retained_lines_bind_exact_original_offsets_and_process() {
        let prefix = "unrelated λ output\n";
        let log = format!("{prefix}{}unrelated tail\n", valid());
        let verified = verify(log.as_bytes(), &expected()).unwrap();
        assert_eq!(verified.audit_lines.len(), 6);
        assert_eq!(verified.audit_lines[0].offset, prefix.len());
        let mut retained = String::new();
        for line in &verified.audit_lines {
            assert_eq!(&log[line.offset..line.offset + line.line.len()], line.line);
            retained.push_str(&line.line);
        }
        assert_eq!(
            verify(retained.as_bytes(), &expected())
                .unwrap()
                .admitted_rows,
            4
        );
        let mut wrong = expected();
        wrong.process_id += 1;
        assert!(verify(retained.as_bytes(), &wrong).is_err());
    }

    #[test]
    fn overflowing_geometry_and_unterminated_prefix_are_rejected() {
        let mut e = expected();
        e.stripes = usize::MAX;
        assert!(verify(valid().as_bytes(), &e).is_err());
        assert!(verify(valid().trim_end().as_bytes(), &expected()).is_err());
    }
}
