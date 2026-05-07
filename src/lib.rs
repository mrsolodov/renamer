use std::collections::HashSet;
use std::ffi::OsStr;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

pub const DEFAULT_PACKAGE_ID: &str = "app.anonymized.default";
pub const DEFAULT_APP_NAME: &str = "Neutral App";

const DEFAULT_EXCLUDED_DIRS: &[&str] = &[
    ".git",
    ".hg",
    ".svn",
    "node_modules",
    "target",
    "build",
    "dist",
    ".gradle",
    ".idea",
    ".next",
    ".nuxt",
    "DerivedData",
];

const DEFAULT_ICON_NAMES: &[&str] = &[
    "AppIcon.png",
    "app_icon.png",
    "ic_launcher.png",
    "ic_launcher_foreground.png",
    "ic_launcher_round.png",
    "Icon.png",
    "icon.png",
    "favicon.ico",
    "favicon.png",
    "tray.png",
    "trayIcon.png",
    "tray-icon.png",
];

/// Builds the built-in neutral default icon as PNG bytes at runtime.
///
/// The repository intentionally does not store binary icon files because some PR
/// systems reject binary diffs. This generator keeps the default icon
/// deterministic, dependency-free, and reviewable as Rust source.
pub fn default_icon_png_bytes() -> Vec<u8> {
    const SIZE: usize = 128;
    let mut scanlines = Vec::with_capacity((SIZE * 4 + 1) * SIZE);

    for y in 0..SIZE {
        scanlines.push(0); // PNG filter type: None.
        for x in 0..SIZE {
            let blend = ((x + y) as u32 * 255 / ((SIZE - 1) as u32 * 2)) as u8;
            let mut red = lerp(38, 42, blend);
            let mut green = lerp(70, 157, blend);
            let mut blue = lerp(83, 143, blend);

            let center_x = x as i32 - (SIZE as i32 / 2);
            let center_y = y as i32 - (SIZE as i32 / 2);
            let distance_sq = center_x * center_x + center_y * center_y;
            let shield_radius = (SIZE as i32 * 31 / 100).pow(2);
            let dot_radius = (SIZE as i32 * 6 / 100).pow(2);

            if distance_sq < shield_radius {
                red = mix_with_white(red, 35);
                green = mix_with_white(green, 35);
                blue = mix_with_white(blue, 35);
            }

            let slash = (y as i32 - x as i32 - SIZE as i32 / 25).abs() < SIZE as i32 / 28
                && x > SIZE / 5
                && x < SIZE * 4 / 5;
            if slash {
                red = 233;
                green = 196;
                blue = 106;
            }

            let left_dot_x = x as i32 - SIZE as i32 * 43 / 100;
            let right_dot_x = x as i32 - SIZE as i32 * 57 / 100;
            let dot_y = y as i32 - SIZE as i32 * 44 / 100;
            if left_dot_x * left_dot_x + dot_y * dot_y < dot_radius
                || right_dot_x * right_dot_x + dot_y * dot_y < dot_radius
            {
                red = 248;
                green = 250;
                blue = 252;
            }

            if x > SIZE * 37 / 100
                && x < SIZE * 63 / 100
                && y > SIZE * 54 / 100
                && y < SIZE * 60 / 100
            {
                red = 248;
                green = 250;
                blue = 252;
            }

            scanlines.extend([red, green, blue, 255]);
        }
    }

    encode_png_rgba(SIZE as u32, SIZE as u32, &scanlines)
}

fn lerp(start: u8, end: u8, amount: u8) -> u8 {
    let start = start as u16;
    let end = end as u16;
    let amount = amount as u16;
    ((start * (255 - amount) + end * amount) / 255) as u8
}

fn mix_with_white(value: u8, percent: u16) -> u8 {
    ((value as u16 * (100 - percent) + 255 * percent) / 100) as u8
}

