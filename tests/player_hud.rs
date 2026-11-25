//! The player's HUD layout: health cells, shield cells, icons, lives.

use bevy::math::Vec2;
use tilerider::player_hud::model::*;

const W: f32 = 800.0;

fn cell_sizes(items: &[Item]) -> Vec<f32> {
    items.iter().map(|i| i.size.x).collect()
}

#[test]
fn the_row_is_centred_with_two_pixels_of_gap_each_side() {
    // 12 health: 3 cells of 24 with 4 between: first cell's middle.
    let first = first_cell(W, 1.0, 12);
    assert_eq!(first, Vec2::new(400.0 - 42.0 + 12.0 + 2.0, 16.0));
    let cells = health_cells(W, 1.0, 12, 12, 12, 0.0, 0.0);
    assert_eq!(cells.len(), 3);
    assert_eq!(cells[1].center.x - cells[0].center.x, 28.0);
}

#[test]
fn it_grows_with_the_screen() {
    let a = health_cells(W, 1.0, 8, 8, 8, 0.0, 0.0);
    let b = health_cells(1600.0, 2.0, 8, 8, 8, 0.0, 0.0);
    assert_eq!(b[0].size, a[0].size * 2.0);
    assert_eq!(b[0].center.y, a[0].center.y * 2.0);
}

#[test]
fn a_part_cell_draws_a_smaller_cell_under_the_full_one() {
    // 6 of 12: one whole cell and half of the next.
    let cells = health_cells(W, 1.0, 12, 6, 6, 0.0, 0.5);
    assert_eq!(cell_sizes(&cells), vec![24.0, 12.0, 24.0, 24.0]);
    // The dark full cell is drawn after the part.
    assert!(cells[2].color[3] < 0.2);
}

#[test]
fn lost_health_fades_from_red_to_dark() {
    let fresh = health_cells(W, 1.0, 12, 4, 12, 1.0, 0.5);
    let old = health_cells(W, 1.0, 12, 4, 12, 0.0, 0.5);
    // Cells 1 and 2 were lost: red at first, dark when the time is up.
    assert!(fresh[1].color[0] > 0.9 && fresh[1].color[1] < 0.1);
    assert!(old[1].color[3] < 0.15);
    // The first cell is kept.
    assert!((fresh[0].color[3] - 0.75).abs() < 1e-6);
}

#[test]
fn gained_health_fades_from_green() {
    let cells = health_cells(W, 1.0, 12, 12, 4, 1.0, 0.5);
    assert!(cells[1].color[1] > cells[1].color[0]);
    assert_eq!(cells[0].color, [1.0, 1.0, 1.0, 0.75]);
}

#[test]
fn the_trail_remembers_for_two_and_a_half_seconds() {
    let mut t = HealthTrail::new(12);
    let s = t.step(1.0, 12);
    assert!(s.lost.is_empty());
    let s = t.step(2.0, 5);
    assert_eq!(t.previous, 12);
    assert!((s.remain - 1.0).abs() < 1e-6);
    // Cell 2 was lost (5 is in cell 1, 12 reached cell 2).
    assert_eq!(s.lost, vec![2]);
    t.step(3.0, 5);
    assert_eq!(t.previous, 12);
    let s = t.step(4.6, 5);
    assert_eq!(t.previous, 5);
    assert!(s.remain > 0.9);
}

#[test]
fn shield_cells_follow_the_health_row_and_dim_when_not_whole() {
    let cells = shield_cells(W, 1.0, 12, 8, 4.0, 0.0);
    // Two cells, the first whole; the second is faint (no part to show).
    assert_eq!(cells.len(), 3);
    let row_end = first_cell(W, 1.0, 12).x + 28.0 * 3.0 - 12.0;
    assert!((cells[0].center.x - (row_end + 4.8)).abs() < 1e-4);
    assert_eq!(cells[0].size, Vec2::new(9.6, 16.0));
    assert!(cells[0].color[3] > 0.9);
    assert!((cells[1].color[3] - 0.25 * 0.95).abs() < 1e-5);
    // The part cell: 0 grown back draws a flat line.
    assert_eq!(cells[2].size.y, 0.0);
}

#[test]
fn a_shield_that_is_down_flashes_yellow_and_black() {
    let on = shield_cells(W, 1.0, 12, 4, 0.0, 0.0);
    let off = shield_cells(W, 1.0, 12, 4, 0.0, 1.0);
    assert!(on[0].color[0] > 0.9 && on[0].color[1] > 0.9);
    assert!(off[0].color[0] < 0.1);
}

