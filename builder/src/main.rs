mod compressor;
mod packer;

use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};
use engine::parser;
use serde_yaml::Value;
use std::{collections::HashMap, path::{Path, PathBuf}};

/// hagane — Installer Engine CLI
#[derive(Parser, Debug)]
#[command(name = "hagane", version, about = "Installer Engine CLI")]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,

    // Legacy (iebuild-compatible) flags. Used when no subcommand is provided.
    #[arg(short, long)]
    manifest: Option<PathBuf>,

    #[arg(short, long)]
    output: Option<PathBuf>,

    #[arg(short = 'l', long)]
    compression_level: Option<i32>,

    #[arg(long)]
    build: bool,

    #[arg(short, long)]
    verbose: bool,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Package manifest + assets into runner/src/generated/embedded.rs
    Build {
        /// Path to installer.yaml
        #[arg(short, long, default_value = "hagane/installer.yaml")]
        manifest: PathBuf,

        /// Output directory for embedded.rs (default: hagane/generated/)
        #[arg(short, long)]
        output: Option<PathBuf>,

        /// Zstd compression level 1-22
        #[arg(short = 'l', long, default_value_t = 9)]
        compression_level: i32,

        /// Print detailed file list during compression
        #[arg(short, long)]
        verbose: bool,
    },

    /// Compile the installer runner (uses generated embedded.rs)
    Pack {
        /// Path to installer.yaml (used to determine output name)
        #[arg(short, long, default_value = "installer.yaml")]
        manifest: PathBuf,

        /// Build with cargo --release
        #[arg(long)]
        release: bool,
    },

    /// Build pipeline in one command: package + compile
    Run {
        /// Path to installer.yaml
        #[arg(default_value = "hagane/installer.yaml")]
        manifest: PathBuf,

        /// Output directory for embedded.rs (default: hagane/generated/)
        #[arg(short, long)]
        output: Option<PathBuf>,

        /// Zstd compression level 1-22
        #[arg(short = 'l', long, default_value_t = 9)]
        compression_level: i32,

        /// Build with cargo --release
        #[arg(long)]
        release: bool,

        /// Print detailed file list during compression
        #[arg(short, long)]
        verbose: bool,
    },
}

#[derive(Debug, Clone)]
struct BuildOptions {
    manifest: PathBuf,
    output: Option<PathBuf>,
    compression_level: i32,
    verbose: bool,
}

fn main() {
    env_logger::Builder::from_env(
        env_logger::Env::default().default_filter_or("info")
    ).init();

    let cli = Cli::parse();
    if let Err(e) = run(cli) {
        log::error!("Build failed: {:#}", e);
        std::process::exit(1);
    }
}

fn run(cli: Cli) -> Result<()> {
    match cli.command {
        Some(Command::Build { manifest, output, compression_level, verbose }) => {
            run_build(BuildOptions { manifest, output, compression_level, verbose })?;
        }
        Some(Command::Pack { manifest, release }) => {
            run_pack(&manifest, release)?;
        }
        Some(Command::Run { manifest, output, compression_level, release, verbose }) => {
            run_build(BuildOptions { manifest: manifest.clone(), output, compression_level, verbose })?;
            run_pack(&manifest, release)?;
        }
        None => {
            // Legacy iebuild-compatible behavior.
            let opts = BuildOptions {
                manifest: cli.manifest.unwrap_or_else(|| PathBuf::from("hagane/installer.yaml")),
                output: cli.output,
                compression_level: cli.compression_level.unwrap_or(9),
                verbose: cli.verbose,
            };
            run_build(opts.clone())?;
            if cli.build {
                run_pack(&opts.manifest, true)?;
            }
        }
    }

    Ok(())
}

