//! Direct host entry point for the Android candidate scenario authority.

use std::{env, error::Error, io::Write as _, path::PathBuf};

fn main() -> Result<(), Box<dyn Error>> {
    let mut arguments = env::args_os().skip(1);
    let mut candidate_record = None;
    let mut candidate_roster = None;
    let mut scenario_dir = None;
    while let Some(option) = arguments.next() {
        let value = arguments
            .next()
            .ok_or_else(|| format!("missing value for {}", option.to_string_lossy()))?;
        let slot = match option.to_str() {
            Some("--candidate-record") => &mut candidate_record,
            Some("--candidate-roster") => &mut candidate_roster,
            Some("--scenario-dir") => &mut scenario_dir,
            _ => return Err(format!("unknown option {}", option.to_string_lossy()).into()),
        };
        if slot.replace(PathBuf::from(value)).is_some() {
            return Err(format!("duplicate option {}", option.to_string_lossy()).into());
        }
    }
    let candidate_record = candidate_record.ok_or("--candidate-record is required")?;
    let candidate_roster = candidate_roster.ok_or("--candidate-roster is required")?;
    let scenario_dir = scenario_dir.ok_or("--scenario-dir is required")?;
    let report = connect_norito_bridge::validate_kagemusha_candidate_scenario_directory_v1(
        &candidate_record,
        &candidate_roster,
        &scenario_dir,
    )
    .map_err(|error| format!("candidate scenario validation failed: {error}"))?;
    std::io::stdout().lock().write_all(&report)?;
    Ok(())
}
