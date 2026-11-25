//! The player's HUD as plain data: what to draw where, for the health
//! cells, the shield cells, the icons and the lives battery. No Bevy app
//! here, so it is tested from `tests/player_hud.rs`.
//!
//! Positions are logical screen pixels from the top left, `k` is the
//! screen's growth over the 800 × 480 C# screen (`UISizeMultiplier`).
//! Colours are straight RGB and an alpha, in 0..1, as Bevy blends them. C#
//! wrote `Color.White * 0.75` (premultiplied: every channel 0.75), which is
//! white with alpha 0.75 here.

use bevy::math::Vec2;

/// `Settings.HealthBarUiSize`: one health cell.
pub const CELL: Vec2 = Vec2::new(24.0, 16.0);
/// The gap between cells, `4 * UISizeMultiplier`.
pub const GAP: f32 = 4.0;
/// `healthScale`: health points in one cell (and shield points in one).
pub const PER_CELL: i32 = 4;
/// The row's distance from the top edge, `16 * UISizeMultiplier`.
pub const TOP: f32 = 16.0;
/// `_prevHealthUpdate`: seconds the damage shows and the heal fades.
pub const TRAIL: f32 = 2.5;

pub const WHITE_PIXEL: &str = "WhitePixel";
pub const BATTERY: &str = "Particles/Battery";
pub const ICON_MAGNET: &str = "UI/GadgetIcons/Magnet";
pub const ICON_ACCELERATOR: &str = "UI/GadgetIcons/Accelerator";
pub const ICON_WEAPON: &str = "UI/GadgetIcons/Weapon";

/// Every picture the HUD draws.
pub const SPRITES: [&str; 5] = [WHITE_PIXEL, BATTERY, ICON_MAGNET, ICON_ACCELERATOR, ICON_WEAPON];

/// A straight colour: RGB and alpha, 0..1.
pub type Rgba = [f32; 4];

const HEALTH: Rgba = [1.0, 1.0, 1.0, 0.75];
const MAX_HEALTH: Rgba = [1.0, 1.0, 1.0, 0.1];
const DAMAGE: Rgba = [1.0, 0.0, 0.0, 0.75];
/// `Color.Green` is (0, 128, 0).
const HEAL: Rgba = [0.0, 128.0 / 255.0, 0.0, 1.0];
const SHIELD: Rgba = [21.0 / 255.0, 194.0 / 255.0, 238.0 / 255.0, 0.95];
/// The gadget's and the weapon's icons: white at 0.75.
const ICON: Rgba = [1.0, 1.0, 1.0, 0.75];

/// One picture to draw.
#[derive(Debug, Clone, PartialEq)]
pub struct Item {
    pub sprite: &'static str,
    pub center: Vec2,
    /// On screen, in pixels.
    pub size: Vec2,
    /// Radians, turning clockwise on the screen (XNA's rotation).
    pub rotation: f32,
    /// Straight sRGB and alpha.
    pub color: [f32; 4],
}

/// The colour at `f` times its opacity (`Color * f` of a premultiplied
/// colour), at most opaque.
pub fn fade(c: Rgba, f: f32) -> Rgba {
    [c[0], c[1], c[2], (c[3] * f).min(1.0)]
}

/// `Color.Lerp`: the colours mix by how much of each there is, the more
/// opaque one counting for more (as a lerp of premultiplied colours did).
pub fn lerp(a: Rgba, b: Rgba, t: f32) -> Rgba {
    let alpha = a[3] + (b[3] - a[3]) * t;
    if alpha <= 0.0 {
        return [0.0; 4];
    }
    let mix = |i: usize| (a[i] * a[3] * (1.0 - t) + b[i] * b[3] * t) / alpha;
    [mix(0), mix(1), mix(2), alpha]
}

fn solid(center: Vec2, size: Vec2, color: Rgba) -> Item {
    Item { sprite: WHITE_PIXEL, center, size, rotation: 0.0, color }
}

/// `maxN`: how many cells the health bar has.
pub fn cell_count(max_health: i32) -> i32 {
    max_health / PER_CELL
}

/// The first cell's middle (`_drawHealthPosition`): the row is centred.
pub fn first_cell(screen_width: f32, k: f32, max_health: i32) -> Vec2 {
    let delta = (CELL.x + GAP) * k;
    Vec2::new(
        screen_width / 2.0 - delta * cell_count(max_health) as f32 / 2.0 + CELL.x * k / 2.0 + (delta - CELL.x * k) / 2.0,
        TOP * k,
    )
}

/// What `DrawHealth` remembers between frames: the health a few seconds
/// ago, which the cells show fading to the health now.
#[derive(Debug, Clone, PartialEq)]
pub struct HealthTrail {
    /// `_previousHealth`.
    pub previous: i32,
    /// `_previousStepHealth`: the last frame's.
    pub previous_step: i32,
    /// `_prevHealthUpdate`: when the timer runs out, seconds.
    pub until: f32,
}

