//! Read-only diagnosis; never executes the detected test command.

use crate::{config, testrun};
use serde::Serialize;
use std::path::Path;

#[derive(Serialize)]
pub struct Report {
    tool: &'static str,
    version: &'static str,
    cwd: String,
    config_source: &'static str,
    config: config::Config,
    test_command: Option<String>,
    detected_from: String,
    command_executed: bool,
    warnings: Vec<String>,
}

pub fn inspect(cwd: &Path) -> Result<Report, String> {
    let config = config::load(cwd)?;
    let (test_command, detected_from) = testrun::detect(cwd, &config);
    let mut warnings = Vec::new();
    if test_command.is_none() {
        warnings.push("no test command detected; set test_command in .stopproof.json or use run --command. run --strict will fail until a command is available.".into());
    }
    if config.mode == "off" {
        warnings.push("hook mode is off; manual run still performs verification".into());
    }
    Ok(Report {
        tool: "stopproof",
        version: crate::VERSION,
        cwd: cwd.display().to_string(),
        config_source: if cwd.join(config::CONFIG_FILE).exists() {
            config::CONFIG_FILE
        } else {
            "defaults"
        },
        config,
        test_command,
        detected_from,
        command_executed: false,
        warnings,
    })
}

pub fn print(report: &Report, json: bool) {
    if json {
        println!(
            "{}",
            serde_json::to_string(report).expect("report contains only JSON values")
        );
        return;
    }
    println!("stopproof {} — doctor\n", report.version);
    println!(
        "directory: {}\nconfiguration: {}",
        report.cwd, report.config_source
    );
    println!(
        "{}",
        serde_json::to_string_pretty(&report.config).expect("config contains only JSON values")
    );
    println!(
        "test command: {} (from {}; not executed)",
        report.test_command.as_deref().unwrap_or("none"),
        report.detected_from
    );
    for warning in &report.warnings {
        println!("[WARN] {}", warning);
    }
}
