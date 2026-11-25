//! The navigation graph: nodes where the map's free rectangles meet, links
//! between them, and the path search over them.
//!
//! C# origin: `AI/Navigation/NavGraph.cs`, `NavNode.cs`, `NavLink.cs`,
//! `NavArea.cs` (2013–2014), and `MapManager`'s `_navigation` (built once the
//! distance field is, `BuildFromDistanceField(DistanceField, CellSize, (x, y)
//! => MapImage.GetPixelSafe(x, y).A != 0)`).
//!
//! # How it is built
//!
//! The free cells (distance field at least 0, and drawn: alpha not 0) are cut
//! into rectangles ([`super::rects`]), the big ones (more than one cell wide
//! or tall) first. Each rectangle is an *area*, grown by half a cell on every
//! side (the work is done in half cells) so neighbours overlap:
//!
//! 1. every two big areas whose overlap is more than 2 half cells wide or
//!    tall get a node in the middle of the overlap (`Horizontal` when the
//!    overlap is taller than wide, else `Vertical`: the way a car passes
//!    through it), listed in both areas;
//! 2. each one-cell area is tied to the big ones it overlaps (more than 2
//!    half cells): with none it is left out; with one it makes a node at its
//!    own middle between the two; with two that are not already joined it
//!    makes a node on each overlap and links them; when the two are joined, or
//!    there are more, it is *attached* to the first (it shares that area's node
//!    list and counts as part of it);
//! 3. in each big area the nodes are linked: every pair directly when the area
//!    has three nodes or fewer, or when it is not worth going through the
//!    middle (the direct way is under 1.1 times the way through the area's
//!    centre, or one of them leads to no other area); otherwise both are
//!    linked to a central node added there.
//!
//! A link's length is the distance between its nodes times the weight of the
//! node it leaves (`AddLink`, `SetWeight`).
//!
//! # The search
//!
//! [`NavGraph::get_path`]: two points in the same area get the straight path.
//! Otherwise the nodes of the start's area are the start, each at its
//! distance times its weight (at least 0.001, which marks it as reached), and
//! a first-in-first-out relaxation (not A\*: no heuristic, no priority queue;
//! a node is queued again whenever a shorter way to it is found) spreads over
//! the links until it reaches a node of the end's area; of those the one with
//! the shortest total, the last leg being the distance to the end times the
//! node's weight, is where the path ends. Following the links back builds the
//! points: a `Horizontal` or `Vertical` node adds two points half a cell
//! either side along its axis, so the car crosses the overlap straight; any
//! other adds its own. The links the path takes that are blocked (a closed
//! door's) are listed in the path; the search itself ignores `blocked`, so a
//! blocked way is found and left for the caller to refuse.
//!
//! Positions are world units with y up, as everywhere in the port (C# kept
//! them with y down; every formula that depended on the sign of y is the same
//! under the flip, see `units.rs`). Only the area lookup knows the map's rows
//! go down.
//!
//! # Not ported
//!
//! - `NavGraph.Build(points, dist)` and `_maxDSq` (the commented-out
//!   `IsLinkPossible` by distance and line of sight: the free-points graph
//!   the area graph replaced; nothing calls `Build`);
//! - `IsOnOneSide`, `GetConectedAreasThroughNode`'s two unused lists, the
//!   `ResizeArrays` scratch (the search allocates its own per call);
//! - `NavNode.Draw`, `NavLink.Draw`, `NavGraph.Draw`, `DebugGetPath` (debug
//!   drawing, see `ai::navigation`'s debug lines) and the `DEBUG && AI`
//!   rectangle drawing in `BuildFromDistanceField`;
//! - a link that already exists is not added again (`AddLink` returned null);
//!   `LinkPoints` then would throw a `NullReferenceException` in C#, here the
//!   link is just left out of the list it returns;
//! - the walk back from the last node is capped at the number of nodes
//!   (a cycle in the back links, which the search cannot make, would hang C#);
//! - fake walls (`FreePoint` counted them as free in the distance field) are
//!   not in the port's grid.

use std::collections::VecDeque;

use bevy::math::Vec2;

use super::path::NavPath;
use super::rects::{IRect, extract_all_rectangles};
use crate::map::grid::MapGrid;

