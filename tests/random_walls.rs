//! `WallWithProbability`: areas painted over the map image when it is made.

use tilerider::content::actions::MapActions;
use tilerider::content::map::MapData;
use tilerider::content::xna::XColor;
use tilerider::map::grid::MapGrid;
use tilerider::map::random_walls::{UnitRng, apply, seed_of};

mod common;

const WALL: XColor = XColor::rgba(200, 10, 10, 255);
const FLOOR: XColor = XColor::rgba(10, 10, 10, 60);

fn item(area: &str, color: &str, probability: f32, group: Option<&str>) -> String {
    let group = group.map_or("<WallGroup Null=\"true\" />".to_string(), |g| format!("<WallGroup>{g}</WallGroup>"));
    format!(
        "<Item Type=\"GameLibrary.WallWithProbability\"><Name>W</Name><RunOnMapStart>false</RunOnMapStart>\
         <Area>{area}</Area><Color>{color}</Color><Probability>{probability}</Probability>{group}</Item>"
    )
}

fn actions(items: &[String]) -> MapActions {
    let xml = format!("<XnaContent><Asset><Actions>{}</Actions></Asset></XnaContent>", items.concat());
    MapActions::parse(&xml).unwrap()
}

/// A 6x2 image of floor.
fn floor() -> Vec<XColor> {
    vec![FLOOR; 12]
}

fn painted(pixels: &[XColor]) -> Vec<usize> {
    pixels.iter().enumerate().filter(|(_, c)| **c == WALL).map(|(i, _)| i).collect()
}

#[test]
fn a_probability_of_one_always_paints_its_area_and_only_it() {
    let a = actions(&[item("1 0 2 1", "FFC80A0A", 1.0, None)]);
    for seed in 0..50 {
        let mut p = floor();
        assert_eq!(apply(&a, &mut p, 6, 2, &mut UnitRng::new(seed)), 1);
        assert_eq!(painted(&p), vec![1, 2]);
    }
}

#[test]
fn a_probability_of_zero_never_paints() {
    let a = actions(&[item("0 0 6 2", "FFC80A0A", 0.0, None)]);
    for seed in 0..50 {
        let mut p = floor();
        assert_eq!(apply(&a, &mut p, 6, 2, &mut UnitRng::new(seed)), 0);
        assert_eq!(p, floor());
    }
}

#[test]
fn a_transparent_colour_digs_out_what_was_there() {
    // Escort2's: alpha 0 over a wall removes it (the pixel is replaced whole).
    let a = actions(&[item("0 0 2 1", "00FFFFFF", 1.0, None)]);
    let mut p = vec![WALL; 12];
    apply(&a, &mut p, 6, 2, &mut UnitRng::new(1));
    assert_eq!(p[0].a, 0);
    assert_eq!(p[1].a, 0);
    assert_eq!(p[2], WALL);
    assert_eq!(p[6], WALL, "the row below is not touched");
}

#[test]
fn actions_without_a_group_draw_one_by_one() {
    // Probability a half: over many seeds each of the two is painted about half of the time, apart.
    let a = actions(&[item("0 0 1 1", "FFC80A0A", 0.5, None), item("5 1 1 1", "FFC80A0A", 0.5, None)]);
    let (mut first, mut second, mut both) = (0, 0, 0);
    for seed in 0..400 {
        let mut p = floor();
        apply(&a, &mut p, 6, 2, &mut UnitRng::new(seed * 7919 + 3));
        let w = painted(&p);
        first += w.contains(&0) as i32;
        second += w.contains(&11) as i32;
        both += (w.len() == 2) as i32;
    }
    assert!((140..260).contains(&first), "{first}");
    assert!((140..260).contains(&second), "{second}");
    assert!((50..150).contains(&both), "independent: about a quarter, got {both}");
}