fn size_of(name: &str) -> Vec2 {
    match name {
        ICON_WEAPON => Vec2::new(66.0, 32.0),
        _ => Vec2::new(32.0, 32.0),
    }
}

#[test]
fn icons_stand_left_of_the_cells_gadget_first() {
    let gear = Gear { magnet: true, accelerator: false, weapon: true };
    let icons = icons(W, 1.0, 12, gear, size_of);
    assert_eq!(icons.len(), 2);
    let first = first_cell(W, 1.0, 12);
    // The cell's height; the magnet turned a quarter of a half turn back.
    assert_eq!(icons[0].sprite, ICON_MAGNET);
    assert_eq!(icons[0].size, Vec2::new(16.0, 16.0));
    assert!((icons[0].rotation + std::f32::consts::FRAC_PI_4).abs() < 1e-6);
    assert!((icons[0].center.x - (first.x - icon_step(16.0, 1.0))).abs() < 1e-4);
    // The gun is 66 × 32: 33 × 16, further left, not turned.
    assert_eq!(icons[1].sprite, ICON_WEAPON);
    assert_eq!(icons[1].size, Vec2::new(33.0, 16.0));
    assert_eq!(icons[1].rotation, 0.0);
    assert!((icons[1].center.x - (icons[0].center.x - icon_step(33.0, 1.0))).abs() < 1e-4);
}

#[test]
fn the_accelerator_has_its_own_icon_and_nothing_means_none() {
    let gear = Gear { magnet: false, accelerator: true, weapon: false };
    let list = icons(W, 1.0, 12, gear, size_of);
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].sprite, ICON_ACCELERATOR);
    assert!(icons(W, 1.0, 12, Gear::default(), size_of).is_empty());
}

#[test]
fn lives_show_a_bar_for_each_and_a_red_one_for_the_spent() {
    let fx = lives_fx(Vec2::new(800.0, 480.0), 1.0, 2);
    // The battery, two white bars, one red.
    assert_eq!(fx.len(), 4);
    assert_eq!(fx[0].sprite, BATTERY);
    assert_eq!(fx[0].center, Vec2::new(400.0, 240.0));
    assert_eq!(fx[1].center.x, 400.0 - 37.0);
    assert_eq!(fx[2].center.x - fx[1].center.x, 33.0);
    assert_eq!(fx[3].rgb, [1.0, 0.0, 0.0]);
    assert_eq!(fx[3].life, 1.0);
    assert_eq!(fx[1].size, Vec2::new(29.0, 43.0));
}

#[test]
fn no_lives_left_is_a_red_battery() {
    let fx = lives_fx(Vec2::new(800.0, 480.0), 1.0, -1);
    assert_eq!(fx.len(), 1);
    assert_eq!(fx[0].rgb, [1.0, 0.0, 0.0]);
}

#[test]
fn fx_fade_and_end() {
    let fx = damage_fx(W, 1.0, 12, 1);
    let half = fx.at(0.25).unwrap();
    assert!((half.color[3] - 0.5).abs() < 1e-6);
    // 256 px/s at the end, from rest: 64 px down after 0.5 s.
    let near_end = fx.at(0.499).unwrap();
    assert!((near_end.center.y - fx.center.y - 64.0).abs() < 0.5);
    assert!(fx.at(0.5).is_none());
}

#[test]
fn fading_a_colour_scales_its_alpha_up_to_opaque() {
    // White at 0.75 times 1.5 is solid white; the colour itself is kept.
    assert_eq!(fade([1.0; 4], 1.5), [1.0; 4]);
    assert_eq!(fade([1.0, 0.0, 0.0, 0.75], 1.5), [1.0, 0.0, 0.0, 1.0]);
    assert_eq!(fade([1.0, 0.0, 0.0, 0.5], 0.5), [1.0, 0.0, 0.0, 0.25]);
}

#[test]
fn colours_mix_by_how_opaque_they_are() {
    let (clear_white, red) = ([1.0, 1.0, 1.0, 0.1], [1.0, 0.0, 0.0, 0.7]);
    let near = |a: [f32; 4], b: [f32; 4]| a.iter().zip(b).all(|(x, y)| (x - y).abs() < 1e-6);
    assert!(near(lerp(clear_white, red, 0.0), clear_white));
    assert!(near(lerp(clear_white, red, 1.0), red));
    // Halfway the red counts for seven parts in eight of the colour.
    let mid = lerp(clear_white, red, 0.5);
    assert!((mid[3] - 0.4).abs() < 1e-6);
    assert!((mid[1] - 0.05 / 0.4).abs() < 1e-6);
    assert_eq!(lerp([1.0, 1.0, 1.0, 0.0], [0.0; 4], 0.5), [0.0; 4]);
}
