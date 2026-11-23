//! The splash screen's clock and layout (`splash::screen`), and the campaign's
//! splashes in `assets/Splashes/Splashes.xml`.

use std::fs;
use std::path::Path;

use bevy::math::Vec2;
use tilerider::content::actions::defs::ShowSplash;
use tilerider::content::map::MapData;
use tilerider::content::map_list::MapList;
use tilerider::content::splashes::SplashesData;
use tilerider::content::xna::XColor;
use tilerider::splash::screen::{Phase, Screen, corner_pivot_shift, draw_order, placement, straight_tint};

const FILE: &str = r#"<XnaContent><Asset Type="GameLibrary.SplashesData"><Splashes>
  <Item><Name>Timer</Name><DrawBackground>false</DrawBackground>
    <AppearFadeToBlack>false</AppearFadeToBlack><TranslationTime>0.5</TranslationTime>
    <ParallaxAmount>2</ParallaxAmount><ScaleUpOnlyOffsetWhenNeeded>true</ScaleUpOnlyOffsetWhenNeeded>
    <Layers>
      <Item><Sprite></Sprite><BindTextId>0</BindTextId><Offset>600 32</Offset></Item>
      <Item><Sprite>UI\ArrowStatic</Sprite><Offset>100 200</Offset><Color>90909090</Color>
        <Height>0.3</Height><Scale>2</Scale><DirectionRad>0.5</DirectionRad></Item>
      <Item><Text>Go</Text><DelaySec>1</DelaySec><Static>true</Static><OnForeground>true</OnForeground></Item>
      <Item><Sprite>WhitePixel</Sprite><StretchToScreen>true</StretchToScreen></Item>
    </Layers></Item>
  <Item><Name>Timer_Win</Name><Layers /></Item>
</Splashes></Asset></XnaContent>"#;

fn data() -> SplashesData {
    SplashesData::parse(FILE).unwrap()
}

fn timer() -> tilerider::content::splashes::SplashData {
    data().get("Timer").unwrap().clone()
}

/// Steps the screen by 0.1 s until `until` is true or 60 s pass.
fn run(screen: &mut Screen, until: impl Fn(&Screen) -> bool) -> f32 {
    let mut t = 0.0;
    while !until(screen) && t < 60.0 {
        screen.update(0.1);
        t += 0.1;
    }
    t
}

#[test]
fn layers_read_their_fields_and_defaults() {
    let d = data();
    let s = d.get("Timer").unwrap();
    assert!(!s.draw_background && !s.appear_fade_to_black);
    assert_eq!(s.translation_time, 0.5);
    assert!(s.ignore_pause, "IgnorePause defaults to true");
    let l = &s.layers.items;
    assert_eq!(l.len(), 4);
    assert_eq!((l[0].bind_text_id, l[0].sprite_name()), (0, None), "an empty sprite is none");
    assert_eq!((l[1].bind_text_id, l[1].height, l[1].scale), (-1, 0.3, 2.0));
    assert_eq!(l[1].color, XColor::rgba(0x90, 0x90, 0x90, 0x90));
    assert_eq!(l[2].text_key(), Some("Go"));
    assert!(l[2].is_static && l[2].on_foreground && l[3].stretch_to_screen);
    assert_eq!((l[3].height, l[3].scale, l[3].color), (-1.0, 1.0, XColor::WHITE));
}

#[test]
fn the_variant_for_the_control_is_used_when_there_is_one() {
    let d = data();
    assert_eq!(d.variant("Timer", "_Win").unwrap().name, "Timer_Win");
    assert_eq!(d.variant("Timer", "_GamePad").unwrap().name, "Timer");
    assert_eq!(d.variant("Timer", "").unwrap().name, "Timer");
    assert!(d.variant("Nothing", "_Win").is_none());
}

