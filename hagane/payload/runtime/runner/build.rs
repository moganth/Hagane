fn main() {
  let fallback_icon = "../sdk/example/assets/icon.ico";
  let require_admin = std::env::var("HAGANE_REQUIRE_ADMIN")
    .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
    .unwrap_or(true);
  let execution_level = if require_admin { "requireAdministrator" } else { "asInvoker" };
  let icon_path = std::env::var("HAGANE_ICON_PATH")
    .ok()
    .filter(|v| !v.trim().is_empty())
    .map(std::path::PathBuf::from)
    .filter(|p| p.exists())
    .or_else(|| {
      let p = std::path::PathBuf::from(fallback_icon);
      if p.exists() {
        Some(p)
      } else {
        None
      }
    });

    #[cfg(windows)]
    {
        let mut res = winres::WindowsResource::new();
        // UAC level is selected from installer.yaml app.require_admin.
        res.set_manifest(&format!(r#"
<assembly xmlns="urn:schemas-microsoft-com:asm.v1" manifestVersion="1.0">
  <trustInfo xmlns="urn:schemas-microsoft-com:asm.v3">
    <security>
      <requestedPrivileges>
        <requestedExecutionLevel level="{}" uiAccess="false"/>
      </requestedPrivileges>
    </security>
  </trustInfo>
  <application xmlns="urn:schemas-microsoft-com:asm.v3">
    <windowsSettings>
      <dpiAware xmlns="http://schemas.microsoft.com/SMI/2005/WindowsSettings">true/pm</dpiAware>
      <dpiAwareness xmlns="http://schemas.microsoft.com/SMI/2016/WindowsSettings">PerMonitorV2</dpiAwareness>
    </windowsSettings>
  </application>
</assembly>
"#, execution_level));
        // Embed app icon if present
    if let Some(path) = icon_path.as_ref() {
      let normalized = normalize_icon_path(path);
      if let Some(icon) = normalized.to_str() {
        res.set_icon(icon);
      }
        }
        if let Err(e) = res.compile() {
            eprintln!("winres compile warning: {}", e);
        }
    }
    println!("cargo:rerun-if-changed=build.rs");
  println!("cargo:rerun-if-env-changed=HAGANE_REQUIRE_ADMIN");
  println!("cargo:rerun-if-env-changed=HAGANE_ICON_PATH");
  if let Some(path) = icon_path {
    println!("cargo:rerun-if-changed={}", path.display());
  } else {
    println!("cargo:rerun-if-changed={}", fallback_icon);
  }
}

fn normalize_icon_path(path: &std::path::Path) -> std::path::PathBuf {
  #[cfg(windows)]
  {
    let s = path.to_string_lossy();
    if let Some(stripped) = s.strip_prefix(r"\\?\") {
      return std::path::PathBuf::from(stripped);
    }
  }
  path.to_path_buf()
}