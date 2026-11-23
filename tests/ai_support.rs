//! `SupportAi` (`SupportAiFriend`) and its `FollowLeader`, run as the plugin
//! runs them (gather, friends, tick, the act on the car) on a car that is a
//! point moving as its stick says, over a map's floor plan.

use std::sync::Arc;

use bevy::math::Vec2;
use bevy::prelude::Entity;
use flatbt_bevy::prelude::*;
use tilerider::ai::trees::support_ai;
use tilerider::ai::{AiBlackboard, Body, Foe, Gun, Mate, Navigation, Orders, tick_mode};
use tilerider::content::map::MapData;
use tilerider::map::grid::MapGrid;
use tilerider::map::terrain::Terrain;
use tilerider::weapon::model::{FireRange, ZOMBIE};

const DT: f32 = 1.0 / 60.0;
/// How fast a full stick drives a point car, units a second.
const SPEED: f32 = 4.0;
const PLAYER: u32 = 50;

fn id(n: u32) -> Entity {
    Entity::from_raw_u32(n).expect("an entity")
}

/// A room `w` × `h` cells inside a ring of wall, a cell a unit.
fn room(w: usize, h: usize) -> MapGrid {
    let mut rows = String::from("\n");
    for y in 0..h + 2 {
        for x in 0..w + 2 {
            rows.push(if x == 0 || y == 0 || x == w + 1 || y == h + 1 { '*' } else { '.' });
        }
        rows.push('\n');
    }
    let xml = format!(
        r#"<XnaContent><Asset Type="GameLibrary.MapData"><Name>T</Name><Scale>32</Scale>
        <ImageWallTreshold>96</ImageWallTreshold>
        <CharMapDictionary>*:#FFFF0000;.:#100000A0</CharMapDictionary><CharMap>{rows}</CharMap></Asset></XnaContent>"#
    );
    MapGrid::build(&MapData::parse(&xml).unwrap(), None)
}

/// A cell's middle in the world (y up).
fn at(x: f32, y: f32) -> Vec2 {
    Vec2::new(x + 0.5, -(y + 0.5))
}


/// The supporter: a point car of team 0 with a gun of the Zombie's range.
struct Support<B: TreeBuilder<AiBlackboard, Orders>> {
    bb: AiBlackboard,
    behavior: Behavior<AiBlackboard, Orders, B>,
    tree: BehaviorTree<AiBlackboard, Orders, B>,
    act: Option<Orders>,
    pos: Vec2,
    stick: Vec2,
    orders: Vec<Orders>,
}

struct Sim<B: TreeBuilder<AiBlackboard, Orders>> {
    grid: MapGrid,
    car: Support<B>,
    /// The player's car (team 0) and how it moves, if any.
    player: Option<(Vec2, Vec2)>,
    /// Cars of another team, standing.
    enemies: Vec<Foe>,
}

impl<B: TreeBuilder<AiBlackboard, Orders> + Copy> Sim<B> {
    /// A `SupportAiFriend` at `pos`, in a room `w` x `h`.
    fn new(builder: B, w: usize, h: usize, pos: Vec2) -> Self {
        let grid = room(w, h);
        let nav = Navigation::build(&grid, tilerider::units::CellSize(1.0));
        let terrain = Arc::new(Terrain::from_grid(&grid, 1.0));
        // The default `ScanDistance`; the friends are watched.
        let mut bb = AiBlackboard::new(3008);
        bb.watch_friends = true;
        bb.nav = Some(nav);
        bb.terrain = Some(terrain);
        let car = Support {
            bb,
            behavior: Behavior::for_tree(builder),
            tree: BehaviorTree::new(&builder, tick_mode),
            act: None,
            pos,
            stick: Vec2::ZERO,
            orders: Vec::new(),
        };
        Sim { grid, car, player: None, enemies: Vec::new() }
    }

    fn step(&mut self) {
        let mut foes: Vec<Foe> = self.enemies.clone();
        let mut mates: Vec<Mate> = Vec::new();
        if let Some((pos, vel)) = self.player {
            foes.push(Foe { entity: id(PLAYER), team: 0, pos, vel, radius: 0.98, weapon_range: Some(ZOMBIE.range) });
            mates.push(Mate { entity: id(PLAYER), team: 0, pos, vel, stick: vel / SPEED, local: true, mind: None });
        }
        let grid = &self.grid;
        let blocked = |a: Vec2, b: Vec2| grid.line_collides(1.0, [a.x, -a.y], [b.x, -b.y]);
        let z = &mut self.car;
        let body = Body {
            pos: z.pos,
            heading: 0.0,
            team: 0,
            gun: Some(Gun {
                complex: false,
                max_angle: 0.0,
                range: FireRange { from: z.pos, direction: 0.0, range: ZOMBIE.range, dispersion: 0.0 },
                immediate: true,
            }),
            velocity: z.stick * SPEED,
            radius: 0.98,
            can_move: true,
            stick: z.stick,
            health: 16,
            max_health: 16,
            shield: 0,
        };
        z.bb.gather(DT, z.act.is_some(), body, &foes, &blocked);
        z.bb.gun_ready = true;
        z.bb.gather_mates(id(SUPPORT), &mates);
        let mode = z.bb.tick.entry_mode().expect("gathered");
        z.act = z.behavior.tick(z.tree.get(), &mut z.bb, mode);
        let orders = z.act.unwrap_or_default();
        if z.bb.take_stick_cleared() {
            z.stick = Vec2::ZERO;
        }
        if let Some(m) = orders.move_dir {
            z.stick = m;
        }
        z.bb.take_trigger();
        let step = z.stick * SPEED * DT;
        for axis in [Vec2::X, Vec2::Y] {
            let to = z.pos + step * axis;
            if !grid.is_wall(to.x.floor() as i32, (-to.y).floor() as i32) {
                z.pos = to;
            }
        }
        z.orders.push(orders);
        if let Some((pos, vel)) = self.player.as_mut() {
            *pos += *vel * DT;
        }
    }