#[test]
fn it_fades_in_holds_and_fades_out() {
    let mut screen = Screen::default();
    assert!(!screen.visible());
    screen.show(timer(), 2.0);
    assert_eq!(screen.phase(), Phase::Appearing);
    assert_eq!(screen.alpha(), 0.0);
    for _ in 0..3 {
        screen.update(0.1);
    }
    assert!((screen.alpha() - 0.6).abs() < 1e-4, "0.3 s of 0.5 s: {}", screen.alpha());
    run(&mut screen, |s| s.phase() == Phase::Shown);
    assert_eq!(screen.alpha(), 1.0);
    run(&mut screen, |s| s.phase() == Phase::Hiding);
    assert!(screen.alpha() > 0.7, "it starts to fade out from full");
    let t = run(&mut screen, |s| s.phase() == Phase::Invisible);
    assert!((0.4..0.7).contains(&t), "fades out in about TranslationTime, took {t}");
    assert!(!screen.visible());
    assert_eq!(screen.alpha(), 0.0);
}

#[test]
fn it_starts_to_hide_when_the_action_on_hide_runs() {
    // `ActionOnHide` runs `AutoHideTimeSec` after `ShowSplash`; the splash
    // starts to fade out then too.
    let mut screen = Screen::default();
    screen.show(timer(), 2.0);
    let t = run(&mut screen, |s| s.phase() == Phase::Hiding);
    assert!((2.0..2.3).contains(&t), "began to hide at {t}");
}

#[test]
fn no_time_stays_until_hidden() {
    let mut screen = Screen::default();
    screen.show(timer(), -1.0);
    run(&mut screen, |s| s.phase() == Phase::Hiding);
    assert_eq!(screen.phase(), Phase::Shown, "nothing hides it in a minute");
    screen.hide();
    assert_eq!(screen.phase(), Phase::Hiding);
    run(&mut screen, |s| !s.visible());
    assert!(!screen.visible());
}

#[test]
fn hiding_while_it_fades_in_fades_out_from_the_same_brightness() {
    let mut screen = Screen::default();
    screen.show(timer(), 9.0);
    for _ in 0..3 {
        screen.update(0.1);
    }
    let before = screen.alpha();
    screen.hide();
    assert_eq!(screen.phase(), Phase::Hiding);
    assert!((screen.alpha() - before).abs() < 1e-4, "{} then {}", before, screen.alpha());
    // Hiding again, or one that is gone, changes nothing.
    screen.hide();
    assert!((screen.alpha() - before).abs() < 1e-4);
    let mut gone = Screen::default();
    gone.hide();
    assert_eq!(gone.phase(), Phase::Invisible);
}

#[test]
fn a_new_splash_replaces_the_shown_one_and_fades_in_again() {
    let mut screen = Screen::default();
    screen.show(timer(), 9.0);
    run(&mut screen, |s| s.phase() == Phase::Shown);
    screen.show(data().get("Timer_Win").unwrap().clone(), 9.0);
    assert_eq!(screen.splash().unwrap().name, "Timer_Win");
    assert_eq!((screen.phase(), screen.alpha()), (Phase::Appearing, 0.0));
    // One fading in carries on instead.
    screen.update(0.2);
    let before = screen.alpha();
    screen.show(data().get("Timer_Win").unwrap().clone(), 9.0);
    assert_eq!(screen.alpha(), before);
}

#[test]
fn the_countdown_reads_tenths_while_held() {
    let mut screen = Screen::default();
    screen.show(timer(), 5.0);
    assert_eq!(screen.countdown(), "", "empty while it fades in");
    run(&mut screen, |s| s.phase() == Phase::Shown);
    let text = screen.countdown();
    let (whole, tenth) = text.split_once('.').expect("one decimal");
    assert!(whole.parse::<u32>().is_ok() && tenth.len() == 1, "{text}");
    let seconds: f32 = text.parse().unwrap();
    assert!((4.0..=4.5).contains(&seconds), "{text}");
    run(&mut screen, |s| s.phase() == Phase::Hiding);
    assert_eq!(screen.countdown(), "", "empty once it hides");
    let mut open = Screen::default();
    open.show(timer(), 0.0);
    run(&mut open, |s| s.phase() == Phase::Shown);
    assert_eq!(open.countdown(), "", "none without a time");
}

