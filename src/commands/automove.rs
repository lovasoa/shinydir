use std::fs;
use std::path::PathBuf;

use colored::Colorize;

use crate::automove::{AutoMoveResult, AutoMoveResultEntry, AutoMoveRule};
use crate::config::{Config, Settings};

pub fn execute(
    config: &Config,
    config_dir: PathBuf,
    target: Option<PathBuf>,
    list: bool,
    dry_run: bool,
) -> anyhow::Result<()> {
    // Setup automove
    let parent = target.map(fs::canonicalize).transpose()?;
    let automove = crate::automove::from_config(config, config_dir, parent)?;

    // Warn user about slow execution time
    let script_warning = config.automove.script_warning
        && automove
            .rules
            .iter()
            .flat_map(|rule| &rule.to_script)
            .count()
            > 0;
    if script_warning {
        // print on stderr to not affect pipe input (e.g. when using --list)
        if config.settings.color {
            eprintln!("{} Your auto-move rules are configured to call scripts {}. If execution time gets too long, {}.", "Heads up!".bright_red().bold(), "(to_script)".white().dimmed(), "scripts are the cause".bold());
        } else {
            eprintln!("Heads up! Your auto-move rules are configured to call scripts (to_script). If execution time gets too long, scripts are the cause.");
        }
    }

    // Warn user about dry run
    if dry_run {
        if config.settings.color {
            eprintln!(
                "{} Auto-move running in {}, no files will actually be moved.",
                "Info!".bright_blue().bold(),
                "dry mode".white().bold()
            );
        } else {
            eprintln!("Info! Auto-move running in dry mode, no files will actually be moved.");
        }
    }

    // Get entries to move
    let mut results = automove.run();

    // Print space after info message
    if (script_warning || dry_run) && !list {
        eprintln!("");
    }

    // Move files
    for result in results.iter_mut() {
        if let AutoMoveResult::Ok { entries, .. } = result {
            for entry_res in entries.iter_mut() {
                if entry_res.is_err() {
                    continue;
                }
                let entry = entry_res.as_ref().unwrap();
                if !dry_run {
                    if let Some(parent) = entry.move_to.parent() {
                        if let Err(err) = fs::create_dir_all(parent).map_err(|err| {
                            anyhow::format_err!(
                                "Couldn't create directory {}: {}",
                                parent.to_string_lossy(),
                                err
                            )
                        }) {
                            *entry_res = Err(err);
                            continue;
                        }
                    }
                }
                let new_err = match entry.move_to.try_exists() {
                    Ok(true) => Some(anyhow::format_err!(
                        "Moving to {} would overwrite a file",
                        entry.move_to.to_string_lossy()
                    )),
                    Ok(false) if !dry_run => fs::rename(&entry.file, &entry.move_to)
                        .map_err(|err| {
                            anyhow::format_err!(
                                "Couldn't move {} to {}: {}",
                                entry.file.to_string_lossy(),
                                entry.move_to.to_string_lossy(),
                                err
                            )
                        })
                        .err(),
                    Ok(false) => None,
                    Err(err) => Some(anyhow::format_err!(
                        "Cannot check overwrite status for {}: {}",
                        entry.move_to.to_string_lossy(),
                        err
                    )),
                };
                if let Some(err) = new_err {
                    *entry_res = Err(err);
                }
            }
        }
    }

    // Display output
    let mut first_it = true;
    for result in results {
        if first_it {
            first_it = false;
        } else if !list {
            println!("");
        }

        match result {
            AutoMoveResult::DirDoesNotExist { rule } if !list => {
                let display_name = if rule.custom_name.is_none() && config.settings.color {
                    format!("{}", rule.display_name().italic())
                } else {
                    rule.display_name()
                };
                if config.settings.color {
                    eprintln!("{} {}", display_name.red(), "Directory does not exist!");
                } else {
                    eprintln!("{}: Directory does not exist!", display_name);
                }
            }
            AutoMoveResult::Ok { rule, entries } => {
                if list {
                    let line_entries = entries
                        .iter()
                        .flat_map(|entry| entry.as_ref().ok())
                        .map(|entry| {
                            format!(
                                "{} {}",
                                entry.file.to_string_lossy().replace(" ", "\\ "),
                                entry.move_to.to_string_lossy().replace(" ", "\\ ")
                            )
                        })
                        .collect::<Vec<_>>();
                    if !line_entries.is_empty() {
                        println!("{}", line_entries.join("\n"));
                    }
                } else {
                    print_entries(&config.settings, rule, entries);
                }
            }
            _ => {}
        };
    }

    Ok(())
}