    fn run(&mut self, secs: f32) {
        for _ in 0..(secs / DT).round() as usize {
            self.step();
        }
    }

    fn path(&self) -> String {
        self.car.behavior.describe(self.car.tree.get()).to_string()
    }
}

const SUPPORT: u32 = 3008;

#[test]
fn with_no_leader_and_no_enemy_it_stands() {
    let mut sim = Sim::new(support_ai, 40, 12, at(5.0, 6.0));
    sim.run(3.0);
    assert!(sim.car.bb.leader_entity().is_none());
    assert!(sim.path().contains("Idle"), "{}", sim.path());
    assert!(sim.car.pos.distance(at(5.0, 6.0)) < 1e-3, "it stood");
}

#[test]
fn it_takes_the_player_near_as_leader_and_wanders_slowly_while_within_ten_of_it() {
    let mut sim = Sim::new(support_ai, 40, 12, at(5.0, 6.0));
    sim.player = Some((at(12.0, 6.0), Vec2::ZERO));
    sim.run(6.0);
    assert_eq!(sim.car.bb.leader_entity(), Some(id(PLAYER)), "{}", sim.path());
    assert!(sim.path().contains("RandomPos"), "{}", sim.path());
    // The wander is at a tenth of the stick, so it never gets far.
    let fastest = sim.car.orders.iter().filter_map(|o| o.move_dir).map(Vec2::length).fold(0.0, f32::max);
    assert!(fastest <= 0.1 + 1e-4, "{fastest}");
    assert!(sim.car.pos.distance(at(12.0, 6.0)) < 10.0 + 1.0, "{}", sim.car.pos);
}

#[test]
fn a_leader_further_than_ten_is_followed_at_a_full_stick_and_kept_within_ten() {
    let mut sim = Sim::new(support_ai, 60, 12, at(5.0, 6.0));
    sim.player = Some((at(14.0, 6.0), Vec2::ZERO));
    sim.run(0.5);
    assert_eq!(sim.car.bb.leader_entity(), Some(id(PLAYER)));
    // The player drives off along the room, out of the scan distance too:
    // the supporter, whose leader still lives, keeps after it.
    sim.player = Some((at(14.0, 6.0), Vec2::new(2.0, 0.0)));
    sim.run(2.0);
    assert!(sim.path().contains("Follow leader"), "{}", sim.path());
    let stick = sim.car.orders.iter().rev().find_map(|o| o.move_dir).unwrap();
    assert!(stick.length() > 0.9, "{stick}");
    sim.run(14.0);
    let player = sim.player.unwrap().0;
    assert!(sim.car.pos.x > at(30.0, 0.0).x, "{} (player at {player}): {}", sim.car.pos, sim.path());
    assert!((sim.car.pos - player).abs().max_element() < 14.0, "{} kept up with {player}", sim.car.pos);
}

#[test]
fn an_enemy_in_sight_is_fought_before_the_leader_is_followed() {
    let mut sim = Sim::new(support_ai, 60, 12, at(5.0, 6.0));
    sim.player = Some((at(5.0, 9.0), Vec2::ZERO));
    sim.run(0.5);
    sim.enemies = vec![Foe { entity: id(70), team: 1, pos: at(14.0, 6.0), vel: Vec2::ZERO, radius: 0.98, weapon_range: Some(ZOMBIE.range) }];
    sim.run(1.0);
    assert_eq!(sim.car.bb.target(), Some(id(70)));
    assert!(sim.path().contains("Attack"), "{}", sim.path());
    assert!(!sim.path().contains("Follow leader"), "{}", sim.path());
    // The enemy gone, it goes back to its leader.
    sim.enemies.clear();
    sim.run(3.0);
    assert!(sim.path().contains("Follow") || sim.path().contains("RandomPos"), "{}", sim.path());
    assert_eq!(sim.car.bb.leader_entity(), Some(id(PLAYER)));
}