#[test]
fn layers_appear_after_their_delay() {
    let mut screen = Screen::default();
    screen.show(timer(), 9.0);
    assert!(!screen.layer_active(0), "nothing before the first frame");
    screen.update(0.1);
    assert!(screen.layer_active(0) && screen.layer_active(1) && screen.layer_active(3));
    assert!(!screen.layer_active(2), "the third waits a second");
    run(&mut screen, |s| s.layer_active(2));
    assert!(screen.layer_active(2));
    // Restarted with the next show.
    screen.show(timer(), 9.0);
    assert!(!screen.layer_active(0));
    assert!(!screen.layer_active(9), "no such layer");
}

#[test]
fn the_layers_sway_by_the_parallax_and_come_back() {
    let mut screen = Screen::default();
    screen.show(timer(), 90.0);
    assert_eq!(screen.sway(), Vec2::ZERO);
    let mut biggest: f32 = 0.0;
    let mut vertical: f32 = 0.0;
    for _ in 0..4000 {
        screen.update(1.0 / 60.0);
        biggest = biggest.max(screen.sway().x.abs());
        vertical = vertical.max(screen.sway().y.abs());
    }
    // 16 px × ParallaxAmount 2, after 66 s (two cycles) of a swing that
    // lags a little behind its target.
    assert!((25.0..=32.0).contains(&biggest), "{biggest}");
    assert_eq!(vertical, 0.0, "without a tilt there is no vertical sway");
}

#[test]
fn a_layer_is_placed_from_the_centre_by_its_height() {
    let d = timer();
    let layers = &d.layers.items;
    let sway = Vec2::new(10.0, 0.0);
    // At 800 × 480 the layout is the file's own, offsets from the corner.
    let p = placement(&d, &layers[1], 1, Vec2::new(800.0, 480.0), sway);
    assert_eq!(p.top_left, Vec2::new(100.0 + 10.0 * 0.3, 200.0));
    assert_eq!(p.scale, 2.0);
    // No height: the position in the list; static: no sway.
    let p = placement(&d, &layers[0], 0, Vec2::new(800.0, 480.0), sway);
    assert_eq!(p.top_left, Vec2::new(600.0, 32.0));
    let p = placement(&d, &layers[2], 2, Vec2::new(800.0, 480.0), sway);
    assert_eq!(p.top_left, Vec2::new(400.0, 240.0) + Vec2::new(-400.0, -240.0), "offset 0 0 is the corner, and static: no sway");
    assert!(placement(&d, &layers[3], 3, Vec2::new(800.0, 480.0), sway).stretch);
    // A bigger screen: this splash grows only its offsets, around the centre.
    let view = Vec2::new(1600.0, 960.0);
    let p = placement(&d, &layers[1], 1, view, Vec2::ZERO);
    assert_eq!(p.top_left, view / 2.0 + (Vec2::new(100.0, 200.0) - Vec2::new(400.0, 240.0)) * 2.0);
    assert_eq!(p.scale, 2.0, "sizes stay");
    // The short side decides on a screen of another shape.
    let p = placement(&d, &layers[1], 1, Vec2::new(1600.0, 480.0), Vec2::ZERO);
    assert_eq!(p.top_left, Vec2::new(800.0, 240.0) + Vec2::new(100.0 - 400.0, 200.0 - 240.0));
}

#[test]
fn a_scaled_up_splash_grows_its_sizes_too() {
    let mut d = timer();
    d.scale_up_when_needed = true;
    let p = placement(&d, &d.layers.items[1], 1, Vec2::new(1600.0, 960.0), Vec2::ZERO);
    assert_eq!(p.scale, 4.0);
}

#[test]
fn foreground_layers_are_drawn_last_and_height_orders_the_rest() {
    let d = timer();
    // Heights: 0.3 for the arrow, none for the others (by position: a
    // quarter for the first, 1 for the last), and the text on the foreground.
    let order = draw_order(&d);
    assert_eq!(*order.last().unwrap(), 2, "the foreground text is on top");
    let at = |layer| order.iter().position(|&l| l == layer).unwrap();
    assert!(at(0) < at(3), "list order without a height");
    assert!(at(0) < at(1), "a height of 0.3 is nearer than the first of four (0.25)");
    assert!(at(1) < at(3), "and behind the last (1)");
}