/// An index into [`NavGraph::nodes`] (`NavNode.Id`).
pub type NodeId = usize;
/// An area's id: 1 and up, 0 is "no area" (`_areasId`'s default).
pub type AreaId = usize;

/// What a blocked link belongs to (`IBlockerComponent`: the door that closes
/// it). The graph only keeps the number; the game says what it stands for.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Blocker(pub u64);

/// `NavNode.OrientationType`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Orientation {
    /// Crossed along x.
    Horizontal,
    /// Crossed along y.
    Vertical,
    None,
}

/// `NavLink`: a way from the node that holds it to `to`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NavLink {
    /// `Node2`.
    pub to: NodeId,
    pub distance: f32,
    /// `RelatedEntityComponent`.
    pub blocker: Option<Blocker>,
    pub blocked: bool,
}

/// `NavNode`.
#[derive(Clone, Debug)]
pub struct NavNode {
    pub id: NodeId,
    pub pos: Vec2,
    pub links: Vec<NavLink>,
    pub areas: Vec<AreaId>,
    pub weight: f32,
    pub orientation: Orientation,
}

impl NavNode {
    /// `AddLink`: a link to `to` of length `value` × this node's weight,
    /// unless there is one already.
    fn add_link(&mut self, to: NodeId, value: f32) -> Option<usize> {
        if self.links.iter().any(|l| l.to == to) {
            return None;
        }
        self.links.push(NavLink { to, distance: value * self.weight, blocker: None, blocked: false });
        Some(self.links.len() - 1)
    }

    /// `SetWeight`: the lengths of the links it leaves change by the ratio.
    fn set_weight(&mut self, new_weight: f32) {
        if new_weight != self.weight {
            let rate = new_weight / self.weight;
            for link in &mut self.links {
                link.distance *= rate;
            }
            self.weight = new_weight;
        }
    }
}

/// One link of the graph: the node it leaves and its place in that node's
/// links (what `LinkPoints` returned, to open or close it later).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LinkRef {
    pub node: NodeId,
    pub index: usize,
}

/// `NavArea`.
#[derive(Clone, Debug)]
struct NavArea {
    /// In half cells, grown by one on every side.
    rect: IRect,
    /// Which node list is this area's (`Nodes`): an attached area shares its
    /// host's.
    list: usize,
    /// `ConnectedAreas`.
    connected: Vec<AreaId>,
}

/// `_areasId`: which area each cell of the map belongs to (a `DataMap<int>`).
#[derive(Clone, Debug)]
struct AreaMap {
    width: f32,
    height: f32,
    cell: f32,
    columns: usize,
    rows: usize,
    data: Vec<AreaId>,
}

impl AreaMap {
    /// `new DataMap<int>((int)(w * scale), (int)(h * scale), scale)`.
    fn new(width_cells: usize, height_cells: usize, cell: f32) -> AreaMap {
        let width = (width_cells as f32 * cell) as i32 as f32;
        let height = (height_cells as f32 * cell) as i32 as f32;
        let columns = (width / cell) as usize + 1;
        let rows = (height / cell) as usize + 1;
        AreaMap { width, height, cell, columns, rows, data: vec![0; columns * rows] }
    }

    /// `GetItem(Vector2)`, for a world point (y up): the map's rows go down.
    fn at(&self, pos: Vec2) -> AreaId {
        let (x, y) = (pos.x, -pos.y);
        if x < 0.0 || y < 0.0 || x >= self.width || y >= self.height {
            return 0;
        }
        let (i, j) = ((x / self.cell) as usize, (y / self.cell) as usize);
        self.data.get(j * self.columns + i).copied().unwrap_or(0)
    }

    /// `FillRect`, in cells.
    fn fill(&mut self, rect: IRect, id: AreaId) {
        for y in rect.y..rect.bottom() {
            for x in rect.x..rect.right() {
                if x >= 0 && y >= 0 && (x as usize) < self.columns && (y as usize) < self.rows {
                    self.data[y as usize * self.columns + x as usize] = id;
                }
            }
        }
    }
}

/// `NavGraph`.
#[derive(Clone, Debug)]
pub struct NavGraph {
    /// `MapManager.CellSize`: a `Horizontal` or `Vertical` node is crossed
    /// half this far either side of it.
    cell: f32,
    nodes: Vec<NavNode>,
    /// Index 0 is no area.
    areas: Vec<Option<NavArea>>,
    /// The nodes of each area (`NavArea.Nodes`).
    lists: Vec<Vec<NodeId>>,
    area_map: AreaMap,
}

