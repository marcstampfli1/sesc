//! `determinism-probe` — twin-instance comparator skeleton.  SPEC §12.3.
//!
//! Real usage: launch two Sekiro instances with the DLL attached, load
//! the same save, warp to the same area, freeze AI in instance B via
//! `all_no_update_ai`, inject identical inputs to both, then diff
//! shared-entity state over 60 seconds.
//!
//! This tool is a stand-alone diff driver — it connects to both
//! instances via a named-pipe or UDS bridge (owned by the DLL's debug
//! overlay), pulls their `WorldChrMan` snapshot, and reports which
//! fields diverge and at what frame.
//!
//! The transport bridge lives in the DLL (Phase F), so this CLI is
//! currently a scaffold that reads two pre-captured `bincode` snapshot
//! files and diffs them.

use std::path::PathBuf;
use std::process::ExitCode;

#[derive(Debug)]
struct SnapRow {
    entity_id: u32,
    hp: i32,
    posture: f32,
    pos: [f32; 3],
    anim: u32,
}

fn load_snap(path: &PathBuf) -> Result<Vec<SnapRow>, String> {
    let _bytes = std::fs::read(path).map_err(|e| format!("read {}: {e}", path.display()))?;
    // TODO(Phase F, SPEC §12.3): deserialise via `bincode` using the
    // snapshot type defined by the overlay bridge.  For the scaffold,
    // we return an empty snap; this lets the diff loop run so the CLI
    // can be smoke-tested without the bridge.
    Ok(Vec::new())
}

fn diff(a: &[SnapRow], b: &[SnapRow]) -> Vec<String> {
    let mut out = Vec::new();
    for (ra, rb) in a.iter().zip(b.iter()) {
        if ra.entity_id != rb.entity_id {
            out.push(format!(
                "row order differs: a={} b={}",
                ra.entity_id, rb.entity_id
            ));
            continue;
        }
        if ra.hp != rb.hp {
            out.push(format!("entity {}: hp {} vs {}", ra.entity_id, ra.hp, rb.hp));
        }
        if (ra.posture - rb.posture).abs() > 0.5 {
            out.push(format!(
                "entity {}: posture {:.2} vs {:.2}",
                ra.entity_id, ra.posture, rb.posture
            ));
        }
        for i in 0..3 {
            if (ra.pos[i] - rb.pos[i]).abs() > 0.05 {
                out.push(format!(
                    "entity {}: pos[{}] {:.3} vs {:.3}",
                    ra.entity_id, i, ra.pos[i], rb.pos[i]
                ));
            }
        }
        if ra.anim != rb.anim {
            out.push(format!("entity {}: anim {} vs {}", ra.entity_id, ra.anim, rb.anim));
        }
    }
    if a.len() != b.len() {
        out.push(format!("row count: {} vs {}", a.len(), b.len()));
    }
    out
}

fn main() -> ExitCode {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_env("SEKIRO_COOP_LOG")
                .unwrap_or_else(|_| "info".into()),
        )
        .init();

    let mut args = std::env::args().skip(1);
    let a = match args.next() {
        Some(p) => PathBuf::from(p),
        None => {
            eprintln!("usage: determinism-probe <snap-a.bin> <snap-b.bin>");
            return ExitCode::FAILURE;
        }
    };
    let b = match args.next() {
        Some(p) => PathBuf::from(p),
        None => {
            eprintln!("usage: determinism-probe <snap-a.bin> <snap-b.bin>");
            return ExitCode::FAILURE;
        }
    };

    let sa = match load_snap(&a) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::FAILURE;
        }
    };
    let sb = match load_snap(&b) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::FAILURE;
        }
    };

    let diffs = diff(&sa, &sb);
    if diffs.is_empty() {
        println!("no divergence detected across {} rows", sa.len().min(sb.len()));
        ExitCode::SUCCESS
    } else {
        println!("divergence report ({} entries):", diffs.len());
        for d in &diffs {
            println!("  {d}");
        }
        ExitCode::from(2)
    }
}
