//! Resolved native-function and symbol-base addresses.
//!
//! Runtime AOB scanner that takes the loaded sekiro.exe image and
//! produces absolute addresses for every documented native function
//! and global pointer symbol from `SEKIRO_OFFSETS.md`.
//!
//! Usage:
//!
//! ```ignore
//! let bytes = unsafe { module.as_bytes() };
//! let natives = Natives::scan(&bytes, module.base);
//! if let Some(addr) = natives.apply_effect {
//!     // ApplyEffect is at `addr` — hook it with MinHook.
//! }
//! ```

use crate::aob::{patterns, resolve_rip_relative, AobPattern};

/// Resolved absolute addresses for all named symbol bases (from
/// `SEKIRO_OFFSETS.md` Part A).  `None` when the AOB didn't match
/// (unexpected patch version or corrupted image).
#[derive(Debug, Clone, Copy, Default)]
pub struct Symbols {
    pub world_chr_man: Option<usize>,
    pub world_chr_man_dbg: Option<usize>,
    pub world_ai_manager: Option<usize>,
    pub game_man: Option<usize>,
    pub field_area: Option<usize>,
    pub solo_param_repository: Option<usize>,
    pub game_data: Option<usize>,
    pub event_man: Option<usize>,
    pub event_flag_man: Option<usize>,
    pub lock_tgt_man: Option<usize>,
    pub damage_management: Option<usize>,
    pub map_item_man: Option<usize>,
    pub dlc: Option<usize>,
    pub debug_menu: Option<usize>,
    pub render_flags: Option<usize>,
    pub targeting_draw_flags: Option<usize>,
    pub rend_man: Option<usize>,
}

/// Resolved absolute addresses for the 13 named native functions.
/// These are hook targets.
#[derive(Debug, Clone, Copy, Default)]
pub struct Functions {
    pub give_item_debug: Option<usize>,
    pub lua_do_string: Option<usize>,
    pub get_flag: Option<usize>,
    pub set_flag: Option<usize>,
    pub draw_debug_mesh: Option<usize>,
    pub give_item: Option<usize>,
    pub apply_effect: Option<usize>,
    pub special_effect_delete_effect: Option<usize>,
    pub upgrade_prosthetics_menu: Option<usize>,
    pub open_skills_menu: Option<usize>,
    pub warp_next_stage_bonfire: Option<usize>,
    pub add_experience: Option<usize>,
    pub enlarge_unk_hkb_array: Option<usize>,
}

/// One-shot container combining both.
#[derive(Debug, Clone, Copy, Default)]
pub struct Natives {
    pub symbols: Symbols,
    pub functions: Functions,
}

impl Natives {
    /// Scan the loaded module image, produce absolute addresses for
    /// every AOB that matches.  Call once on DLL attach.
    pub fn scan(image: &[u8], module_base: usize) -> Self {
        let symbols = scan_symbols(image, module_base);
        let functions = scan_functions(image, module_base);
        Self { symbols, functions }
    }
}

// ---------------------------------------------------------------------
//  Symbols — each AOB points at an instruction with a RIP-relative
//  displacement that references the static global.  We scan, compute
//  the RIP-relative target, and return it as an absolute address.
// ---------------------------------------------------------------------

fn scan_symbols(image: &[u8], module_base: usize) -> Symbols {
    // Format: (name, pattern, disp_offset, instr_len)
    let entries: &[(
        &str,
        fn() -> AobPattern,
        usize,
        usize,
    )] = &[
        ("world_chr_man",         patterns::world_chr_man,         6, 10),
        ("world_chr_man_dbg",     patterns::world_chr_man_dbg,     6, 10),
        ("world_ai_manager",      patterns::world_ai_manager,      12, 16),
        ("game_man",              patterns::game_man,              12, 16),
        ("field_area",            patterns::field_area,            10, 14),
        ("solo_param_repository", patterns::solo_param_repository, 3, 7),
        ("game_data",             patterns::game_data,             12, 16),
        ("event_man",             patterns::event_man,             6, 10),
        ("event_flag_man",        patterns::event_flag_man,        7, 11),
        ("lock_tgt_man",          patterns::lock_tgt_man,          3, 7),
        ("damage_management",     patterns::damage_management,     3, 7),
        ("map_item_man",          patterns::map_item_man,          8, 12),
        ("dlc",                   patterns::dlc,                   3, 7),
        ("debug_menu",            patterns::debug_menu,            12, 17),
        ("render_flags",          patterns::render_flags,          13, 17),
        ("targeting_draw_flags",  patterns::targeting_draw_flags,  11, 15),
        ("rend_man",              patterns::rend_man,              3, 7),
    ];

    let mut s = Symbols::default();
    for (name, pat_fn, disp, ilen) in entries {
        let pat = pat_fn();
        let scan_hit = match pat.scan(image) {
            Ok(off) => off,
            Err(_) => continue,
        };
        let abs = match resolve_rip_relative(image, scan_hit, *disp, *ilen) {
            Ok(rva) => module_base + rva,
            Err(_) => continue,
        };
        assign_symbol(&mut s, name, abs);
    }
    s
}

fn assign_symbol(s: &mut Symbols, name: &str, addr: usize) {
    match name {
        "world_chr_man" => s.world_chr_man = Some(addr),
        "world_chr_man_dbg" => s.world_chr_man_dbg = Some(addr),
        "world_ai_manager" => s.world_ai_manager = Some(addr),
        "game_man" => s.game_man = Some(addr),
        "field_area" => s.field_area = Some(addr),
        "solo_param_repository" => s.solo_param_repository = Some(addr),
        "game_data" => s.game_data = Some(addr),
        "event_man" => s.event_man = Some(addr),
        "event_flag_man" => s.event_flag_man = Some(addr),
        "lock_tgt_man" => s.lock_tgt_man = Some(addr),
        "damage_management" => s.damage_management = Some(addr),
        "map_item_man" => s.map_item_man = Some(addr),
        "dlc" => s.dlc = Some(addr),
        "debug_menu" => s.debug_menu = Some(addr),
        "render_flags" => s.render_flags = Some(addr),
        "targeting_draw_flags" => s.targeting_draw_flags = Some(addr),
        "rend_man" => s.rend_man = Some(addr),
        _ => {}
    }
}