/// `CenterNodeThresholdDistanceRate`.
const CENTER_NODE_THRESHOLD_DISTANCE_RATE: f32 = 1.1;

impl NavGraph {
    /// The graph of a map: `BuildFromDistanceField` over the grid's
    /// distance field, with the cells that are drawn (alpha not 0) as the
    /// ones to cover. `cell` is the size of a cell in world units.
    pub fn from_grid(grid: &MapGrid, cell: f32) -> NavGraph {
        let distance = grid.chamfer_distance();
        NavGraph::build_from_distance_field(&distance, grid.width, grid.height, cell, |x, y| {
            grid.cell(x, y).is_some_and(|c| c.color.a != 0)
        })
    }

    /// `BuildFromDistanceField(distanceField, scale, predicate)`: `distance` is
    /// row-major, `width` cells to a row, and only its sign and order matter
    /// (free cells are 0 or more, the biggest rectangles start where it is
    /// largest); `cell` is the size of a cell in world units (`scale`),
    /// `usable` the predicate.
    pub fn build_from_distance_field(
        distance: &[f32],
        width: usize,
        height: usize,
        cell: f32,
        usable: impl Fn(i32, i32) -> bool,
    ) -> NavGraph {
        let mut graph = NavGraph {
            cell,
            nodes: Vec::new(),
            areas: vec![None],
            lists: Vec::new(),
            area_map: AreaMap::new(width, height, cell),
        };
        let rects = extract_all_rectangles(width, height, distance, |x, y, d| d >= 0.0 && usable(x, y));

        // From here the work is in half cells, which is `scale /= 2`.
        let half = cell / 2.0;
        // A point of the map (half cells, y down) in the world.
        let at = |(x, y): (i32, i32)| Vec2::new(x as f32 * half, -(y as f32) * half);

        let (mut big, mut small) = (Vec::new(), Vec::new());
        for mut r in rects {
            let id = graph.areas.len();
            graph.area_map.fill(r, id);
            if r.w > 1 || r.h > 1 {
                big.push(id);
            } else {
                small.push(id);
            }
            r.x *= 2;
            r.y *= 2;
            r.w *= 2;
            r.h *= 2;
            r.inflate(1, 1);
            graph.lists.push(Vec::new());
            graph.areas.push(Some(NavArea { rect: r, list: graph.lists.len() - 1, connected: Vec::new() }));
        }

        // Big areas that overlap meet at a node.
        for (i, &id1) in big.iter().enumerate() {
            let r1 = graph.rect(id1);
            for &id2 in &big[i + 1..] {
                let r2 = graph.rect(id2);
                if r1.intersects(&r2) {
                    let overlap = r1.intersection(&r2);
                    if overlap.w > 2 || overlap.h > 2 {
                        let node = graph.add_node(at(overlap.center()));
                        graph.nodes[node].orientation = orientation(overlap);
                        graph.join(node, id1, id2);
                    }
                }
            }
        }

        // One-cell areas: a node between the big ones they touch, or part of one.
        for &id1 in &small {
            let r1 = graph.rect(id1);
            let touched: Vec<AreaId> = big
                .iter()
                .copied()
                .filter(|&id2| {
                    let overlap = r1.intersection(&graph.rect(id2));
                    r1.intersects(&graph.rect(id2)) && (overlap.w > 2 || overlap.h > 2)
                })
                .collect();
            match touched.len() {
                0 => {}
                1 => {
                    let node = graph.add_node(at(r1.center()));
                    graph.join(node, id1, touched[0]);
                }
                2 => {
                    let (id2, id3) = (touched[0], touched[1]);
                    let list2 = graph.areas[id2].as_ref().expect("an area").list;
                    // Joined already if some node of the first leads to the second.
                    let joined = graph.lists[list2].iter().any(|&n| graph.nodes[n].areas.contains(&id3));
                    if joined {
                        graph.attach(id2, id1);
                    } else {
                        let overlap = r1.intersection(&graph.rect(id2));
                        let node1 = graph.add_node(at(overlap.center()));
                        graph.join(node1, id1, id2);
                        graph.nodes[node1].orientation = orientation(overlap);
                        let overlap = r1.intersection(&graph.rect(id3));
                        let node2 = graph.add_node(at(overlap.center()));
                        graph.join(node2, id1, id3);
                        graph.nodes[node2].orientation = orientation(overlap);
                        graph.link_between(node1, node2);
                    }
                }
                _ => graph.attach(touched[0], id1),
            }
        }

        for &id in &big {
            graph.connect_nodes(id, &at);
        }
        graph
    }