fn encode_png_rgba(width: u32, height: u32, filtered_rgba: &[u8]) -> Vec<u8> {
    let mut png = Vec::new();
    png.extend(b"\x89PNG\r\n\x1a\n");

    let mut ihdr = Vec::with_capacity(13);
    ihdr.extend(width.to_be_bytes());
    ihdr.extend(height.to_be_bytes());
    ihdr.extend([8, 6, 0, 0, 0]); // 8-bit RGBA, deflate, no interlace.
    append_png_chunk(&mut png, b"IHDR", &ihdr);

    let mut zlib = vec![0x78, 0x01]; // Deflate stream with fastest/no compression settings.
    let mut remaining = filtered_rgba;
    while !remaining.is_empty() {
        let chunk_len = remaining.len().min(u16::MAX as usize);
        let is_last = chunk_len == remaining.len();
        zlib.push(if is_last { 0x01 } else { 0x00 });
        let len = chunk_len as u16;
        zlib.extend(len.to_le_bytes());
        zlib.extend((!len).to_le_bytes());
        zlib.extend(&remaining[..chunk_len]);
        remaining = &remaining[chunk_len..];
    }
    zlib.extend(adler32(filtered_rgba).to_be_bytes());
    append_png_chunk(&mut png, b"IDAT", &zlib);
    append_png_chunk(&mut png, b"IEND", &[]);
    png
}

fn append_png_chunk(png: &mut Vec<u8>, chunk_type: &[u8; 4], data: &[u8]) {
    png.extend((data.len() as u32).to_be_bytes());
    png.extend(chunk_type);
    png.extend(data);
    let mut crc_input = Vec::with_capacity(chunk_type.len() + data.len());
    crc_input.extend(chunk_type);
    crc_input.extend(data);
    png.extend(crc32(&crc_input).to_be_bytes());
}

fn adler32(data: &[u8]) -> u32 {
    const MOD_ADLER: u32 = 65_521;
    let mut a = 1u32;
    let mut b = 0u32;
    for byte in data {
        a = (a + *byte as u32) % MOD_ADLER;
        b = (b + a) % MOD_ADLER;
    }
    (b << 16) | a
}

fn crc32(data: &[u8]) -> u32 {
    let mut crc = 0xffff_ffffu32;
    for byte in data {
        crc ^= *byte as u32;
        for _ in 0..8 {
            let mask = (crc & 1).wrapping_neg();
            crc = (crc >> 1) ^ (0xedb8_8320 & mask);
        }
    }
    !crc
}

/// A text replacement rule used to anonymize names, package IDs, bundle IDs, and keywords.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Replacement {
    pub from: String,
    pub to: String,
}

impl Replacement {
    pub fn new(from: impl Into<String>, to: impl Into<String>) -> Self {
        Self {
            from: from.into(),
            to: to.into(),
        }
    }
}

/// Configuration for an anonymization run.
#[derive(Debug, Clone)]
pub struct RunConfig {
    pub root: PathBuf,
    pub replacements: Vec<Replacement>,
    pub icon_source: Option<PathBuf>,
    pub icon_names: Vec<String>,
    pub dry_run: bool,
    pub rename_paths: bool,
}

impl RunConfig {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self {
            root: root.into(),
            replacements: Vec::new(),
            icon_source: None,
            icon_names: DEFAULT_ICON_NAMES
                .iter()
                .map(|name| name.to_string())
                .collect(),
            dry_run: true,
            rename_paths: true,
        }
    }
}

/// A higher-level project description used by desktop shells before creating a `RunConfig`.
#[derive(Debug, Clone)]
pub struct AnonymizationProject {
    pub project_dir: PathBuf,
    pub old_package: Option<String>,
    pub new_package: Option<String>,
    pub replace_names_and_strings: bool,
    pub string_replacements: Vec<Replacement>,
    pub anonymize_icons: bool,
    pub icon_source: Option<PathBuf>,
    pub rename_paths: bool,
    pub dry_run: bool,
}

impl AnonymizationProject {
    pub fn new(project_dir: impl Into<PathBuf>) -> Self {
        Self {
            project_dir: project_dir.into(),
            old_package: None,
            new_package: None,
            replace_names_and_strings: true,
            string_replacements: Vec::new(),
            anonymize_icons: true,
            icon_source: None,
            rename_paths: true,
            dry_run: true,
        }
    }

    pub fn effective_new_package(&self) -> &str {
        self.new_package
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or(DEFAULT_PACKAGE_ID)
    }

    pub fn into_run_config(self) -> RunConfig {
        let effective_new_package = self.effective_new_package().to_string();
        let mut config = RunConfig::new(self.project_dir);
        config.dry_run = self.dry_run;
        config.rename_paths = self.rename_paths;

        if let Some(old_package) = self
            .old_package
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            config
                .replacements
                .push(Replacement::new(old_package, effective_new_package));
        }

