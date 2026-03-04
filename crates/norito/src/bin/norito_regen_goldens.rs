use std::{
    env,
    error::Error,
    fs,
    path::{Path, PathBuf},
};

use norito::json;

fn default_golden_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("data")
        .join("json_golden")
}

fn collect_cases(dir: &Path) -> Result<Vec<String>, Box<dyn Error>> {
    let mut bases = Vec::new();
    for entry in fs::read_dir(dir)? {
        let path = entry?.path();
        if let Some(name) = path.file_name().and_then(|s| s.to_str())
            && let Some(stripped) = name.strip_suffix(".in.json")
        {
            bases.push(stripped.to_owned());
        }
    }
    bases.sort();
    if bases.is_empty() {
        Err(format!("no *.in.json files found in {dir:?}").into())
    } else {
        Ok(bases)
    }
}

fn regenerate_case(dir: &Path, base: &str) -> Result<(), Box<dyn Error>> {
    let input_path = dir.join(format!("{base}.in.json"));
    let output_path = dir.join(format!("{base}.out.json"));

    let input = fs::read_to_string(&input_path)?;
    let value = json::parse_value(&input)
        .map_err(|e| format!("failed to parse {}: {e}", input_path.display()))?;
    let canonical = json::to_string(&value)?;

    let mut output = canonical;
    output.push('\n');
    fs::write(&output_path, output)?;
    Ok(())
}

fn main() -> Result<(), Box<dyn Error>> {
    let dir = env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(default_golden_dir);
    if !dir.is_dir() {
        return Err(format!("golden directory does not exist: {}", dir.display()).into());
    }

    let cases = collect_cases(&dir)?;
    println!(
        "[norito] regenerating {} golden cases in {}",
        cases.len(),
        dir.display()
    );
    for base in &cases {
        regenerate_case(&dir, base)?;
        println!("[norito] refreshed {base}.out.json");
    }

    println!("[norito] regeneration complete");
    Ok(())
}
