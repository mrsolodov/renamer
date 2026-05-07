use renamer::{run, Replacement, RunConfig};
use std::env;
use std::path::PathBuf;
use std::process;

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();
    let config = match parse_args(&args) {
        Ok(config) => config,
        Err(message) => {
            eprintln!("{message}\n");
            print_usage();
            process::exit(2);
        }
    };

    match run(&config) {
        Ok(summary) => {
            let mode = if config.dry_run { "dry-run" } else { "apply" };
            println!("Mode: {mode}");
            println!("Scanned files: {}", summary.scanned_files);
            println!("Text files to change: {}", summary.changed_text_files.len());
            for path in &summary.changed_text_files {
                println!("  text: {}", path.display());
            }
            println!(
                "Icon files to replace: {}",
                summary.changed_icon_files.len()
            );
            for path in &summary.changed_icon_files {
                println!("  icon: {}", path.display());
            }
            println!("Paths to rename: {}", summary.renamed_paths.len());
            for (from, to) in &summary.renamed_paths {
                println!("  path: {} -> {}", from.display(), to.display());
            }
            println!("Skipped binary files: {}", summary.skipped_binary_files);

            if config.dry_run && summary.has_changes() {
                println!("Run again with --apply to write these changes.");
            }
        }
        Err(error) => {
            eprintln!("renamer failed: {error}");
            process::exit(1);
        }
    }
}

fn parse_args(args: &[String]) -> Result<RunConfig, String> {
    let mut config = RunConfig::new(".");
    let mut index = 0;

    while index < args.len() {
        match args[index].as_str() {
            "--path" | "-p" => {
                index += 1;
                config.root = args
                    .get(index)
                    .map(PathBuf::from)
                    .ok_or_else(|| "--path requires a project directory".to_string())?;
            }
            "--replace" | "-r" => {
                index += 1;
                let raw = args
                    .get(index)
                    .ok_or_else(|| "--replace requires FROM=TO".to_string())?;
                let (from, to) = raw
                    .split_once('=')
                    .ok_or_else(|| "--replace must use FROM=TO format".to_string())?;
                if from.is_empty() {
                    return Err("--replace FROM cannot be empty".to_string());
                }
                config.replacements.push(Replacement::new(from, to));
            }
            "--icon-source" => {
                index += 1;
                config.icon_source = Some(
                    args.get(index)
                        .map(PathBuf::from)
                        .ok_or_else(|| "--icon-source requires an image path".to_string())?,
                );
            }
            "--icon-name" => {
                index += 1;
                let name = args
                    .get(index)
                    .ok_or_else(|| "--icon-name requires a file name".to_string())?;
                config.icon_names.push(name.to_string());
            }
            "--apply" => config.dry_run = false,
            "--dry-run" => config.dry_run = true,
            "--no-rename-paths" => config.rename_paths = false,
            "--help" | "-h" => {
                print_usage();
                process::exit(0);
            }
            unknown => return Err(format!("unknown argument: {unknown}")),
        }
        index += 1;
    }

    if config.replacements.is_empty() && config.icon_source.is_none() {
        return Err("provide at least one --replace rule or --icon-source".to_string());
    }

    Ok(config)
}

fn print_usage() {
    eprintln!(
        "Usage:\n  renamer --path <PROJECT> --replace <FROM=TO> [--replace <FROM=TO> ...] [--icon-source <IMAGE>] [--icon-name <FILE>] [--no-rename-paths] [--apply]\n\nExamples:\n  renamer -p ./android-app -r com.old.brand=com.neutral.app -r OldBrand=NeutralApp --apply\n  renamer -p ./web-app -r OldBrand=NeutralApp --icon-source ./assets/icon.png --apply\n\nBy default renamer runs in dry-run mode and only reports planned changes."
    );
}
