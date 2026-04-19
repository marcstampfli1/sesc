//! Typed live accessors for every documented pointer chain.
//!
//! Source: OSINT §1.1 "Resolved pointer chains".  Each function below
//! wraps exactly the chain listed there.
//!
//! ```text
//!   Player position [f32; 4]  → player_position → +0x48 → +0x28 → +0x80
//!   Animation speed (f32)     → player_position → +0x48 → +0x28 → +0xA40 → +0x4C0 → +0x250 → +0x10 → +0xD00
//!   IGT (u32)                 → igt → +0x9C
//!   Quitout trigger (u8)      → quitout → +0x23C
//!   FPS (f32)                 → fps → +0x2BC
//!   Debug show bit0           → debug_show → +0x6F
//!   Grapple debug path        → grapple_debug → +0xC8 → +0x20 → +GRAPPLE_FINAL
//! ```
//!
//! All functions return `Option<T>` — `None` when any pointer in the
//! chain is null (e.g. the player isn't spawned yet).  Call every frame;
//! never cache the returned value.

use crate::memory::{PtrChain, RawPtr};
use crate::offsets::{grapple_debug_final_offset, BaseAddrs};
use crate::version::GameVersion;

/// Resolve a one-hop-then-read chain: `base → +off → read::<T>`.
///
/// # Safety
/// `chain` must point to live memory for the current frame.
unsafe fn resolve_read<T: Copy>(chain: &PtrChain) -> Option<T> {
    let final_ptr = chain.resolve();
    if final_ptr.is_null() {
        return None;
    }
    Some(final_ptr.read())
}

/// Player XYZ position (ignore the 4th component — it's a W-padding /
/// heading value depending on animation state).
///
/// # Safety
/// `addrs` must match the running version; module must be loaded.
pub unsafe fn player_position(addrs: &BaseAddrs, module_base: usize) -> Option<[f32; 4]> {
    let base = rva_to_raw(module_base, addrs.player_position)?;
    let chain = PtrChain::new(base, [0x48, 0x28, 0x80]);
    resolve_read(&chain)
}

/// Convert an RVA into an absolute `RawPtr`, returning `None` if the
/// RVA is `UNRESOLVED` (sentinel for addresses we don't have on this
/// patch version).
#[inline]
fn rva_to_raw(module_base: usize, rva: usize) -> Option<RawPtr> {
    if rva == crate::chrins::UNRESOLVED {
        return None;
    }
    Some(RawPtr(module_base.wrapping_add(rva)))
}

/// Animation playback speed multiplier.  1.0 = normal, 0.0 = paused,
/// 2.0 = 2× speed.  Useful for the determinism probe (freeze one
/// instance's AI via `all_no_update_ai`, observe that the other
/// progresses).
///
/// # Safety
/// See [`player_position`].
pub unsafe fn animation_speed(addrs: &BaseAddrs, module_base: usize) -> Option<f32> {
    let base = rva_to_raw(module_base, addrs.player_position)?;
    let chain = PtrChain::new(
        base,
        [0x48, 0x28, 0xA40, 0x4C0, 0x250, 0x10, 0xD00],
    );
    resolve_read(&chain)
}

/// In-Game Time in milliseconds.
///
/// # Safety
/// See [`player_position`].
pub unsafe fn igt_ms(addrs: &BaseAddrs, module_base: usize) -> Option<u32> {
    let base = rva_to_raw(module_base, addrs.igt)?;
    let chain = PtrChain::new(base, [0x9C]);
    resolve_read(&chain)
}

/// Current frame time in milliseconds (inverse of FPS).
///
/// # Safety
/// See [`player_position`].
pub unsafe fn fps(addrs: &BaseAddrs, module_base: usize) -> Option<f32> {
    let base = rva_to_raw(module_base, addrs.fps)?;
    let chain = PtrChain::new(base, [0x2BC]);
    resolve_read(&chain)
}

/// Quitout trigger byte.  Writing 1 makes the game quit back to the
/// Sculptor's Idol; writing 0 is a no-op.
///
/// # Safety
/// See [`player_position`].
pub unsafe fn quitout_trigger(addrs: &BaseAddrs, module_base: usize) -> Option<u8> {
    let base = rva_to_raw(module_base, addrs.quitout)?;
    let chain = PtrChain::new(base, [0x23C]);
    resolve_read(&chain)
}

