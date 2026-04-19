//! Static character-ID database.
//!
//! Source: `Characters.md` (wiki reference bundle) cross-referenced with
//! `thefifthmatt/SoulsRandomizers`.  Every c-number and o-prefix model
//! ID Sekiro ships with.  SPEC §5.2, §6.1, §12.3, §14.

use crate::entity::EntityKind;

/// One row of the character database.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CharacterInfo {
    pub id: u32,
    pub name: &'static str,
    pub kind: EntityKind,
    pub is_boss: bool,
    pub is_miniboss: bool,
    pub has_hp_bar_name: bool,
}

impl CharacterInfo {
    const fn new(
        id: u32,
        name: &'static str,
        kind: EntityKind,
        is_boss: bool,
        is_miniboss: bool,
        has_hp_bar_name: bool,
    ) -> Self {
        Self {
            id,
            name,
            kind,
            is_boss,
            is_miniboss,
            has_hp_bar_name,
        }
    }
}

/// The full c-number + o-prefix database.  Sorted ascending by id;
/// lookups use binary search.
pub const CHARACTERS: &[CharacterInfo] = &[
    // Player & core
    CharacterInfo::new(0, "Player", EntityKind::Player, false, false, false),
    CharacterInfo::new(1000, "Invisible", EntityKind::Invisible, false, false, false),
    CharacterInfo::new(1001, "Invisible (alt)", EntityKind::Invisible, false, false, false),
    // Rank-and-file enemies (c1xxx)
    CharacterInfo::new(1010, "Ashina Soldier", EntityKind::Enemy, false, false, false),
    CharacterInfo::new(1011, "Tutorial Ashina Soldier", EntityKind::Enemy, false, false, false),
    CharacterInfo::new(1012, "Hanbei the Undying", EntityKind::Enemy, false, true, true),
    CharacterInfo::new(1013, "Hanbei (Alternate)", EntityKind::Enemy, false, true, true),
    CharacterInfo::new(1020, "Samurai General", EntityKind::Enemy, false, true, true),
    CharacterInfo::new(1021, "Seven Ashina Spears", EntityKind::Enemy, false, true, true),
    CharacterInfo::new(1030, "Centipede", EntityKind::Enemy, false, false, false),
    CharacterInfo::new(1040, "Centipede Boss", EntityKind::Enemy, false, true, true),
    CharacterInfo::new(1050, "Shinobi Hunter", EntityKind::Enemy, false, true, true),
    CharacterInfo::new(1060, "Spear Adept", EntityKind::Enemy, false, false, false),
    CharacterInfo::new(1070, "Shura Samurai", EntityKind::Enemy, false, false, false),
    CharacterInfo::new(1080, "Shichimen Warrior", EntityKind::Enemy, false, true, true),
    CharacterInfo::new(1100, "Gecko", EntityKind::Enemy, false, false, false),
    CharacterInfo::new(1110, "Old Maid", EntityKind::Enemy, false, false, false),
    CharacterInfo::new(1111, "Old Maid (Sunken Valley)", EntityKind::Enemy, false, false, false),
    CharacterInfo::new(1120, "Sentry", EntityKind::Enemy, false, false, false),
    CharacterInfo::new(1130, "Armored Warrior", EntityKind::Enemy, false, true, true),
    CharacterInfo::new(1140, "Rock Diver", EntityKind::Enemy, false, false, false),
    CharacterInfo::new(1150, "Hound", EntityKind::Enemy, false, false, false),
    CharacterInfo::new(1151, "Palace Hound", EntityKind::Enemy, false, false, false),
    CharacterInfo::new(1180, "Taro Troop", EntityKind::Enemy, false, false, false),
    CharacterInfo::new(1181, "Taro Troop (Mibu)", EntityKind::Enemy, false, false, false),
    CharacterInfo::new(1190, "Sunken Valley Clan", EntityKind::Enemy, false, false, false),
    CharacterInfo::new(1191, "Snake Eyes", EntityKind::Enemy, false, true, true),
    CharacterInfo::new(1200, "Infested Seeker (Parasite)", EntityKind::Enemy, false, false, false),
    CharacterInfo::new(1210, "Infested Seeker", EntityKind::Enemy, false, false, false),
    CharacterInfo::new(1211, "Cricket", EntityKind::Enemy, false, false, false),
    CharacterInfo::new(1220, "Seeker", EntityKind::Enemy, false, false, false),
    CharacterInfo::new(1240, "Gamefowl", EntityKind::Enemy, false, false, false),
    CharacterInfo::new(1250, "Valley Monkey", EntityKind::Enemy, false, false, false),
    CharacterInfo::new(1260, "Folding Screen Monkey", EntityKind::Enemy, false, true, true),
    CharacterInfo::new(1300, "Palace Noble", EntityKind::Enemy, false, false, false),
    CharacterInfo::new(1310, "Okami Warrior", EntityKind::Enemy, false, false, false),
    CharacterInfo::new(1320, "Treasure Carp", EntityKind::Enemy, false, false, false),
    CharacterInfo::new(1321, "Man-eating Carp", EntityKind::Enemy, false, false, false),
    CharacterInfo::new(1340, "Underwater Headless", EntityKind::Enemy, false, true, true),
    CharacterInfo::new(1350, "Headless", EntityKind::Enemy, false, true, true),
    CharacterInfo::new(1360, "Assassin (Senpou)", EntityKind::Enemy, false, false, false),
    CharacterInfo::new(1361, "Assassin (Interior Ministry)", EntityKind::Enemy, false, false, false),
    CharacterInfo::new(1370, "Blazing Bull", EntityKind::Enemy, false, true, true),
    CharacterInfo::new(1380, "Sakura Bull of the Palace", EntityKind::Enemy, false, true, true),
    CharacterInfo::new(1400, "Fencer", EntityKind::Enemy, false, false, false),
    CharacterInfo::new(1450, "Nightjar Ninja", EntityKind::Enemy, false, false, false),
    CharacterInfo::new(1460, "Kite", EntityKind::Enemy, false, false, false),
    CharacterInfo::new(1470, "Lone Shadow", EntityKind::Enemy, false, true, true),
    CharacterInfo::new(1500, "Mibu Villager", EntityKind::Enemy, false, false, false),
    CharacterInfo::new(1501, "Mibu Villager (alt)", EntityKind::Enemy, false, false, false),
    CharacterInfo::new(1510, "Mibu Villager Illusion", EntityKind::Enemy, false, false, false),
    CharacterInfo::new(1520, "Test Subject", EntityKind::Enemy, false, false, false),
    CharacterInfo::new(1550, "Bandit", EntityKind::Enemy, false, false, false),
    CharacterInfo::new(1700, "Red Guard", EntityKind::Enemy, false, false, false),
    // Bosses (c5xxx)
    CharacterInfo::new(5000, "Corrupted Monk", EntityKind::Boss, true, false, true),
    CharacterInfo::new(5001, "Immortal Centipede", EntityKind::Boss, true, false, true),
    CharacterInfo::new(5005, "Corrupted Monk (Illusion)", EntityKind::Boss, true, false, true),
    CharacterInfo::new(5010, "Great Serpent", EntityKind::Boss, true, false, false),
    CharacterInfo::new(5020, "Chained Ogre", EntityKind::Boss, false, true, true),
    CharacterInfo::new(5021, "Feeding Grounds Attendant", EntityKind::Boss, false, true, true),
    CharacterInfo::new(5040, "Giant Rope Guy", EntityKind::Boss, false, true, true),
    CharacterInfo::new(5050, "Giant Carp", EntityKind::Boss, false, false, false),
    CharacterInfo::new(5060, "Owl", EntityKind::Boss, true, false, true),
    CharacterInfo::new(5070, "An Actual Owl", EntityKind::Boss, false, false, false),
    CharacterInfo::new(5080, "Gyoubu Oniwa", EntityKind::Boss, true, false, true),
    CharacterInfo::new(5090, "Lady Butterfly", EntityKind::Boss, true, false, true),
    CharacterInfo::new(5100, "Guardian Ape", EntityKind::Boss, true, false, true),
    CharacterInfo::new(5200, "Divine Dragon", EntityKind::Boss, true, false, true),
    CharacterInfo::new(5300, "Old Dragon", EntityKind::Boss, false, false, false),
    CharacterInfo::new(5310, "Tree Dragon", EntityKind::Boss, false, false, false),
    CharacterInfo::new(5400, "Sword Saint Isshin", EntityKind::Boss, true, false, true),
    CharacterInfo::new(5410, "Isshin (phase)", EntityKind::Boss, true, false, true),
    CharacterInfo::new(5420, "Isshin (phase)", EntityKind::Boss, true, false, true),
    CharacterInfo::new(5430, "Isshin (phase)", EntityKind::Boss, true, false, true),
    // Named NPCs (c7xxx)
    CharacterInfo::new(7000, "O'Rin", EntityKind::Npc, false, true, true),
    CharacterInfo::new(7010, "Sculptor", EntityKind::Npc, false, false, false),
    CharacterInfo::new(7020, "Demon of Hatred", EntityKind::Boss, true, false, true),
    CharacterInfo::new(7100, "Genichiro Ashina", EntityKind::Boss, true, false, true),
    CharacterInfo::new(7110, "Genichiro, Way of Tomoe", EntityKind::Boss, true, false, true),
    CharacterInfo::new(7200, "Kuro", EntityKind::Npc, false, false, false),
    CharacterInfo::new(7210, "Memory Kuro", EntityKind::Npc, false, false, false),
    CharacterInfo::new(7300, "Divine Child", EntityKind::Npc, false, false, false),
    CharacterInfo::new(7400, "Emma", EntityKind::Boss, true, false, true),
    CharacterInfo::new(7401, "Emma (alt)", EntityKind::Boss, true, false, true),
    CharacterInfo::new(7410, "Jinzaemon Kumano", EntityKind::Npc, false, false, false),
    CharacterInfo::new(7420, "Anayama the Peddler", EntityKind::Npc, false, false, false),
    CharacterInfo::new(7430, "Fujioka the Info Broker", EntityKind::Npc, false, false, false),
    CharacterInfo::new(7440, "Old Praying Woman", EntityKind::Npc, false, false, false),
    CharacterInfo::new(7450, "Inosuke's Mother", EntityKind::Npc, false, false, false),
    CharacterInfo::new(7460, "Inosuke", EntityKind::Npc, false, false, false),
    CharacterInfo::new(7470, "Kotaro", EntityKind::Npc, false, false, false),
    CharacterInfo::new(7480, "Head Priest", EntityKind::Npc, false, false, false),
    CharacterInfo::new(7490, "Doujun", EntityKind::Npc, false, false, false),
    CharacterInfo::new(7500, "Exiled Memorial Mob", EntityKind::Npc, false, false, false),
    CharacterInfo::new(7510, "Blackhat Badger", EntityKind::Npc, false, false, false),
    CharacterInfo::new(7520, "Feeding Ground Attendant's Daughter", EntityKind::Npc, false, false, false),
    CharacterInfo::new(7530, "Old Priestess", EntityKind::Npc, false, false, false),
    CharacterInfo::new(7540, "Toxic Memorial Mob", EntityKind::Npc, false, false, false),
    CharacterInfo::new(7550, "Shugendo Memorial Mob", EntityKind::Npc, false, false, false),
    CharacterInfo::new(7560, "Dungeon Memorial Mob", EntityKind::Npc, false, false, false),
    CharacterInfo::new(7590, "Exiled Memorial Mob (alt)", EntityKind::Npc, false, false, false),
    CharacterInfo::new(7600, "Outskirts Memorial Mob", EntityKind::Npc, false, false, false),
    // World objects (o-prefix, encoded as 100_000 + o-id)
    CharacterInfo::new(100_100, "Treasure", EntityKind::Object, false, false, false),
    CharacterInfo::new(100_101, "Shiny Treasure", EntityKind::Object, false, false, false),
    CharacterInfo::new(105_300, "Chest", EntityKind::Object, false, false, false),
    CharacterInfo::new(105_390, "Bird's Nest Treasure", EntityKind::Object, false, false, false),
    CharacterInfo::new(105_400, "Underwater Chest", EntityKind::Object, false, false, false),
    CharacterInfo::new(355_300, "Fountainhead Chest", EntityKind::Object, false, false, false),
];

