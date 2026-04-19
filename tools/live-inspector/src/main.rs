//! `live-inspector` — attaches to a running sekiro.exe and runs every
//! AOB + pointer-chain validation we have, using the native Windows
//! `OpenProcess` + `ReadProcessMemory` APIs.  This lets us do live
//! reconnaissance *without* loading the DLL into Sekiro.
//!
//! Usage:
//!
//! ```text
//! live-inspector                          # auto-detect sekiro.exe PID
//! live-inspector --pid 12345              # specific PID
//! live-inspector --json findings.json     # machine-readable output
//! ```
//!
//! Output covers:
//!
//! - Module base, size, detected version
//! - Every OSINT §1.1/§1.2 AOB: hit/miss + resolved RVA
//! - Every documented pointer chain: resolved address + value
//! - Heuristic ChrIns offset candidates if the player pointer resolves

#![cfg(target_os = "windows")]

use anyhow::{anyhow, bail, Context, Result};
use sekiro_sdk_sys::aob::{patterns, resolve_rip_relative, AobPattern};
use sekiro_sdk_sys::version::{detect_version_live, GameVersion};
use serde::Serialize;
use std::ffi::c_void;
use std::path::PathBuf;
use windows::core::PSTR;
use windows::Win32::Foundation::{CloseHandle, HANDLE};
use windows::Win32::System::Diagnostics::Debug::ReadProcessMemory;
use windows::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, Module32First, Module32Next, Process32First, Process32Next,
    MODULEENTRY32, PROCESSENTRY32, TH32CS_SNAPMODULE, TH32CS_SNAPMODULE32, TH32CS_SNAPPROCESS,
};
use windows::Win32::System::Threading::{
    OpenProcess, PROCESS_QUERY_INFORMATION, PROCESS_VM_READ,
};