    fn rect(&self, area: AreaId) -> IRect {
        self.areas[area].as_ref().expect("an area").rect
    }

    /// `AddNode`.
    pub fn add_node(&mut self, pos: Vec2) -> NodeId {
        let id = self.nodes.len();
        self.nodes.push(NavNode { id, pos, links: Vec::new(), areas: Vec::new(), weight: 1.0, orientation: Orientation::None });
        id
    }

    /// A node that belongs to two areas, and the areas to each other.
    fn join(&mut self, node: NodeId, area1: AreaId, area2: AreaId) {
        for area in [area1, area2] {
            let list = self.areas[area].as_ref().expect("an area").list;
            self.lists[list].push(node);
            self.nodes[node].areas.push(area);
        }
        self.areas[area1].as_mut().expect("an area").connected.push(area2);
        self.areas[area2].as_mut().expect("an area").connected.push(area1);
    }

    /// `NavArea.AttachArea`: `small` shares `host`'s nodes.
    fn attach(&mut self, host: AreaId, small: AreaId) {
        let list = self.areas[host].as_ref().expect("an area").list;
        self.areas[small].as_mut().expect("an area").list = list;
        for &node in &self.lists[list] {
            self.nodes[node].areas.push(small);
        }
        self.areas[host].as_mut().expect("an area").connected.push(small);
    }

    /// `AddLinkBetween`: both ways, as long as the distance between them.
    fn link_between(&mut self, a: NodeId, b: NodeId) {
        let value = self.nodes[a].pos.distance(self.nodes[b].pos);
        self.nodes[a].add_link(b, value);
        self.nodes[b].add_link(a, value);
    }

    /// `NavArea.GetConectedAreasThroughNode`: how many areas the neighbours of
    /// `node`'s other areas lead to that `area` does not already.
    fn connected_areas_through(&self, area: AreaId, node: NodeId) -> usize {
        let own = &self.areas[area].as_ref().expect("an area").connected;
        self.nodes[node]
            .areas
            .iter()
            .filter(|&&a| a != area)
            .flat_map(|&a| self.areas[a].as_ref().expect("an area").connected.iter())
            .filter(|&&x| x != area && !own.contains(&x))
            .count()
    }

    /// `NavArea.ConnectNodes`: links the nodes of a big area to each other,
    /// by way of a central node where going through the middle is no worse.
    /// The list grows (with the central node) while it is walked, as in C#.
    fn connect_nodes(&mut self, area: AreaId, at: &impl Fn((i32, i32)) -> Vec2) {
        let (list, rect) = {
            let a = self.areas[area].as_ref().expect("an area");
            (a.list, a.rect)
        };
        if self.lists[list].is_empty() {
            return;
        }
        // `Nodes.Average`: summed as doubles.
        let n = self.lists[list].len() as f64;
        let (sx, sy) = self.lists[list]
            .iter()
            .fold((0.0f64, 0.0f64), |(sx, sy), &id| (sx + self.nodes[id].pos.x as f64, sy + self.nodes[id].pos.y as f64));
        let mean = Vec2::new((sx / n) as f32, (sy / n) as f32);
        let rect_center = at(rect.center());
        let center = mean + (rect_center - mean) * 0.5;

        let mut central: Option<NodeId> = None;
        let mut ni1 = 0;
        while ni1 < self.lists[list].len() {
            let node1 = self.lists[list][ni1];
            let connected1 = self.connected_areas_through(area, node1);
            let mut connected_to_center = false;
            let mut ni2 = ni1 + 1;
            while ni2 < self.lists[list].len() {
                let node2 = self.lists[list][ni2];
                let mut individual = self.lists[list].len() <= 3;
                let connected2 = self.connected_areas_through(area, node2);
                if !individual && connected1 > 0 && connected2 > 0 {
                    let (p1, p2) = (self.nodes[node1].pos, self.nodes[node2].pos);
                    let direct = p1.distance(p2);
                    let through = p1.distance(center) + p2.distance(center);
                    individual = direct * CENTER_NODE_THRESHOLD_DISTANCE_RATE < through;
                }
                if individual {
                    self.link_between(node1, node2);
                } else {
                    let middle = match central {
                        Some(c) => c,
                        None => {
                            let c = self.add_node(center);
                            self.nodes[c].areas.push(area);
                            self.lists[list].push(c);
                            central = Some(c);
                            c
                        }
                    };
                    if !connected_to_center {
                        connected_to_center = true;
                        self.link_between(middle, node1);
                    }
                    self.link_between(middle, node2);
                }
                ni2 += 1;
            }
            ni1 += 1;
        }
    }

