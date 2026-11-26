//! What the map menu holds, as plain data (no Bevy app code, so it is
//! unit-tested in `tests/menu.rs`): the two lists, which tab and entry are
//! chosen, the index file of all maps, and how a pointer's press, drag and
//! release are told apart (a tap picks, a drag scrolls), and the rows of
//! the options tab.

use bevy::ecs::resource::Resource;
use bevy::math::Vec2;

use crate::settings::{ControlScheme, Settings};

/// The tabs of the menu.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Tab {
    /// The maps of `MapList.xml`, in the campaign's order.
    Campaign,
    /// Every single-player map there is, by name.
    All,
    /// The settings ([`OptionRow`]s), not maps.
    Options,
}

impl Tab {
    /// The tabs that list maps.
    pub const BOTH: [Tab; 2] = [Tab::Campaign, Tab::All];
    pub const ALL: [Tab; 3] = [Tab::Campaign, Tab::All, Tab::Options];

    pub fn title(self) -> &'static str {
        match self {
            Tab::Campaign => "Campaign",
            Tab::All => "All maps",
            Tab::Options => "Options",
        }
    }

    /// A tab by its name (`campaign`, `all`, `options`; any case).
    pub fn parse(name: &str) -> Option<Tab> {
        Tab::ALL.into_iter().find(|t| t.title().eq_ignore_ascii_case(name) || format!("{t:?}").eq_ignore_ascii_case(name))
    }

    fn index(self) -> usize {
        self as usize
    }
}

/// A row of the options tab.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum OptionRow {
    Controls,
    AutoFire,
    AutoAim,
    TapToGo,
    Music,
    Language,
}

impl OptionRow {
    pub const ALL: [OptionRow; 6] =
        [OptionRow::Controls, OptionRow::AutoFire, OptionRow::AutoAim, OptionRow::TapToGo, OptionRow::Music, OptionRow::Language];

    pub fn name(self) -> &'static str {
        match self {
            OptionRow::Controls => "Controls",
            OptionRow::AutoFire => "Auto-fire",
            OptionRow::AutoAim => "Auto-aim",
            OptionRow::TapToGo => "Tap to go",
            OptionRow::Music => "Music",
            OptionRow::Language => "Language",
        }
    }

    /// The small line under the name.
    pub fn hint(self) -> &'static str {
        match self {
            OptionRow::Controls => "Smart: one finger (not built yet, plays as Classic)",
            OptionRow::AutoFire => "Smart: fire while a target is locked",
            OptionRow::AutoAim => "Smart: turn the gun on the locked target",
            OptionRow::TapToGo => "Smart: double-tap the ground to drive there",
            OptionRow::Music => "The level music (M while playing)",
            OptionRow::Language => "The maps' texts; shown from the next start",
        }
    }

    /// The value as the row shows it.
    pub fn value(self, settings: &Settings) -> String {
        let on = |b: bool| if b { "On" } else { "Off" }.to_string();
        match self {
            OptionRow::Controls => settings.control_scheme.label().to_string(),
            OptionRow::AutoFire => on(settings.auto_fire),
            OptionRow::AutoAim => on(settings.auto_aim),
            OptionRow::TapToGo => on(settings.tap_to_go),
            OptionRow::Music => on(settings.music),
            OptionRow::Language => settings.language_name().to_string(),
        }
    }

    /// The smart controls' options do nothing while the scheme is
    /// `Classic`: the row is shown dimmed (still changeable, so the choice
    /// is there when the player switches).
    pub fn dimmed(self, settings: &Settings) -> bool {
        matches!(self, OptionRow::AutoFire | OptionRow::AutoAim | OptionRow::TapToGo)
            && settings.control_scheme != ControlScheme::Smart
    }

    /// Changes the value one step (`dir` 1 or -1; a switch flips either
    /// way).
    pub fn adjust(self, settings: &mut Settings, dir: i32) {
        match self {
            OptionRow::Controls => {
                let all = ControlScheme::BOTH;
                let at = all.iter().position(|c| *c == settings.control_scheme).unwrap_or(0) as i32;
                settings.control_scheme = all[(at + dir).rem_euclid(all.len() as i32) as usize];
            }
            OptionRow::AutoFire => settings.auto_fire = !settings.auto_fire,
            OptionRow::AutoAim => settings.auto_aim = !settings.auto_aim,
            OptionRow::TapToGo => settings.tap_to_go = !settings.tap_to_go,
            OptionRow::Music => settings.set_music(!settings.music),
            OptionRow::Language => settings.cycle_language(dir),
        }
    }
}

