mod compressor;
mod packer;

use anyhow::{bail, Context, Result};
use clap::Parser;
use engine::parser;
use serde_yaml::Value;
use std::{collections::HashMap, path::{Path, PathBuf}};

/// iebuild — Installer Engine Build Tool
/// Packages your installer.yaml + assets + payload into a deployable .exe
#[derive(Parser, Debug)]
#[command(name = "iebuild", version, about)]
struct Args {
    /// Path to installer.yaml
    #[arg(short, long, default_value = "installer.yaml")]
    manifest: PathBuf,

    /// Output directory for the generated embedded.rs (default: runner/src/generated/)
    #[arg(short, long)]
    output: Option<PathBuf>,

    /// Zstd compression level 1–22 (default: 9, use 19–22 for max compression)
    #[arg(short = 'l', long, default_value_t = 9)]
    compression_level: i32,

    /// Also invoke `cargo build --release` after generating embedded.rs
    #[arg(long)]
    build: bool,

    /// Print detailed file list during compression
    #[arg(short, long)]
    verbose: bool,
}

fn main() {
    env_logger::Builder::from_env(
        env_logger::Env::default().default_filter_or("info")
    ).init();

    let args = Args::parse();
    if let Err(e) = run(args) {
        log::error!("Build failed: {:#}", e);
        std::process::exit(1);
    }
}

fn run(args: Args) -> Result<()> {
    // ── 1. Load and validate manifest ─────────────────────────────────────────
    log::info!("Loading manifest: {}", args.manifest.display());
    let manifest = parser::load_from_file(&args.manifest)
        .context("Manifest load failed")?;
    log::info!("App: {} v{} by {}", manifest.app.name, manifest.app.version, manifest.app.publisher);

    let manifest_dir = args.manifest.parent()
        .unwrap_or(Path::new("."))
        .to_path_buf();

    let manifest_yaml_raw = std::fs::read_to_string(&args.manifest)?;
    let manifest_yaml = normalize_manifest_yaml(&manifest_yaml_raw, &manifest_dir)
        .context("Failed to preprocess manifest")?;

    // ── 2. Load assets ────────────────────────────────────────────────────────
    let logo_bytes   = packer::read_optional_asset(&manifest_dir, manifest.app.logo.as_deref());
    let banner_bytes = packer::read_optional_asset(&manifest_dir, manifest.app.banner.as_deref());
    let icon_bytes   = packer::read_optional_asset(&manifest_dir, manifest.app.icon.as_deref());

    // ── 3. Compress payload archives ─────────────────────────────────────────
    // Scan for archive source directories referenced in steps
    let archive_sources = collect_archive_sources(&manifest, &manifest_dir)?;
    let mut archives: HashMap<String, Vec<u8>> = HashMap::new();

    for (name, source_path) in &archive_sources {
        log::info!("Compressing archive '{}' from: {}", name, source_path.display());
        let compressed = if source_path.is_dir() {
            compressor::compress_directory(source_path, args.compression_level)
        } else {
            compressor::compress_file(source_path, args.compression_level)
        }.with_context(|| format!("Compression failed for archive '{}'", name))?;

        log::info!("  Archive '{}': {} KB compressed", name, compressed.len() / 1024);
        archives.insert(name.clone(), compressed);
    }

    // ── 4. Generate embedded.rs ───────────────────────────────────────────────
    let workspace_root = find_workspace_root(&args.manifest)?;
    let out_dir = args.output.unwrap_or_else(|| {
        workspace_root.join("runner").join("src").join("generated")
    });
    let embedded_rs_path = out_dir.join("embedded.rs");

    packer::generate_embedded_rs(
        &manifest_yaml,
        &logo_bytes,
        &banner_bytes,
        &icon_bytes,
        &archives,
        &embedded_rs_path,
    )?;

    // ── 5. Optionally run cargo build ─────────────────────────────────────────
    if args.build {
        log::info!("Running: cargo build --release");
        let status = std::process::Command::new("cargo")
            .args(["build", "--release", "-p", "runner"])
            .current_dir(&workspace_root)
            .status()
            .context("Failed to invoke cargo")?;

        if !status.success() {
            bail!("cargo build --release failed");
        }

        let exe_path = workspace_root
            .join("target").join("release")
            .join(format!("{}-setup.exe", sanitize_name(&manifest.app.name)));

        // Rename the output binary
        let default_exe = workspace_root.join("target").join("release").join("installer.exe");
        if default_exe.exists() {
            // Windows rename fails if destination already exists.
            if exe_path.exists() {
                if let Err(e) = std::fs::remove_file(&exe_path) {
                    bail!(
                        "Cannot overwrite existing output '{}': {}. Close any running installer and try again.",
                        exe_path.display(),
                        e
                    );
                }
            }
            std::fs::rename(&default_exe, &exe_path)?;
            log::info!("Output: {}", exe_path.display());
        }
    } else {
        log::info!("embedded.rs generated. Run `cargo build --release -p runner` to compile.");
    }

    log::info!("Build complete.");
    Ok(())
}

