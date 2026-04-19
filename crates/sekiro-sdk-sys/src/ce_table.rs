//! Cielos CE table XML parser.
//!
//! Cheat Engine `.CT` files are XML trees of `<CheatEntry>` nodes.
//! Each entry has a description (the field name), an address or base
//! symbol, and an `<Offsets>` list forming a pointer chain.  For
//! ChrIns, we want the final offset — the one applied to the
//! `WorldChrMan → chr_list → ChrIns[i]` base.
//!
//! Usage: drop `sekiro-coop-chrins.xml` (exported CE table) next to the
//! DLL; on startup, call [`load_chrins_layout_from_path`] to populate
//! [`crate::chrins::ChrInsLayout`].
//!
//! **P0 gap #1** (SPEC §11): the *contents* of the XML still need to
//! come from someone with the Cielos table; this module just parses
//! whatever's dropped next to the DLL.

use crate::chrins::{ChrInsLayout, UNRESOLVED};
use quick_xml::events::Event;
use quick_xml::Reader;
use std::collections::HashMap;
use std::path::Path;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum CeError {
    #[error("i/o: {0}")]
    Io(#[from] std::io::Error),
    #[error("xml: {0}")]
    Xml(#[from] quick_xml::Error),
    #[error("encoding: {0}")]
    Utf8(#[from] std::str::Utf8Error),
    #[error("no ChrIns group found in CE table")]
    NoChrInsGroup,
}

/// Flat field map: description → final offset (bytes from start of ChrIns).
#[derive(Debug, Clone, Default)]
pub struct ChrInsFields {
    pub offsets: HashMap<String, usize>,
}

impl ChrInsFields {
    pub fn new() -> Self {
        Self::default()
    }

    /// Normalise a description like "HP", "Max HP", "Health", etc. to the
    /// canonical ChrInsLayout field name.  Case-insensitive; strips
    /// whitespace and punctuation.
    fn canonical(description: &str) -> Option<&'static str> {
        let simple: String = description
            .chars()
            .filter(|c| !c.is_whitespace() && *c != '_' && *c != '-')
            .collect::<String>()
            .to_ascii_lowercase();
        Some(match simple.as_str() {
            "entityid" | "entityhandle" | "handle" => "entity_id",
            "charid" | "characterid" | "modelid" => "char_id",
            "hp" | "currenthp" | "health" => "hp",
            "maxhp" | "maxhealth" => "max_hp",
            "posture" | "currentposture" => "posture",
            "maxposture" => "max_posture",
            "animation" | "animationid" | "currentanim" | "animid" => "animation_id",
            "animationframe" | "animframe" | "animationtime" => "animation_frame",
            "position" | "coords" | "xyz" => "position",
            "rotation" | "quat" | "orientation" => "rotation",
            "velocity" | "vel" => "velocity",
            "team" | "teamtype" => "team_type",
            "targetlock" | "lockon" | "targetentity" => "target_lock",
            "aicommand" | "currentaicommand" => "ai_command",
            "aislot" | "aicommandslot" => "ai_slot",
            "isdeflecting" | "deflecting" | "deflectstate" => "is_deflecting",
            "networkauthority" | "authority" => "network_authority",
            _ => return None,
        })
    }

    /// Populate a [`ChrInsLayout`] from this field map.  Leaves
    /// untouched fields as `UNRESOLVED`.
    pub fn apply(&self, layout: &mut ChrInsLayout) -> u32 {
        let mut hits = 0;
        for (k, off) in &self.offsets {
            let canonical = match Self::canonical(k) {
                Some(v) => v,
                None => continue,
            };
            match canonical {
                "entity_id" => layout.entity_id = *off,
                "char_id" => layout.char_id = *off,
                "hp" => layout.hp = *off,
                "max_hp" => layout.max_hp = *off,
                "posture" => layout.posture = *off,
                "max_posture" => layout.max_posture = *off,
                "animation_id" => layout.animation_id = *off,
                "animation_frame" => layout.animation_frame = *off,
                "position" => layout.position = *off,
                "rotation" => layout.rotation = *off,
                "velocity" => layout.velocity = *off,
                "team_type" => layout.team_type = *off,
                "target_lock" => layout.target_lock = *off,
                "ai_command" => layout.ai_command = *off,
                "ai_slot" => layout.ai_slot = *off,
                "is_deflecting" => layout.is_deflecting = *off,
                "network_authority" => layout.network_authority = *off,
                _ => continue,
            }
            hits += 1;
        }
        hits
    }
}

/// Parse the Cielos CE table XML string.  Returns a map of
/// `description → final offset` for entries that belong to a ChrIns
/// group (GroupHeader whose description contains "ChrIns").
pub fn parse_ce_table(xml: &str) -> Result<ChrInsFields, CeError> {
    let mut reader = Reader::from_str(xml);
    reader.trim_text(true);

    let mut buf = Vec::new();
    let mut stack: Vec<EntryFrame> = Vec::new();
    let mut fields = ChrInsFields::new();

    let mut current_tag: Option<Tag> = None;
    let mut current_text = String::new();

    loop {
        match reader.read_event_into(&mut buf)? {
            Event::Start(e) => match e.name().as_ref() {
                b"CheatEntry" => {
                    stack.push(EntryFrame::default());
                    current_tag = None;
                }
                b"Description" => current_tag = Some(Tag::Description),
                b"Address" => current_tag = Some(Tag::Address),
                b"Offsets" => current_tag = Some(Tag::Offsets),
                b"Offset" => current_tag = Some(Tag::Offset),
                b"GroupHeader" => current_tag = Some(Tag::GroupHeader),
                _ => current_tag = None,
            },
            Event::Empty(e) => match e.name().as_ref() {
                b"GroupHeader" => {
                    if let Some(top) = stack.last_mut() {
                        top.is_group = true;
                    }
                }
                _ => {}
            },
            Event::Text(t) => {
                // Only accumulate when we're inside a tag we're tracking;
                // otherwise stray text (e.g. <VariableType>4 Bytes</…>)
                // would leak into the next tracked tag.
                if current_tag.is_some() {
                    let s = t.unescape().unwrap_or_default().to_string();
                    current_text.push_str(&s);
                }
            }
            Event::End(e) => {
                match e.name().as_ref() {
                    b"Description" => {
                        if let Some(top) = stack.last_mut() {
                            top.description = current_text.trim().trim_matches('"').to_string();
                        }
                        current_text.clear();
                    }
                    b"Address" => {
                        if let Some(top) = stack.last_mut() {
                            top.address = current_text.trim().to_string();
                        }
                        current_text.clear();
                    }
                    b"Offset" => {
                        if let Some(top) = stack.last_mut() {
                            top.last_offset = parse_hex(current_text.trim());
                        }
                        current_text.clear();
                    }
                    b"GroupHeader" => {
                        if let Some(top) = stack.last_mut() {
                            top.is_group = true;
                        }
                    }
                    b"CheatEntry" => {
                        let frame = stack.pop().unwrap_or_default();
                        // A leaf entry belongs to ChrIns when any ancestor
                        // frame still on the stack is a GroupHeader whose
                        // description contains "chrins".
                        if !frame.is_group {
                            let under_chrins = stack.iter().any(|p| {
                                p.is_group && p.description.to_lowercase().contains("chrins")
                            });
                            if under_chrins {
                                let off = frame.last_offset.or_else(|| {
                                    parse_hex(frame.address.trim_start_matches('+'))
                                });
                                if let Some(off) = off {
                                    fields.offsets.insert(frame.description.clone(), off);
                                }
                            }
                        }
                    }
                    _ => {}
                }
                current_tag = None;
            }
            Event::Eof => break,
            _ => {}
        }
        buf.clear();
    }

    Ok(fields)
}

/// Read + parse + apply to a [`ChrInsLayout`].  Returns the number of
/// fields successfully resolved.
pub fn load_chrins_layout_from_path(
    path: impl AsRef<Path>,
    layout: &mut ChrInsLayout,
) -> Result<u32, CeError> {
    let xml = std::fs::read_to_string(path)?;
    let fields = parse_ce_table(&xml)?;
    Ok(fields.apply(layout))
}

#[derive(Debug, Default, Clone)]
struct EntryFrame {
    description: String,
    address: String,
    last_offset: Option<usize>,
    is_group: bool,
}

#[derive(Debug, Clone, Copy)]
enum Tag {
    Description,
    Address,
    Offsets,
    Offset,
    GroupHeader,
}

fn parse_hex(s: &str) -> Option<usize> {
    let s = s.trim();
    let s = s.strip_prefix('+').unwrap_or(s);
    let (radix, cleaned) = if let Some(r) = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
        (16, r)
    } else if s.chars().all(|c| c.is_ascii_digit()) {
        (10, s)
    } else {
        (16, s)
    };
    usize::from_str_radix(cleaned, radix).ok()
}

/// True iff the layout has every field populated (i.e. every field
/// offset is no longer `UNRESOLVED`).
pub fn layout_fully_populated(layout: &ChrInsLayout) -> bool {
    layout.entity_id != UNRESOLVED
        && layout.char_id != UNRESOLVED
        && layout.hp != UNRESOLVED
        && layout.max_hp != UNRESOLVED
        && layout.posture != UNRESOLVED
        && layout.max_posture != UNRESOLVED
        && layout.animation_id != UNRESOLVED
        && layout.animation_frame != UNRESOLVED
        && layout.position != UNRESOLVED
        && layout.rotation != UNRESOLVED
        && layout.velocity != UNRESOLVED
        && layout.team_type != UNRESOLVED
        && layout.target_lock != UNRESOLVED
        && layout.ai_command != UNRESOLVED
        && layout.ai_slot != UNRESOLVED
        && layout.is_deflecting != UNRESOLVED
        && layout.network_authority != UNRESOLVED
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE: &str = r#"
<CheatTable>
  <CheatEntries>
    <CheatEntry>
      <Description>"ChrIns"</Description>
      <GroupHeader/>
      <CheatEntries>
        <CheatEntry>
          <Description>"HP"</Description>
          <VariableType>4 Bytes</VariableType>
          <Address>WorldChrMan</Address>
          <Offsets>
            <Offset>0x1F90</Offset>
          </Offsets>
        </CheatEntry>
        <CheatEntry>
          <Description>"Max HP"</Description>
          <VariableType>4 Bytes</VariableType>
          <Offsets>
            <Offset>0x1F94</Offset>
          </Offsets>
        </CheatEntry>
        <CheatEntry>
          <Description>"Posture"</Description>
          <Offsets>
            <Offset>0x1FA8</Offset>
          </Offsets>
        </CheatEntry>
        <CheatEntry>
          <Description>"Animation ID"</Description>
          <Offsets>
            <Offset>0x6B4</Offset>
          </Offsets>
        </CheatEntry>
      </CheatEntries>
    </CheatEntry>
    <CheatEntry>
      <Description>"UnrelatedThing"</Description>
      <Offsets>
        <Offset>0xFFFF</Offset>
      </Offsets>
    </CheatEntry>
  </CheatEntries>
</CheatTable>
"#;

    #[test]
    fn parses_chrins_entries() {
        let fields = parse_ce_table(FIXTURE).expect("parse");
        assert_eq!(fields.offsets.get("HP").copied(), Some(0x1F90));
        assert_eq!(fields.offsets.get("Max HP").copied(), Some(0x1F94));
        assert_eq!(fields.offsets.get("Posture").copied(), Some(0x1FA8));
        assert_eq!(fields.offsets.get("Animation ID").copied(), Some(0x6B4));
        // "UnrelatedThing" is outside the ChrIns group — should not appear.
        assert!(!fields.offsets.contains_key("UnrelatedThing"));
    }

    #[test]
    fn applies_to_layout() {
        let fields = parse_ce_table(FIXTURE).expect("parse");
        let mut layout = ChrInsLayout::unresolved();
        let hits = fields.apply(&mut layout);
        assert_eq!(hits, 4);
        assert_eq!(layout.hp, 0x1F90);
        assert_eq!(layout.max_hp, 0x1F94);
        assert_eq!(layout.posture, 0x1FA8);
        assert_eq!(layout.animation_id, 0x6B4);
        assert_eq!(layout.entity_id, UNRESOLVED); // untouched
    }

    #[test]
    fn hex_parse_tolerates_formats() {
        assert_eq!(parse_hex("0x80"), Some(0x80));
        assert_eq!(parse_hex("0X1F"), Some(0x1F));
        assert_eq!(parse_hex("+0x40"), Some(0x40));
        assert_eq!(parse_hex("FF"), Some(0xFF));
        assert_eq!(parse_hex("128"), Some(128));
        assert_eq!(parse_hex("garbage"), None);
    }

    #[test]
    fn canonical_name_normalisation() {
        assert_eq!(ChrInsFields::canonical("HP"), Some("hp"));
        assert_eq!(ChrInsFields::canonical("Max HP"), Some("max_hp"));
        assert_eq!(ChrInsFields::canonical("Character ID"), Some("char_id"));
        assert_eq!(ChrInsFields::canonical("Animation_ID"), Some("animation_id"));
        assert_eq!(ChrInsFields::canonical("unknown field"), None);
    }
}
