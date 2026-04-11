fn main() {
  let fallback_icon = "../sdk/example/assets/icon.ico";
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
        // Request admin elevation via UAC
        res.set_manifest(r#"
<assembly xmlns="urn:schemas-microsoft-com:asm.v1" manifestVersion="1.0">
  <trustInfo xmlns="urn:schemas-microsoft-com:asm.v3">
    <security>
      <requestedPrivileges>
        <requestedExecutionLevel level="requireAdministrator" uiAccess="false"/>
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
"#);
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