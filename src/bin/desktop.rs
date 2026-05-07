use renamer::{
    default_icon_png_bytes, run, AnonymizationProject, Replacement, DEFAULT_APP_NAME,
    DEFAULT_PACKAGE_ID,
};
use std::collections::HashMap;
use std::env;
use std::fs;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::thread;

fn main() -> std::io::Result<()> {
    let bind_addr = env::var("RENAMER_DESKTOP_ADDR").unwrap_or_else(|_| "127.0.0.1:0".to_string());
    let listener = TcpListener::bind(bind_addr)?;
    let url = format!("http://{}", listener.local_addr()?);
    println!("Renamer Desktop is running at {url}");
    if env::var("RENAMER_DESKTOP_NO_OPEN").is_err() {
        open_in_system_browser(&url);
    }

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                thread::spawn(|| {
                    if let Err(error) = handle_connection(stream) {
                        eprintln!("desktop request failed: {error}");
                    }
                });
            }
            Err(error) => eprintln!("desktop connection failed: {error}"),
        }
    }
    Ok(())
}

fn handle_connection(mut stream: TcpStream) -> std::io::Result<()> {
    let mut buffer = [0; 64 * 1024];
    let bytes_read = stream.read(&mut buffer)?;
    if bytes_read == 0 {
        return Ok(());
    }
    let request = String::from_utf8_lossy(&buffer[..bytes_read]);
    let mut lines = request.lines();
    let request_line = lines.next().unwrap_or_default();
    let body = request.split("\r\n\r\n").nth(1).unwrap_or_default();

    if request_line.starts_with("GET /") {
        respond_html(&mut stream, &render_page("", ""))?;
    } else if request_line.starts_with("POST /choose-folder") {
        let path = choose_folder().unwrap_or_default();
        respond_html(&mut stream, &render_page("Папка выбрана.", &path))?;
    } else if request_line.starts_with("POST /detect") {
        let form = parse_form(body);
        let folder = form.get("project_dir").cloned().unwrap_or_default();
        let detected = detect_android_values(Path::new(&folder));
        respond_html(
            &mut stream,
            &render_page_with_values(
                "Автоопределение завершено. Проверьте поля перед запуском.",
                &folder,
                detected.package.as_deref().unwrap_or_default(),
                DEFAULT_PACKAGE_ID,
                detected.app_name.as_deref().unwrap_or_default(),
                DEFAULT_APP_NAME,
                "",
                true,
                true,
                true,
                true,
                "",
            ),
        )?;
    } else if request_line.starts_with("POST /run") {
        let form = parse_form(body);
        let response = execute_form(&form);
        respond_html(
            &mut stream,
            &render_page_with_values(
                &response.status,
                form.get("project_dir")
                    .map(String::as_str)
                    .unwrap_or_default(),
                form.get("old_package")
                    .map(String::as_str)
                    .unwrap_or_default(),
                form.get("new_package")
                    .map(String::as_str)
                    .unwrap_or(DEFAULT_PACKAGE_ID),
                form.get("app_name_old")
                    .map(String::as_str)
                    .unwrap_or_default(),
                form.get("app_name_new")
                    .map(String::as_str)
                    .unwrap_or(DEFAULT_APP_NAME),
                form.get("extra_rules")
                    .map(String::as_str)
                    .unwrap_or_default(),
                form.contains_key("replace_icons"),
                form.contains_key("replace_names"),
                form.contains_key("rename_paths"),
                form.contains_key("dry_run"),
                &response.output,
            ),
        )?;
    } else {
        respond(
            &mut stream,
            "404 Not Found",
            "text/plain; charset=utf-8",
            "Not found",
        )?;
    }
    Ok(())
}

struct ExecuteResponse {
    status: String,
    output: String,
}