/// What a frame of [`HealthTrail::step`] found.
#[derive(Debug, Clone, PartialEq)]
pub struct Step {
    /// `_prevHealthUpdate.RemainPart`: 1 right after a change, 0 when it
    /// has run out.
    pub remain: f32,
    /// The cells (counted from the first) that have just been lost and
    /// drop a red cell (`CreateHealthBarDamage`).
    pub lost: Vec<i32>,
}

impl HealthTrail {
    /// A fresh bar (`Init`: both are `MaxHealth`; `Reset` made them 0, so
    /// a car that comes back fills up with a green fade).
    pub fn new(previous: i32) -> HealthTrail {
        HealthTrail { previous, previous_step: previous, until: 0.0 }
    }

    /// The top of `DrawHealth`, up to the damage particles: `now` in
    /// seconds, `health` the car's.
    pub fn step(&mut self, now: f32, health: i32) -> Step {
        if health != self.previous_step {
            self.until = now + TRAIL;
        } else if now > self.until {
            // `NeedUpdate` re-arms the timer as it answers.
            self.until = now + TRAIL;
            self.previous = health;
        }
        let mut lost = Vec::new();
        if self.previous_step > health {
            let int_health = (health - 1) / PER_CELL;
            let count = (self.previous_step - 1) / PER_CELL - int_health;
            lost = (0..count).map(|i| int_health + 1 + i).collect();
        }
        self.previous_step = health;
        Step { remain: (self.until - now) / TRAIL, lost }
    }
}

/// The health cells, back to front (`DrawHealth` up to the shield).
/// `phase` is `Sawtooth_Period6280`.
pub fn health_cells(screen_width: f32, k: f32, max_health: i32, health: i32, previous: i32, remain: f32, phase: f32) -> Vec<Item> {
    let max_n = cell_count(max_health);
    let cur_n = health / PER_CELL;
    let last = (health - cur_n * PER_CELL) as f32 / PER_CELL as f32;
    let prev_n = (previous as f32 / PER_CELL as f32 + 0.5) as i32;
    let (mut heal_n, mut dam_n) = (-1, cur_n);
    if previous > health {
        dam_n = prev_n;
    } else if previous < health {
        heal_n = previous / PER_CELL;
    }
    // The shimmer that runs along the bar.
    let highlight = if cur_n > 0 {
        let n = (phase * max_n as f32 * 5.0 + 0.5) as i32 % (cur_n * 5);
        if n > cur_n { -1 } else { n }
    } else {
        (phase * max_n as f32 + 0.5) as i32 % 2
    };
    let first = first_cell(screen_width, k, max_health);
    let size = CELL * k;
    let delta = (CELL.x + GAP) * k;
    let mut c = HEALTH;
    let mut out = Vec::new();
    for i in 0..max_n {
        let alpha = if i == highlight { 1.5 } else { 1.0 };
        let center = first + Vec2::new(delta * i as f32, 0.0);
        if i == heal_n {
            c = lerp(HEALTH, HEAL, remain);
        }
        if i == cur_n {
            if last > 0.0 {
                out.push(solid(center, size * last, fade(c, alpha)));
            }
            c = lerp(MAX_HEALTH, DAMAGE, remain);
        }
        if i == dam_n {
            c = MAX_HEALTH;
        }
        out.push(solid(center, size, fade(c, alpha)));
    }
    out
}

/// The shield's thin cells after the health's (`DrawHealth`'s shield
/// half). `value` is `FloatValue`, `triangle` `Triangle_Period500`.
pub fn shield_cells(screen_width: f32, k: f32, max_health: i32, max: i32, value: f32, triangle: f32) -> Vec<Item> {
    let first = first_cell(screen_width, k, max_health);
    let delta = (CELL.x + GAP) * k;
    let size = Vec2::new(CELL.x * 0.4 * k, CELL.y * k);
    let step = size.x + GAP * k;
    // From the next cell's left edge.
    let mut x = first.x + delta * cell_count(max_health) as f32 - CELL.x * k * 0.5;
    let points = value as i32;
    let max_n = max / PER_CELL;
    let cur_n = points / PER_CELL;
    let c = if points == 0 {
        // Yellow to black, at 0.8.
        fade(lerp([1.0, 1.0, 0.0, 1.0], [0.0, 0.0, 0.0, 1.0], triangle), 0.8)
    } else {
        SHIELD
    };
    let mut alpha = 1.0;
    let mut out = Vec::new();
    for i in 0..max_n {
        if i >= cur_n && points > 0 {
            alpha = 0.25;
        }
        // The thin cells grow from their left edge, centred in height.
        out.push(solid(Vec2::new(x + size.x / 2.0, first.y), size, fade(c, alpha)));
        if i == cur_n {
            let part = (value % PER_CELL as f32) / PER_CELL as f32;
            alpha = 1.0;
            out.push(solid(Vec2::new(x + size.x / 2.0, first.y), Vec2::new(size.x, size.y * part), fade(c, alpha)));
        }
        x += step;
    }
    out
}