/// The text of `Maps/AllMaps.txt` (one map name a line, written by
/// `tools/import_content.py`) as a list: blank lines dropped, sorted by name
/// ignoring case, each name once.
pub fn parse_index(text: &str) -> Vec<String> {
    let mut names: Vec<String> = text.lines().map(str::trim).filter(|l| !l.is_empty()).map(String::from).collect();
    names.sort_by(|a, b| a.to_lowercase().cmp(&b.to_lowercase()).then_with(|| a.cmp(b)));
    names.dedup();
    names
}

/// The lists and the choice in them.
#[derive(Resource, Clone, Debug)]
pub struct MenuModel {
    lists: [Vec<String>; 2],
    tab: Tab,
    selected: [usize; 3],
}

impl MenuModel {
    pub fn new(campaign: Vec<String>, all: Vec<String>) -> Self {
        MenuModel { lists: [campaign, all], tab: Tab::Campaign, selected: [0; 3] }
    }

    pub fn tab(&self) -> Tab {
        self.tab
    }

    /// The maps of a list (none for the options tab).
    pub fn entries(&self, tab: Tab) -> &[String] {
        self.lists.get(tab.index()).map_or(&[], Vec::as_slice)
    }

    /// How many rows a tab has.
    pub fn len(&self, tab: Tab) -> usize {
        match tab {
            Tab::Options => OptionRow::ALL.len(),
            _ => self.entries(tab).len(),
        }
    }

    pub fn is_empty(&self, tab: Tab) -> bool {
        self.len(tab) == 0
    }

    /// The option row chosen on the options tab.
    pub fn chosen_option(&self) -> OptionRow {
        OptionRow::ALL[self.selected[Tab::Options.index()].min(OptionRow::ALL.len() - 1)]
    }

    pub fn selected(&self, tab: Tab) -> usize {
        self.selected[tab.index()]
    }

    pub fn set_tab(&mut self, tab: Tab) {
        self.tab = tab;
    }

    /// The tab `step` tabs on (it wraps).
    pub fn switch_tab(&mut self, step: i32) {
        let n = Tab::ALL.len() as i32;
        self.tab = Tab::ALL[(self.tab.index() as i32 + step).rem_euclid(n) as usize];
    }

    /// The chosen entry of the shown list moves by `step`, stopping at the ends.
    pub fn step(&mut self, step: i32) {
        let len = self.len(self.tab);
        if len == 0 {
            return;
        }
        let i = (self.selected[self.tab.index()] as i64 + step as i64).clamp(0, len as i64 - 1);
        self.selected[self.tab.index()] = i as usize;
    }

    /// Chooses an entry of a list (the one tapped), and shows that list.
    pub fn select(&mut self, tab: Tab, index: usize) {
        if index < self.len(tab) {
            self.tab = tab;
            self.selected[tab.index()] = index;
        }
    }

    /// The map chosen in the shown list.
    pub fn chosen(&self) -> Option<&str> {
        self.entries(self.tab).get(self.selected(self.tab)).map(String::as_str)
    }

    /// Puts each list's choice on `map` where it has it (the map being
    /// played, when the menu opens over it), showing the campaign if it is
    /// there, else the full list.
    pub fn focus_on(&mut self, map: &str) {
        let mut shown = None;
        for tab in Tab::BOTH {
            if let Some(i) = self.entries(tab).iter().position(|m| m == map) {
                self.selected[tab.index()] = i;
                shown.get_or_insert(tab);
            }
        }
        if let Some(tab) = shown {
            self.tab = tab;
        }
    }
}

/// How far a pointer may move (logical pixels) and still be a tap.
pub const TAP_SLOP: f32 = 10.0;

/// One press of a finger or the mouse: a tap until it moves past
/// [`TAP_SLOP`], a drag after.
#[derive(Clone, Copy, Debug)]
pub struct Gesture {
    start: Vec2,
    last: Vec2,
    dragging: bool,
}

impl Gesture {
    pub fn press(at: Vec2) -> Self {
        Gesture { start: at, last: at, dragging: false }
    }

    /// The pointer is at `at` now: how far to scroll the list by (the way
    /// the content follows the finger: a pointer moving up scrolls on, a
    /// positive amount), zero for a press that is still a tap.
    pub fn moved(&mut self, at: Vec2) -> f32 {
        if !self.dragging && (at - self.start).length() > TAP_SLOP {
            self.dragging = true;
            // The slop itself is not lost: the list catches up with the finger.
            self.last = self.start;
        }
        let scroll = if self.dragging { self.last.y - at.y } else { 0.0 };
        self.last = at;
        scroll
    }

    /// The pointer is up: where, if it was a tap.
    pub fn released(self) -> Option<Vec2> {
        (!self.dragging).then_some(self.last)
    }
}
