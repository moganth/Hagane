fn main() {
  let fallback_icon = "../sdk/example/assets/icon.ico";
  let require_admin = std::env::var("HAGANE_REQUIRE_ADMIN")
    .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
    .unwrap_or(true);
  #[cfg_attr(not(windows), allow(unused_variables))]
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
  generate_theme_registry();
  println!("cargo:rerun-if-env-changed=HAGANE_REQUIRE_ADMIN");
  println!("cargo:rerun-if-env-changed=HAGANE_ICON_PATH");
  if let Some(path) = icon_path {
    println!("cargo:rerun-if-changed={}", path.display());
  } else {
    println!("cargo:rerun-if-changed={}", fallback_icon);
  }
}

fn generate_theme_registry() {
  use std::fmt::Write as _;

  let manifest_dir = match std::env::var("CARGO_MANIFEST_DIR") {
    Ok(v) => std::path::PathBuf::from(v),
    Err(_) => return,
  };
  let themes_dir = manifest_dir.join("../ui/themes");
  println!("cargo:rerun-if-changed={}", themes_dir.display());

  let out_dir = match std::env::var("OUT_DIR") {
    Ok(v) => std::path::PathBuf::from(v),
    Err(_) => return,
  };
  let out_file = out_dir.join("theme_registry.rs");

  let mut themes: Vec<String> = match std::fs::read_dir(&themes_dir) {
    Ok(rd) => rd
      .filter_map(Result::ok)
      .filter(|e| e.path().is_dir())
      .filter_map(|e| e.file_name().into_string().ok())
      .filter(|name| name != "default")
      .collect(),
    Err(_) => Vec::new(),
  };
  themes.sort();

  let mut code = String::new();
  code.push_str("#[allow(dead_code)]\nfn register_themed_html(map: &mut std::collections::HashMap<String, String>) {\n");

  for theme in &themes {
    let theme_dir = themes_dir.join(theme);
    let html_dir = theme_dir.join("html");
    let css_pages_dir = theme_dir.join("css/pages");
    let css_global = theme_dir.join("css/global.css");
    let progress_js = theme_dir.join("js/progress.js");

    println!("cargo:rerun-if-changed={}", theme_dir.display());
    println!("cargo:rerun-if-changed={}", html_dir.display());
    println!("cargo:rerun-if-changed={}", css_pages_dir.display());
    println!("cargo:rerun-if-changed={}", css_global.display());
    println!("cargo:rerun-if-changed={}", progress_js.display());

    let mut html_files: Vec<String> = match std::fs::read_dir(&html_dir) {
      Ok(rd) => rd
        .filter_map(Result::ok)
        .filter_map(|e| e.file_name().into_string().ok())
        .filter(|n| n.ends_with(".html"))
        .collect(),
      Err(_) => Vec::new(),
    };
    html_files.sort();

    for file in html_files {
      let page = file.trim_end_matches(".html");
      let html_rel = format!("../ui/themes/{}/html/{}", theme, file);
      if page == "progress" && progress_js.exists() {
        let js_rel = format!("../ui/themes/{}/js/progress.js", theme);
        let _ = writeln!(
          code,
          "    map.insert(\"{}__theme__{}\".into(), render_theme_page_html(include_str!(concat!(env!(\"CARGO_MANIFEST_DIR\"), \"/{}\")), Some(include_str!(concat!(env!(\"CARGO_MANIFEST_DIR\"), \"/{}\")))));",
          page,
          theme,
          html_rel,
          js_rel,
        );
      } else {
        let _ = writeln!(
          code,
          "    map.insert(\"{}__theme__{}\".into(), render_theme_page_html(include_str!(concat!(env!(\"CARGO_MANIFEST_DIR\"), \"/{}\")), None));",
          page,
          theme,
          html_rel,
        );
      }
    }
  }
  code.push_str("}\n\n");

  code.push_str("#[allow(dead_code)]\nfn theme_css_bundle_generated(preset: &str) -> Option<(&'static str, std::collections::HashMap<&'static str, &'static str>)> {\n");
  code.push_str("    match preset {\n");

  for theme in &themes {
    let theme_dir = themes_dir.join(theme);
    let css_pages_dir = theme_dir.join("css/pages");
    let css_global = theme_dir.join("css/global.css");

    let mut css_files: Vec<String> = match std::fs::read_dir(&css_pages_dir) {
      Ok(rd) => rd
        .filter_map(Result::ok)
        .filter_map(|e| e.file_name().into_string().ok())
        .filter(|n| n.ends_with(".css"))
        .collect(),
      Err(_) => Vec::new(),
    };
    css_files.sort();

    let _ = writeln!(code, "        \"{}\" => {{", theme);
    code.push_str("            let mut pages = std::collections::HashMap::new();\n");
    for file in css_files {
      let page = file.trim_end_matches(".css");
      let rel = format!("../ui/themes/{}/css/pages/{}", theme, file);
      let _ = writeln!(
        code,
        "            pages.insert(\"{}\", include_str!(concat!(env!(\"CARGO_MANIFEST_DIR\"), \"/{}\")));",
        page,
        rel,
      );
    }

    if css_global.exists() {
      let rel = format!("../ui/themes/{}/css/global.css", theme);
      let _ = writeln!(
        code,
        "            Some((include_str!(concat!(env!(\"CARGO_MANIFEST_DIR\"), \"/{}\")), pages))",
        rel,
      );
    } else {
      code.push_str("            Some((include_str!(concat!(env!(\"CARGO_MANIFEST_DIR\"), \"/../ui/themes/default/global.css\")), pages))\n");
    }
    code.push_str("        }\n");
  }

  code.push_str("        _ => None,\n");
  code.push_str("    }\n");
  code.push_str("}\n");

  let _ = std::fs::write(out_file, code);
}

#[cfg(windows)]
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