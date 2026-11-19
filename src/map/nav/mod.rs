//! Navigation: where a car can go and how to get there. Pure algorithms,
//! unit-tested without the app (`tests/nav.rs`).
//!
//! C# origin: `AI/Navigation/*` (`NavGraph`, `NavNode`, `NavLink`, `NavArea`,
//! `NavPath`) and the part of `Helper/DataMap.cs` that cuts the map into
//! rectangles.
//!
//! | Module | What it is | C# |
//! |---|---|---|
//! | [`rects`] | the free cells cut into rectangles | `DataMap.ExtractAllRectangles` |
//! | [`graph`] | [`NavGraph`]: the graph made of them, `get_path`, weights, links | `NavGraph`, `NavNode`, `NavLink`, `NavArea` |
//! | [`path`] | [`NavPath`]: a path and the carrot that runs along it | `NavPath` |
//!
//! The Bevy side (the resource built at map load, doors' links, the `SetPath`
//! action) is in [`crate::ai::navigation`].

pub mod graph;
pub mod path;
pub mod rects;

pub use graph::{Blocker, LinkRef, NavGraph, NavLink, NavNode, NodeId, Orientation};
pub use path::NavPath;
