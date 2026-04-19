//! Per-version base addresses for Sekiro.
//!
//! Source: OSINT §1.1 (verbatim from `libsekiro`'s `offsets.rs`).
//! RVA offsets, added to the loaded module base address.

use crate::version::GameVersion;

/// Named symbols we can reach by either AOB scan or version-indexed RVA.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Symbol {
    Quitout,
    RenderWorld,
    DebugRender,
    Igt,
    PlayerPosition,
    DebugFlags,
    ShowCursor,
    NoLogo,
    FontPatch,
    DebugShow,
    GrappleDebug,
    Fps,
}

/// The resolved base-address table for one concrete game version.
#[derive(Debug, Clone, Copy)]
pub struct BaseAddrs {
    pub version: GameVersion,
    pub quitout: usize,
    pub render_world: usize,
    pub debug_render: usize,
    pub igt: usize,
    pub player_position: usize,
    pub debug_flags: usize,
    pub show_cursor: usize,
    pub no_logo: usize,
    pub font_patch: usize,
    pub debug_show: usize,
    pub grapple_debug: usize,
    pub fps: usize,
}

impl BaseAddrs {
    /// Look up the version-specific RVA table. The returned addresses are
    /// module-relative; callers must add the module base to use them.
    pub const fn for_version(version: GameVersion) -> Option<Self> {
        match version {
            GameVersion::V1_02 => Some(Self {
                version,
                quitout: 0x3b55048,
                render_world: 0x39007c8,
                debug_render: 0x3b65bc0,
                igt: 0x3b47cf0,
                player_position: 0x3b67df0,
                debug_flags: 0x3b67f59,
                show_cursor: 0x3b77048,
                no_logo: 0xdebf2b,
                font_patch: 0x2505974,
                debug_show: 0x3b67f98,
                grapple_debug: 0x3b5b240,
                fps: 0x3c8c2c8,
            }),
            GameVersion::V1_03_04 => Some(Self {
                version,
                quitout: 0x3b56088,
                render_world: 0x39017c8,
                debug_render: 0x3b66c00,
                igt: 0x3b48d30,
                player_position: 0x3b68e30,
                debug_flags: 0x3b68f99,
                show_cursor: 0x3b78088,
                no_logo: 0xdec85b,
                font_patch: 0x25068e4,
                debug_show: 0x3b68fd8,
                grapple_debug: 0x3b5c280,
                fps: 0x3c8d308,
            }),
            GameVersion::V1_05 => Some(Self {
                version,
                quitout: 0x3d67368,
                render_world: 0x3b01838,
                debug_render: 0x3d77f04,
                igt: 0x3d5aa20,
                player_position: 0x3d7a140,
                debug_flags: 0x3d7a2c9,
                show_cursor: 0x3d8986c,
                no_logo: 0xe1b1ab,
                font_patch: 0x263b894,
                debug_show: 0x3d7a2e8,
                grapple_debug: 0x3d6d5a0,
                fps: 0x3e9f6a8,
            }),
            GameVersion::V1_06 => Some(Self {
                version,
                quitout: 0x3d67408,
                render_world: 0x3b01838,
                debug_render: 0x3d77fa4,
                igt: 0x3d5aac0,
                player_position: 0x3d7a1e0,
                debug_flags: 0x3d7a369,
                show_cursor: 0x3d8990c,
                no_logo: 0xe1b51b,
                font_patch: 0x263bc14,
                debug_show: 0x3d7a388,
                grapple_debug: 0x3d6d640,
                fps: 0x3e9f748,
            }),
            GameVersion::Unknown => None,
        }
    }

    pub fn get(&self, sym: Symbol) -> usize {
        match sym {
            Symbol::Quitout => self.quitout,
            Symbol::RenderWorld => self.render_world,
            Symbol::DebugRender => self.debug_render,
            Symbol::Igt => self.igt,
            Symbol::PlayerPosition => self.player_position,
            Symbol::DebugFlags => self.debug_flags,
            Symbol::ShowCursor => self.show_cursor,
            Symbol::NoLogo => self.no_logo,
            Symbol::FontPatch => self.font_patch,
            Symbol::DebugShow => self.debug_show,
            Symbol::GrappleDebug => self.grapple_debug,
            Symbol::Fps => self.fps,
        }
    }
}

/// The grapple-debug pointer chain differs across versions: final offset
/// is `0xEC8` on 1.02–1.04 and `0xF68` on 1.05–1.06. Source: OSINT §1.1.
pub fn grapple_debug_final_offset(version: GameVersion) -> usize {
    match version {
        GameVersion::V1_02 | GameVersion::V1_03_04 => 0xEC8,
        GameVersion::V1_05 | GameVersion::V1_06 => 0xF68,
        GameVersion::Unknown => 0xF68,
    }
}

/// On 1.05+ the `player_no_dead`, `player_exterminate`, and
/// `player_exterminate_stamina` debug-flag offsets are written relative
/// to a different base (negative offsets). See OSINT §1.1 debug-flag table.
#[derive(Debug, Clone, Copy)]
pub struct DebugFlagOffsets {
    pub no_goods_consume: isize,
    pub no_resource_item_consume: isize,
    pub no_revival_consume: isize,
    pub player_hide: isize,
    pub player_silence: isize,
    pub all_no_dead: isize,
    pub all_no_damage: isize,
    pub all_no_hit: isize,
    pub all_no_attack: isize,
    pub all_no_move: isize,
    pub all_no_update_ai: isize,
    pub all_no_stamina_consume: isize,
    pub player_no_dead: isize,
    pub player_exterminate: isize,
    pub player_exterminate_stamina: isize,
}

impl DebugFlagOffsets {
    pub const fn for_version(version: GameVersion) -> Self {
        // Common offsets (shared across all versions).
        let mut d = Self {
            no_goods_consume: 0,
            no_resource_item_consume: 1,
            no_revival_consume: 2,
            player_hide: 6,
            player_silence: 7,
            all_no_dead: 8,
            all_no_damage: 9,
            all_no_hit: 10,
            all_no_attack: 11,
            all_no_move: 12,
            all_no_update_ai: 13,
            all_no_stamina_consume: 20,
            player_no_dead: 33,
            player_exterminate: 52,
            player_exterminate_stamina: -1,
        };
        match version {
            GameVersion::V1_05 | GameVersion::V1_06 => {
                d.player_no_dead = -3;
                d.player_exterminate = -2;
                d.player_exterminate_stamina = -1;
            }
            _ => {}
        }
        d
    }
}