#[derive(Debug, Default, Serialize)]
struct Report {
    pid: u32,
    module_name: String,
    module_base: String,
    module_size: u64,
    detected_version: String,
    aob_hits: Vec<AobResult>,
    chain_results: Vec<ChainResult>,
    natives: Vec<NativeHit>,
    chrins_base: Option<String>,
    chrins_candidates: Vec<ChrInsCandidate>,
    notes: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
struct NativeHit {
    name: String,
    address: Option<String>,
}

#[derive(Debug, Serialize)]
struct ChrInsCandidate {
    field: &'static str,
    offset: String,
    value: String,
    confidence: f32,
}

#[derive(Debug, Serialize)]
struct AobResult {
    name: &'static str,
    hit: bool,
    offset_in_module: Option<String>,
    resolved_rva: Option<String>,
    mismatch_vs_hardcoded: Option<String>,
}

#[derive(Debug, Serialize)]
struct ChainResult {
    name: &'static str,
    path: &'static str,
    final_address: Option<String>,
    value_hex: Option<String>,
    ok: bool,
}

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_env("SEKIRO_COOP_LOG")
                .unwrap_or_else(|_| "info".into()),
        )
        .init();

    let args: Vec<String> = std::env::args().collect();
    let mut pid: Option<u32> = None;
    let mut json_out: Option<PathBuf> = None;
    let mut verbose = false;
    let mut watch_secs: Option<u32> = None;
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--pid" => {
                i += 1;
                pid = Some(
                    args.get(i)
                        .and_then(|s| s.parse().ok())
                        .ok_or_else(|| anyhow!("--pid expects a u32"))?,
                );
            }
            "--json" => {
                i += 1;
                json_out = Some(PathBuf::from(
                    args.get(i).ok_or_else(|| anyhow!("--json expects a path"))?,
                ));
            }
            "--watch" => {
                i += 1;
                watch_secs = Some(
                    args.get(i)
                        .and_then(|s| s.parse().ok())
                        .ok_or_else(|| anyhow!("--watch expects seconds"))?,
                );
            }
            "-v" | "--verbose" => verbose = true,
            "-h" | "--help" => {
                usage();
                return Ok(());
            }
            other => bail!("unknown flag: {other}"),
        }
        i += 1;
    }

    let pid = match pid {
        Some(p) => p,
        None => find_sekiro_pid().context("auto-detect sekiro.exe")?,
    };
    tracing::info!(pid, "attaching");

    let target = OpenTarget::open(pid)?;
    let module = target.find_module("sekiro.exe")?;

    // --- Watch mode: repeatedly sample the ChrIns region, then print
    // every offset that changed value during the observation window ---
    if let Some(secs) = watch_secs {
        return run_watch(&target, &module, secs);
    }
    tracing::info!(
        base = format!("{:#x}", module.base),
        size = module.size,
        "sekiro.exe module found"
    );

    let image = target.read_module(&module)?;
    let version = unsafe { detect_version_live(&image, module.size as u32) };

    let mut report = Report {
        pid,
        module_name: module.name.clone(),
        module_base: format!("{:#x}", module.base),
        module_size: module.size as u64,
        detected_version: format!("{version}"),
        ..Default::default()
    };

    if verbose {
        println!("module base: {:#x}", module.base);
        println!("module size: {} bytes", module.size);
        println!("detected version: {version}");
    }

    // --- AOB sweep ---
    let entries = aob_entries(version);
    for entry in entries {
        let hit = entry.pat.scan(&image).ok();
        let mut result = AobResult {
            name: entry.name,
            hit: hit.is_some(),
            offset_in_module: hit.map(|o| format!("{:#x}", o)),
            resolved_rva: None,
            mismatch_vs_hardcoded: None,
        };
        if let Some(hit_off) = hit {
            if entry.instr_len > 0 {
                if let Ok(rva) =
                    resolve_rip_relative(&image, hit_off, entry.disp_offset, entry.instr_len)
                {
                    result.resolved_rva = Some(format!("{:#x}", rva));
                    if let Some(expected_rva) = entry.hardcoded_rva {
                        if rva != expected_rva {
                            result.mismatch_vs_hardcoded = Some(format!(
                                "scan={:#x} hardcoded={:#x}",
                                rva, expected_rva
                            ));
                        }
                    }
                }
            }
        }
        report.aob_hits.push(result);
    }

    // --- Pointer-chain walks ---
    // Prefer AOB-resolved RVAs over the hardcoded version table; that
    // way chain walks succeed even when version detection fails.
    let aob_rva = |sym_name: &str| -> Option<usize> {
        report
            .aob_hits
            .iter()
            .find(|r| r.name == sym_name)
            .and_then(|r| r.resolved_rva.as_ref())
            .and_then(|s| usize::from_str_radix(s.trim_start_matches("0x"), 16).ok())
    };
    let version_addrs = sekiro_sdk_sys::offsets::BaseAddrs::for_version(version);
    let hardcoded = |sym: &str| -> Option<usize> {
        version_addrs.map(|a| match sym {
            "player_position" => a.player_position,
            "igt" => a.igt,
            "fps" => a.fps,
            _ => usize::MAX,
        })
    };

    // Pointer chains for the player's state.  SEKIRO_OFFSETS.md D.4
    // says WorldChrMan and libsekiro's `player_position` share the
    // SAME pointer at RVA 0x3d7a1e0; we use `player_position` as the
    // symbol name here since that's what my BaseAddrs table has.
    //
    // Player state (all rooted at WorldChrMan → +0x88 → +0x1ff8):
    //   HP          = → +0x18 → +0x130
    //   MaxHP       = → +0x18 → +0x134
    //   Posture     = → +0x18 → +0x148
    //   MaxPosture  = → +0x18 → +0x14C
    //   Position    = → +0x68 → +0x80/84/88
    //   CurrentAnim = → +0x10 → +0x20
    //   PlaySpeed   = → +0x28 → +0xD00
    let chains: &[(&str, &str, &[isize], &str, usize)] = &[
        (
            "player_position(libsekiro)",
            "player_position",
            &[0x48, 0x28, 0x80],
            "player_position → +0x48 → +0x28 → +0x80",
            16,
        ),
        (
            "player HP",
            "player_position",
            &[0x88, 0x1ff8, 0x18, 0x130],
            "WorldChrMan → +0x88 → +0x1ff8 → +0x18 → +0x130",
            4,
        ),
        (
            "player MaxHP",
            "player_position",
            &[0x88, 0x1ff8, 0x18, 0x134],
            "WorldChrMan → +0x88 → +0x1ff8 → +0x18 → +0x134",
            4,
        ),
        (
            "player Posture",
            "player_position",
            &[0x88, 0x1ff8, 0x18, 0x148],
            "WorldChrMan → +0x88 → +0x1ff8 → +0x18 → +0x148",
            4,
        ),
        (
            "player MaxPosture",
            "player_position",
            &[0x88, 0x1ff8, 0x18, 0x14C],
            "WorldChrMan → +0x88 → +0x1ff8 → +0x18 → +0x14C",
            4,
        ),
        (
            "player Pos[xyz]",
            "player_position",
            &[0x88, 0x1ff8, 0x68, 0x80],
            "WorldChrMan → +0x88 → +0x1ff8 → +0x68 → +0x80",
            12,
        ),
        (
            "player CurrentAnim",
            "player_position",
            &[0x88, 0x1ff8, 0x10, 0x20],
            "WorldChrMan → +0x88 → +0x1ff8 → +0x10 → +0x20",
            4,
        ),
        (
            "player PlaySpeed",
            "player_position",
            &[0x88, 0x1ff8, 0x28, 0xD00],
            "WorldChrMan → +0x88 → +0x1ff8 → +0x28 → +0xD00",
            4,
        ),
        ("igt_ms u32", "igt", &[0x9C], "igt → +0x9C", 4),
        ("fps f32", "fps", &[0x2BC], "fps → +0x2BC", 4),
    ];
    for (name, sym, offsets, path, read_len) in chains {
        let rva = aob_rva(sym).or_else(|| hardcoded(sym));
        let result = rva
            .filter(|r| *r != usize::MAX)
            .map(|r| module.base + r)
            .and_then(|sym_addr| target.walk_chain(sym_addr, offsets, *read_len).ok())
            .map(|(final_addr, value)| (final_addr, hexdump(&value)));
        let ok = result.is_some();
        report.chain_results.push(ChainResult {
            name,
            path,
            final_address: result.as_ref().map(|(a, _)| format!("{:#x}", a)),
            value_hex: result.map(|(_, v)| v),
            ok,
        });
    }

    // --- Override detected version if all AOBs match a known table ---
    if let Some(final_version) = infer_version_from_aobs(&report.aob_hits) {
        report.detected_version = format!("{final_version} (from AOB scan)");
    }

    // --- Scan the extended 18 symbol bases + 13 native functions ---
    let natives = sekiro_sdk_sys::natives::Natives::scan(&image, module.base);
    for (name, addr) in sekiro_sdk_sys::natives::dump(&natives) {
        report.natives.push(NativeHit {
            name: name.to_string(),
            address: addr.map(|a| format!("{:#x}", a)),
        });
    }

    // --- ChrIns discovery (when player_position chain resolved) ---
    let player_chain = report
        .chain_results
        .iter()
        .find(|c| c.name == "player_position[f32;4]")
        .and_then(|c| c.final_address.as_ref())
        .and_then(|s| usize::from_str_radix(s.trim_start_matches("0x"), 16).ok());
    if let Some(pos_addr) = player_chain {
        let chrins_base = pos_addr.saturating_sub(0x80);
        match target.read_buffer(chrins_base, 0x2000) {
            Ok(buf) => {
                let scan = scan_chrins_candidates(&buf, chrins_base);
                report.chrins_base = Some(format!("{:#x}", chrins_base));
                report.chrins_candidates = scan;
            }
            Err(e) => report
                .notes
                .push(format!("chrins read failed at {:#x}: {e}", chrins_base)),
        }
    }

    // --- Render report ---
    render_text(&report);
    if let Some(path) = json_out {
        std::fs::write(&path, render_json(&report))?;
        tracing::info!(path = %path.display(), "json report written");
    }

    Ok(())
}