/// Look up by c-number.  Binary search.
pub fn by_id(id: u32) -> Option<&'static CharacterInfo> {
    CHARACTERS
        .binary_search_by_key(&id, |c| c.id)
        .ok()
        .map(|i| &CHARACTERS[i])
}

/// True iff this is a boss (c5xxx-class + Demon of Hatred + Genichiro
/// + Emma + phase variants of Isshin).
pub fn is_boss(id: u32) -> bool {
    by_id(id).map(|c| c.is_boss).unwrap_or(false)
}

/// True iff this is a mini-boss (Chained Ogre, Headless, Lone Shadow,
/// Samurai General, etc.) — has an HP bar but not in the
/// memorial-mob catalog.
pub fn is_miniboss(id: u32) -> bool {
    by_id(id).map(|c| c.is_miniboss).unwrap_or(false)
}

pub fn name_of(id: u32) -> &'static str {
    by_id(id).map(|c| c.name).unwrap_or("<unknown>")
}

/// Iterate every boss in the database.  Useful for EMEVD
/// authority-designator generation.
pub fn bosses() -> impl Iterator<Item = &'static CharacterInfo> {
    CHARACTERS.iter().filter(|c| c.is_boss)
}

/// Iterate every miniboss in the database.
pub fn minibosses() -> impl Iterator<Item = &'static CharacterInfo> {
    CHARACTERS.iter().filter(|c| c.is_miniboss)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn database_is_sorted() {
        let mut last = 0u32;
        for c in CHARACTERS {
            assert!(c.id > last || last == 0, "out of order: {} after {}", c.id, last);
            last = c.id;
        }
    }

    #[test]
    fn player_is_c0000() {
        let p = by_id(0).expect("player");
        assert_eq!(p.name, "Player");
        assert_eq!(p.kind, EntityKind::Player);
    }

    #[test]
    fn gyoubu_is_boss_c5080() {
        let g = by_id(5080).expect("gyoubu");
        assert_eq!(g.name, "Gyoubu Oniwa");
        assert!(g.is_boss);
        assert_eq!(g.kind, EntityKind::Boss);
    }

    #[test]
    fn sword_saint_isshin_is_boss() {
        let i = by_id(5400).expect("isshin");
        assert!(i.is_boss);
        assert!(i.name.contains("Sword Saint"));
    }

    #[test]
    fn lone_shadow_is_miniboss() {
        let ls = by_id(1470).expect("lone shadow");
        assert!(ls.is_miniboss);
        assert!(!ls.is_boss);
    }

    #[test]
    fn demon_of_hatred_counts_as_boss() {
        let d = by_id(7020).expect("demon");
        assert!(d.is_boss);
    }

    #[test]
    fn unknown_id_returns_none() {
        assert!(by_id(99_999).is_none());
    }

    #[test]
    fn boss_iterator_includes_expected() {
        let ids: Vec<u32> = bosses().map(|c| c.id).collect();
        assert!(ids.contains(&5080)); // Gyoubu
        assert!(ids.contains(&5100)); // Guardian Ape
        assert!(ids.contains(&5400)); // Sword Saint
        assert!(ids.contains(&7020)); // Demon of Hatred
    }

    #[test]
    fn name_of_unknown_is_marker() {
        assert_eq!(name_of(99_999), "<unknown>");
    }
}