// ---------------------------------------------------------------------
//  Functions — each AOB matches a byte inside the function body; the
//  named offset is the adjustment to reach the function prologue.
//  These are absolute (scan_hit + module_base + offset), NOT
//  RIP-relative.
// ---------------------------------------------------------------------

fn scan_functions(image: &[u8], module_base: usize) -> Functions {
    // Format: (name, pattern, offset_from_scan_hit)
    // Offsets match the "AOB offset" column in SEKIRO_OFFSETS.md Part B.
    let entries: &[(&str, fn() -> AobPattern, isize)] = &[
        ("give_item_debug",             patterns::fn_give_item_debug,             0),
        ("lua_do_string",               patterns::fn_lua_do_string,               0),
        ("get_flag",                    patterns::fn_get_flag,                    -13),
        ("set_flag",                    patterns::fn_set_flag,                    -19),
        ("draw_debug_mesh",             patterns::fn_draw_debug_mesh,             -15),
        ("give_item",                   patterns::fn_give_item,                   -48),
        ("apply_effect",                patterns::fn_apply_effect,                -107),
        ("special_effect_delete_effect", patterns::fn_special_effect_delete_effect, 0),
        ("upgrade_prosthetics_menu",    patterns::fn_upgrade_prosthetics_menu,    -57),
        ("open_skills_menu",            patterns::fn_open_skills_menu,            -57),
        ("warp_next_stage_bonfire",     patterns::fn_warp_next_stage_bonfire,     -25),
        ("add_experience",              patterns::fn_add_experience,              -15),
        ("enlarge_unk_hkb_array",       patterns::fn_enlarge_unk_hkb_array,       -63),
    ];

    let mut f = Functions::default();
    for (name, pat_fn, off) in entries {
        let pat = pat_fn();
        let scan_hit = match pat.scan(image) {
            Ok(o) => o,
            Err(_) => continue,
        };
        let abs = (module_base as isize + scan_hit as isize + *off) as usize;
        assign_function(&mut f, name, abs);
    }
    f
}

fn assign_function(f: &mut Functions, name: &str, addr: usize) {
    match name {
        "give_item_debug" => f.give_item_debug = Some(addr),
        "lua_do_string" => f.lua_do_string = Some(addr),
        "get_flag" => f.get_flag = Some(addr),
        "set_flag" => f.set_flag = Some(addr),
        "draw_debug_mesh" => f.draw_debug_mesh = Some(addr),
        "give_item" => f.give_item = Some(addr),
        "apply_effect" => f.apply_effect = Some(addr),
        "special_effect_delete_effect" => f.special_effect_delete_effect = Some(addr),
        "upgrade_prosthetics_menu" => f.upgrade_prosthetics_menu = Some(addr),
        "open_skills_menu" => f.open_skills_menu = Some(addr),
        "warp_next_stage_bonfire" => f.warp_next_stage_bonfire = Some(addr),
        "add_experience" => f.add_experience = Some(addr),
        "enlarge_unk_hkb_array" => f.enlarge_unk_hkb_array = Some(addr),
        _ => {}
    }
}

/// Pretty-report helper — for the overlay + inspector.
pub fn dump(natives: &Natives) -> Vec<(&'static str, Option<usize>)> {
    vec![
        ("WorldChrMan", natives.symbols.world_chr_man),
        ("WorldChrManDbg", natives.symbols.world_chr_man_dbg),
        ("WorldAiManager", natives.symbols.world_ai_manager),
        ("GameMan", natives.symbols.game_man),
        ("FieldArea", natives.symbols.field_area),
        ("SoloParamRepository", natives.symbols.solo_param_repository),
        ("GameData", natives.symbols.game_data),
        ("EventMan", natives.symbols.event_man),
        ("EventFlagMan", natives.symbols.event_flag_man),
        ("LockTgtMan", natives.symbols.lock_tgt_man),
        ("DamageManagement", natives.symbols.damage_management),
        ("MapItemMan", natives.symbols.map_item_man),
        ("Dlc", natives.symbols.dlc),
        ("DebugMenu", natives.symbols.debug_menu),
        ("RenderFlags", natives.symbols.render_flags),
        ("TargetingDrawFlags", natives.symbols.targeting_draw_flags),
        ("RendMan", natives.symbols.rend_man),
        ("fn GiveItemDebug", natives.functions.give_item_debug),
        ("fn LuaDoString", natives.functions.lua_do_string),
        ("fn GetFlag", natives.functions.get_flag),
        ("fn SetFlag", natives.functions.set_flag),
        ("fn DrawDebugMesh", natives.functions.draw_debug_mesh),
        ("fn GiveItem", natives.functions.give_item),
        ("fn ApplyEffect", natives.functions.apply_effect),
        ("fn SpecialEffectDeleteEffect", natives.functions.special_effect_delete_effect),
        ("fn UpgradeProstheticsMenu", natives.functions.upgrade_prosthetics_menu),
        ("fn OpenSkillsMenu", natives.functions.open_skills_menu),
        ("fn WarpNextStage_Bonfire", natives.functions.warp_next_stage_bonfire),
        ("fn AddExperience", natives.functions.add_experience),
        ("fn EnlargeUnkHkbArray", natives.functions.enlarge_unk_hkb_array),
    ]
}
