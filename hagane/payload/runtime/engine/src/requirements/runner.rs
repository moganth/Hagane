use anyhow::Result;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use crate::parser::schema::Requirement;
use super::{disk, memory, os, registry};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckResult {
    pub id: String,
    pub label: String,
    pub passed: bool,
    pub detail: String,
}

/// Runs all requirements in parallel. Returns results in the same order as input.
/// Each check is a direct native API call — no subprocess spawning.
pub fn run_all(requirements: &[Requirement], install_dir: &str) -> Vec<CheckResult> {
    requirements
        .par_iter()
        .enumerate()
        .map(|(i, req)| run_single(i, req, install_dir))
        .collect()
}

fn run_single(idx: usize, req: &Requirement, install_dir: &str) -> CheckResult {
    match req {
        Requirement::Os(r) => {
            let label = r.label.clone().unwrap_or_else(|| {
                format!("Windows {}", r.min_build.map(build_to_name).unwrap_or_default())
            });
            match run_os_check(r) {
                Ok(passed) => {
                    let detail = if passed {
                        match os::get_os_info() {
                            Ok(info) => format!("Build {} detected", info.build),
                            Err(_) => "Passed".into(),
                        }
                    } else {
                        format!("Minimum build {} required", r.min_build.unwrap_or(0))
                    };
                    CheckResult { id: format!("os_{}", idx), label, passed, detail }
                }
                Err(e) => CheckResult { id: format!("os_{}", idx), label, passed: false, detail: e.to_string() },
            }
        }

        Requirement::Ram(r) => {
            let label = r.label.clone().unwrap_or_else(|| format!("{}MB RAM", r.min_mb));
            match memory::get_total_ram_mb() {
                Ok(actual_mb) => {
                    let passed = actual_mb >= r.min_mb;
                    let detail = format!("{}MB available, {}MB required", actual_mb, r.min_mb);
                    CheckResult { id: format!("ram_{}", idx), label, passed, detail }
                }
                Err(e) => CheckResult { id: format!("ram_{}", idx), label, passed: false, detail: e.to_string() },
            }
        }

        Requirement::Disk(r) => {
            let path = r.path.as_deref().unwrap_or(install_dir);
            let drive = extract_drive(path);
            let label = r.label.clone().unwrap_or_else(|| format!("{}MB free on {}", r.min_mb, drive));
            match disk::get_free_disk_mb(&drive) {
                Ok(free_mb) => {
                    let passed = free_mb >= r.min_mb;
                    let detail = format!("{}MB free, {}MB required", free_mb, r.min_mb);
                    CheckResult { id: format!("disk_{}", idx), label, passed, detail }
                }
                Err(e) => CheckResult { id: format!("disk_{}", idx), label, passed: false, detail: e.to_string() },
            }
        }

        Requirement::Dotnet(r) => {
            let label = r.label.clone().unwrap_or_else(|| format!(".NET Framework {}", r.min_version));
            match registry::check_dotnet_framework(&r.min_version) {
                Ok(passed) => {
                    let detail = if passed { format!(".NET {} or newer found", r.min_version) }
                                 else { format!(".NET {} or newer not found", r.min_version) };
                    CheckResult { id: format!("dotnet_{}", idx), label, passed, detail }
                }
                Err(e) => CheckResult { id: format!("dotnet_{}", idx), label, passed: false, detail: e.to_string() },
            }
        }

        Requirement::VcRedist(r) => {
            let arch = r.arch.as_deref();
            let label = r.label.clone().unwrap_or_else(|| {
                format!("VC++ {} Redistributable ({})", r.year, arch.unwrap_or("x64"))
            });
            match registry::check_vc_redist(&r.year, arch) {
                Ok(passed) => {
                    let detail = if passed { "Installed".into() } else { "Not installed".into() };
                    CheckResult { id: format!("vc_{}", idx), label, passed, detail }
                }
                Err(e) => CheckResult { id: format!("vc_{}", idx), label, passed: false, detail: e.to_string() },
            }
        }

        Requirement::Custom(r) => {
            // Custom checks are always treated as optional warnings
            log::warn!("Custom requirement '{}' — native check not available", r.id);
            CheckResult {
                id: r.id.clone(),
                label: r.label.clone(),
                passed: true,
                detail: "Custom check skipped (native mode)".into(),
            }
        }
    }
}

fn run_os_check(r: &crate::parser::schema::OsRequirement) -> Result<bool> {
    if r.platform != "windows" {
        return Ok(cfg!(windows));
    }
    if let Some(min_build) = r.min_build {
        return os::meets_build_requirement(min_build);
    }
    Ok(true)
}

fn extract_drive(path: &str) -> String {
    // Extract drive letter root, e.g. "C:\some\path" → "C:\"
    if path.len() >= 2 && path.chars().nth(1) == Some(':') {
        format!("{}\\", &path[..2])
    } else {
        "C:\\".to_string()
    }
}

fn build_to_name(build: u32) -> String {
    match build {
        b if b >= os::builds::WIN11_22H2 => format!("11 22H2+ (build {})", b),
        b if b >= os::builds::WIN11_RTM  => format!("11 (build {})", b),
        b if b >= os::builds::WIN10_1903 => format!("10 1903+ (build {})", b),
        b if b >= os::builds::WIN10_RTM  => format!("10 (build {})", b),
        b => format!("build {}", b),
    }
}