        if self.replace_names_and_strings {
            config
                .replacements
                .extend(self.string_replacements.into_iter().filter(|replacement| {
                    !replacement.from.trim().is_empty() && replacement.from != replacement.to
                }));
        }

        if self.anonymize_icons {
            config.icon_source = self.icon_source;
        }

        config
    }
}

/// Summary returned after scanning or applying changes.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct RunSummary {
    pub scanned_files: usize,
    pub changed_text_files: Vec<PathBuf>,
    pub changed_icon_files: Vec<PathBuf>,
    pub skipped_binary_files: usize,
    pub renamed_paths: Vec<(PathBuf, PathBuf)>,
}

impl RunSummary {
    pub fn has_changes(&self) -> bool {
        !self.changed_text_files.is_empty()
            || !self.changed_icon_files.is_empty()
            || !self.renamed_paths.is_empty()
    }
}

pub fn run(config: &RunConfig) -> io::Result<RunSummary> {
    let root = fs::canonicalize(&config.root)?;
    let icon_names = config
        .icon_names
        .iter()
        .map(|name| name.as_str())
        .collect::<HashSet<_>>();
    let icon_source = match &config.icon_source {
        Some(path) => Some(fs::read(path)?),
        None => None,
    };

    let mut summary = RunSummary::default();
    visit_dir(
        &root,
        &root,
        config,
        &icon_names,
        icon_source.as_deref(),
        &mut summary,
    )?;
    Ok(summary)
}

fn visit_dir(
    root: &Path,
    dir: &Path,
    config: &RunConfig,
    icon_names: &HashSet<&str>,
    icon_source: Option<&[u8]>,
    summary: &mut RunSummary,
) -> io::Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        let file_type = entry.file_type()?;

        if file_type.is_dir() {
            if should_skip_dir(path.file_name()) {
                continue;
            }
            visit_dir(root, &path, config, icon_names, icon_source, summary)?;
            rename_path_if_needed(root, &path, true, config, summary)?;
        } else if file_type.is_file() {
            summary.scanned_files += 1;
            process_file(&path, config, icon_names, icon_source, summary)?;
            rename_path_if_needed(root, &path, false, config, summary)?;
        }
    }
    Ok(())
}

fn rename_path_if_needed(
    root: &Path,
    path: &Path,
    is_dir: bool,
    config: &RunConfig,
    summary: &mut RunSummary,
) -> io::Result<()> {
    if !config.rename_paths {
        return Ok(());
    }

    let rename_target = if is_dir {
        let Ok(relative_path) = path.strip_prefix(root) else {
            return Ok(());
        };
        let Some(relative_path) = relative_path.to_str() else {
            return Ok(());
        };
        relative_path.to_string()
    } else {
        let Some(file_name) = path.file_name().and_then(OsStr::to_str) else {
            return Ok(());
        };
        file_name.to_string()
    };

    let mut renamed = rename_target.clone();
    for replacement in &config.replacements {
        if replacement.from.is_empty() {
            continue;
        }

        renamed = renamed.replace(&replacement.from, &replacement.to);
        if is_dir {
            renamed = renamed.replace(
                &replacement.from.replace('.', "/"),
                &replacement.to.replace('.', "/"),
            );
        }
        renamed = renamed.replace(
            &replacement.from.replace('.', "_"),
            &replacement.to.replace('.', "_"),
        );
        renamed = renamed.replace(
            &replacement.from.replace('.', "-"),
            &replacement.to.replace('.', "-"),
        );
    }

    if renamed == rename_target {
        return Ok(());
    }

    let destination = if is_dir {
        root.join(renamed)
    } else {
        path.with_file_name(renamed)
    };
    summary
        .renamed_paths
        .push((path.to_path_buf(), destination.clone()));
    if !config.dry_run {
        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::rename(path, destination)?;
    }
    Ok(())
}

fn should_skip_dir(name: Option<&OsStr>) -> bool {
    let Some(name) = name.and_then(OsStr::to_str) else {
        return false;
    };
    DEFAULT_EXCLUDED_DIRS.contains(&name)
}

