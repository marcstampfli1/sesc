# sekiro-coop — agent handoff

Last updated: 2026-04-19. Session transfer: one user, moving machines. The next
agent reads this first, then the cited source files, then starts work.

---

## 1. What this project is

A from-scratch co-op mod for **Sekiro: Shadows Die Twice v1.06** (70066176-byte
build, module base `0x140000000`), written in Rust as a DLL injected by
[**me3 v0.11.0**](https://github.com/garyttierney/me3). End goal: two real
Sekiro instances on separate machines playing through the game together, seeing
each other's character, synced world state, synced boss HP.

- User owns Sekiro at `Z:\SteamLibrary\steamapps\common\Sekiro\sekiro.exe`.
- User platform: Windows 11 Pro, bash shell (Unix shell syntax in commands).
- Primary working directory: `Z:\dev\sesc\sekiro-coop`.
- MSVC Rust toolchain, stable. `cargo build --release -p sekiro-coop-dll` is
  the main build.

## 2. What works end-to-end

All validated live against a running Sekiro instance + the `peer-simulator` test
harness (acts as the second Sekiro). No crashes on the happy path.

| Subsystem                | State           | Notes                                                                                   |
|--------------------------|-----------------|-----------------------------------------------------------------------------------------|
| DLL injection via me3    | ✅ works         | `--disable-arxan true --online false -p sekiro-coop.me3`                                |
| 6 native hooks installed | ✅ works         | SetFlag, ApplyEffect, DeleteEffect, GiveItem, AddExperience, WarpBonfire                |
| UDP session + handshake  | ✅ works         | Reliability layer w/ sequence + ACK bitmap, retries                                     |
| PlayerSnapshot sync      | ✅ bidirectional | 60Hz unreliable; full state each tick                                                   |
| BridgeEvents sync        | ✅ bidirectional | Edge-triggered (only on value change), reliable                                         |
| Remote BridgeEvent apply | ✅ gated         | Trampoline-direct calls, no feedback loop; `SEKIRO_COOP_APPLY_REMOTE=1` env-var gate   |
| Enemy registry           | ✅ 42 entities   | `HashMap<handle, ChrIns*>` fed from hook args (ApplyEffect's `entity`)                 |
| EnemyStates broadcast    | ✅ host-only     | 5Hz, delta-filtered (skip unchanged entities via 64-bit digest)                         |
| Handle-based lookup      | ✅ 42/42 matched | Remote's handle → local ChrIns*, tested via peer-sim echo                               |
| HP write via state chain | ✅ works         | `ChrIns+0x1ff8→+0x18→+0x130`; game accepts, decrement-only policy                       |
| Authority model          | ✅ wired         | Host broadcasts enemy state + ignores inbound enemy claims; Client broadcasts only own  |

## 3. What is blocked and why

The single remaining gap for a visible co-op experience is rendering the remote
player in the local game world. All technical progress on that gap has hit the
same wall:

**No AOBs for the native functions behind the EMEVD opcodes we'd need.**

We have an EMEVD instruction catalog at
`crates/sekiro-coop-emevd/src/catalog.rs:195,206,221` listing `RequestAnimationPlayback`
(EVENT 1), `ForceAnimationPlayback` (EVENT 18), `CharacterWarpRequest` (CHARACTER
3), `SetCharacterAiState` (CHARACTER 1), `SetCharacterTeamType` (CHARACTER 2),
`ChangeCharacterEnableState` (CHARACTER 5). These are the functions we need
to drive a puppet NPC. They are **EMEVD opcodes** — dispatched by Sekiro's EMEVD
VM to native C++ functions in `sekiro.exe`. We have the opcodes, not the native
addresses.

Writing to `ChrIns` position directly (the state-module chain) is accepted by
the memory but ignored by the render path. Confirmed via experiment (see
`puppet experiment` log lines from the session): `actual = target`, but the
targeted NPC is not visibly teleported. Physics/render reads from somewhere
else. Without the native warp function, we can't move NPCs visibly.

## 4. The planned unblock: UXM + Yabber pipeline

The user greenlit downloading UXM and Yabber and driving the full EMEVD-patch
pipeline. **This is the next work to do. It has not been started.** Detailed
step-by-step below.

### Why this works

We don't need the native function AOBs if we can patch `common.emevd.dcx`
instead. The game's own EMEVD VM dispatches opcodes to native functions
already. If we add a custom event (e.g. event 99100) that calls
`RequestAnimationPlayback(target_id, anim_id, ...)`, and trigger it by setting
an event flag from our DLL, the VM runs the native function for us.

The mod setup already has the plumbing:
- `crates/sekiro-coop-emevd` — full EMEVD binary reader/writer. Injects events
  99000-99003 currently; easy to add more.
- me3's asset-override hook serves files from
  `tools/me3/sekiro-coop-mods/` in place of the game's archived versions.
- Our DLL already hooks SetFlag — setting a flag is a one-liner.

### Blockers to resolve, in order

**Blocker 1: No baseline `common.emevd.dcx`.** The game file lives inside
`Z:\SteamLibrary\steamapps\common\Sekiro\Data0.bdt` (or similar), which is
RSA-encrypted per version. We can't modify what we don't have. We cannot build
one from scratch — it'd strip every game event (idols break, cutscenes don't
fire, bosses don't spawn).

**Blocker 2: No DCX codec.** Our EMEVD library writes raw `.emevd`, not DCX.
`crates/sekiro-coop-emevd/src/format.rs:21` explicitly disclaims DCX: *"DCX
compression is out-of-scope: run Yabber first."* The game loads `.emevd.dcx`.

**Blocker 3: No extraction tool.** Sekiro's archives are decrypted by UXM;
individual files are then DCX-decompressed by Yabber.

### Step-by-step pipeline

Each step should be done on the target machine. Report each outcome before
proceeding to the next.

**Step 0 — Verify environment**

```bash
ls /z/SteamLibrary/steamapps/common/Sekiro/      # game install present
ls /z/dev/sesc/sekiro-coop/tools/me3/            # me3 folder
cat /z/dev/sesc/sekiro-coop/tools/me3/sekiro-coop.me3  # profile
```

Expect `Data0.bdt Data1.bdt ... sekiro.exe`. The `sekiro-coop.me3` profile
should list our DLL as a native + `sekiro-coop-mods` as a package.

**Step 1 — Install UXM (Sekiro Unpacker)**

UXM decrypts and extracts Sekiro's game archives. Download from the maintained
fork:

- Source: https://github.com/JKAnderson/UXM (original, may not have Sekiro keys)
- Sekiro-specific: https://github.com/Nordgaren/UXM-Selective-Unpacker (recommended)

Target install path: `Z:\dev\sesc\sekiro-coop\tools\uxm\`

UXM GUI: point it at `sekiro.exe`, click Unpack. Writes unpacked files *next to
sekiro.exe* — so `common.emevd.dcx` ends up at
`Z:\SteamLibrary\steamapps\common\Sekiro\event\common.emevd.dcx` or similar.

**⚠️ UXM modifies the game folder.** Keep a backup or make sure the user is
fine with their install becoming unpacked. Unpacked installs are supported by
Sekiro and preferred by the modding community.

**Step 2 — Locate `common.emevd.dcx`**

After UXM unpack:

```bash
find /z/SteamLibrary/steamapps/common/Sekiro -iname "common.emevd*"
```

Expected path: `.../Sekiro/event/common.emevd.dcx` (possibly other `mXX_XX_XX_XX.emevd.dcx`
files for individual maps). Copy to a working dir:

```bash
mkdir -p /z/dev/sesc/sekiro-coop/build/emevd
cp <path>/common.emevd.dcx /z/dev/sesc/sekiro-coop/build/emevd/common.emevd.dcx.orig
```

**Step 3 — Install Yabber (DCX codec)**

- Source: https://github.com/JKAnderson/Yabber (original)
- Forks with Sekiro support exist — search "Yabber Sekiro".

Target install: `Z:\dev\sesc\sekiro-coop\tools\yabber\`

Drag-and-drop UX: dropping `.dcx` on Yabber.exe decompresses; dropping `.emevd`
on it recompresses. Command-line also works.

**Step 4 — Decompress common.emevd.dcx → common.emevd**

```bash
cd /z/dev/sesc/sekiro-coop/build/emevd
cp common.emevd.dcx.orig common.emevd.dcx
/z/dev/sesc/sekiro-coop/tools/yabber/Yabber.exe common.emevd.dcx
# produces common.emevd
```

**Step 5 — Patch with our EMEVD tool**

The `sekiro-coop-emevd` CLI already injects events 99000-99003 via
`build_custom_events`. To add event 99100 for animation testing, modify
`crates/sekiro-coop-emevd/src/gen.rs:build_custom_events` to append:

```rust
// Event 99100 — drive a target NPC's animation from a flag trigger.
// When flag 99100 is set, play animation 790020 (walk) on entity 10000 (Hero) first
// as a visible smoke test. After that works, parameterize target + anim.
let mut body = Vec::new();
body.push(InstructionBuilder::end_if_event_flag(
    EventEndType::End, false, /* standard flag type */ 0, 99_100,
));
body.push(Instruction::new(
    class::EVENT, 1, // RequestAnimationPlayback
    vec![
        Arg::I32(10_000),  // target: player for smoke test
        Arg::I32(790_020), // walk anim
        Arg::U8(0), Arg::U8(0), Arg::U8(0), Arg::f32(0.0),
    ],
));
body.push(InstructionBuilder::set_event_flag(99_100, false)); // clear trigger
body.push(InstructionBuilder::end_unconditionally(EventEndType::End));
prog.add(Event {
    id: 99_100,
    restart: RestartKind::Restart, // keep listening after fire
    body,
});
```

Then run the CLI:

```bash
cd /z/dev/sesc/sekiro-coop
cargo run --release -p sekiro-coop-emevd -- \
    build/emevd/common.emevd \
    build/emevd/common.emevd.patched
```

Expect `patch complete promoted=... injected=4` (or 5 if you added 99100).

**Step 6 — Recompress → common.emevd.dcx**

```bash
mv build/emevd/common.emevd.patched build/emevd/common.emevd  # Yabber expects matching name
/z/dev/sesc/sekiro-coop/tools/yabber/Yabber.exe build/emevd/common.emevd
# produces common.emevd.dcx
```

**Step 7 — Stage into me3 package**

me3 serves files from `tools/me3/sekiro-coop-mods/` under paths that mirror
game archive structure. Target path:

```
tools/me3/sekiro-coop-mods/event/common.emevd.dcx
```

```bash
mkdir -p /z/dev/sesc/sekiro-coop/tools/me3/sekiro-coop-mods/event
cp build/emevd/common.emevd.dcx \
   /z/dev/sesc/sekiro-coop/tools/me3/sekiro-coop-mods/event/common.emevd.dcx
```

**Step 8 — Trigger from DLL**

Add a debug path: every N seconds, or on a keybind, our DLL calls the existing
SetFlag trampoline with `(EVENT_FLAG_MAN_PTR, 99100, 1, 0)`. This writes the
trigger flag; the game's EMEVD VM notices on next tick and runs our event.

Reference: `hooks::apply_remote_event` in `crates/sekiro-coop-dll/src/hooks.rs`
already does trampoline-direct SetFlag calls — copy that pattern.

Gate behind `SEKIRO_COOP_EMEVD_TEST=1` env var.

**Step 9 — Observe**

Launch via `launch.ps1`. Navigate into save. Enable the trigger (env var set).
Expected: the Hero plays a brief walk animation when flag 99100 is set,
independent of player input. If you see this, **native `RequestAnimationPlayback`
is firing via the EMEVD VM — the primitive works.**

**Step 10 — Parameterize + target an NPC**

Replace hardcoded `(10_000, 790_020, ...)` with a parameterized event
(`InitializeEvent(slot, id, params)` takes a u32 param pack). Wire the DLL to
resolve target entity ID + anim ID at runtime and call initialize-event via
another flag. This enables driving any ChrIns by handle.

**Step 11 — Teleport**

Repeat for `CharacterWarpRequest(target, category, warp_point_flag, unk)`. Note
that this warps to a **pre-defined warp point** (identified by flag ID), not
arbitrary XYZ. Map out known warp points. Options:

- a) For the boss-area MVP, pre-define warp points near each idol and warp the
  puppet to whichever is closest to the remote player.
- b) Find `WarpPlayer`'s native target-setting path and call that — harder RE.
- c) Combine with raw position write (state-chain) — may now stick if
  `CharacterWarpRequest` also updates the physics mirror.

## 5. Risks and fallbacks

**UXM may not work on v1.06.** UXM-Selective-Unpacker is the most recent fork;
verify it has keys for build `0x5fa3e066` (Sekiro 1.06 exe stamp, visible in
our crash event logs). If it fails, search NexusMods for a pre-unpacked Sekiro
template; or pull from a public Sekiro mod repo.

**me3 asset override might not catch `event/common.emevd.dcx`.** me3 hooks
`device_manager`, `mount_ebl`, `ebl` — confirmed in the startup log. Verify by
placing any dummy file at `sekiro-coop-mods/event/common.emevd.dcx` and
checking the game still boots. If it doesn't serve, try placing under
`script/event/` instead; Sekiro's archive path may differ.

**Event parameters may be fragile.** Our EMEVD writer hasn't been tested
against a real game. Smoke-test with event 99100 using hardcoded Hero entity
ID before attempting parameterized events.

**`CharacterWarpRequest` limits.** It warps to predefined points, not XYZ. For
a smooth follow-cam puppet, we may still need the native warp function.

**Sekiro wasn't built for multiplayer.** There's no "phantom player" slot like
DS3/ER. All puppets must be NPC hijacks (disable AI, relocate, animate).

## 6. Source tree guide

```
Z:\dev\sesc\sekiro-coop
├── Cargo.toml                       # workspace root
├── launch.ps1                       # launcher: preflight, build, stage, me3 run
├── run-peer.cmd                     # peer-simulator launcher (avoids PS `&` issues)
├── kill-sekiro.ps1                  # clean shutdown helper
├── docs/
│   ├── HANDOFF.md                   # ← this file
│   ├── GAPS.md                      # original SPEC gap inventory
│   └── INSTALL.md
├── crates/
│   ├── sekiro-sdk-sys/              # layer 1: hooks, memory, AOBs, offsets
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── offsets.rs           # per-version BaseAddrs (V1_02..V1_06)
│   │       ├── aob.rs               # AOB patterns (13 native fns + symbols)
│   │       ├── memory.rs            # RawPtr, PtrChain, Module
│   │       ├── natives.rs           # AOB resolution → Natives table
│   │       ├── live.rs              # ChrIns state readers: player_hp, chrins_snapshot,
│   │       │                        # chrins_write_hp, chrins_write_position,
│   │       │                        # chrins_write_animation_id
│   │       ├── chrins.rs            # ChrInsLayout (unresolved for most fields — we use
│   │       │                        # state-module chains instead)
│   │       ├── chrins_discover.rs   # heuristic scanner (unused)
│   │       ├── worldchrman.rs       # array-style iterator scaffold
│   │       ├── worldchrman_scan.rs  # blind pointer sweep — env-gated, dangerous
│   │       ├── paramrepo.rs         # SoloParamRepository walker; atk/speffect param lookups
│   │       └── params.rs, ce_table.rs, version.rs
│   ├── sekiro-sdk-core/             # layer 2: enums, characters, items, atkparam
│   │   └── src/
│   │       ├── enums.rs             # TeamType (Host=19, Coop=20, Hostile=21, Enemy=6, ...)
│   │       │                        # MultiplayerState (Host=0, Client=1, ...)
│   │       │                        # AuthorityLevel, ChrUpdateFreq, ClientType
│   │       ├── characters.rs        # char_id → name_of() lookup
│   │       ├── items.rs             # 169 weapons, 4 protectors, 290 goods, baked from CT XML
│   │       ├── hook.rs              # MinHook x64 shim via sekiro-minhook vendored lib
│   │       ├── debug_patch.rs, tae.rs
│   ├── sekiro-sdk-bridge/           # layer 3: BridgeEvent dispatcher
│   │   └── src/
│   │       ├── events.rs            # BridgeEvent enum, BridgeDispatcher
│   │       ├── world.rs             # EventFlagEvent, WorldBridge
│   │       ├── combat.rs, ai.rs
│   ├── sekiro-coop-net/             # UDP reliability + wire format
│   │   └── src/
│   │       ├── session.rs           # handshake, Grace, LinkState
│   │       ├── transport.rs         # UdpTransport
│   │       ├── wire.rs              # PacketBody (PlayerSnapshot, EnemyStates,
│   │       │                        # BridgeEvents, Handshake, StateDelta, Input, ...)
│   │       ├── lobby.rs, reliability.rs
│   ├── sekiro-coop-authority/       # authority table + match seed + RNG
│   │   └── src/  (driver.rs, table.rs, rng.rs)
│   ├── sekiro-coop-rollback/        # snapshot ring, predictor, resim (unwired)
│   ├── sekiro-coop-emevd/           # EMEVD compiler (for the UXM+Yabber pipeline)
│   │   └── src/
│   │       ├── catalog.rs           # 386 EMEVD opcodes with arg specs
│   │       ├── gen.rs               # InstructionBuilder + build_custom_events
│   │       ├── format.rs            # binary reader/writer (no DCX — Yabber wraps)
│   │       ├── patch.rs             # load, promote_solo_branches, inject_program, save
│   │       ├── main.rs              # CLI: `sekiro-coop-emevd in.emevd out.emevd`
│   └── sekiro-coop-dll/             # the injected mod
│       └── src/
│           ├── lib.rs               # on_attach, on_frame, apply_incoming,
│           │                        # chrins_registry, last_sent_hash,
│           │                        # authority gating, delta filter
│           ├── hooks.rs             # 6 detours: setflag, apply_effect, delete_effect,
│           │                        # give_item, add_experience, warp_bonfire;
│           │                        # apply_remote_event (trampoline-direct)
│           ├── tick.rs              # 60Hz fallback ticker
│           └── overlay.rs           # hudhook scaffold (disabled by default)
├── tools/
│   ├── me3/                         # downloaded me3 v0.11.0
│   │   ├── bin/me3.exe, me3-launcher.exe, me3_mod_host.dll
│   │   ├── sekiro-coop.me3          # profile: natives + package
│   │   └── sekiro-coop-mods/        # staging dir for DLL + (future) emevd override
│   ├── peer-simulator/              # CLI: stands in as second Sekiro over UDP
│   ├── live-inspector/              # offline pointer-chain walker
│   ├── aob-scanner/                 # offline AOB validation
│   └── determinism-probe/
├── vendor/
│   └── lib/MinHook.x64.lib          # vendored static lib for hook installation
└── target/release/
    ├── sekiro_coop.dll
    ├── peer-simulator.exe
    └── ...
```

## 7. Launch sequence

```bash
# Build the DLL
cargo build --release -p sekiro-coop-dll

# Clean restart
taskkill //F //IM sekiro.exe 2>/dev/null
taskkill //F //IM me3.exe 2>/dev/null
taskkill //F //IM me3-launcher.exe 2>/dev/null
taskkill //F //IM peer-simulator.exe 2>/dev/null
rm -f /c/Users/ur2ba/AppData/Local/sekiro-coop/sekiro-coop.log

# Launch Sekiro (runs foreground). launch.ps1 handles preflight: MinHook lib,
# me3, Sekiro install, Steam running, DLL build, DLL stage, profile.
powershell -ExecutionPolicy Bypass -File /z/dev/sesc/sekiro-coop/launch.ps1

# In another terminal, optionally:
/z/dev/sesc/sekiro-coop/target/release/peer-simulator.exe \
    --bind 0.0.0.0:28001 --peer 127.0.0.1:28000 --as client
# Flags: --fake-events (send synthetic BridgeEvents every 5s),
#        --rate 60 (tick rate)
```

User then navigates into a save manually. **Do not auto-key the game window —
user pushed back on this hard. AutoEnter is opt-in via `-AutoEnter` flag but
should not default on.** Wait for the user to say `im in` before checking logs.

## 8. Environment variables

All set by `launch.ps1:Set-SessionEnv`. Only the first three are essential.

| Var                            | Purpose                                                      |
|--------------------------------|--------------------------------------------------------------|
| `SEKIRO_COOP_PEER`             | `host` or `client`, drives authority behaviour               |
| `SEKIRO_COOP_BIND`             | UDP bind e.g. `0.0.0.0:28000`                                |
| `SEKIRO_COOP_PEER_ADDR`        | peer endpoint e.g. `127.0.0.1:28001`                         |
| `SEKIRO_COOP_LOG`              | tracing filter, default `info,sekiro_coop=debug`             |
| `SEKIRO_COOP_APPLY_REMOTE`     | enables HP-write apply on Client side                        |
| `SEKIRO_COOP_SCAN_WORLDCHRMAN` | enables blind WorldChrMan sweep; **crash-prone, keep off**   |
| `SEKIRO_COOP_PUPPET_EXPERIMENT`| old raw-pos-write experiment; **proved ineffective**         |

## 9. Known observables during healthy play

With peer-simulator running as client, you should see within ~10s of being
in-game:

```
session established seed=MatchSeed(...)
chrins registry count=42 teams={6: 31, 0: 8, 26: 2, 1: 1} sample=[...]
sent EnemyStates (delta) frame=N count=M total_live=42 skipped=(42-M)
```

With peer-simulator echoing back (default behavior of the echo path):

```
remote enemy states frame=N count=42 matched=42 unmatched_count=0
    divergences=["h268468311:local=... remote=... delta=-1", ...]
```

`matched=42 unmatched_count=0` is the key assertion — handle resolution works.
`skipped=41` is common on idle frames — the delta filter is skipping unchanged
entities.

## 10. User preferences (don't relearn these)

These are in `C:\Users\ur2ba\.claude\projects\Z--dev-sesc\memory\` too; repeated
here so the handoff is self-contained.

- **Don't auto-key the game window.** No `SendKeys`, no key-spam loops.
- **Don't restart Sekiro unnecessarily.** Each restart is a ~30s setup tax +
  loading screen + manual save navigation. Batch changes before testing.
- **Trust the research tables.** Don't re-verify offsets that are already
  working. If a chain reads correct HP, don't guess it wrong.
- **Ask "can I test now?" in one short sentence** when you need a restart —
  don't dump a plan or options.
- **Be honest when blocked.** The user called this out multiple times. If
  something needs RE work we don't have data for, say so rather than flailing.
- User doesn't want me to spend cycles on things I can't validate — if we
  can't test it, say so and stop.

## 11. Crucial gotchas

- **bincode enum tag stability.** Adding a variant mid-`PacketBody` enum
  breaks wire compat with any pre-compiled peer. **Rebuild `peer-simulator`
  after any `wire.rs` change.** Symptom: peer and DLL both stuck on
  `handshake timed out`.
- **PowerShell em-dash.** PowerShell parsing chokes on `—` in scripts — use
  ASCII `-`. Bit us multiple times.
- **`$Pid` is reserved in PowerShell.** Use `$ProcId` in custom scripts.
- **Sekiro APPCRASH before menu.** If the scanner runs too early, it sweeps
  uninitialised pointer fields in WorldChrMan and AVs. Gate every cross-ChrIns
  read on `IGT advancing for N consecutive frames` (already done in
  `on_frame`). If you add new memory-sweeping code, use the same gate.
- **Log location.** `C:\Users\ur2ba\AppData\Local\sekiro-coop\sekiro-coop.log`.
  Tail with `tail -30 /c/Users/ur2ba/AppData/Local/sekiro-coop/sekiro-coop.log`.
  me3's own log at `C:\Users\ur2ba\AppData\Local\garyttierney\me3\data\logs\sekiro-coop\...`.
- **Windows Event Log** for APPCRASH diagnostics:
  `powershell -Command "Get-EventLog -LogName Application -Source 'Application Error' -Newest 1 | Format-List"`
  Gives `Fehleroffset` = faulting-DLL RVA; cross-reference to `target/release/sekiro_coop.dll`
  disassembly.
- **me3 run-in-background quirk.** `launch.ps1` default is foreground (waits
  for me3 to exit). me3 doesn't exit as long as Sekiro runs, so the launcher
  hangs. Run via Bash's `run_in_background` tool arg to free the shell.

## 12. Decision log (short)

- **Registry via ApplyEffect hook, not WorldChrMan scanning.** Scanning
  repeatedly crashed on stale pointers. Hook-supplied `entity` arg is
  known-good by construction. 42 entities observable; sufficient.
- **State-module chain (`+0x1ff8 → +0x18 → +0x130`) for HP.** Validated
  against live HP reads. Same chain used for all ChrIns, not just Hero.
- **Authority: host owns enemies.** Matches the conventional FromSoft co-op
  model. Host broadcasts + ignores enemy-state claims; client mirrors +
  applies.
- **Delta filter for EnemyStates.** ~97% skip rate on idle frames.
- **Edge-triggered flag emission.** Flags 11_105_730 and similar were being
  re-asserted every frame. Skip unless value changed.
- **Peer-simulator as second Sekiro stand-in.** Validates wire paths without
  two real installs. Echoes EnemyStates with `HP-1` to exercise the apply
  path.
- **Puppet experiment via raw-pos write: FAILED.** Writes succeed (address
  holds written value) but render ignores it. Need native warp, not mirror
  writes.
- **EMEVD patching is the unblock.** Current task. See §4.

## 13. Open tasks (post-UXM+Yabber pipeline)

- (P0) Remote-player visual puppet — depends on §4 succeeding.
- (P1) ApplyDamage hook — still no AOB. May fall out of §4 if we can emit
  damage events via EMEVD.
- (P2) Reconnect logic on session drop.
- (P2) Stale ChrIns registry entry pruning (enemies that despawn leave stale
  pointers; mostly self-healing on handle re-registration).
- (P3) Save-state sync on handshake (host dumps current flags, client applies).
- (P3) Full-world ingame overlay via hudhook (scaffolded, unwired).
- (P3) Steam P2P transport (currently raw UDP).

## 14. Session transcript reference

The full prior conversation is saved at
`C:\Users\ur2ba\.claude\projects\Z--dev-sesc\4303444e-f945-4ad4-b5d2-8a3eb6daa56e.jsonl`
if any decision here needs context.

---

**Handoff checklist for the next agent:**

1. Read this file fully.
2. Read `crates/sekiro-coop-dll/src/lib.rs:on_frame` and `hooks.rs` end-to-end.
3. Read `crates/sekiro-coop-emevd/src/{catalog.rs,gen.rs,patch.rs}`.
4. Confirm with user: "Ready to download UXM + Yabber and walk the pipeline
   from §4 Step 1?"
5. Do **not** start another speculative direction without confirming.