fn execute_form(form: &HashMap<String, String>) -> ExecuteResponse {
    let root = PathBuf::from(
        form.get("project_dir")
            .map(String::as_str)
            .unwrap_or_default(),
    );
    if !root.is_dir() {
        return ExecuteResponse {
            status: "Ошибка: выберите существующую папку проекта.".to_string(),
            output: String::new(),
        };
    }

    let replace_icons = form.contains_key("replace_icons");
    let icon_source = if replace_icons {
        match write_default_icon() {
            Ok(path) => Some(path),
            Err(error) => {
                return ExecuteResponse {
                    status: format!("Не удалось подготовить дефолтную иконку: {error}"),
                    output: String::new(),
                }
            }
        }
    } else {
        None
    };

    let mut project = AnonymizationProject::new(root);
    project.old_package = value_or_none(
        form.get("old_package")
            .map(String::as_str)
            .unwrap_or_default(),
    );
    project.new_package = value_or_none(
        form.get("new_package")
            .map(String::as_str)
            .unwrap_or_default(),
    );
    project.replace_names_and_strings = form.contains_key("replace_names");
    project.anonymize_icons = replace_icons;
    project.icon_source = icon_source;
    project.rename_paths = form.contains_key("rename_paths");
    project.dry_run = form.contains_key("dry_run");

    if project.replace_names_and_strings {
        if let Some(old_name) = value_or_none(
            form.get("app_name_old")
                .map(String::as_str)
                .unwrap_or_default(),
        ) {
            let new_name = value_or_none(
                form.get("app_name_new")
                    .map(String::as_str)
                    .unwrap_or_default(),
            )
            .unwrap_or_else(|| DEFAULT_APP_NAME.to_string());
            project
                .string_replacements
                .push(Replacement::new(old_name, new_name));
        }
        for line in form
            .get("extra_rules")
            .map(String::as_str)
            .unwrap_or_default()
            .lines()
        {
            let Some((from, to)) = line.split_once("=>") else {
                continue;
            };
            if let Some(from) = value_or_none(from) {
                project
                    .string_replacements
                    .push(Replacement::new(from, to.trim().to_string()));
            }
        }
    }

    let config = project.into_run_config();
    match run(&config) {
        Ok(summary) => ExecuteResponse {
            status: if config.dry_run {
                "Предпросмотр готов. Файлы не изменены.".to_string()
            } else {
                "Обезличивание применено.".to_string()
            },
            output: format_summary(config.dry_run, &summary),
        },
        Err(error) => ExecuteResponse {
            status: format!("Ошибка выполнения: {error}"),
            output: String::new(),
        },
    }
}

#[derive(Default)]
struct DetectedValues {
    package: Option<String>,
    app_name: Option<String>,
}

fn detect_android_values(root: &Path) -> DetectedValues {
    let mut detected = DetectedValues::default();
    let mut stack = vec![root.to_path_buf()];
    let mut scanned = 0usize;

    while let Some(dir) = stack.pop() {
        let Ok(entries) = fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let Ok(file_type) = entry.file_type() else {
                continue;
            };
            if file_type.is_dir() {
                if should_skip_gui_dir(&path) {
                    continue;
                }
                stack.push(path);
                continue;
            }
            if !file_type.is_file() || scanned > 500 {
                continue;
            }
            scanned += 1;
            let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
                continue;
            };
            if !matches!(
                name,
                "build.gradle" | "build.gradle.kts" | "AndroidManifest.xml" | "strings.xml"
            ) {
                continue;
            }
            let Ok(contents) = fs::read_to_string(&path) else {
                continue;
            };
            if detected.package.is_none() {
                detected.package = extract_after(&contents, "namespace = \"")
                    .or_else(|| extract_after(&contents, "applicationId = \""))
                    .or_else(|| extract_after(&contents, "package=\""));
            }
            if detected.app_name.is_none() {
                detected.app_name = extract_between(&contents, "name=\"app_name\">", "</string>")
                    .or_else(|| extract_after(&contents, "android:label=\""));
            }
            if detected.package.is_some() && detected.app_name.is_some() {
                return detected;
            }
        }
    }

    detected
}

fn render_page(status: &str, project_dir: &str) -> String {
    render_page_with_values(
        status,
        project_dir,
        "",
        DEFAULT_PACKAGE_ID,
        "",
        DEFAULT_APP_NAME,
        "",
        true,
        true,
        true,
        true,
        "",
    )
}