/// Collect archive name → source path mappings from Extract steps.
fn collect_archive_sources(
    manifest: &engine::parser::schema::InstallerManifest,
    manifest_dir: &Path,
) -> Result<Vec<(String, PathBuf)>> {
    let mut sources = Vec::new();
    let mut seen = std::collections::HashSet::new();

    for step in &manifest.steps {
        if let engine::parser::schema::InstallStep::Extract(e) = step {
            if seen.contains(&e.archive) { continue; }
            seen.insert(e.archive.clone());

            // Convention: archive source lives alongside installer.yaml
            // in a folder named after the archive (without extension)
            let archive_stem = Path::new(&e.archive)
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();

            let source_dir = manifest_dir.join(&archive_stem);
            if !source_dir.exists() {
                log::warn!(
                    "Archive source directory not found: {} — archive '{}' will be empty",
                    source_dir.display(), e.archive
                );
                continue;
            }
            sources.push((e.archive.clone(), source_dir));
        }
    }
    Ok(sources)
}

fn find_workspace_root(manifest: &Path) -> Result<PathBuf> {
    // Walk upward until we find Cargo.toml with [workspace]
    let mut dir = manifest.parent().unwrap_or(Path::new(".")).canonicalize()?;
    loop {
        let cargo = dir.join("Cargo.toml");
        if cargo.exists() {
            let content = std::fs::read_to_string(&cargo).unwrap_or_default();
            if content.contains("[workspace]") {
                return Ok(dir);
            }
        }
        match dir.parent() {
            Some(p) => dir = p.to_path_buf(),
            None => bail!("Could not find workspace root (Cargo.toml with [workspace])"),
        }
    }
}

fn sanitize_name(name: &str) -> String {
    name.chars()
        .map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '-' })
        .collect::<String>()
        .to_lowercase()
}

/// Allow clean manifests by supporting `pages[].license_file`.
///
/// Example:
/// pages:
///   - type: license
///     license_file: docs/license.txt
///
/// This loader injects the file content into `data.text` for runtime use.
fn normalize_manifest_yaml(raw_yaml: &str, manifest_dir: &Path) -> Result<String> {
    let mut root: Value = serde_yaml::from_str(raw_yaml)
        .context("YAML parse failed during preprocessing")?;

    let Some(pages) = root.get_mut("pages").and_then(|v| v.as_sequence_mut()) else {
        return Ok(raw_yaml.to_string());
    };

    for page in pages.iter_mut() {
        let is_license = page
            .get("type")
            .and_then(|v| v.as_str())
            .map(|t| t.eq_ignore_ascii_case("license"))
            .unwrap_or(false);
        if !is_license {
            continue;
        }

        let license_file = page
            .get("license_file")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let Some(rel_path) = license_file else {
            continue;
        };

        let full = manifest_dir.join(&rel_path);
        let text = std::fs::read_to_string(&full)
            .with_context(|| format!("Failed to read license_file '{}': {}", rel_path, full.display()))?;

        let page_map = page
            .as_mapping_mut()
            .context("Page node is not a map")?;

        let data_key = Value::String("data".to_string());
        if !page_map.contains_key(&data_key) {
            page_map.insert(data_key.clone(), Value::Mapping(Default::default()));
        }

        let data_map = page_map
            .get_mut(&data_key)
            .and_then(|v| v.as_mapping_mut())
            .context("pages[].data must be a map")?;

        data_map.insert(Value::String("text".to_string()), Value::String(text));

        // Keep the final embedded manifest clean/portable after preprocessing.
        page_map.remove(Value::String("license_file".to_string()));
    }

    serde_yaml::to_string(&root).context("Failed to serialize preprocessed manifest")
}