fn render_json(r: &Report) -> String {
    // Minimal JSON emitter — avoids pulling serde_json.
    let mut s = String::new();
    s.push_str("{\n");
    s.push_str(&format!("  \"pid\": {},\n", r.pid));
    s.push_str(&format!("  \"module_name\": {:?},\n", r.module_name));
    s.push_str(&format!("  \"module_base\": {:?},\n", r.module_base));
    s.push_str(&format!("  \"module_size\": {},\n", r.module_size));
    s.push_str(&format!("  \"detected_version\": {:?},\n", r.detected_version));
    s.push_str("  \"aob_hits\": [\n");
    for (i, a) in r.aob_hits.iter().enumerate() {
        s.push_str("    {");
        s.push_str(&format!("\"name\": {:?}, ", a.name));
        s.push_str(&format!("\"hit\": {}, ", a.hit));
        s.push_str(&format!("\"offset\": {:?}, ",
            a.offset_in_module.as_deref().unwrap_or("null")));
        s.push_str(&format!("\"rva\": {:?}, ",
            a.resolved_rva.as_deref().unwrap_or("null")));
        s.push_str(&format!("\"mismatch\": {:?}",
            a.mismatch_vs_hardcoded.as_deref().unwrap_or("null")));
        s.push_str(if i + 1 == r.aob_hits.len() { "}\n" } else { "},\n" });
    }
    s.push_str("  ],\n");
    s.push_str("  \"chains\": [\n");
    for (i, c) in r.chain_results.iter().enumerate() {
        s.push_str("    {");
        s.push_str(&format!("\"name\": {:?}, ", c.name));
        s.push_str(&format!("\"path\": {:?}, ", c.path));
        s.push_str(&format!("\"ok\": {}, ", c.ok));
        s.push_str(&format!("\"address\": {:?}, ",
            c.final_address.as_deref().unwrap_or("null")));
        s.push_str(&format!("\"value\": {:?}",
            c.value_hex.as_deref().unwrap_or("null")));
        s.push_str(if i + 1 == r.chain_results.len() { "}\n" } else { "},\n" });
    }
    s.push_str("  ]\n}\n");
    s
}