#[test]
fn a_named_group_is_painted_together_or_not_at_all_by_its_first_probability() {
    // The second member's own 0 does not count: the first member's does.
    let a = actions(&[
        item("0 0 1 1", "FFC80A0A", 0.5, Some("door")),
        item("5 1 1 1", "FFC80A0A", 0.0, Some("door")),
    ]);
    let (mut none, mut all) = (0, 0);
    for seed in 0..400 {
        let mut p = floor();
        apply(&a, &mut p, 6, 2, &mut UnitRng::new(seed * 7919 + 3));
        match painted(&p).as_slice() {
            [] => none += 1,
            [0, 11] => all += 1,
            other => panic!("a group is split: {other:?}"),
        }
    }
    assert!(none > 100 && all > 100, "{none} {all}");
}

#[test]
fn a_group_of_spaces_is_not_a_group() {
    let a = actions(&[item("0 0 1 1", "FFC80A0A", 0.5, Some("  ")), item("5 1 1 1", "FFC80A0A", 0.5, Some("  "))]);
    let split = (0..200).any(|seed| {
        let mut p = floor();
        apply(&a, &mut p, 6, 2, &mut UnitRng::new(seed * 31 + 1));
        painted(&p).len() == 1
    });
    assert!(split, "each draws for itself");
}

#[test]
fn an_area_over_the_edge_is_clipped() {
    let a = actions(&[item("4 1 5 3", "FFC80A0A", 1.0, None), item("-3 -3 4 4", "FFC80A0A", 1.0, None)]);
    let mut p = floor();
    apply(&a, &mut p, 6, 2, &mut UnitRng::new(1));
    assert_eq!(painted(&p), vec![0, 10, 11]);
}

#[test]
fn the_same_seed_gives_the_same_walls_and_names_give_different_seeds() {
    let a = actions(&(0..6).map(|x| item(&format!("{x} 0 1 1"), "FFC80A0A", 0.5, None)).collect::<Vec<_>>());
    let run = |seed| {
        let mut p = floor();
        apply(&a, &mut p, 6, 2, &mut UnitRng::new(seed));
        painted(&p)
    };
    assert_eq!(run(seed_of("Escort2")), run(seed_of("Escort2")));
    assert_ne!(seed_of("Escort2"), seed_of("Swarm"));
    assert!((0..40).any(|s| run(s) != run(0)), "the draws vary with the seed");
    let mut rng = UnitRng::new(5);
    assert!((0..1000).map(|_| rng.next()).all(|n| (0.0..1.0).contains(&n)));
}

#[test]
fn the_grid_is_built_from_the_painted_image() {
    let xml = format!(
        "<XnaContent><Asset Type=\"GameLibrary.MapData\"><Name>T</Name><Scale>64</Scale>\
         <ImageWallTreshold>96</ImageWallTreshold><CharMapDictionary>*:#FFFF0000;.:#100000A0</CharMapDictionary>\
         <CharMap>\n*****\n*...*\n*****\n</CharMap><Actions>{}</Actions></Asset></XnaContent>",
        // A hole in the top wall, and a wall in the floor.
        [item("2 0 1 1", "00FFFFFF", 1.0, None), item("2 1 1 1", "FFFF0000", 1.0, None)].concat()
    );
    let map = MapData::parse(&xml).unwrap();
    let grid = MapGrid::build(&map, None);
    assert!(!grid.is_wall(2, 0), "the wall was dug out");
    assert!(grid.is_wall(2, 1), "the floor was walled");
    assert!(grid.is_wall(1, 0));
}

#[test]
fn escort2_digs_out_four_cells_of_swarm_s_image() {
    let (map, grid) = common::load_map("Escort2");
    assert!(map.actions.0.iter().any(|a| a.kind == "WallWithProbability"));
    // `Area` 29 18 4 1, colour 00FFFFFF, probability 1: those four pixels are
    // replaced whole (alpha 0), and their neighbours are not touched.
    for x in 29..33 {
        assert_eq!(grid.cell(x, 18).map(|c| c.color.a), Some(0), "cell {x}");
    }
    assert_eq!(grid.cell(28, 18).map(|c| c.color.a), Some(149));
    assert!(grid.cell(29, 19).is_some_and(|c| c.color.a == 85));
    let (_, swarm) = common::load_map("Swarm");
    assert_eq!(swarm.cell(30, 18).map(|c| c.color.a), Some(85), "Swarm has none");
}