/// What the icons need to know.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Gear {
    pub magnet: bool,
    pub accelerator: bool,
    /// Any weapon, one the player fires or not.
    pub weapon: bool,
}

/// The gadget's and the weapon's icons left of the health bar
/// (`DrawGadgetIcon`): each is the cell's height, 0.75 white, the
/// gadget's turned a quarter of a half turn counter-clockwise, placed
/// leftwards from the first cell by [`icon_step`]. `size` is a
/// picture's own size.
pub fn icons(screen_width: f32, k: f32, max_health: i32, gear: Gear, size: impl Fn(&str) -> Vec2) -> Vec<Item> {
    let mut at = first_cell(screen_width, k, max_health);
    let mut out = Vec::new();
    let gadget = if gear.magnet {
        Some(ICON_MAGNET)
    } else if gear.accelerator {
        Some(ICON_ACCELERATOR)
    } else {
        None
    };
    let list = [(gadget, -std::f32::consts::FRAC_PI_4), (gear.weapon.then_some(ICON_WEAPON), 0.0)];
    for (sprite, rotation) in list {
        let Some(sprite) = sprite else { continue };
        let own = size(sprite);
        let scale = CELL.y * k / own.y;
        let drawn = own * scale;
        at.x -= icon_step(drawn.x, k);
        out.push(Item { sprite, center: at, size: drawn, rotation, color: ICON });
    }
    out
}

/// How far an icon's middle lies left of the last one:
/// `icon.Width * scale / 2 + size.X / 2 + 4 * UISizeMultiplier`.
pub fn icon_step(drawn_width: f32, k: f32) -> f32 {
    drawn_width / 2.0 + CELL.x * k / 2.0 + GAP * k
}

/// A picture that fades (and may fall) for a while: a lost health cell,
/// the lives battery.
#[derive(Debug, Clone, PartialEq)]
pub struct Fx {
    pub sprite: &'static str,
    pub center: Vec2,
    pub size: Vec2,
    pub rgb: [f32; 3],
    pub life: f32,
    /// The speed (px/s, down the screen) it reaches at the end of its life,
    /// from rest.
    pub fall_speed: f32,
}

impl Fx {
    /// What it looks like `age` seconds in; none once it is over. Colour to
    /// transparent, as the particle's styles blend.
    pub fn at(&self, age: f32) -> Option<Item> {
        if age >= self.life {
            return None;
        }
        let t = age / self.life;
        let drop = self.fall_speed * age * age / (2.0 * self.life);
        Some(Item {
            sprite: self.sprite,
            center: self.center + Vec2::new(0.0, drop),
            size: self.size,
            rotation: 0.0,
            color: [self.rgb[0], self.rgb[1], self.rgb[2], 1.0 - t],
        })
    }
}

/// `CreateHealthBarDamage` for a lost cell: a red cell that falls away,
/// `_healthBarDamageUiStyle` (0.5 s, speed 0 to 256).
pub fn damage_fx(screen_width: f32, k: f32, max_health: i32, cell: i32) -> Fx {
    let delta = (CELL.x + GAP) * k;
    Fx {
        sprite: WHITE_PIXEL,
        center: first_cell(screen_width, k, max_health) + Vec2::new(delta * cell as f32, 0.0),
        size: CELL * k,
        rgb: [1.0, 0.0, 0.0],
        life: 0.5,
        fall_speed: 256.0,
    }
}

/// `ShowEnergy(lives)`: the battery in the middle of the screen with a
/// white bar for each life left and a red one for the one just spent
/// (`_battery*UiStyle`); with none left (`lives < 0`) a red battery.
pub fn lives_fx(screen: Vec2, k: f32, lives: i32) -> Vec<Fx> {
    let middle = screen / 2.0;
    let battery = |rgb: [f32; 3]| Fx {
        sprite: BATTERY,
        center: middle,
        size: Vec2::new(128.0, 64.0) * k,
        rgb,
        life: 3.0,
        fall_speed: 0.0,
    };
    if lives < 0 {
        return vec![battery([1.0, 0.0, 0.0])];
    }
    let mut out = vec![battery([1.0; 3])];
    // `GetMaxWidth() + 4`, the bar's width (29 × k) and a gap of 4 pixels.
    let w = 29.0 * k + 4.0;
    let mut x = middle.x - (w + 4.0);
    let bar = |x: f32, rgb: [f32; 3], life: f32| Fx {
        sprite: WHITE_PIXEL,
        center: Vec2::new(x, middle.y),
        size: Vec2::new(29.0, 43.0) * k,
        rgb,
        life,
        fall_speed: 0.0,
    };
    for _ in 0..lives {
        out.push(bar(x, [1.0; 3], 3.0));
        x += w;
    }
    out.push(bar(x, [1.0, 0.0, 0.0], 1.0));
    out
}
