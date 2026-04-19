//! `sekiro-coop-emevd` CLI.  SPEC §8.3.

use std::path::PathBuf;
use std::process::ExitCode;

use sekiro_coop_emevd::gen::build_custom_events;
use sekiro_coop_emevd::patch::{MultiplayerState, PatchPlan};

fn usage() {
    eprintln!(
        "sekiro-coop-emevd — patch common.emevd for two-player coop

Usage:
    sekiro-coop-emevd <input.emevd> <output.emevd> \\
        [--promote-to host|client] \\
        [--rng-range START END] \\
        [--boss-id ID]... \\
        [--no-inject]
"
    );
}

fn main() -> ExitCode {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_env("SEKIRO_COOP_LOG")
                .unwrap_or_else(|_| "info".into()),
        )
        .init();

    let args: Vec<String> = std::env::args().collect();
    if args.len() < 3 {
        usage();
        return ExitCode::FAILURE;
    }

    let input = PathBuf::from(&args[1]);
    let output = PathBuf::from(&args[2]);
    let mut promote_to = MultiplayerState::HOST;
    let mut rng_range = (30_000u32, 30_063u32);
    let mut bosses: Vec<i32> = Vec::new();
    let mut inject = true;

    let mut i = 3;
    while i < args.len() {
        match args[i].as_str() {
            "--promote-to" => {
                i += 1;
                promote_to = match args.get(i).map(|s| s.as_str()) {
                    Some("host") => MultiplayerState::HOST,
                    Some("client") => MultiplayerState::CLIENT,
                    _ => {
                        eprintln!("error: --promote-to expects 'host' or 'client'");
                        return ExitCode::FAILURE;
                    }
                };
            }
            "--rng-range" => {
                i += 1;
                let s: u32 = match args.get(i).and_then(|s| s.parse().ok()) {
                    Some(v) => v,
                    None => {
                        eprintln!("error: --rng-range expects two u32");
                        return ExitCode::FAILURE;
                    }
                };
                i += 1;
                let e: u32 = match args.get(i).and_then(|s| s.parse().ok()) {
                    Some(v) => v,
                    None => {
                        eprintln!("error: --rng-range expects two u32");
                        return ExitCode::FAILURE;
                    }
                };
                rng_range = (s, e);
            }
            "--boss-id" => {
                i += 1;
                let id: i32 = match args.get(i).and_then(|s| s.parse().ok()) {
                    Some(v) => v,
                    None => {
                        eprintln!("error: --boss-id expects i32");
                        return ExitCode::FAILURE;
                    }
                };
                bosses.push(id);
            }
            "--no-inject" => inject = false,
            other => {
                eprintln!("unknown flag: {other}");
                usage();
                return ExitCode::FAILURE;
            }
        }
        i += 1;
    }

    let inject_events = if inject {
        Some(build_custom_events(rng_range, &bosses))
    } else {
        None
    };

    let plan = PatchPlan {
        input,
        output,
        promote_to,
        inject_events,
    };

    match plan.run() {
        Ok(report) => {
            tracing::info!(
                promoted = report.promoted,
                injected = report.injected,
                "patch complete"
            );
            ExitCode::SUCCESS
        }
        Err(e) => {
            tracing::error!(%e, "patch failed");
            ExitCode::FAILURE
        }
    }
}
