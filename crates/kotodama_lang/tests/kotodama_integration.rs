//! Consolidated integration-test harness for Kotodama language coverage.
#[path = "branded_declarations.rs"]
mod branded_declarations;
#[path = "compile_fail_goldens.rs"]
mod compile_fail_goldens;
#[path = "cst_lossless.rs"]
mod cst_lossless;
#[path = "cst_structure.rs"]
mod cst_structure;
#[path = "documentation_fences.rs"]
mod documentation_fences;
#[path = "frontend_budgets.rs"]
mod frontend_budgets;
#[path = "parser_recovery.rs"]
mod parser_recovery;
#[path = "secret_security_diagnostics.rs"]
mod secret_security_diagnostics;
#[path = "sugar_zero_cost.rs"]
mod sugar_zero_cost;
#[path = "v1_contract_edges.rs"]
mod v1_contract_edges;