/// Watch mode: sample the ChrIns region repeatedly, diff each pair of
/// samples, and report per-offset ranges of observed values.  An
/// offset whose value varied across samples is a strong candidate for
/// a live field (HP, posture, animation_id, etc.).
fn run_watch(target: &OpenTarget, module: &LoadedModule, secs: u32) -> Result<()> {
    use sekiro_sdk_sys::offsets::BaseAddrs;
    use std::collections::HashMap;
    use std::time::{Duration, Instant};

    let image = target.read_module(module)?;
    let version = unsafe { detect_version_live(&image, module.size as u32) };
    let addrs = BaseAddrs::for_version(version)
        .ok_or_else(|| anyhow!("no base-addrs for version {version}"))?;

    // Resolve player_position symbol → walk chain → container = pos − 0x80.
    let sym_addr = module.base + addrs.player_position;
    let (pos_addr, _) = target.walk_chain(sym_addr, &[0x48, 0x28, 0x80], 16)?;
    let chrins_base = pos_addr - 0x80;
    println!("watching ChrIns @ {:#x} for {}s ...", chrins_base, secs);

    const BUF_SIZE: usize = 0x4000; // enlarged — HP might be beyond 0x2000
    let interval = Duration::from_millis(200);
    let deadline = Instant::now() + Duration::from_secs(secs as u64);

    // Per-offset: track observed values.  We record (min, max, first, last,
    // change_count).  Only offsets with >1 distinct value are reported.
    #[derive(Default, Clone, Copy, Debug)]
    struct Trace {
        first_u32: u32,
        last_u32: u32,
        min_u32: u32,
        max_u32: u32,
        changes: u32,
    }
    let mut seen: HashMap<usize, Trace> = HashMap::new();
    let mut samples = 0u32;
    let mut prev: Option<Vec<u8>> = None;

    while Instant::now() < deadline {
        let buf = match target.read_buffer(chrins_base, BUF_SIZE) {
            Ok(b) => b,
            Err(e) => {
                eprintln!("read failed: {e}");
                std::thread::sleep(interval);
                continue;
            }
        };
        samples += 1;
        for off in (0..buf.len()).step_by(4) {
            if off + 4 > buf.len() {
                break;
            }
            let v = u32::from_le_bytes(buf[off..off + 4].try_into().unwrap());
            let entry = seen.entry(off).or_insert_with(|| Trace {
                first_u32: v,
                last_u32: v,
                min_u32: v,
                max_u32: v,
                changes: 0,
            });
            if entry.last_u32 != v {
                entry.changes = entry.changes.saturating_add(1);
                entry.last_u32 = v;
                if v < entry.min_u32 {
                    entry.min_u32 = v;
                }
                if v > entry.max_u32 {
                    entry.max_u32 = v;
                }
            }
        }
        prev = Some(buf);
        std::thread::sleep(interval);
    }
    let _ = prev; // unused

    println!(
        "\n=== watch done: {samples} samples, ChrIns @ {:#x} ===\n",
        chrins_base
    );

    // Collect changing offsets and rank by "interestingness":
    //  - i32 range looks like HP (min ≥ 0, max ≤ 20000, range ≥ 50)
    //  - f32 range looks like posture (finite, range 0–2000)
    let mut changed: Vec<(usize, Trace)> =
        seen.iter().filter(|(_, t)| t.changes > 0).map(|(o, t)| (*o, *t)).collect();
    changed.sort_by_key(|(off, _)| *off);

    println!("offset     changes  first    last     min      max      i32-range  f32?");
    println!("--------   -------  -------  -------  -------  -------  ---------  --------");
    for (off, t) in &changed {
        let as_i32_first = t.first_u32 as i32;
        let as_i32_last = t.last_u32 as i32;
        let as_i32_min = t.min_u32 as i32;
        let as_i32_max = t.max_u32 as i32;
        let i32_range = (as_i32_max as i64) - (as_i32_min as i64);
        let f_first = f32::from_bits(t.first_u32);
        let f_last = f32::from_bits(t.last_u32);
        let likely_hp = i32_range > 10 && (0..=20_000).contains(&as_i32_min)
            && (0..=20_000).contains(&as_i32_max);
        let likely_posture = f_first.is_finite()
            && f_last.is_finite()
            && f_first.abs() <= 2000.0
            && f_last.abs() <= 2000.0
            && f_first != f_last;
        let flag = if likely_hp {
            "  <-HP?"
        } else if likely_posture {
            "  <-posture/f32?"
        } else {
            ""
        };
        println!(
            "{:#08x}  {:>7}  {:>7}  {:>7}  {:>7}  {:>7}  {:>9}  f={:>7.1} -> {:>7.1}{}",
            off,
            t.changes,
            as_i32_first,
            as_i32_last,
            as_i32_min,
            as_i32_max,
            i32_range,
            f_first,
            f_last,
            flag
        );
    }

    Ok(())
}

