# sekiro-coop — install & build

## What this is

Two-player seamless co-op DLL for Sekiro: Shadows Die Twice with rollback netcode over a shared-entity band. Loaded via [me3](https://github.com/garyttierney/me3).

## Status

Scaffold + testable layers. See [GAPS.md](GAPS.md) for items that still require reverse-engineering work against a running Sekiro binary before the mod is end-user playable.

## Build prerequisites

- Rust `1.78.0` (pinned via `rust-toolchain.toml`)
- Windows + MSVC toolchain (`rustup target add x86_64-pc-windows-msvc`)
- [MinHook](https://github.com/TsudaKageyu/minhook) built for x64 → produces `MinHook.x64.lib`
- [me3](https://github.com/garyttierney/me3) for loading the DLL
- [Yabber](https://github.com/JKAnderson/Yabber) for `.dcx` decompression (only needed for patching EMEVD files)

Build MinHook:

```powershell
git clone https://github.com/TsudaKageyu/minhook.git
cd minhook\build\VC17
# Open MinHook.sln in Visual Studio and build x64/Release.
# Output: bin\VC17\x64\Release\MinHook.x64.lib
```

Point the build script at it:

```powershell
$env:MINHOOK_LIB_DIR = "C:\path\to\minhook\bin\VC17\x64\Release"
```

## Build the DLL

```powershell
cargo build -p sekiro-coop-dll --release
# Output: target\release\sekiro_coop.dll
```

With the ImGui overlay (requires hudhook's transitive DX11 deps):

```powershell
cargo build -p sekiro-coop-dll --release --features overlay
```

## Build the offline tools

```powershell
cargo build --release --bin sekiro-coop-emevd --bin aob-scanner --bin determinism-probe
```

- `sekiro-coop-emevd` — patches `common.emevd` for two-player (see below)
- `aob-scanner` — runs every documented AOB against a `sekiro.exe`; reports hits
- `determinism-probe` — diffs two snapshot files (used by the §12.3 probe)

## Patch `common.emevd`

1. Use Yabber to decompress `sekiro/event/common.emevd.dcx` → `common.emevd`.
2. Run:

   ```powershell
   cargo run --release --bin sekiro-coop-emevd -- `
     common.emevd out\common.emevd `
     --promote-to host `
     --rng-range 30000 30063 `
     --boss-id 5080 --boss-id 5090 --boss-id 5100
   ```

3. Re-pack `out\common.emevd` with Yabber → `common.emevd.dcx`.
4. Place under me3's `replacement` directory for the profile.

## Install into me3

1. Copy `sekiro_coop.dll` into me3's natives dir for your profile.
2. Copy `dist/me3-profile.toml` into `<me3>/profiles/sekiro-coop.toml`. Adjust paths.
3. Drop a Cielos CE table XML next to the DLL as `sekiro-coop-chrins.xml` so the mod can self-populate `ChrInsLayout` on startup (see [GAPS.md](GAPS.md)).
4. Launch via me3, selecting the `sekiro-coop` profile.

## Connection setup

Until the matchmaking UI lands, connections are configured via environment variables. Both players must use the same `SEKIRO_COOP_LOG` level if you want diffable logs.

Host:

```powershell
$env:SEKIRO_COOP_PEER = "host"
$env:SEKIRO_COOP_BIND = "0.0.0.0:28000"
$env:SEKIRO_COOP_PEER_ADDR = "<client-ip>:28000"
```

Client:

```powershell
$env:SEKIRO_COOP_PEER = "client"
$env:SEKIRO_COOP_BIND = "0.0.0.0:28000"
$env:SEKIRO_COOP_PEER_ADDR = "<host-ip>:28000"
```

Then launch Sekiro via me3 on both.

## Development: running the test suite

```powershell
cargo test --workspace --exclude sekiro-coop-dll
```

The DLL crate's tests require `MinHook.x64.lib` on the link path (same as the release build). Non-DLL tests cover ~37 assertions across reliability, delta, desync, EMEVD format, CE-table parsing, rollback stepper, and proximity handoff.

## Troubleshooting

| Symptom | Cause / Fix |
|---|---|
| `could not find native static library 'MinHook.x64'` | Set `MINHOOK_LIB_DIR` before `cargo build`. |
| DLL loads but overlay never appears | Built without `--features overlay`, or DX11 swapchain not created yet. |
| Session stays in "handshake timed out" | Firewall blocking UDP on the configured port, or peer env mismatch. |
| Boss doesn't take damage on client | Likely a damage-hook AOB gap — see [GAPS.md](GAPS.md) §P0 gap #3. |
| "layout has 17 unresolved fields" in logs | `sekiro-coop-chrins.xml` missing; rollback stepper will no-op for safety. |