/// Grapple-debug bit.  Version-dependent final offset.
///
/// # Safety
/// See [`player_position`].
pub unsafe fn grapple_debug_byte(
    addrs: &BaseAddrs,
    module_base: usize,
    version: GameVersion,
) -> Option<u8> {
    let base = rva_to_raw(module_base, addrs.grapple_debug)?;
    let chain = PtrChain::new(
        base,
        [0xC8, 0x20, grapple_debug_final_offset(version) as isize],
    );
    resolve_read(&chain)
}

/// "Debug show" bit (bit 0 of the byte at `debug_show + 0x6F`).
///
/// # Safety
/// See [`player_position`].
pub unsafe fn debug_show_bit(addrs: &BaseAddrs, module_base: usize) -> Option<bool> {
    let base = rva_to_raw(module_base, addrs.debug_show)?;
    let chain = PtrChain::new(base, [0x6F]);
    let b: u8 = resolve_read(&chain)?;
    Some((b & 0x01) != 0)
}

// ---------------------------------------------------------------------
// Player state readers via ChrIns state-module chains.
// Chain base: WorldChrMan → +0x88 (Hero) → +0x1ff8 (state module) → ...
// Validated empirically against Sekiro 1.06 (HP=320→96, Posture=0→91,
// anim=790010→790060).  Source: SEKIRO_OFFSETS.md D.2 + SEKIRO_MULTIPLAYER.md §1.3.
// ---------------------------------------------------------------------

/// Current HP (signed 32-bit).
///
/// # Safety
/// See [`player_position`].
pub unsafe fn player_hp(addrs: &BaseAddrs, module_base: usize) -> Option<i32> {
    let base = rva_to_raw(module_base, addrs.player_position)?;
    let chain = PtrChain::new(base, [0x88, 0x1ff8, 0x18, 0x130]);
    resolve_read(&chain)
}

/// Max HP.
///
/// # Safety
/// See [`player_position`].
pub unsafe fn player_max_hp(addrs: &BaseAddrs, module_base: usize) -> Option<i32> {
    let base = rva_to_raw(module_base, addrs.player_position)?;
    let chain = PtrChain::new(base, [0x88, 0x1ff8, 0x18, 0x134]);
    resolve_read(&chain)
}

/// Current posture (called "Stamina" internally; same field).
///
/// # Safety
/// See [`player_position`].
pub unsafe fn player_posture(addrs: &BaseAddrs, module_base: usize) -> Option<i32> {
    let base = rva_to_raw(module_base, addrs.player_position)?;
    let chain = PtrChain::new(base, [0x88, 0x1ff8, 0x18, 0x148]);
    resolve_read(&chain)
}

/// Max posture.
///
/// # Safety
/// See [`player_position`].
pub unsafe fn player_max_posture(addrs: &BaseAddrs, module_base: usize) -> Option<i32> {
    let base = rva_to_raw(module_base, addrs.player_position)?;
    let chain = PtrChain::new(base, [0x88, 0x1ff8, 0x18, 0x14C]);
    resolve_read(&chain)
}

/// Player [x, y, z] via the CE chain.  libsekiro's short
/// [`player_position`] chain lands at the same address but returns
/// `[f32; 4]` (the trailing W is a quat scratch slot).
///
/// # Safety
/// See [`player_position`].
pub unsafe fn player_position_xyz(addrs: &BaseAddrs, module_base: usize) -> Option<[f32; 3]> {
    let base = rva_to_raw(module_base, addrs.player_position)?;
    let chain = PtrChain::new(base, [0x88, 0x1ff8, 0x68, 0x80]);
    resolve_read(&chain)
}

/// Currently-playing animation ID (u32 in EMEVD's animation-id namespace).
///
/// # Safety
/// See [`player_position`].
pub unsafe fn player_current_anim(addrs: &BaseAddrs, module_base: usize) -> Option<u32> {
    let base = rva_to_raw(module_base, addrs.player_position)?;
    let chain = PtrChain::new(base, [0x88, 0x1ff8, 0x10, 0x20]);
    resolve_read(&chain)
}

/// Seconds the current animation has been playing (f32).
///
/// # Safety
/// See [`player_position`].
pub unsafe fn player_anim_elapsed(addrs: &BaseAddrs, module_base: usize) -> Option<f32> {
    let base = rva_to_raw(module_base, addrs.player_position)?;
    let chain = PtrChain::new(base, [0x88, 0x1ff8, 0x10, 0x24]);
    resolve_read(&chain)
}