fn usage() {
    println!(
        "live-inspector — attach to a running sekiro.exe and validate AOBs + pointer chains

Usage:
    live-inspector                          # auto-detect sekiro.exe PID
    live-inspector --pid 12345              # specific PID
    live-inspector --json out/findings.json # machine-readable output
    live-inspector --watch 30               # sample every ~200ms for 30s
                                            # and print diffs that change
    live-inspector -v                       # verbose"
    );
}

/// Check AOB-resolved RVAs against each known version's table and
/// return the first one that matches all available symbols.
fn infer_version_from_aobs(hits: &[AobResult]) -> Option<GameVersion> {
    use sekiro_sdk_sys::offsets::BaseAddrs;
    let probe_version = |v: GameVersion| -> bool {
        let addrs = match BaseAddrs::for_version(v) {
            Some(a) => a,
            None => return false,
        };
        let checks: &[(&str, usize)] = &[
            ("quitout", addrs.quitout),
            ("render_world", addrs.render_world),
            ("igt", addrs.igt),
            ("player_position", addrs.player_position),
            ("fps", addrs.fps),
        ];
        let mut any_resolved = false;
        for (name, expected) in checks {
            if let Some(hit) = hits.iter().find(|r| r.name == *name) {
                if let Some(rva_str) = &hit.resolved_rva {
                    let rva = usize::from_str_radix(rva_str.trim_start_matches("0x"), 16)
                        .unwrap_or(0);
                    if rva == *expected {
                        any_resolved = true;
                    } else {
                        return false;
                    }
                }
            }
        }
        any_resolved
    };
    for v in [
        GameVersion::V1_06,
        GameVersion::V1_05,
        GameVersion::V1_03_04,
        GameVersion::V1_02,
    ] {
        if probe_version(v) {
            return Some(v);
        }
    }
    None
}

/// Heuristic ChrIns candidate scan over a copied byte buffer.
/// Mirrors `sekiro-sdk-sys::chrins_discover::discover` but without
/// needing RawPtr::read (we already have the bytes in memory).
fn scan_chrins_candidates(buf: &[u8], base_addr: usize) -> Vec<ChrInsCandidate> {
    let mut out = Vec::new();

    // Player entity ID (10000) — pinpoint exact match.
    for off in (0..buf.len()).step_by(4) {
        if off + 4 > buf.len() {
            break;
        }
        let v = u32::from_le_bytes(buf[off..off + 4].try_into().unwrap());
        if v == 10_000 {
            out.push(ChrInsCandidate {
                field: "entity_id",
                offset: format!("{:#x}", off),
                value: format!("10000"),
                confidence: 0.95,
            });
            break;
        }
    }

    // HP pair: (i32, i32) where hp in [1, 5000], max_hp >= hp, max_hp >= 100.
    // Skip the struct header (< 0x100) — header contains pointers + handles.
    // Collect up to 5 candidates, most-plausible first.
    let mut hp_candidates: Vec<(usize, i32, i32)> = Vec::new();
    for off in (0x100..buf.len()).step_by(4) {
        if off + 8 > buf.len() {
            break;
        }
        let hp = i32::from_le_bytes(buf[off..off + 4].try_into().unwrap());
        let max = i32::from_le_bytes(buf[off + 4..off + 8].try_into().unwrap());
        if (100..=10_000).contains(&hp) && max >= hp && max <= 20_000 {
            hp_candidates.push((off, hp, max));
        }
    }
    for (rank, (off, hp, max)) in hp_candidates.iter().take(5).enumerate() {
        let conf = 0.75 - (rank as f32) * 0.10;
        out.push(ChrInsCandidate {
            field: if rank == 0 { "hp" } else { "hp?" },
            offset: format!("{:#x}", off),
            value: format!("{hp} (max {max})"),
            confidence: conf,
        });
    }

    // Posture pair (f32, f32).  Require offset > 0x200 and max_posture in
    // [50, 2000] — rules out header floats and tiny accumulators.
    let mut posture_candidates: Vec<(usize, f32, f32)> = Vec::new();
    for off in (0x200..buf.len()).step_by(4) {
        if off + 8 > buf.len() {
            break;
        }
        let p = f32::from_le_bytes(buf[off..off + 4].try_into().unwrap());
        let m = f32::from_le_bytes(buf[off + 4..off + 8].try_into().unwrap());
        if p.is_finite()
            && m.is_finite()
            && (0.0..=2000.0).contains(&p)
            && (50.0..=2000.0).contains(&m)
            && m >= p
        {
            posture_candidates.push((off, p, m));
        }
    }
    for (rank, (off, p, m)) in posture_candidates.iter().take(5).enumerate() {
        let conf = 0.55 - (rank as f32) * 0.08;
        out.push(ChrInsCandidate {
            field: if rank == 0 { "posture" } else { "posture?" },
            offset: format!("{:#x}", off),
            value: format!("{p:.1} (max {m:.1})"),
            confidence: conf,
        });
    }

    // Animation ID — u32 in [1, 99_999] — looking for values like 7010
    // (hit), 3000 (idle), etc.  Restrict offset > 0x300 to skip
    // pointer-heavy header.
    let mut anim_candidates: Vec<(usize, u32)> = Vec::new();
    for off in (0x300..buf.len()).step_by(4) {
        if off + 4 > buf.len() {
            break;
        }
        let v = u32::from_le_bytes(buf[off..off + 4].try_into().unwrap());
        if (1..=99_999).contains(&v) {
            anim_candidates.push((off, v));
        }
    }
    for (rank, (off, v)) in anim_candidates.iter().take(3).enumerate() {
        let conf = 0.40 - (rank as f32) * 0.05;
        out.push(ChrInsCandidate {
            field: "anim_id?",
            offset: format!("{:#x}", off),
            value: format!("{v}"),
            confidence: conf,
        });
    }

    // Position at offset 0x80 is a given (that's how we got here);
    // read it back to confirm.
    if buf.len() >= 0x90 {
        let x = f32::from_le_bytes(buf[0x80..0x84].try_into().unwrap());
        let y = f32::from_le_bytes(buf[0x84..0x88].try_into().unwrap());
        let z = f32::from_le_bytes(buf[0x88..0x8C].try_into().unwrap());
        out.push(ChrInsCandidate {
            field: "position[f32;3]",
            offset: "0x80".into(),
            value: format!("({x:.2}, {y:.2}, {z:.2})"),
            confidence: 1.0,
        });
    }

    let _ = base_addr;
    out
}

