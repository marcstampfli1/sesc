//! Offline AOB scanner.  Reads a binary (`sekiro.exe`) and runs every
//! AOB from `sekiro_sdk_sys::aob::patterns`.
//!
//! Useful for:
//! - Comparing hardcoded RVAs against AOB-resolved addresses (should match
//!   on all 5 known patch versions).
//! - First-time testing when a patch version changes.

use std::path::PathBuf;
use std::process::ExitCode;

use sekiro_sdk_sys::aob::{patterns, AobPattern, scan_and_resolve};

struct Named {
    name: &'static str,
    pat: AobPattern,
    disp_offset: usize,
    instr_len: usize,
}

fn entries() -> Vec<Named> {
    vec![
        Named { name: "quitout", pat: patterns::quitout(), disp_offset: 3, instr_len: 7 },
        Named { name: "render_world", pat: patterns::render_world(), disp_offset: 2, instr_len: 7 },
        Named { name: "debug_render", pat: patterns::debug_render(), disp_offset: 4, instr_len: 8 },
        Named { name: "igt", pat: patterns::igt(), disp_offset: 3, instr_len: 7 },
        Named { name: "player_position", pat: patterns::player_position(), disp_offset: 3, instr_len: 8 },
        Named { name: "debug_flags", pat: patterns::debug_flags(), disp_offset: 2, instr_len: 7 },
        Named { name: "show_cursor", pat: patterns::show_cursor(), disp_offset: 3, instr_len: 7 },
        Named { name: "no_logo", pat: patterns::no_logo(), disp_offset: 0, instr_len: 0 },
        Named { name: "font_patch", pat: patterns::font_patch(), disp_offset: 0, instr_len: 0 },
        Named { name: "activate_debug_menu", pat: patterns::activate_debug_menu(), disp_offset: 0, instr_len: 0 },
        Named { name: "menu_draw_hook", pat: patterns::menu_draw_hook(), disp_offset: 0, instr_len: 0 },
    ]
}

fn main() -> ExitCode {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_env("SEKIRO_COOP_LOG")
                .unwrap_or_else(|_| "info".into()),
        )
        .init();

    let mut args = std::env::args().skip(1);
    let path = match args.next() {
        Some(p) => PathBuf::from(p),
        None => {
            eprintln!("usage: aob-scanner <path-to-sekiro.exe>");
            return ExitCode::FAILURE;
        }
    };
    let bytes = match std::fs::read(&path) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("read {}: {e}", path.display());
            return ExitCode::FAILURE;
        }
    };

    let mut ok = 0;
    let mut fail = 0;
    for e in entries() {
        let (offset, note) = match e.pat.scan(&bytes) {
            Ok(at) if e.instr_len > 0 => {
                match scan_and_resolve(&bytes, &e.pat, e.disp_offset, e.instr_len) {
                    Ok(resolved) => (Some(resolved), format!("@ {:#x} -> {:#x}", at, resolved)),
                    Err(err) => (Some(at), format!("@ {:#x} (resolve failed: {err})", at)),
                }
            }
            Ok(at) => (Some(at), format!("@ {:#x}", at)),
            Err(err) => (None, format!("{err}")),
        };
        match offset {
            Some(_) => {
                println!("[ok]  {:<24} {}", e.name, note);
                ok += 1;
            }
            None => {
                println!("[miss] {:<24} {}", e.name, note);
                fail += 1;
            }
        }
    }
    println!("\n{ok} hits, {fail} misses");
    if fail == 0 {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(2)
    }
}