    /// `NavGraph._nodes`.
    pub fn nodes(&self) -> &[NavNode] {
        &self.nodes
    }

    /// The area at a world point, or 0 (`_areasId.GetItem`).
    fn area_at(&self, pos: Vec2) -> AreaId {
        self.area_map.at(pos)
    }

    /// `IsLinkPossible(pos1, pos2)`: both in the same area.
    fn same_area(&self, a: Vec2, b: Vec2) -> bool {
        let (id1, id2) = (self.area_at(a), self.area_at(b));
        id1 == id2 && id1 != 0
    }

    /// `IsLinkPossible(pos, node)`: the node is in the area `pos` is in.
    fn in_area_of(&self, pos: Vec2, node: &NavNode) -> bool {
        let id = self.area_at(pos);
        id != 0 && node.areas.contains(&id)
    }

    /// `GetDistance(pos, node)`: the way there, times the node's weight.
    fn distance_to(pos: Vec2, node: &NavNode) -> f32 {
        pos.distance(node.pos) * node.weight
    }

    /// `GetNearest`: the nodes of the area the point is in; `None` outside
    /// every area.
    pub fn get_nearest(&self, pos: Vec2) -> Option<&[NodeId]> {
        let id = self.area_at(pos);
        if id > 0 {
            let list = self.areas[id].as_ref()?.list;
            Some(&self.lists[list])
        } else {
            None
        }
    }

    /// `GetNodes`: the nodes within `distance` of `pos` on both axes.
    pub fn get_nodes(&self, pos: Vec2, distance: f32) -> impl Iterator<Item = &NavNode> {
        self.nodes.iter().filter(move |n| {
            let d = pos - n.pos;
            d.x.abs() < distance && d.y.abs() < distance
        })
    }

    /// `SetWeight(weight, leftTop, rightBottom)`: every node inside the
    /// rectangle (edges included) takes the weight, a road made dear or cheap
    /// for the search. `min` and `max` are the rectangle's corners in the
    /// world (y up).
    pub fn set_weight(&mut self, weight: f32, min: Vec2, max: Vec2) {
        for node in &mut self.nodes {
            let p = node.pos;
            if p.x >= min.x && p.y >= min.y && p.x <= max.x && p.y <= max.y {
                node.set_weight(weight);
            }
        }
    }

    /// `AddPoint`: a node at `point`, linked to every node in the area the
    /// point is in.
    pub fn add_point(&mut self, point: Vec2) -> NodeId {
        let node = self.add_node(point);
        let known = self.nodes.len() - 1;
        for other in 0..known {
            if self.in_area_of(point, &self.nodes[other]) {
                self.link_between(node, other);
            }
        }
        node
    }

    /// `LinkPoints`: a link between two new nodes (both ways unless
    /// `one_way`) that `blocker` can close ([`set_link`](Self::set_link)).
    pub fn link_points(&mut self, a: Vec2, b: Vec2, one_way: bool, blocker: Option<Blocker>) -> Vec<LinkRef> {
        let node1 = self.add_point(a);
        let node2 = self.add_point(b);
        let distance = a.distance(b);
        let mut result = Vec::with_capacity(if one_way { 1 } else { 2 });
        let mut make = |graph: &mut NavGraph, from: NodeId, to: NodeId| {
            if let Some(index) = graph.nodes[from].add_link(to, distance) {
                graph.nodes[from].links[index].blocker = blocker;
                result.push(LinkRef { node: from, index });
            }
        };
        make(self, node1, node2);
        if !one_way {
            make(self, node2, node1);
        }
        result
    }