/// Animation PlaySpeed multiplier (1.0 = normal, 0.0 = paused).
///
/// # Safety
/// See [`player_position`].
pub unsafe fn player_play_speed(addrs: &BaseAddrs, module_base: usize) -> Option<f32> {
    let base = rva_to_raw(module_base, addrs.player_position)?;
    let chain = PtrChain::new(base, [0x88, 0x1ff8, 0x28, 0xD00]);
    resolve_read(&chain)
}

/// Player entity handle (low 32 bits).  ChrIns+0x08.
///
/// # Safety
/// See [`player_position`].
pub unsafe fn player_handle(addrs: &BaseAddrs, module_base: usize) -> Option<u32> {
    let base = rva_to_raw(module_base, addrs.player_position)?;
    let chain = PtrChain::new(base, [0x88, 0x08]);
    resolve_read(&chain)
}

/// Player character id (c-number).  ChrIns+0x68.  Always 0 for the host
/// player (c0000).
///
/// # Safety
/// See [`player_position`].
pub unsafe fn player_char_id(addrs: &BaseAddrs, module_base: usize) -> Option<u32> {
    let base = rva_to_raw(module_base, addrs.player_position)?;
    let chain = PtrChain::new(base, [0x88, 0x68]);
    resolve_read(&chain)
}

/// Player TeamType raw byte (see `sekiro-sdk-core::enums::TeamType` for
/// typed decoding).  ChrIns+0x74.
///
/// # Safety
/// See [`player_position`].
pub unsafe fn player_team_type(addrs: &BaseAddrs, module_base: usize) -> Option<u8> {
    let base = rva_to_raw(module_base, addrs.player_position)?;
    let chain = PtrChain::new(base, [0x88, 0x74]);
    resolve_read(&chain)
}

/// Read `team_type` for an arbitrary ChrIns pointer.
///
/// # Safety
/// `chrins` must be a live ChrIns instance.
pub unsafe fn team_type_of(chrins: RawPtr) -> u8 {
    chrins.offset(0x74).read::<u8>()
}

/// Read `character_id` (c-number) for an arbitrary ChrIns pointer.
///
/// # Safety
/// `chrins` must be a live ChrIns instance.
pub unsafe fn char_id_of(chrins: RawPtr) -> u32 {
    chrins.offset(0x68).read::<u32>()
}

/// Read `handle` (entity handle) for an arbitrary ChrIns pointer.
///
/// # Safety
/// `chrins` must be a live ChrIns instance.
pub unsafe fn handle_of(chrins: RawPtr) -> u32 {
    chrins.offset(0x08).read::<u32>()
}

/// State bundle for an arbitrary ChrIns (enemy, NPC, or Hero).  Every
/// field is an `Option` so we can snapshot while the state module is
/// still being set up (transitions / respawn).  Uses the same three-hop
/// chain the Hero uses: `ChrIns+0x1ff8 → +0x18 → +0x130` (HP etc.),
/// `+0x1ff8 → +0x68 → +0x80` (position), `+0x1ff8 → +0x10 → +0x20` (anim).
#[derive(Debug, Clone, Copy, Default)]
pub struct ChrInsStateSnapshot {
    pub handle: u32,
    pub char_id: u32,
    pub team_type: u8,
    pub hp: Option<i32>,
    pub max_hp: Option<i32>,
    pub posture: Option<i32>,
    pub max_posture: Option<i32>,
    pub position: Option<[f32; 3]>,
    pub animation_id: Option<u32>,
}

/// Follow a fixed chain of `deref → add → deref → add → … → final` from
/// a known-good ChrIns pointer.  Unlike [`PtrChain::resolve`] which
/// starts by dereffing `base` (meant for *addresses of* pointer
/// variables), this helper assumes `chrins` is already a real instance
/// pointer.  Returns `None` on any null along the way.
#[inline]
unsafe fn chain_read<T: Copy>(chrins: RawPtr, offsets: &[isize]) -> Option<T> {
    if chrins.is_null() {
        return None;
    }
    let (last_off, mid) = offsets.split_last()?;
    let (first_off, rest) = mid.split_first()?;
    let mut p: usize = chrins.offset(*first_off).read();
    for &off in rest {
        if p == 0 {
            return None;
        }
        p = RawPtr(p).offset(off).read();
    }
    if p == 0 {
        return None;
    }
    Some(RawPtr(p).offset(*last_off).read::<T>())
}

