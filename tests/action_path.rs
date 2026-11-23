//! `AddPathComponent`: the legs of a dotted route line.

use bevy::math::Vec2;
use tilerider::actions::{legs, offset, uv};
use tilerider::content::map::MapData;

/// `Special\DottedLine`'s width.
const WIDTH: u32 = 128;

#[test]
fn a_leg_is_a_whole_number_of_four_pixel_texels() {
    // Derby: 5 units, 40 texels of 1/8 unit.
    let l = legs(&[Vec2::new(0.0, 0.0), Vec2::new(5.0, 0.0)], WIDTH);
    assert_eq!(l.len(), 1);
    assert_eq!(l[0].texels, 40);
    assert!((l[0].texel_length - 0.125).abs() < 1e-6);
    assert!((l[0].length() - 5.0).abs() < 1e-4);
    // A leg's end is cut short by the part of a texel left over.
    let l = legs(&[Vec2::ZERO, Vec2::new(0.0, 1.07)], WIDTH);
    assert_eq!(l[0].texels, 8);
    assert!((l[0].angle - std::f32::consts::FRAC_PI_2).abs() < 1e-5);
}

#[test]
fn the_longest_leg_stretches_every_texel_alike() {
    // 31 units is two strips (124 texels of 1/8 unit is 15.5): texels double.
    let l = legs(&[Vec2::ZERO, Vec2::new(31.0, 0.0), Vec2::new(31.0, 7.75)], WIDTH);
    assert_eq!(l[0].texels, 124);
    assert_eq!(l[1].texels, 31);
    assert!((l[0].texel_length - 0.25).abs() < 1e-6);
    assert!((l[1].texel_length - 0.25).abs() < 1e-6);
}

#[test]
fn nothing_is_drawn_for_a_point_a_degenerate_leg_or_less_than_a_texel() {
    assert!(legs(&[], WIDTH).is_empty());
    assert!(legs(&[Vec2::ONE], WIDTH).is_empty());
    assert!(legs(&[Vec2::ONE, Vec2::ONE], WIDTH).is_empty());
    assert!(legs(&[Vec2::ZERO, Vec2::new(0.1, 0.0)], WIDTH).is_empty());
}

#[test]
fn the_dots_march_a_period_a_second_towards_the_end() {
    let seen: Vec<u32> = [0.0, 0.1, 0.3, 0.6, 0.8, 1.3].iter().map(|&t| offset(t)).collect();
    assert_eq!(seen, [4, 3, 2, 1, 0, 2]);
    let leg = legs(&[Vec2::ZERO, Vec2::new(2.0, 0.0)], WIDTH)[0];
    let m = uv(&leg, WIDTH, 2);
    assert!((m.matrix2.x_axis.x - 16.0 / 128.0).abs() < 1e-6);
    assert!((m.translation.x - 2.0 / 128.0).abs() < 1e-6);
}

#[test]
fn the_action_reads_from_the_map() {
    let map = MapData::parse(
        r#"<XnaContent><Asset Type="GameLibrary.MapData"><Name>T</Name><Actions>
        <Item Type="GameLibrary.AddPathComponent"><Name>PathComponent</Name><RunOnMapStart>true</RunOnMapStart>
        <Path>17 3.5 14.5 3.5 14.5 5.5</Path><ItemId>3006</ItemId><Color>FFFFFFFF</Color><Depth>0</Depth><Width>5</Width></Item>
        </Actions></Asset></XnaContent>"#,
    )
    .unwrap();
    assert_eq!(map.actions.0.len(), 1);
}