#[test]
fn colours_are_straightened_from_premultiplied() {
    assert_eq!(straight_tint(XColor::WHITE, 1.0), [1.0, 1.0, 1.0, 1.0]);
    // `80808080` is white at half strength, not grey.
    let [r, g, b, a] = straight_tint(XColor::rgba(128, 128, 128, 128), 1.0);
    assert!((r - 1.0).abs() < 1e-6 && (g - 1.0).abs() < 1e-6 && (b - 1.0).abs() < 1e-6);
    assert!((a - 128.0 / 255.0).abs() < 1e-6);
    // The fade multiplies the alpha only.
    assert!((straight_tint(XColor::rgba(128, 128, 128, 128), 0.5)[3] - 64.0 / 255.0).abs() < 1e-6);
    // A channel above the alpha stays above 1: it adds light.
    assert!(straight_tint(XColor::rgba(0, 255, 255, 128), 1.0)[2] > 1.9);
    assert_eq!(straight_tint(XColor::rgba(255, 255, 255, 0), 1.0), [0.0; 4]);
}

#[test]
fn a_turn_about_the_corner_moves_the_centre() {
    let size = Vec2::new(10.0, 4.0);
    assert_eq!(corner_pivot_shift(size, 0.0), Vec2::ZERO);
    // A quarter turn clockwise (y down): the centre (5, 2) goes to (-2, 5).
    let shift = corner_pivot_shift(size, std::f32::consts::FRAC_PI_2);
    assert!((shift - Vec2::new(-7.0, 3.0)).length() < 1e-5, "{shift}");
    // The corner stays: the box's top left after the turn is where it was.
    let centre = size / 2.0 + shift;
    let (s, c) = std::f32::consts::FRAC_PI_2.sin_cos();
    let corner = centre - Vec2::new(c * size.x / 2.0 - s * size.y / 2.0, s * size.x / 2.0 + c * size.y / 2.0);
    assert!(corner.length() < 1e-5, "{corner}");
}

fn assets() -> &'static Path {
    Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/assets"))
}

#[test]
fn every_splash_the_campaign_shows_is_in_the_file_with_its_sprites() {
    let all = SplashesData::parse(&fs::read_to_string(assets().join("Splashes/Splashes.xml")).unwrap()).unwrap();
    assert!(all.splashes.items.len() > 50);
    let list = MapList::parse(&fs::read_to_string(assets().join("Maps/MapList.xml")).unwrap()).unwrap();
    let mut shown = std::collections::BTreeSet::new();
    for map in list.singleplayer.iter() {
        let xml = fs::read_to_string(assets().join(format!("Maps/Singleplayer/{map}.xml"))).unwrap();
        let map = MapData::parse(&xml).unwrap();
        for action in map.actions.0.iter().filter(|a| a.kind == "ShowSplash") {
            let show = ShowSplash::read_fields(&action.node).unwrap();
            if let Some(name) = show.splash_name.filter(|n| !n.is_empty()) {
                shown.insert(name);
            }
        }
    }
    assert!(shown.len() >= 10, "{shown:?}");
    for name in &shown {
        for suffix in ["", "_Win", "_GamePad"] {
            let Some(splash) = all.variant(name, suffix) else {
                panic!("splash {name} is not in Splashes.xml");
            };
            for layer in splash.layers.iter() {
                if let Some(sprite) = layer.sprite_name() {
                    assert!(
                        tilerider::assets::sprite_path(sprite).is_some(),
                        "{} needs the sprite {sprite}: run tools/import_content.py",
                        splash.name
                    );
                }
                assert!(layer.sound.is_none(), "{} has a layer sound, which is not ported", splash.name);
                assert!(
                    layer.text_key().is_none() || layer.bind_text_id < 0,
                    "{} has a layer with a text and a bound text, which is not ported",
                    splash.name
                );
            }
            assert!(!splash.appear_fade_to_black && !splash.draw_background, "{} is a menu splash", splash.name);
        }
    }
}
