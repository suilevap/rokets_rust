//! What the map menu holds, as plain data (no Bevy app code, so it is
//! unit-tested in `tests/menu.rs`): the two lists, which tab and entry are
//! chosen, the index file of all maps, and how a pointer's press, drag and
//! release are told apart (a tap picks, a drag scrolls).

use bevy::ecs::resource::Resource;
use bevy::math::Vec2;

/// The two lists of the menu.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Tab {
    /// The maps of `MapList.xml`, in the campaign's order.
    Campaign,
    /// Every single-player map there is, by name.
    All,
}

impl Tab {
    pub const BOTH: [Tab; 2] = [Tab::Campaign, Tab::All];

    pub fn title(self) -> &'static str {
        match self {
            Tab::Campaign => "Campaign",
            Tab::All => "All maps",
        }
    }

    fn index(self) -> usize {
        self as usize
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
    selected: [usize; 2],
}

impl MenuModel {
    pub fn new(campaign: Vec<String>, all: Vec<String>) -> Self {
        MenuModel { lists: [campaign, all], tab: Tab::Campaign, selected: [0; 2] }
    }

    pub fn tab(&self) -> Tab {
        self.tab
    }

    pub fn entries(&self, tab: Tab) -> &[String] {
        &self.lists[tab.index()]
    }

    pub fn selected(&self, tab: Tab) -> usize {
        self.selected[tab.index()]
    }

    pub fn set_tab(&mut self, tab: Tab) {
        self.tab = tab;
    }

    /// The other tab, `step` tabs on (there are two; it wraps).
    pub fn switch_tab(&mut self, step: i32) {
        if step % 2 != 0 {
            self.tab = if self.tab == Tab::Campaign { Tab::All } else { Tab::Campaign };
        }
    }

    /// The chosen entry of the shown list moves by `step`, stopping at the ends.
    pub fn step(&mut self, step: i32) {
        let len = self.entries(self.tab).len();
        if len == 0 {
            return;
        }
        let i = (self.selected[self.tab.index()] as i64 + step as i64).clamp(0, len as i64 - 1);
        self.selected[self.tab.index()] = i as usize;
    }

    /// Chooses an entry of a list (the one tapped), and shows that list.
    pub fn select(&mut self, tab: Tab, index: usize) {
        if index < self.entries(tab).len() {
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