fn run_build(args: BuildOptions) -> Result<()> {
    let manifest_path = resolve_manifest_path(&args.manifest)?;
    let workspace_root = resolve_backend_workspace(&manifest_path)?;

    if args.verbose {
        log::debug!("Verbose mode enabled");
    }

    // ── 1. Load and validate manifest ─────────────────────────────────────────
    log::info!("Loading manifest: {}", manifest_path.display());
    let manifest = parser::load_from_file(&manifest_path)
        .context("Manifest load failed")?;
    log::info!("App: {} v{} by {}", manifest.app.name, manifest.app.version, manifest.app.publisher);

    let manifest_dir = manifest_path.parent()
        .unwrap_or(Path::new("."))
        .to_path_buf();

    let manifest_yaml_raw = std::fs::read_to_string(&manifest_path)?;
    let manifest_yaml = normalize_manifest_yaml(&manifest_yaml_raw, &manifest_dir)
        .context("Failed to preprocess manifest")?;

    // When building the Hagane installer itself, stage a fresh runtime workspace
    // into hagane/payload/runtime *before* archive compression so payload.zst
    // always contains up-to-date engine/runner/ui sources.
    if is_hagane_self_manifest(&manifest_path) {
        prepare_bundled_runtime_payload(&workspace_root, &manifest_dir)?;
    }

    // ── 2. Load assets ────────────────────────────────────────────────────────
    let logo_bytes   = packer::read_optional_asset(&manifest_dir, manifest.app.logo.as_deref());
    let banner_bytes = packer::read_optional_asset(&manifest_dir, manifest.app.banner.as_deref());
    let icon_bytes   = packer::read_optional_asset(&manifest_dir, manifest.app.icon.as_deref());

    // ── 3. Compress payload archives ─────────────────────────────────────────
    // Scan for archive source directories referenced in steps
    let archive_sources = collect_archive_sources(&manifest, &manifest_dir)?;
    let mut archives: HashMap<String, Vec<u8>> = HashMap::new();

    if args.verbose {
        for (name, source_path) in &archive_sources {
            log::info!("Archive source: {} -> {}", name, source_path.display());
        }
    }

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
    let out_dir = args.output.unwrap_or_else(|| {
        workspace_root.join("hagane").join("generated")
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

    log::info!("embedded.rs generated. Run `hagane pack --release` to compile.");

    log::info!("Build complete.");
    Ok(())
}

fn run_pack(manifest_path: &Path, release: bool) -> Result<()> {
    let manifest_path = resolve_manifest_path(manifest_path)?;
    let manifest = parser::load_from_file(&manifest_path)
        .context("Manifest load failed")?;
    let manifest_dir = manifest_path
        .parent()
        .unwrap_or(Path::new("."))
        .to_path_buf();
    let workspace_root = resolve_backend_workspace(&manifest_path)?;

    if release {
        log::info!("Running: cargo build --release -p runner");
    } else {
        log::info!("Running: cargo build -p runner");
    }

    let mut cmd = std::process::Command::new("cargo");
    cmd.arg("build");
    if release {
        cmd.arg("--release");
    }

    // Drive runner UAC manifest level from installer.yaml app.require_admin.
    cmd.env(
        "HAGANE_REQUIRE_ADMIN",
        if manifest.app.require_admin { "1" } else { "0" },
    );

    if let Some(icon_rel) = manifest.app.icon.as_deref() {
        let icon_path = manifest_dir.join(icon_rel);
        if icon_path.exists() {
            let icon_for_env = normalize_win_path_for_tools(&icon_path);
            cmd.env("HAGANE_ICON_PATH", &icon_for_env);
            log::info!("Using EXE icon: {}", icon_for_env.display());
        } else {
            log::warn!(
                "Manifest icon configured but not found for EXE stamping: {}",
                icon_path.display()
            );
        }
    }

    cmd.args(["-p", "runner"])
        .current_dir(&workspace_root);

    let status = cmd.status().context("Failed to invoke cargo")?;
    if !status.success() {
        bail!("cargo build failed");
    }

    let profile = if release { "release" } else { "debug" };
    let exe_path = workspace_root
        .join("target").join(profile)
        .join(format!("{}-setup.exe", sanitize_name(&manifest.app.name)));

    let default_exe = workspace_root.join("target").join(profile).join("installer.exe");
    if default_exe.exists() {
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

        // Always copy output near the manifest for NSIS-style discoverability.
        if let Some(manifest_dir) = manifest_path.parent() {
            let project_copy = manifest_dir.join(exe_path.file_name().unwrap_or_default());
            if project_copy.exists() {
                std::fs::remove_file(&project_copy)?;
            }
            std::fs::copy(&exe_path, &project_copy)?;
            log::info!("Copied to project: {}", project_copy.display());
        }

        // In this source workspace, also mirror to hagane/bin.
        let hagane_bin_dir = workspace_root.join("hagane").join("bin");
        if hagane_bin_dir.exists() {
            std::fs::create_dir_all(&hagane_bin_dir)?;
            let hagane_copy = hagane_bin_dir.join(exe_path.file_name().unwrap_or_default());
            if hagane_copy.exists() {
                std::fs::remove_file(&hagane_copy)?;
            }
            std::fs::copy(&exe_path, &hagane_copy)?;
            log::info!("Copied to: {}", hagane_copy.display());
        }
    } else {
        bail!("Expected output not found: {}", default_exe.display());
    }

    log::info!("Pack complete.");
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

fn find_workspace_root(start: &Path) -> Result<PathBuf> {
    // Walk upward until we find Cargo.toml with [workspace]
    let mut dir = start.canonicalize()?;
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

fn resolve_backend_workspace(manifest: &Path) -> Result<PathBuf> {
    let mut candidates: Vec<(String, PathBuf)> = Vec::new();

    // 1) Prefer workspace near the manifest (repo/dev flow)
    let manifest_start = manifest
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
    candidates.push(("manifest directory".to_string(), manifest_start));

    // 2) Optional manual override for power users / CI.
    if let Ok(override_root) = std::env::var("HAGANE_WORKSPACE_ROOT") {
        if !override_root.trim().is_empty() {
            candidates.push((
                "HAGANE_WORKSPACE_ROOT".to_string(),
                PathBuf::from(override_root),
            ));
        }
    }

    // 3) Fallback to installed hagane runtime bundle:
    //    <install>/bin/hagane.exe + <install>/runtime/Cargo.toml
    if let Ok(exe) = std::env::current_exe() {
        if let Some(exe_dir) = exe.parent() {
            let install_root = exe_dir.parent().unwrap_or(exe_dir);
            candidates.push((
                "bundled runtime next to hagane.exe".to_string(),
                install_root.join("runtime"),
            ));

            // 4) Also support dev binary locations (target/debug or target/release)
            // by walking upward from the executable directory itself.
            candidates.push(("directory of hagane.exe".to_string(), exe_dir.to_path_buf()));
        }
    }

    let mut tried = Vec::new();
    for (label, candidate) in candidates {
        if !candidate.exists() {
            tried.push(format!("{}: {} (missing)", label, candidate.display()));
            continue;
        }

        match find_workspace_root(&candidate) {
            Ok(ws) => {
                log::info!("Using backend workspace: {} ({})", ws.display(), label);
                return Ok(ws);
            }
            Err(e) => {
                tried.push(format!("{}: {} ({})", label, candidate.display(), e));
            }
        }
    }

    let details = if tried.is_empty() {
        "no candidate paths were available".to_string()
    } else {
        tried.join("; ")
    };

    bail!(
        "Could not locate backend workspace. Tried: {}. If you are running hagane from outside the source repo, install it with bundled runtime at '<install>/runtime' or set HAGANE_WORKSPACE_ROOT to your Installer-Engine workspace root.",
        details
    )
}

fn is_hagane_self_manifest(manifest: &Path) -> bool {
    manifest
        .parent()
        .and_then(|p| p.file_name())
        .map(|n| n.to_string_lossy().eq_ignore_ascii_case("hagane"))
        .unwrap_or(false)
}

fn prepare_bundled_runtime_payload(workspace_root: &Path, manifest_dir: &Path) -> Result<()> {
    let payload_root = manifest_dir.join("payload");
    let runtime_root = payload_root.join("runtime");

    if runtime_root.exists() {
        std::fs::remove_dir_all(&runtime_root)
            .with_context(|| format!("Failed to clean runtime payload at {}", runtime_root.display()))?;
    }
    std::fs::create_dir_all(&runtime_root)?;

    // Minimal standalone workspace used by installed hagane.
    let runtime_cargo = r#"[workspace]
members = [
    "engine",
    "runner",
]
resolver = "2"

[workspace.dependencies]
serde = { version = "1", features = ["derive"] }
serde_json = "1"
serde_yaml = "0.9"
zstd = "0.13"
rayon = "1.10"
tokio = { version = "1", features = ["full"] }
anyhow = "1"
thiserror = "1"
log = "0.4"
env_logger = "0.11"
chrono = { version = "0.4", features = ["serde"] }
uuid = { version = "1", features = ["v4"] }

[profile.release]
opt-level = 3
lto = true
codegen-units = 1
strip = true
"#;
    std::fs::write(runtime_root.join("Cargo.toml"), runtime_cargo)?;

    copy_dir_recursive(&workspace_root.join("engine"), &runtime_root.join("engine"))?;
    copy_dir_recursive(&workspace_root.join("runner"), &runtime_root.join("runner"))?;
    copy_dir_recursive(&workspace_root.join("ui"), &runtime_root.join("ui"))?;

    let generated_dir = runtime_root.join("hagane").join("generated");
    std::fs::create_dir_all(&generated_dir)?;
    let target_embedded = generated_dir.join("embedded.rs");
    std::fs::write(
        target_embedded,
        "// Runtime placeholder. `hagane build` rewrites this before pack.\n\
pub static MANIFEST_YAML: &[u8] = b\"\";\n\
pub static ASSET_LOGO: &[u8] = &[];\n\
pub static ASSET_BANNER: &[u8] = &[];\n\
pub static ASSET_ICON: &[u8] = &[];\n\
pub static ARCHIVE_MAP: &[u8] = b\"{}\";\n",
    )?;

    log::info!("Bundled runtime prepared at {}", runtime_root.display());
    Ok(())
}

fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<()> {
    if !src.exists() {
        bail!("Required source path not found for runtime bundle: {}", src.display());
    }

    for entry in walkdir::WalkDir::new(src) {
        let entry = entry?;
        let path = entry.path();

        let rel = path.strip_prefix(src)?;
        if rel.as_os_str().is_empty() {
            continue;
        }

        let target = dst.join(rel);
        if entry.file_type().is_dir() {
            std::fs::create_dir_all(&target)?;
            continue;
        }

        let rel_str = rel.to_string_lossy();
        if rel_str.contains("target/") || rel_str.contains("target\\") {
            continue;
        }
        if rel_str.starts_with("src/generated/") || rel_str.starts_with("src\\generated\\") {
            continue;
        }

        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::copy(path, &target)
            .with_context(|| format!("Failed to copy {} -> {}", path.display(), target.display()))?;
    }

    Ok(())
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

fn resolve_manifest_path(input: &Path) -> Result<PathBuf> {
    if input.exists() {
        return Ok(input.canonicalize().unwrap_or_else(|_| input.to_path_buf()));
    }

    // Convenience fallback for repo-root usage:
    // `hagane run installer.yaml` resolves to the repo's root-level hagane package.
    if input.components().count() == 1 {
        let hagane_fallback = Path::new("hagane").join(input);
        if hagane_fallback.exists() {
            return Ok(hagane_fallback
                .canonicalize()
                .unwrap_or(hagane_fallback));
        }

        let example_fallback = Path::new("sdk").join("example").join(input);
        if example_fallback.exists() {
            return Ok(example_fallback
                .canonicalize()
                .unwrap_or(example_fallback));
        }
    }

    bail!(
        "Manifest not found: {}. Run from the manifest directory or pass --manifest with a valid path.",
        input.display()
    )
}

fn normalize_win_path_for_tools(path: &Path) -> PathBuf {
    #[cfg(windows)]
    {
        let s = path.to_string_lossy();
        if let Some(stripped) = s.strip_prefix(r"\\?\") {
            return PathBuf::from(stripped);
        }
    }
    path.to_path_buf()
}