#[allow(clippy::too_many_arguments)]
fn render_page_with_values(
    status: &str,
    project_dir: &str,
    old_package: &str,
    new_package: &str,
    app_name_old: &str,
    app_name_new: &str,
    extra_rules: &str,
    replace_icons: bool,
    replace_names: bool,
    rename_paths: bool,
    dry_run: bool,
    output: &str,
) -> String {
    format!(
        r#"<!doctype html>
<html lang="ru">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>Renamer Desktop</title>
<style>
:root {{ color-scheme: light dark; --accent:#2a9d8f; --bg:#17252b; --panel:#20353d; --text:#f8fafc; }}
body {{ margin:0; font:15px/1.45 system-ui,-apple-system,Segoe UI,sans-serif; background:linear-gradient(135deg,#142129,#264653); color:var(--text); }}
main {{ max-width:1040px; margin:0 auto; padding:28px; }}
section {{ background:rgba(32,53,61,.92); border:1px solid rgba(255,255,255,.10); border-radius:18px; padding:18px; margin:16px 0; box-shadow:0 14px 44px rgba(0,0,0,.24); }}
h1 {{ margin:0 0 8px; font-size:34px; }} h2 {{ margin:0 0 12px; font-size:20px; }}
label {{ display:block; margin:12px 0 6px; font-weight:650; }}
input[type=text], textarea {{ width:100%; box-sizing:border-box; border-radius:12px; border:1px solid rgba(255,255,255,.24); padding:11px 12px; background:#102027; color:var(--text); }}
textarea {{ min-height:110px; font-family:ui-monospace,SFMono-Regular,Consolas,monospace; }}
.row {{ display:grid; grid-template-columns:1fr 1fr; gap:14px; }}
.check {{ display:flex; gap:10px; align-items:center; margin:10px 0; }} .check label {{ margin:0; }}
button {{ border:0; border-radius:999px; padding:10px 16px; margin:6px 8px 6px 0; background:var(--accent); color:#06221e; font-weight:800; cursor:pointer; }}
button.secondary {{ background:#e9c46a; color:#2c2204; }}
.status {{ color:#e9c46a; font-weight:800; }}
pre {{ white-space:pre-wrap; background:#08161b; padding:14px; border-radius:14px; overflow:auto; }}
small {{ color:#cbd5e1; }}
</style>
</head>
<body><main>
<h1>Renamer Desktop</h1>
<p>Универсальное desktop-приложение для macOS и Windows. Оно запускает локальный UI и использует тот же core, что и CLI.</p>
<p class="status">{status}</p>
<form method="post">
<section><h2>1. Новый проект обезличивания</h2>
<label>Папка проекта</label>
<input name="project_dir" type="text" value="{project_dir}" placeholder="/Users/me/project или C:\\Users\\me\\project">
<button formaction="/choose-folder" formmethod="post" class="secondary">Выбрать папку…</button>
<button formaction="/detect" formmethod="post" class="secondary">Автоопределить пакет и имя</button>
<small>На macOS/Windows кнопка выбора папки вызывает системный диалог. Если ОС не поддержана — вставьте путь вручную.</small>
</section>
<section><h2>2. Пакет приложения</h2>
<div class="row"><div><label>Текущий пакет</label><input name="old_package" type="text" value="{old_package}" placeholder="com.old.brand"></div>
<div><label>Новый пакет</label><input name="new_package" type="text" value="{new_package}" placeholder="{default_package}"></div></div>
<div class="check"><input id="rename_paths" name="rename_paths" type="checkbox" {rename_paths_checked}><label for="rename_paths">Переименовывать package-директории и подходящие имена файлов</label></div>
<small>Если новое имя пакета пустое, будет использован статичный дефолт: <b>{default_package}</b>.</small>
</section>
<section><h2>3. Названия и строки</h2>
<div class="check"><input id="replace_names" name="replace_names" type="checkbox" {replace_names_checked}><label for="replace_names">Обезличивать имя приложения и отдельные строки из ресурсов/кода</label></div>
<div class="row"><div><label>Старое имя приложения</label><input name="app_name_old" type="text" value="{app_name_old}" placeholder="Old App"></div>
<div><label>Новое имя приложения</label><input name="app_name_new" type="text" value="{app_name_new}" placeholder="{default_app_name}"></div></div>
<label>Дополнительные строки, по одной на строку в формате <code>старое =&gt; новое</code></label>
<textarea name="extra_rules" placeholder="SecretBrand =&gt; NeutralBrand&#10;old keyword =&gt; neutral keyword">{extra_rules}</textarea>
</section>
<section><h2>4. Иконки</h2>
<div class="check"><input id="replace_icons" name="replace_icons" type="checkbox" {replace_icons_checked}><label for="replace_icons">Обезличивать иконки дефолтным нейтральным набором</label></div>
<small>Встроенная нейтральная PNG-иконка генерируется из Rust-кода во время запуска, поэтому в PR нет бинарных ассетов.</small>
</section>
<section><h2>5. Запуск</h2>
<div class="check"><input id="dry_run" name="dry_run" type="checkbox" {dry_run_checked}><label for="dry_run">Dry-run: показать план без записи файлов</label></div>
<button formaction="/run" formmethod="post">Запустить</button>
<pre>{output}</pre>
</section>
</form>
</main></body></html>"#,
        status = escape_html(status),
        project_dir = escape_html(project_dir),
        old_package = escape_html(old_package),
        new_package = escape_html(new_package),
        default_package = DEFAULT_PACKAGE_ID,
        app_name_old = escape_html(app_name_old),
        app_name_new = escape_html(app_name_new),
        default_app_name = DEFAULT_APP_NAME,
        extra_rules = escape_html(extra_rules),
        output = escape_html(output),
        replace_icons_checked = checked(replace_icons),
        replace_names_checked = checked(replace_names),
        rename_paths_checked = checked(rename_paths),
        dry_run_checked = checked(dry_run),
    )
}

fn respond_html(stream: &mut TcpStream, body: &str) -> std::io::Result<()> {
    respond(stream, "200 OK", "text/html; charset=utf-8", body)
}

fn respond(
    stream: &mut TcpStream,
    status: &str,
    content_type: &str,
    body: &str,
) -> std::io::Result<()> {
    write!(
        stream,
        "HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    )
}

fn parse_form(body: &str) -> HashMap<String, String> {
    body.split('&')
        .filter(|pair| !pair.is_empty())
        .map(|pair| {
            let (key, value) = pair.split_once('=').unwrap_or((pair, ""));
            (url_decode(key), url_decode(value))
        })
        .collect()
}

fn url_decode(value: &str) -> String {
    let bytes = value.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        match bytes[index] {
            b'+' => decoded.push(b' '),
            b'%' if index + 2 < bytes.len() => {
                if let Ok(hex) = u8::from_str_radix(&value[index + 1..index + 3], 16) {
                    decoded.push(hex);
                    index += 2;
                }
            }
            byte => decoded.push(byte),
        }
        index += 1;
    }
    String::from_utf8_lossy(&decoded).to_string()
}

fn escape_html(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn checked(value: bool) -> &'static str {
    if value {
        "checked"
    } else {
        ""
    }
}

fn choose_folder() -> Option<String> {
    #[cfg(target_os = "macos")]
    {
        let output = Command::new("osascript")
            .args([
                "-e",
                "POSIX path of (choose folder with prompt \"Choose project folder\")",
            ])
            .output()
            .ok()?;
        return output
            .status
            .success()
            .then(|| String::from_utf8_lossy(&output.stdout).trim().to_string());
    }
    #[cfg(target_os = "windows")]
    {
        let script = "Add-Type -AssemblyName System.Windows.Forms; $d = New-Object System.Windows.Forms.FolderBrowserDialog; if ($d.ShowDialog() -eq 'OK') { $d.SelectedPath }";
        let output = Command::new("powershell")
            .args(["-NoProfile", "-Command", script])
            .output()
            .ok()?;
        return output
            .status
            .success()
            .then(|| String::from_utf8_lossy(&output.stdout).trim().to_string());
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        None
    }
}

fn open_in_system_browser(url: &str) {
    #[cfg(target_os = "macos")]
    let command = ("open", vec![url]);
    #[cfg(target_os = "windows")]
    let command = ("cmd", vec!["/C", "start", "", url]);
    #[cfg(all(not(target_os = "macos"), not(target_os = "windows")))]
    let command = ("xdg-open", vec![url]);

    let _ = Command::new(command.0).args(command.1).spawn();
}

fn should_skip_gui_dir(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| {
            matches!(
                name,
                ".git" | "node_modules" | "target" | "build" | ".gradle"
            )
        })
}

fn extract_after(contents: &str, marker: &str) -> Option<String> {
    let start = contents.find(marker)? + marker.len();
    let rest = &contents[start..];
    let end = rest.find('"')?;
    Some(rest[..end].to_string())
}

fn extract_between(contents: &str, start_marker: &str, end_marker: &str) -> Option<String> {
    let start = contents.find(start_marker)? + start_marker.len();
    let rest = &contents[start..];
    let end = rest.find(end_marker)?;
    Some(rest[..end].to_string())
}

fn value_or_none(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn write_default_icon() -> std::io::Result<PathBuf> {
    let path = std::env::temp_dir().join("renamer-neutral-default-icon.png");
    fs::write(&path, default_icon_png_bytes())?;
    Ok(path)
}

fn format_summary(dry_run: bool, summary: &renamer::RunSummary) -> String {
    let mut lines = vec![
        format!("Mode: {}", if dry_run { "dry-run" } else { "apply" }),
        format!("Scanned files: {}", summary.scanned_files),
        format!("Text files to change: {}", summary.changed_text_files.len()),
    ];
    lines.extend(
        summary
            .changed_text_files
            .iter()
            .map(|path| format!("  text: {}", path.display())),
    );
    lines.push(format!(
        "Icon files to replace: {}",
        summary.changed_icon_files.len()
    ));
    lines.extend(
        summary
            .changed_icon_files
            .iter()
            .map(|path| format!("  icon: {}", path.display())),
    );
    lines.push(format!("Paths to rename: {}", summary.renamed_paths.len()));
    lines.extend(
        summary
            .renamed_paths
            .iter()
            .map(|(from, to)| format!("  path: {} -> {}", from.display(), to.display())),
    );
    lines.push(format!(
        "Skipped binary files: {}",
        summary.skipped_binary_files
    ));
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn form_parser_decodes_unicode_and_checkboxes() {
        let form = parse_form(
            "project_dir=%2Ftmp%2Fapp&replace_icons=on&extra_rules=Old+Name+%3D%3E+New+Name",
        );
        assert_eq!(form.get("project_dir"), Some(&"/tmp/app".to_string()));
        assert!(form.contains_key("replace_icons"));
        assert_eq!(
            form.get("extra_rules"),
            Some(&"Old Name => New Name".to_string())
        );
    }

    #[test]
    fn execute_form_uses_default_package_and_default_icon() {
        let project = temp_dir("desktop-form");
        write_file(
            &project.join("app/src/main/java/com/old/brand/MainActivity.kt"),
            "package com.old.brand\nclass MainActivity",
        );
        write_file(
            &project.join("app/src/main/res/values/strings.xml"),
            "<resources><string name=\"app_name\">Old App</string></resources>",
        );
        write_bytes(
            &project.join("app/src/main/res/mipmap-hdpi/ic_launcher.png"),
            b"old-icon",
        );

        let mut form = HashMap::new();
        form.insert("project_dir".to_string(), project.display().to_string());
        form.insert("old_package".to_string(), "com.old.brand".to_string());
        form.insert("new_package".to_string(), "".to_string());
        form.insert("replace_names".to_string(), "on".to_string());
        form.insert("app_name_old".to_string(), "Old App".to_string());
        form.insert("app_name_new".to_string(), "".to_string());
        form.insert("replace_icons".to_string(), "on".to_string());
        form.insert("rename_paths".to_string(), "on".to_string());

        let response = execute_form(&form);
        assert!(response.status.contains("Обезличивание применено"));
        assert!(project
            .join("app/src/main/java/app/anonymized/default/MainActivity.kt")
            .exists());
        assert!(fs::read_to_string(
            project.join("app/src/main/java/app/anonymized/default/MainActivity.kt")
        )
        .unwrap()
        .contains(DEFAULT_PACKAGE_ID));
        assert!(
            fs::read_to_string(project.join("app/src/main/res/values/strings.xml"))
                .unwrap()
                .contains(DEFAULT_APP_NAME)
        );
        assert_eq!(
            fs::read(project.join("app/src/main/res/mipmap-hdpi/ic_launcher.png")).unwrap(),
            default_icon_png_bytes()
        );
    }

    fn write_file(path: &Path, contents: &str) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(path, contents).unwrap();
    }

    fn write_bytes(path: &Path, contents: &[u8]) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(path, contents).unwrap();
    }

    fn temp_dir(label: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("renamer-desktop-{label}-{nonce}"))
    }
}