/// Decode a chain-walk value based on the chain name.
fn print_chain_value(name: &str, hex: &str) {
    let bytes: Vec<u8> = hex
        .split_whitespace()
        .filter_map(|s| u8::from_str_radix(s, 16).ok())
        .collect();
    match name {
        n if n.contains("HP") || n == "player CurrentAnim" => {
            if bytes.len() >= 4 {
                let v = i32::from_le_bytes(bytes[..4].try_into().unwrap());
                print!("{v}");
                return;
            }
        }
        "player Posture" | "player MaxPosture" => {
            if bytes.len() >= 4 {
                let v = i32::from_le_bytes(bytes[..4].try_into().unwrap());
                print!("{v}");
                return;
            }
        }
        "player Pos[xyz]" => {
            if bytes.len() >= 12 {
                let x = f32::from_le_bytes(bytes[0..4].try_into().unwrap());
                let y = f32::from_le_bytes(bytes[4..8].try_into().unwrap());
                let z = f32::from_le_bytes(bytes[8..12].try_into().unwrap());
                print!("({x:.2}, {y:.2}, {z:.2})");
                return;
            }
        }
        "player PlaySpeed" | "fps f32" => {
            if bytes.len() >= 4 {
                let v = f32::from_le_bytes(bytes[..4].try_into().unwrap());
                print!("{v:.3}");
                return;
            }
        }
        "igt_ms u32" => {
            if bytes.len() >= 4 {
                let v = u32::from_le_bytes(bytes[..4].try_into().unwrap());
                let secs = v / 1000;
                print!("{v} ms ({:02}:{:02}:{:02})", secs / 3600, (secs / 60) % 60, secs % 60);
                return;
            }
        }
        "player_position(libsekiro)" => {
            if bytes.len() >= 16 {
                let x = f32::from_le_bytes(bytes[0..4].try_into().unwrap());
                let y = f32::from_le_bytes(bytes[4..8].try_into().unwrap());
                let z = f32::from_le_bytes(bytes[8..12].try_into().unwrap());
                let w = f32::from_le_bytes(bytes[12..16].try_into().unwrap());
                print!("({x:.2}, {y:.2}, {z:.2}, w={w:.2})");
                return;
            }
        }
        _ => {}
    }
    print!("{hex}");
}

fn hexdump(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|b| format!("{:02x}", b))
        .collect::<Vec<_>>()
        .join(" ")
}

