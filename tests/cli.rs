use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

#[test]
fn cli_dry_run_reports_android_changes_without_writing() {
    let project = android_fixture("dry-run");
    let output = Command::new(env!("CARGO_BIN_EXE_renamer"))
        .args([
            "--path",
            project.to_str().unwrap(),
            "--replace",
            "com.fossify.calculator=io.neutral.calc",
            "--replace",
            "Fossify Calculator=Neutral Calculator",
            "--icon-source",
            project.join("neutral.png").to_str().unwrap(),
        ])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("Mode: dry-run"));
    assert!(stdout.contains("Text files to change: 3"));
    assert!(stdout.contains("Icon files to replace: 1"));
    assert!(stdout.contains("Paths to rename: 1"));
    assert!(project
        .join("app/src/main/java/com/fossify/calculator/MainActivity.kt")
        .exists());
    assert_eq!(
        fs::read_to_string(project.join("app/src/main/res/values/strings.xml")).unwrap(),
        "<resources><string name=\"app_name\">Fossify Calculator</string></resources>"
    );
}

#[test]
fn cli_apply_anonymizes_android_fixture() {
    let project = android_fixture("apply");
    let output = Command::new(env!("CARGO_BIN_EXE_renamer"))
        .args([
            "--path",
            project.to_str().unwrap(),
            "--replace",
            "com.fossify.calculator=io.neutral.calc",
            "--replace",
            "Fossify Calculator=Neutral Calculator",
            "--icon-source",
            project.join("neutral.png").to_str().unwrap(),
            "--apply",
        ])
        .output()
        .unwrap();

    assert!(output.status.success());
    assert!(project
        .join("app/src/main/java/io/neutral/calc/MainActivity.kt")
        .exists());
    assert!(fs::read_to_string(project.join("app/build.gradle.kts"))
        .unwrap()
        .contains("namespace = \"io.neutral.calc\""));
    assert!(
        fs::read_to_string(project.join("app/src/main/res/values/strings.xml"))
            .unwrap()
            .contains("Neutral Calculator")
    );
    assert_eq!(
        fs::read(project.join("app/src/main/res/mipmap-hdpi/ic_launcher.png")).unwrap(),
        b"neutral-icon"
    );
}

fn android_fixture(label: &str) -> PathBuf {
    let root = temp_dir(label);
    write_file(
        &root.join("app/src/main/java/com/fossify/calculator/MainActivity.kt"),
        "package com.fossify.calculator\nclass MainActivity",
    );
    write_file(
        &root.join("app/src/main/res/values/strings.xml"),
        "<resources><string name=\"app_name\">Fossify Calculator</string></resources>",
    );
    write_file(
        &root.join("app/build.gradle.kts"),
        "android { namespace = \"com.fossify.calculator\" }",
    );
    write_bytes(
        &root.join("app/src/main/res/mipmap-hdpi/ic_launcher.png"),
        b"old-icon",
    );
    write_bytes(&root.join("neutral.png"), b"neutral-icon");
    root
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
    std::env::temp_dir().join(format!("renamer-cli-{label}-{nonce}"))
}