fn print_entries(
    settings: &Settings,
    rule: &AutoMoveRule,
    entries: Vec<Result<AutoMoveResultEntry, anyhow::Error>>,
) {
    let display_name = if rule.custom_name.is_none() && settings.color {
        format!("{}", rule.display_name().italic())
    } else {
        rule.display_name()
    };

    if entries.is_empty() {
        let checkmark = if settings.unicode { "\u{f00c}" } else { "OK" };
        if settings.color {
            println!("{} {}", display_name.blue(), checkmark.green().bold())
        } else {
            println!("{} {}", display_name, checkmark);
        }
        return;
    }

    let valid_entries = entries.iter().filter(|entry| entry.is_ok()).count();
    let errors = entries.iter().filter(|entry| entry.is_err()).count();

    let dot = if settings.unicode { "\u{f444}" } else { "-" };
    let mut info = Vec::new();
    if valid_entries > 0 {
        let msg = format!("{} files moved", valid_entries);
        if settings.color {
            info.push(format!("{}", msg.bright_yellow()));
        } else {
            info.push(msg);
        }
    }
    if errors > 0 {
        let msg = format!("{} errors", errors);
        if settings.color {
            info.push(format!("{}", msg.bright_red()));
        } else {
            info.push(msg);
        }
    }
    let info_sep = if settings.color { " " } else { ", " };
    if settings.color {
        println!(
            "{} {} {}",
            display_name.blue(),
            dot.white().dimmed(),
            info.join(info_sep)
        );
    } else {
        println!("{} {} {}", display_name, dot, info.join(info_sep));
    }

    let moved_to_dirs_no_dedup = entries
        .iter()
        .flat_map(|entry| entry.as_ref().ok())
        .flat_map(|entry| entry.move_to.parent())
        .map(|path| path.to_path_buf())
        .collect::<Vec<_>>();
    let mut moved_to_dirs = moved_to_dirs_no_dedup.clone();
    moved_to_dirs.sort();
    moved_to_dirs.dedup();

    if moved_to_dirs.is_empty() {
        for err in entries.iter().flat_map(|entry| entry.as_ref().err()) {
            eprintln!("{}", format!("{}", err).bright_red().italic());
        }
        return;
    }

    let arrow = "=>";
    let rel_dirs_it = moved_to_dirs
        .iter()
        .map(|path| {
            let count = moved_to_dirs_no_dedup
                .iter()
                .filter(|&dir| path.eq(dir))
                .count();
            (path, count)
        })
        .map(|(path, count)| (path.strip_prefix(&rule.directory).unwrap_or(path), count));
    if settings.color {
        let tmp = rel_dirs_it
            .map(|(path, count)| {
                format!(
                    "{} {}",
                    path.to_string_lossy().bright_blue(),
                    format!("({})", count).dimmed()
                )
            })
            .collect::<Vec<_>>();
        println!(
            "{} {} {}",
            arrow.black(),
            "Moved To".bold(),
            tmp.join(&format!("{}", ", ".bright_black()))
        );
    } else {
        let tmp = rel_dirs_it
            .map(|(path, count)| format!("{} {}", path.to_string_lossy(), count))
            .collect::<Vec<_>>();
        println!("{} Moved To: {}", arrow, tmp.join(", "))
    }

    for err in entries.iter().flat_map(|entry| entry.as_ref().err()) {
        eprintln!("{}", format!("{}", err).bright_red().italic());
    }
}