/// Walk the three-hop HP chain (ChrIns+0x1ff8 → +0x18 → +0x130) and
/// return the *address* of the HP field, or `None` if any deref along
/// the way is null.  Callers may use the address to read or write.
///
/// # Safety
/// `chrins` must be a live ChrIns pointer.
pub unsafe fn chrins_hp_addr(chrins: RawPtr) -> Option<RawPtr> {
    if chrins.is_null() {
        return None;
    }
    let state_module: usize = chrins.offset(0x1ff8).read();
    if state_module == 0 {
        return None;
    }
    let substruct: usize = RawPtr(state_module).offset(0x18).read();
    if substruct == 0 {
        return None;
    }
    Some(RawPtr(substruct).offset(0x130))
}

/// Write HP on an arbitrary ChrIns.  Returns true on success, false if
/// the chain failed to resolve.  Direct field write; the game may
/// overwrite this on the next simulation tick.
///
/// # Safety
/// `chrins` must be a live ChrIns.  Caller is responsible for enforcing
/// any "decrement-only" policy — this function writes whatever HP is
/// passed.
pub unsafe fn chrins_write_hp(chrins: RawPtr, hp: i32) -> bool {
    if let Some(addr) = chrins_hp_addr(chrins) {
        (addr.0 as *mut i32).write(hp);
        true
    } else {
        false
    }
}

/// Write position on an arbitrary ChrIns via the same state-module
/// chain used by the reader: `+0x1ff8 → +0x68 → +0x80` → `[f32; 3]`.
/// Experimental — the physics/render path may read from a separate
/// mirror and overwrite this on next tick.
///
/// # Safety
/// `chrins` must be a live ChrIns pointer.
pub unsafe fn chrins_write_position(chrins: RawPtr, xyz: [f32; 3]) -> bool {
    if chrins.is_null() {
        return false;
    }
    let sm: usize = chrins.offset(0x1ff8).read();
    if sm == 0 {
        return false;
    }
    let sub: usize = RawPtr(sm).offset(0x68).read();
    if sub == 0 {
        return false;
    }
    let p = RawPtr(sub).offset(0x80).0 as *mut [f32; 3];
    p.write(xyz);
    true
}

/// Write animation-id (current playing anim) via the chain used by
/// [`player_current_anim`]: `+0x1ff8 → +0x10 → +0x20` → `u32`.
/// Experimental — animation playback typically requires a native
/// `RequestAnimationPlayback` call rather than a field write; a raw
/// write of the id may or may not actually trigger playback.
///
/// # Safety
/// `chrins` must be a live ChrIns pointer.
pub unsafe fn chrins_write_animation_id(chrins: RawPtr, anim_id: u32) -> bool {
    if chrins.is_null() {
        return false;
    }
    let sm: usize = chrins.offset(0x1ff8).read();
    if sm == 0 {
        return false;
    }
    let sub: usize = RawPtr(sm).offset(0x10).read();
    if sub == 0 {
        return false;
    }
    let p = RawPtr(sub).offset(0x20).0 as *mut u32;
    p.write(anim_id);
    true
}

/// Snapshot an arbitrary ChrIns's full state.  Cheap (handful of
/// pointer derefs); safe to call once per frame per entity.
///
/// # Safety
/// `chrins` must be a live ChrIns — the intended source is the DLL's
/// hook-supplied `entity` argument (the registry).
pub unsafe fn chrins_snapshot(chrins: RawPtr) -> ChrInsStateSnapshot {
    ChrInsStateSnapshot {
        handle: handle_of(chrins),
        char_id: char_id_of(chrins),
        team_type: team_type_of(chrins),
        hp: chain_read::<i32>(chrins, &[0x1ff8, 0x18, 0x130]),
        max_hp: chain_read::<i32>(chrins, &[0x1ff8, 0x18, 0x134]),
        posture: chain_read::<i32>(chrins, &[0x1ff8, 0x18, 0x148]),
        max_posture: chain_read::<i32>(chrins, &[0x1ff8, 0x18, 0x14C]),
        position: chain_read::<[f32; 3]>(chrins, &[0x1ff8, 0x68, 0x80]),
        animation_id: chain_read::<u32>(chrins, &[0x1ff8, 0x10, 0x20]),
    }
}