fn render_text(r: &Report) {
    println!("\n=== live-inspector report ===");
    println!("pid:            {}", r.pid);
    println!("module:         {}", r.module_name);
    println!("base:           {}", r.module_base);
    println!("size:           {} bytes", r.module_size);
    println!("detected:       {}", r.detected_version);

    println!("\n--- AOB sweep ({}/{} hits) ---",
        r.aob_hits.iter().filter(|a| a.hit).count(),
        r.aob_hits.len()
    );
    for a in &r.aob_hits {
        let status = if a.hit { "[ok]  " } else { "[miss]" };
        print!("{status} {:<24}", a.name);
        if let Some(off) = &a.offset_in_module {
            print!(" @ {}", off);
        }
        if let Some(rva) = &a.resolved_rva {
            print!(" → {}", rva);
        }
        if let Some(m) = &a.mismatch_vs_hardcoded {
            print!(" !! {}", m);
        }
        println!();
    }

    println!("\n--- Pointer-chain walks ---");
    for c in &r.chain_results {
        let status = if c.ok { "[ok]  " } else { "[fail]" };
        println!("{status} {:<28} {}", c.name, c.path);
        if let Some(addr) = &c.final_address {
            print!("       addr: {}", addr);
        }
        if let Some(val) = &c.value_hex {
            print!("  value: ");
            print_chain_value(c.name, val);
            println!();
        } else {
            println!();
        }
    }

    if !r.natives.is_empty() {
        let hits = r.natives.iter().filter(|n| n.address.is_some()).count();
        println!("\n--- Extended symbols + native fns ({}/{} hits) ---",
            hits, r.natives.len());
        for n in &r.natives {
            let status = if n.address.is_some() { "[ok]  " } else { "[miss]" };
            let addr = n.address.as_deref().unwrap_or("—");
            println!("{status} {:<32} {}", n.name, addr);
        }
    }

    if let Some(base) = &r.chrins_base {
        println!("\n--- ChrIns discovery ({} candidates at base {}) ---",
            r.chrins_candidates.len(), base);
        for c in &r.chrins_candidates {
            println!(
                "       {:<18} offset={:<10} value={:<30} conf={:.2}",
                c.field, c.offset, c.value, c.confidence
            );
        }
    }

    for n in &r.notes {
        println!("\nnote: {n}");
    }
}

// ---------------------------------------------------------------------
//  AOB table for the inspector
// ---------------------------------------------------------------------

struct AobEntry {
    name: &'static str,
    pat: AobPattern,
    disp_offset: usize,
    instr_len: usize,
    hardcoded_rva: Option<usize>,
}

fn aob_entries(version: GameVersion) -> Vec<AobEntry> {
    let addrs = sekiro_sdk_sys::offsets::BaseAddrs::for_version(version);
    macro_rules! e {
        ($name:expr, $pat:expr, $disp:expr, $ilen:expr, $sym:ident) => {
            AobEntry {
                name: $name,
                pat: $pat,
                disp_offset: $disp,
                instr_len: $ilen,
                hardcoded_rva: addrs.map(|a| a.$sym),
            }
        };
    }
    vec![
        e!("quitout",         patterns::quitout(),         3, 7, quitout),
        e!("render_world",    patterns::render_world(),    2, 7, render_world),
        e!("debug_render",    patterns::debug_render(),    4, 8, debug_render),
        e!("igt",             patterns::igt(),             3, 7, igt),
        e!("player_position", patterns::player_position(), 3, 8, player_position),
        e!("debug_flags",     patterns::debug_flags(),     2, 7, debug_flags),
        e!("show_cursor",     patterns::show_cursor(),     3, 7, show_cursor),
        e!("no_logo",         patterns::no_logo(),         0, 0, no_logo),
        e!("font_patch",      patterns::font_patch(),      0, 0, font_patch),
        AobEntry {
            name: "activate_debug_menu",
            pat: patterns::activate_debug_menu(),
            disp_offset: 0,
            instr_len: 0,
            hardcoded_rva: None,
        },
        AobEntry {
            name: "menu_draw_hook",
            pat: patterns::menu_draw_hook(),
            disp_offset: 0,
            instr_len: 0,
            hardcoded_rva: None,
        },
    ]
}

// ---------------------------------------------------------------------
//  Process attach / memory read
// ---------------------------------------------------------------------

struct OpenTarget {
    handle: HANDLE,
    pid: u32,
}

struct LoadedModule {
    name: String,
    base: usize,
    size: usize,
}

impl OpenTarget {
    fn open(pid: u32) -> Result<Self> {
        let handle = unsafe {
            OpenProcess(PROCESS_QUERY_INFORMATION | PROCESS_VM_READ, false, pid)
                .context("OpenProcess")?
        };
        Ok(Self { handle, pid })
    }

    fn find_module(&self, name: &str) -> Result<LoadedModule> {
        let snap = unsafe {
            CreateToolhelp32Snapshot(TH32CS_SNAPMODULE | TH32CS_SNAPMODULE32, self.pid)
                .context("CreateToolhelp32Snapshot")?
        };
        let mut entry = MODULEENTRY32::default();
        entry.dwSize = core::mem::size_of::<MODULEENTRY32>() as u32;
        let mut ok = unsafe { Module32First(snap, &mut entry) };
        while ok.is_ok() {
            let name_c = cstr_from_bytes_i8(&entry.szModule);
            if name_c.eq_ignore_ascii_case(name) {
                let m = LoadedModule {
                    name: name_c,
                    base: entry.modBaseAddr as usize,
                    size: entry.modBaseSize as usize,
                };
                unsafe { let _ = CloseHandle(snap); };
                return Ok(m);
            }
            ok = unsafe { Module32Next(snap, &mut entry) };
        }
        unsafe { let _ = CloseHandle(snap); };
        bail!("module {name} not found in pid {}", self.pid)
    }