fn process_file(
    path: &Path,
    config: &RunConfig,
    icon_names: &HashSet<&str>,
    icon_source: Option<&[u8]>,
    summary: &mut RunSummary,
) -> io::Result<()> {
    if let Some(source) = icon_source {
        if path
            .file_name()
            .and_then(OsStr::to_str)
            .is_some_and(|name| icon_names.contains(name))
        {
            summary.changed_icon_files.push(path.to_path_buf());
            if !config.dry_run {
                fs::write(path, source)?;
            }
            return Ok(());
        }
    }

    let bytes = fs::read(path)?;
    if looks_binary(&bytes) {
        summary.skipped_binary_files += 1;
        return Ok(());
    }

    let original = String::from_utf8_lossy(&bytes);
    let mut changed = original.to_string();
    for replacement in &config.replacements {
        if !replacement.from.is_empty() {
            changed = changed.replace(&replacement.from, &replacement.to);
        }
    }

    if changed != original {
        summary.changed_text_files.push(path.to_path_buf());
        if !config.dry_run {
            fs::write(path, changed.as_bytes())?;
        }
    }

    Ok(())
}

fn looks_binary(bytes: &[u8]) -> bool {
    bytes.iter().take(1024).any(|byte| *byte == 0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn replaces_keywords_and_package_ids() {
        let temp = temp_dir("text");
        fs::create_dir_all(&temp).unwrap();
        let manifest = temp.join("AndroidManifest.xml");
        fs::write(
            &manifest,
            "<manifest package=\"com.old.brand\"><application android:label=\"OldBrand\" /></manifest>",
        )
        .unwrap();

        let mut config = RunConfig::new(&temp);
        config.replacements = vec![
            Replacement::new("com.old.brand", "com.new.brand"),
            Replacement::new("OldBrand", "NeutralApp"),
        ];
        config.dry_run = false;

        let summary = run(&config).unwrap();
        assert_eq!(summary.changed_text_files, vec![manifest.clone()]);
        let rewritten = fs::read_to_string(manifest).unwrap();
        assert!(rewritten.contains("com.new.brand"));
        assert!(rewritten.contains("NeutralApp"));
    }

    #[test]
    fn dry_run_reports_without_writing() {
        let temp = temp_dir("dry-run");
        fs::create_dir_all(&temp).unwrap();
        let file = temp.join("Info.plist");
        fs::write(&file, "CFBundleIdentifier=com.old.brand").unwrap();

        let mut config = RunConfig::new(&temp);
        config.replacements = vec![Replacement::new("com.old.brand", "com.new.brand")];

        let summary = run(&config).unwrap();
        assert_eq!(summary.changed_text_files, vec![file.clone()]);
        assert_eq!(
            fs::read_to_string(file).unwrap(),
            "CFBundleIdentifier=com.old.brand"
        );
    }

    #[test]
    fn replaces_matching_icon_names() {
        let temp = temp_dir("icons");
        fs::create_dir_all(temp.join("res/mipmap-hdpi")).unwrap();
        let source = temp.join("new.png");
        let target = temp.join("res/mipmap-hdpi/ic_launcher.png");
        fs::write(&source, b"new-icon").unwrap();
        fs::write(&target, b"old-icon").unwrap();

        let mut config = RunConfig::new(&temp);
        config.icon_source = Some(source);
        config.dry_run = false;

        let summary = run(&config).unwrap();
        assert_eq!(summary.changed_icon_files, vec![target.clone()]);
        assert_eq!(fs::read(target).unwrap(), b"new-icon");
    }

    #[test]
    fn renames_package_directory_paths() {
        let temp = temp_dir("paths");
        let old_package_dir = temp.join("app/src/main/java/com/old/brand");
        fs::create_dir_all(&old_package_dir).unwrap();
        fs::write(
            old_package_dir.join("MainActivity.kt"),
            "package com.old.brand\nclass MainActivity",
        )
        .unwrap();

        let mut config = RunConfig::new(&temp);
        config.replacements = vec![Replacement::new("com.old.brand", "io.neutral.app")];
        config.dry_run = false;

        let summary = run(&config).unwrap();
        let new_activity = temp.join("app/src/main/java/io/neutral/app/MainActivity.kt");
        assert!(new_activity.exists());
        assert!(summary
            .renamed_paths
            .iter()
            .any(|(_, to)| to == &temp.join("app/src/main/java/io/neutral/app")));
        assert_eq!(
            fs::read_to_string(new_activity).unwrap(),
            "package io.neutral.app\nclass MainActivity"
        );
    }

    #[test]
    fn skips_excluded_dependency_directories() {
        let temp = temp_dir("excluded");
        fs::create_dir_all(temp.join("node_modules/pkg")).unwrap();
        fs::create_dir_all(temp.join("app")).unwrap();
        fs::write(temp.join("node_modules/pkg/file.txt"), "OldBrand").unwrap();
        fs::write(temp.join("app/file.txt"), "OldBrand").unwrap();

        let mut config = RunConfig::new(&temp);
        config.replacements = vec![Replacement::new("OldBrand", "NeutralApp")];
        config.dry_run = false;

        let summary = run(&config).unwrap();
        assert_eq!(summary.changed_text_files.len(), 1);
        assert_eq!(
            fs::read_to_string(temp.join("node_modules/pkg/file.txt")).unwrap(),
            "OldBrand"
        );
        assert_eq!(
            fs::read_to_string(temp.join("app/file.txt")).unwrap(),
            "NeutralApp"
        );
    }

    #[test]
    fn supports_custom_icon_names_in_dry_run() {
        let temp = temp_dir("custom-icon");
        fs::create_dir_all(&temp).unwrap();
        let source = temp.join("neutral.png");
        let target = temp.join("status-bar.png");
        fs::write(&source, b"new-icon").unwrap();
        fs::write(&target, b"old-icon").unwrap();

        let mut config = RunConfig::new(&temp);
        config.icon_source = Some(source);
        config.icon_names.push("status-bar.png".to_string());

        let summary = run(&config).unwrap();
        assert_eq!(summary.changed_icon_files, vec![target.clone()]);
        assert_eq!(fs::read(target).unwrap(), b"old-icon");
    }

    #[test]
    fn skips_binary_files_during_text_replacement() {
        let temp = temp_dir("binary");
        fs::create_dir_all(&temp).unwrap();
        let binary = temp.join("image.bin");
        fs::write(&binary, b"OldBrand\0OldBrand").unwrap();

        let mut config = RunConfig::new(&temp);
        config.replacements = vec![Replacement::new("OldBrand", "NeutralApp")];
        config.dry_run = false;

        let summary = run(&config).unwrap();
        assert_eq!(summary.skipped_binary_files, 1);
        assert!(summary.changed_text_files.is_empty());
        assert_eq!(fs::read(binary).unwrap(), b"OldBrand\0OldBrand");
    }

    #[test]
    fn default_icon_generator_returns_png_bytes_without_binary_assets() {
        let icon = default_icon_png_bytes();
        assert!(icon.starts_with(b"\x89PNG\r\n\x1a\n"));
        assert!(icon.windows(4).any(|chunk_type| chunk_type == b"IHDR"));
        assert!(icon.windows(4).any(|chunk_type| chunk_type == b"IDAT"));
        assert!(icon.windows(4).any(|chunk_type| chunk_type == b"IEND"));
    }

    #[test]
    fn project_uses_default_package_when_new_package_is_blank() {
        let temp = temp_dir("project-default-package");
        let project = AnonymizationProject {
            project_dir: temp,
            old_package: Some("com.old.brand".to_string()),
            new_package: Some("   ".to_string()),
            replace_names_and_strings: false,
            string_replacements: Vec::new(),
            anonymize_icons: false,
            icon_source: None,
            rename_paths: true,
            dry_run: true,
        };

        let config = project.into_run_config();
        assert_eq!(
            config.replacements,
            vec![Replacement::new("com.old.brand", DEFAULT_PACKAGE_ID)]
        );
        assert!(config.icon_source.is_none());
    }

    #[test]
    fn project_filters_empty_string_replacements_when_names_are_enabled() {
        let temp = temp_dir("project-strings");
        let mut project = AnonymizationProject::new(temp);
        project.old_package = None;
        project.anonymize_icons = false;
        project.string_replacements = vec![
            Replacement::new("OldName", "NewName"),
            Replacement::new("", "Ignored"),
            Replacement::new("Same", "Same"),
        ];

        let config = project.into_run_config();
        assert_eq!(
            config.replacements,
            vec![Replacement::new("OldName", "NewName")]
        );
    }

    fn temp_dir(label: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("renamer-{label}-{nonce}"))
    }
}