/// Comprehensive live player-state bundle.  Call from the overlay or
/// per-tick diagnostics.  Each field is `Option` so partial resolves
/// (e.g. player not loaded yet) yield a partial snapshot.
#[derive(Debug, Clone, Copy, Default)]
pub struct PlayerStateLive {
    pub hp: Option<i32>,
    pub max_hp: Option<i32>,
    pub posture: Option<i32>,
    pub max_posture: Option<i32>,
    pub position: Option<[f32; 3]>,
    pub current_anim: Option<u32>,
    pub anim_elapsed: Option<f32>,
    pub play_speed: Option<f32>,
    pub handle: Option<u32>,
    pub char_id: Option<u32>,
    pub team_type: Option<u8>,
    pub igt_ms: Option<u32>,
    pub fps: Option<f32>,
}

/// # Safety
/// See [`player_position`].
pub unsafe fn sample_player_state(addrs: &BaseAddrs, module_base: usize) -> PlayerStateLive {
    PlayerStateLive {
        hp: player_hp(addrs, module_base),
        max_hp: player_max_hp(addrs, module_base),
        posture: player_posture(addrs, module_base),
        max_posture: player_max_posture(addrs, module_base),
        position: player_position_xyz(addrs, module_base),
        current_anim: player_current_anim(addrs, module_base),
        anim_elapsed: player_anim_elapsed(addrs, module_base),
        play_speed: player_play_speed(addrs, module_base),
        handle: player_handle(addrs, module_base),
        char_id: player_char_id(addrs, module_base),
        team_type: player_team_type(addrs, module_base),
        igt_ms: igt_ms(addrs, module_base),
        fps: fps(addrs, module_base),
    }
}

/// Bundle of live reads executed in one pass.  Convenient for the
/// overlay.
#[derive(Debug, Clone, Copy, Default)]
pub struct LiveState {
    pub player_position: Option<[f32; 4]>,
    pub animation_speed: Option<f32>,
    pub igt_ms: Option<u32>,
    pub fps: Option<f32>,
    pub debug_show: Option<bool>,
}

/// # Safety
/// See [`player_position`].
pub unsafe fn sample_live_state(
    addrs: &BaseAddrs,
    module_base: usize,
) -> LiveState {
    LiveState {
        player_position: player_position(addrs, module_base),
        animation_speed: animation_speed(addrs, module_base),
        igt_ms: igt_ms(addrs, module_base),
        fps: fps(addrs, module_base),
        debug_show: debug_show_bit(addrs, module_base),
    }
}

#[cfg(test)]
mod tests {
    //! Fixture-based tests.  We fabricate a memory graph that exercises
    //! each pointer chain and confirm the resolver walks it correctly.
    //!
    //! Layout per test: one heap-allocated buffer hosts the whole
    //! graph, then we pick specific offsets as "node pointers" and
    //! write [f32;4] / f32 / u32 values at the final sites.
    use super::*;
    use crate::chrins::UNRESOLVED;
    use crate::offsets::BaseAddrs;
    use crate::version::GameVersion;

    /// Lay out a graph that matches the player_position chain:
    ///   base(symbol) → [+0x48] → [+0x28] → [+0x80 = value]
    ///
    /// We allocate three 16-byte-aligned nodes in one buffer, wire the
    /// pointers, and return the "module base" (buf ptr) + the RVA that
    /// `base(symbol)` should have.
    struct Graph {
        buf: Vec<u8>,
    }

    impl Graph {
        fn new(size: usize) -> Self {
            Self { buf: vec![0u8; size] }
        }
        fn ptr(&self) -> usize {
            self.buf.as_ptr() as usize
        }
        fn write_u64(&mut self, off: usize, v: u64) {
            self.buf[off..off + 8].copy_from_slice(&v.to_le_bytes());
        }
        fn write_f32(&mut self, off: usize, v: f32) {
            self.buf[off..off + 4].copy_from_slice(&v.to_le_bytes());
        }
        fn write_u32(&mut self, off: usize, v: u32) {
            self.buf[off..off + 4].copy_from_slice(&v.to_le_bytes());
        }
        fn write_array_f32_4(&mut self, off: usize, v: [f32; 4]) {
            for i in 0..4 {
                self.write_f32(off + i * 4, v[i]);
            }
        }
    }

