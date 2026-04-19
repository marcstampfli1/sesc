# sekiro-coop

Two-player seamless co-op for *Sekiro: Shadows Die Twice* with rollback netcode over a shared-entity band. Implemented as a DLL loaded via [me3](https://github.com/garyttierney/me3).

## Highlights

- Rust workspace, 10 crates, 5 architecture layers (hook substrate → typed entity API → bridge hooks → rollback band → authority/sync → session/transport)
- Deterministic seeded RNG keyed on `(match_seed, frame, call_site_id)`
- Sliding-window reliable UDP with sequence + ack-bitmap + retransmit queue
- State-delta compression with spawn/despawn tracking
- 60-frame hash-exchange desync detector with three-strike session kill
- Real EMEVD binary format reader/writer (not a byte-blob hack)
- Cielos CE table XML ingestion so `ChrInsLayout` self-populates
- Proximity-based authority handoff with hysteresis + inflight suppression
- `ChrInsStepper` writes snapshot state back through a validated layout
- 60 Hz fallback tick thread plus hudhook ImGui overlay (feature-gated)

## Layout

```
sekiro-coop/
├── crates/
│   ├── sekiro-sdk-sys/        # AOBs, pointers, memory, version, Cielos XML
│   ├── sekiro-sdk-core/       # Typed entities, AtkParam, SpEffect, hook registry
│   ├── sekiro-sdk-bridge/     # Combat / AI / world hooks + dispatcher
│   ├── sekiro-coop-rollback/  # Shared band, snapshots, deltas, stepper, re-sim
│   ├── sekiro-coop-authority/ # Authority table, handoff, seeded RNG, driver
│   ├── sekiro-coop-net/       # Transport, session, wire, reliability, desync
│   ├── sekiro-coop-dll/       # DLL entry, ticker, overlay
│   └── sekiro-coop-emevd/     # EMEVD structured reader/writer + CLI
├── tools/
│   ├── aob-scanner/           # Runs every documented AOB against a sekiro.exe
│   └── determinism-probe/     # Twin-instance snapshot diff
├── dist/
│   └── me3-profile.toml
└── docs/
    ├── INSTALL.md
    └── GAPS.md
```

## Build

See [docs/INSTALL.md](docs/INSTALL.md).

TL;DR:

```powershell
$env:MINHOOK_LIB_DIR = "C:\path\to\minhook\bin\VC17\x64\Release"
cargo build --release
```

## Test

```powershell
cargo test --workspace --exclude sekiro-coop-dll
```

Current: **37 passing tests** covering reliability, delta compression, desync detection, EMEVD format round-trip, Cielos XML parsing, rollback write-back, proximity handoff, seeded RNG determinism, and wire-protocol quaternion roundtrips.

## Remaining gaps

See [docs/GAPS.md](docs/GAPS.md).

The work that can't be done from documentation alone: the damage-application function AOB, ChrIns field offsets (the XML parser is ready — the *data* needs a Cielos export), the 21 network-EMEVD native AOBs, and Steam P2P integration.

## License

AGPL-3.0-or-later.