    /// Read the whole module image into memory for AOB scanning.
    fn read_module(&self, module: &LoadedModule) -> Result<Vec<u8>> {
        let mut buf = vec![0u8; module.size];
        let mut read = 0usize;
        unsafe {
            ReadProcessMemory(
                self.handle,
                module.base as *const c_void,
                buf.as_mut_ptr() as *mut c_void,
                module.size,
                Some(&mut read),
            )
            .context("ReadProcessMemory(module)")?;
        }
        buf.truncate(read);
        Ok(buf)
    }

    /// Walk a pointer chain starting at `sym_addr` (the address of a
    /// symbol that holds a pointer).  Returns (final_addr, value_bytes).
    fn walk_chain(
        &self,
        sym_addr: usize,
        offsets: &[isize],
        read_len: usize,
    ) -> Result<(usize, Vec<u8>)> {
        // Step 1: read the symbol to get the root node.
        let mut root = 0u64;
        self.read_at(sym_addr, bytes_of_mut(&mut root))?;
        let mut p = root as usize;
        if p == 0 {
            bail!("null root at {:#x}", sym_addr);
        }
        for (idx, &off) in offsets.iter().enumerate() {
            if idx + 1 == offsets.len() {
                let final_addr = (p as isize + off) as usize;
                let mut buf = vec![0u8; read_len];
                self.read_at(final_addr, &mut buf)?;
                return Ok((final_addr, buf));
            }
            let mut next = 0u64;
            self.read_at((p as isize + off) as usize, bytes_of_mut(&mut next))?;
            if next == 0 {
                bail!("null intermediate at depth {idx}");
            }
            p = next as usize;
        }
        bail!("unreachable")
    }

    fn read_at(&self, addr: usize, buf: &mut [u8]) -> Result<()> {
        let mut read = 0usize;
        unsafe {
            ReadProcessMemory(
                self.handle,
                addr as *const c_void,
                buf.as_mut_ptr() as *mut c_void,
                buf.len(),
                Some(&mut read),
            )
            .context("ReadProcessMemory")?;
        }
        if read != buf.len() {
            bail!("short read at {:#x}: {}/{}", addr, read, buf.len());
        }
        Ok(())
    }

    fn read_buffer(&self, addr: usize, len: usize) -> Result<Vec<u8>> {
        let mut buf = vec![0u8; len];
        self.read_at(addr, &mut buf)?;
        Ok(buf)
    }
}

impl Drop for OpenTarget {
    fn drop(&mut self) {
        unsafe { let _ = CloseHandle(self.handle); };
    }
}

fn bytes_of_mut<T>(v: &mut T) -> &mut [u8] {
    unsafe {
        core::slice::from_raw_parts_mut(
            v as *mut T as *mut u8,
            core::mem::size_of::<T>(),
        )
    }
}

fn cstr_from_bytes_i8(bytes: &[i8]) -> String {
    let end = bytes.iter().position(|b| *b == 0).unwrap_or(bytes.len());
    let slice = unsafe {
        core::slice::from_raw_parts(bytes.as_ptr() as *const u8, end)
    };
    String::from_utf8_lossy(slice).into_owned()
}

fn cstr_from_bytes_u8(bytes: &[u8]) -> String {
    let end = bytes.iter().position(|b| *b == 0).unwrap_or(bytes.len());
    String::from_utf8_lossy(&bytes[..end]).into_owned()
}

fn find_sekiro_pid() -> Result<u32> {
    let snap = unsafe {
        CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0)
            .context("CreateToolhelp32Snapshot")?
    };
    let mut entry = PROCESSENTRY32::default();
    entry.dwSize = core::mem::size_of::<PROCESSENTRY32>() as u32;
    let mut ok = unsafe { Process32First(snap, &mut entry) };
    while ok.is_ok() {
        let name = cstr_from_bytes_i8(&entry.szExeFile);
        if name.eq_ignore_ascii_case("sekiro.exe") {
            let pid = entry.th32ProcessID;
            unsafe { let _ = CloseHandle(snap); };
            return Ok(pid);
        }
        ok = unsafe { Process32Next(snap, &mut entry) };
    }
    unsafe { let _ = CloseHandle(snap); };
    bail!("sekiro.exe is not running — launch it first")
}

