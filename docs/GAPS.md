# Remaining gaps

Every item below needs live-binary reverse-engineering work that can't be performed from documentation alone. Each is traceable back to `SEKIRO_SEAMLESS_ROLLBACK_SPEC.md` §11.

## P0 — blocks end-user play

| # | Item | Current state | Next step |
|---|---|---|---|
| 1 | `ChrIns` field offsets | `ChrInsLayout::unresolved()` at startup; CE-table parser ready to ingest a `sekiro-coop-chrins.xml` | Obtain Cielos' CE table from the Fearless Revolution thread (SPEC URL), export the ChrIns group as XML, drop next to DLL |
| 2 | `WorldChrMan` AOB + walker | Walker written; AOB + offset-into-struct unknown | Scan near known player-position chain (`player_position → +0x48 → +0x28 → +0x80`) for the ancestor table; validate by iterating until player `ChrIns` is yielded |
| 3 | Damage-application function AOB | `CombatBridge::damage_fn` is `None` | Locate via string references near `"AtkParam"` / deflect-related text; confirm by hooking + watching Gyoubu parry counts |
| 4 | `SetSpeffect` native AOB | Phase-A hook not installed | Hook, log every call, confirm every in-game SpEffect application appears (SPEC Phase A exit criterion) |
| 5 | Live param-table memory layout | `ParamIndex::default()` — all tables `None` | Reuse Yapped's offset or find the global param-repository pointer; validate by reading a known AtkParam row |

## P1 — needed for core functionality

| # | Item | Current state | Next step |
|---|---|---|---|
| 6 | Death event wiring | Not wired | Use `IfCharacterDeadAlive` + per-character death-flag extraction from per-boss EMEVD files |
| 7 | Boss kill event IDs | Not catalogued | Extract from per-boss `.emevd.dcx` files via `sekiro-coop-emevd` once the reader tolerates real files |
| 8 | Deflect-animation ID tables per character | Table in `DeflectAnimTable`; empty | Observation during real play; `CombatClassifier::learn_deflect` grows the set each time we observe `AtkStam > 0` |
| 9 | TAE event dispatch hook | Not hooked | Hook per-block dispatchers (`InvokeAttackBehavior[1]`, `AddSpEffect[67]`); needs TAE dispatch AOB |
| 10 | Native tick function hook | Fallback 60 Hz thread covers for now | Identify via frame counter at `fps + 0x2BC`; hook its caller |

## P2 — needed for netcode

| # | Item | Current state | Next step |
|---|---|---|---|
| 11 | 21 network-EMEVD instruction AOBs | None resolved | RE via string references to the function names, or Sekiro-Online source transfer (LukeYui) |
| 12 | Connected-event-flag range | `ConnectedFlagRange::None`; `should_sync` returns false | Runtime probe: set a flag, observe whether it syncs, binary-search the boundary |
| 13 | `MultiplayerState` enum values | DS3 precedent baked into `MultiplayerState::{SOLO, HOST, CLIENT, INVADER, SUMMONED}` | Validate against native function argument handling |
| 14 | `AuthorityLevel` enum values | Defaulted to 0=Host / 1=Client in EMEVD emitter | Validate once `SetNetworkUpdateAuthority` AOB lands |
| 15 | Steam P2P transport | Stub behind `steam` feature | Swap in `steamworks-rs`; Seamless Co-op pattern |

## Non-blocking polish

- DCX compression in the EMEVD tool (currently requires external Yabber pre-pass)
- Matchmaking UI (currently driven by env vars)
- Custom HKX authoring (engine-limitation gap)
- Full event-layer support in the EMEVD writer (DS3 holdover; unused by Sekiro `common.emevd` in practice)

## What is already working

- Deterministic PCG RNG keyed on `(match_seed, frame, call_site)`
- Wire protocol: magic + versioned header + serde-backed body
- Reliable UDP: sliding-window ACKs, retransmit queue, ack-bitmap stamping
- State-delta compression with spawn/despawn tracking
- Desync detector (60-frame hash exchange, three-strike kill)
- EMEVD structured binary format round-trip (event/instruction/param/arg tables)
- SOLO-branch promotion through the real format, not byte-blob scanning
- Cielos CE table XML parser → `ChrInsLayout`
- Proximity handoff driver with hysteresis + inflight-suppression
- `ChrInsStepper` safe write-back path, gated on validated layout
- 60 Hz fallback ticker + hudhook overlay scaffolding
- Offline tools: `aob-scanner`, `sekiro-coop-emevd`, `determinism-probe`
- 37 passing unit tests across reliability, delta, desync, format, CE-table, rollback, handoff, and RNG
