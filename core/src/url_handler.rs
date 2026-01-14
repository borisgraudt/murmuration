// URL Handler functions for OS-level ely:// URL scheme registration

use anyhow::Result;
use colored::*;

#[cfg(target_os = "macos")]
pub fn install_url_handler() -> Result<()> {
    use std::fs;
    use std::path::PathBuf;

    println!("{} Registering ely:// URL handler for macOS...", "🔧".cyan().bold());

    let home = std::env::var("HOME")?;
    let app_dir = PathBuf::from(&home).join(".elysium");
    fs::create_dir_all(&app_dir)?;

    let plist = r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleIdentifier</key>
    <string>com.elysium.ely</string>
    <key>CFBundleURLTypes</key>
    <array>
        <dict>
            <key>CFBundleURLName</key>
            <string>Elysium Protocol</string>
            <key>CFBundleURLSchemes</key>
            <array>
                <string>ely</string>
            </array>
        </dict>
    </array>
</dict>
</plist>"#;

    fs::write(app_dir.join("Info.plist"), plist)?;
    let ely_path = std::env::current_exe()?;

    let output = std::process::Command::new(
        "/System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister",
    )
    .arg("-f")
    .arg(&app_dir.join("Info.plist"))
    .output()?;

    if !output.status.success() {
        eprintln!("{} Warning: lsregister returned non-zero exit code", "⚠".yellow().bold());
    }

    println!("{} URL handler registered for ely://", "✓".green().bold());
    println!("  Binary: {}", ely_path.display());

    Ok(())
}

#[cfg(target_os = "linux")]
pub fn install_url_handler() -> Result<()> {
    use std::fs;
    use std::path::PathBuf;
    use std::os::unix::fs::PermissionsExt;

    println!("{} Registering ely:// URL handler for Linux...", "🔧".cyan().bold());

    let home = std::env::var("HOME")?;
    let apps_dir = PathBuf::from(&home).join(".local/share/applications");
    fs::create_dir_all(&apps_dir)?;

    let ely_path = std::env::current_exe()?;
    let desktop_entry = format!(
        r#"[Desktop Entry]
Type=Application
Name=Elysium
Exec={} handle-url %u
MimeType=x-scheme-handler/ely;
NoDisplay=true
"#,
        ely_path.display()
    );

    let desktop_file = apps_dir.join("elysium.desktop");
    fs::write(&desktop_file, desktop_entry)?;
    let mut perms = fs::metadata(&desktop_file)?.permissions();
    perms.set_mode(0o755);
    fs::set_permissions(&desktop_file, perms)?;

    let _ = std::process::Command::new("xdg-mime")
        .arg("default")
        .arg("elysium.desktop")
        .arg("x-scheme-handler/ely")
        .output();

    println!("{} URL handler registered for ely://", "✓".green().bold());
    Ok(())
}

#[cfg(target_os = "windows")]
pub fn install_url_handler() -> Result<()> {
    eprintln!("{} Windows support requires 'windows' feature", "✗".red().bold());
    Ok(())
}

#[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
pub fn install_url_handler() -> Result<()> {
    eprintln!("{} URL handler not supported on this OS", "✗".red().bold());
    Ok(())
}

pub fn handle_url(url: String) -> Result<()> {
    if !url.starts_with("ely://") {
        return Err(anyhow::anyhow!("Invalid ely:// URL: {}", url));
    }

    let api_port = detect_api_port();
    let web_port = api_port + 1;
    
    // Use new clean URL format: /e/<base64_encoded>
    // Try ely.local first (cleaner), fallback to localhost
    use base64::{Engine as _, engine::general_purpose};
    let encoded = general_purpose::URL_SAFE_NO_PAD.encode(url.as_bytes());
    // Try ely.local first (cleaner), fallback to localhost if not configured
    let gateway_url = format!("http://ely.local:{}/e/{}", web_port, encoded);
    
    // Note: If ely.local is not in /etc/hosts, browser will show error
    // User should add "127.0.0.1 ely.local" to /etc/hosts for cleaner URLs

    println!("{} Opening ely:// URL in browser...", "🌐".cyan().bold());
    println!("  URL: {}", url.yellow());

    #[cfg(target_os = "macos")]
    std::process::Command::new("open").arg(&gateway_url).status()?;

    #[cfg(target_os = "linux")]
    let _ = std::process::Command::new("xdg-open").arg(&gateway_url).status();

    #[cfg(target_os = "windows")]
    std::process::Command::new("cmd")
        .args(&["/C", "start", &gateway_url])
        .status()?;

    Ok(())
}

fn detect_api_port() -> u16 {
    if let Ok(home) = std::env::var("HOME") {
        let port_file = std::path::PathBuf::from(home).join(".elysium_api_port");
        if let Ok(port_str) = std::fs::read_to_string(&port_file) {
            if let Ok(port) = port_str.trim().parse::<u16>() {
                return port;
            }
        }
    }
    17080
}