    fn fake_addrs(pp: usize, igt: usize, fps: usize) -> BaseAddrs {
        BaseAddrs {
            version: GameVersion::V1_06,
            quitout: UNRESOLVED,
            render_world: UNRESOLVED,
            debug_render: UNRESOLVED,
            igt,
            player_position: pp,
            debug_flags: UNRESOLVED,
            show_cursor: UNRESOLVED,
            no_logo: UNRESOLVED,
            font_patch: UNRESOLVED,
            debug_show: UNRESOLVED,
            grapple_debug: UNRESOLVED,
            fps,
        }
    }

    #[test]
    fn player_position_walks_three_hop_chain() {
        let mut g = Graph::new(4096);
        // Node A at offset 0x100; holds pointer at +0x48 → node B at 0x400.
        // Node B at 0x400; holds pointer at +0x28 → node C at 0x800.
        // Node C at 0x800; holds [f32;4] at +0x80.
        let base = g.ptr();
        g.write_u64(0x100 + 0x48, (base + 0x400) as u64);
        g.write_u64(0x400 + 0x28, (base + 0x800) as u64);
        g.write_array_f32_4(0x800 + 0x80, [1.0, 2.0, 3.0, 4.0]);

        // The symbol's RVA (player_position) points at a `usize` whose
        // value is the address of node A. But the chain starts at the
        // symbol itself and first *reads* the u64 there to get node A.
        // So lay out the symbol slot at 0x080 → read → node A.
        g.write_u64(0x80, (base + 0x100) as u64);

        let addrs = fake_addrs(0x80, UNRESOLVED, UNRESOLVED);
        let pos = unsafe { player_position(&addrs, base) }.expect("resolved");
        assert_eq!(pos, [1.0, 2.0, 3.0, 4.0]);
    }

    #[test]
    fn igt_walks_one_hop() {
        let mut g = Graph::new(512);
        let base = g.ptr();
        g.write_u64(0x10, (base + 0x100) as u64); // symbol → node
        g.write_u32(0x100 + 0x9C, 123_456);
        let addrs = fake_addrs(UNRESOLVED, 0x10, UNRESOLVED);
        let v = unsafe { igt_ms(&addrs, base) }.expect("resolved");
        assert_eq!(v, 123_456);
    }

    #[test]
    fn fps_walks_one_hop() {
        let mut g = Graph::new(4096);
        let base = g.ptr();
        g.write_u64(0x200, (base + 0x300) as u64);
        g.write_f32(0x300 + 0x2BC, 60.0);
        let addrs = fake_addrs(UNRESOLVED, UNRESOLVED, 0x200);
        let v = unsafe { fps(&addrs, base) }.expect("resolved");
        assert!((v - 60.0).abs() < 1e-6);
    }

    #[test]
    fn null_root_yields_none() {
        let mut g = Graph::new(512);
        let base = g.ptr();
        // Symbol slot is 0 — null root.
        g.write_u64(0x10, 0);
        let addrs = fake_addrs(UNRESOLVED, 0x10, UNRESOLVED);
        let v = unsafe { igt_ms(&addrs, base) };
        assert!(v.is_none());
    }

    #[test]
    fn sample_bundle_returns_all_available() {
        let mut g = Graph::new(8192);
        let base = g.ptr();
        // player_position
        g.write_u64(0x80, (base + 0x100) as u64);
        g.write_u64(0x100 + 0x48, (base + 0x400) as u64);
        g.write_u64(0x400 + 0x28, (base + 0x800) as u64);
        g.write_array_f32_4(0x800 + 0x80, [7.0, 8.0, 9.0, 10.0]);
        // igt
        g.write_u64(0x200, (base + 0x300) as u64);
        g.write_u32(0x300 + 0x9C, 42);
        // fps
        g.write_u64(0x500, (base + 0x600) as u64);
        g.write_f32(0x600 + 0x2BC, 59.94);

        let addrs = fake_addrs(0x80, 0x200, 0x500);
        let s = unsafe { sample_live_state(&addrs, base) };
        assert_eq!(s.player_position, Some([7.0, 8.0, 9.0, 10.0]));
        assert_eq!(s.igt_ms, Some(42));
        assert!(matches!(s.fps, Some(v) if (v - 59.94).abs() < 1e-3));
    }
}