    /// Changes a link's length and whether it is blocked (what `Door`'s
    /// `OpenDoor` and `BreakNavigation` did to its links).
    pub fn set_link(&mut self, link: LinkRef, distance: f32, blocked: bool) {
        if let Some(l) = self.nodes.get_mut(link.node).and_then(|n| n.links.get_mut(link.index)) {
            l.distance = distance;
            l.blocked = blocked;
        }
    }

    /// `GetPath`: the way from `start` to `end`, or `None` when `start` is
    /// outside the areas or no node of its area leads to `end`'s.
    pub fn get_path(&self, start: Vec2, end: Vec2) -> Option<NavPath> {
        if self.same_area(start, end) {
            return Some(NavPath::new(vec![start, end], false));
        }
        let start_nodes = self.get_nearest(start)?;
        if start_nodes.is_empty() {
            return None;
        }
        let mut distances = vec![0.0f32; self.nodes.len()];
        let mut back: Vec<Option<LinkRef>> = vec![None; self.nodes.len()];
        let mut queue: VecDeque<NodeId> = VecDeque::new();
        for &id in start_nodes {
            // 0.001 marks the start nodes as reached.
            distances[id] = NavGraph::distance_to(start, &self.nodes[id]).max(0.001);
            queue.push_back(id);
        }
        let mut final_distance = f32::MAX;
        let mut final_node = None;
        while let Some(id) = queue.pop_front() {
            let node = &self.nodes[id];
            if self.in_area_of(end, node) {
                let d = distances[id] + NavGraph::distance_to(end, node);
                if d < final_distance {
                    final_distance = d;
                    final_node = Some(id);
                }
            } else {
                for (index, link) in node.links.iter().enumerate() {
                    let d = distances[id] + link.distance;
                    if d < distances[link.to] || distances[link.to] == 0.0 {
                        distances[link.to] = d;
                        back[link.to] = Some(LinkRef { node: id, index });
                        queue.push_back(link.to);
                    }
                }
            }
        }

        let mut node = final_node?;
        let mut points = vec![end];
        let mut blockers = Vec::new();
        let mut steps = 0;
        while let Some(link) = back[node] {
            self.add_points_to_path(node, &mut points);
            node = link.node;
            let l = &self.nodes[link.node].links[link.index];
            if l.blocked {
                blockers.push(l.blocker);
            }
            steps += 1;
            if steps > self.nodes.len() {
                break;
            }
        }
        self.add_points_to_path(node, &mut points);
        points.push(start);
        points.reverse();
        blockers.reverse();
        Some(NavPath::with_blockers(points, blockers, false))
    }

    /// `NavNode.AddPointsToPath`: the way through a node, added backwards
    /// (the path is built from its end): a node crossed along an axis adds
    /// the point half a cell before it and the one half a cell after, on the
    /// side the path comes from last, so the car goes straight through.
    fn add_points_to_path(&self, id: NodeId, path: &mut Vec<Vec2>) {
        let node = &self.nodes[id];
        let half = self.cell * 0.5;
        let last = *path.last().expect("the path starts with its end");
        match node.orientation {
            Orientation::Horizontal => {
                let sign = sign(node.pos.x - last.x);
                path.push(node.pos - Vec2::X * (sign * half));
                path.push(node.pos + Vec2::X * (sign * half));
            }
            Orientation::Vertical => {
                let sign = sign(node.pos.y - last.y);
                path.push(node.pos - Vec2::Y * (sign * half));
                path.push(node.pos + Vec2::Y * (sign * half));
            }
            Orientation::None => path.push(node.pos),
        }
    }
}

/// `Math.Sign(float)` as a float: -1, 0 or 1.
fn sign(v: f32) -> f32 {
    if v > 0.0 {
        1.0
    } else if v < 0.0 {
        -1.0
    } else {
        0.0
    }
}

/// An overlap taller than wide is crossed along x.
fn orientation(overlap: IRect) -> Orientation {
    if overlap.h > overlap.w { Orientation::Horizontal } else { Orientation::Vertical }
}
