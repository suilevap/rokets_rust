//! The splash's clock and layout, as plain data (no Bevy app code, so it is
//! unit-tested): when it fades in, holds, fades out, what its countdown
//! reads, which layers have appeared, how the layers sway, and where each
//! layer goes on a screen of any size.
//!
//! C# origin: `RenderableWithTransition` (the appear / hold / hide state
//! machine) and the drawing in `SplashScreen.DrawNormal` /
//! `LoadSplashesData` (scales and offsets). Written down branch by branch in
//! the doc of the parent module.

use bevy::math::Vec2;

use crate::content::splashes::{LayerData, SplashData};
use crate::content::xna::XColor;

/// The screen C# laid the splashes out for (`Settings.ViewWidth/Height`).
pub const DESIGN: Vec2 = Vec2::new(800.0, 480.0);

/// Sway of a layer, in pixels, at parallax 1 and height 1
/// (`SplashScreen.DrawNormal`'s `amount`).
const SWAY_PIXELS: f32 = 16.0;

/// Seconds of one sway cycle (`_timeFromApply % 35`).
const SWAY_PERIOD: f32 = 35.0;

/// How fast the sway follows its swing (`HardasticVector2` K).
const SWAY_FOLLOW: f32 = 3.0;

/// Draw depths of C# (`DepthConst.SplashBackground`, `SplashForeground`):
/// further back is greater.
const DEPTH_BACKGROUND: f32 = -11.0;
const DEPTH_FOREGROUND: f32 = -12.0;

/// `RenderableWithTransition.TranslationState`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    /// Fading in.
    Appearing,
    /// Fully shown (C#'s `None`).
    Shown,
    /// Fading out.
    Hiding,
    /// Gone (or not shown yet).
    Invisible,
}

/// The splash on screen, if any.
#[derive(Clone, Debug)]
pub struct Screen {
    splash: Option<SplashData>,
    phase: Phase,
    /// Seconds into the fade in or out (`_currentTranslationTime`).
    fade: f32,
    /// Seconds since `show`; the hold ends at `auto_hide`.
    since_show: f32,
    /// Seconds of the hold, 0 for no end (`_autoHideTime`).
    auto_hide: f32,
    /// `_timeFromApply`: drives the sway and the layers' delays.
    since_apply: f32,
    sway: Vec2,
    activated: Vec<bool>,
}

impl Default for Screen {
    fn default() -> Self {
        Screen {
            splash: None,
            phase: Phase::Invisible,
            fade: 0.0,
            since_show: 0.0,
            auto_hide: 0.0,
            since_apply: 0.0,
            sway: Vec2::ZERO,
            activated: Vec::new(),
        }
    }
}

impl Screen {
    /// `SplashScreen.Show` (`Init`, then `RenderableWithTransition.Show`):
    /// the splash replaces the one on screen at once and fades in. The
    /// clock of a fade in progress goes on, one that was fully shown or
    /// gone starts again, and one fading out turns to fade in from where it
    /// was. `auto_hide` is the seconds until it hides itself; none (0 or
    /// less) stays until `hide`.
    pub fn show(&mut self, splash: SplashData, auto_hide: f32) {
        self.activated = vec![false; splash.layers.items.len()];
        self.splash = Some(splash);
        if self.phase != Phase::Appearing {
            if matches!(self.phase, Phase::Invisible | Phase::Shown) {
                self.fade = 0.0;
            }
            self.phase = Phase::Appearing;
        }
        self.auto_hide = auto_hide.max(0.0);
        self.since_show = 0.0;
        self.since_apply = 0.0;
        self.sway = Vec2::ZERO;
    }

    /// `RenderableWithTransition.Hide` (`SplashScreen.HideScreen`): fade out
    /// from now; one that is still fading in fades out from the same
    /// brightness. Nothing happens to one that is gone or already fading.
    pub fn hide(&mut self) {
        if self.phase == Phase::Invisible || self.phase == Phase::Hiding {
            return;
        }
        self.fade = if self.phase == Phase::Appearing { (self.translation() - self.fade).max(0.0) } else { 0.0 };
        self.phase = Phase::Hiding;
    }

    /// Takes the splash off at once, with no fade.
    pub fn clear(&mut self) {
        *self = Screen::default();
    }

    /// Seconds of a fade (`SplashData.TranslationTime`).
    fn translation(&self) -> f32 {
        self.splash.as_ref().map_or(0.0, |s| s.translation_time)
    }

    /// Goes `dt` seconds on (`RenderableWithTransition.Render`, with the
    /// sway and the layers' delays of `SplashScreen.DrawNormal`).
    pub fn update(&mut self, dt: f32) {
        let Some((t, parallax)) = self.splash.as_ref().map(|s| (s.translation_time, s.parallax_amount)) else {
            return;
        };
        match self.phase {
            Phase::Appearing if self.fade >= t => {
                self.phase = Phase::Shown;
                self.fade = 0.0;
            }
            Phase::Hiding if self.fade >= t => self.phase = Phase::Invisible,
            Phase::Shown if self.auto_hide > 0.0 && self.since_show > self.auto_hide => self.hide(),
            _ => {}
        }
        if self.phase == Phase::Invisible {
            return;
        }
        let amount = SWAY_PIXELS * parallax;
        let swing = ((self.since_apply % SWAY_PERIOD) / SWAY_PERIOD * std::f32::consts::TAU).sin();
        let destination = Vec2::new(swing * amount, 0.0);
        self.fade += dt;
        self.since_show += dt;
        self.since_apply += dt;
        if dt * SWAY_FOLLOW > 1.0 {
            self.sway = destination;
        } else {
            self.sway += (destination - self.sway) * SWAY_FOLLOW * dt;
        }
        let layers = self.splash.iter().flat_map(|s| s.layers.items.iter());
        for (done, layer) in self.activated.iter_mut().zip(layers) {
            if !*done && self.since_apply > layer.delay_sec {
                *done = true;
            }
        }
    }

    pub fn splash(&self) -> Option<&SplashData> {
        self.splash.as_ref()
    }

    pub fn phase(&self) -> Phase {
        self.phase
    }

    /// Whether anything is drawn (C#'s `Visible`).
    pub fn visible(&self) -> bool {
        self.splash.is_some() && self.phase != Phase::Invisible
    }

    /// How opaque the whole splash is: 0 to 1 over the fade in, 1 held,
    /// 1 to 0 over the fade out.
    pub fn alpha(&self) -> f32 {
        let t = self.translation();
        let progress = if t > 0.0 { self.fade / t } else { 1.0 };
        match self.phase {
            Phase::Appearing => progress.min(1.0),
            Phase::Shown => 1.0,
            Phase::Hiding => (1.0 - progress).max(0.0),
            Phase::Invisible => 0.0,
        }
    }

    /// The text of the countdown slot (`BindText[0]`): the seconds left of
    /// the hold to a tenth, while it is held; empty otherwise.
    pub fn countdown(&self) -> String {
        let left = self.auto_hide - self.since_show;
        if self.phase == Phase::Shown && left > 0.0 { format!("{left:.1}") } else { String::new() }
    }

    /// Whether the layer has appeared (its delay is over).
    pub fn layer_active(&self, layer: usize) -> bool {
        self.activated.get(layer).copied().unwrap_or(false)
    }

    /// The layers' sway now, before each layer's height.
    pub fn sway(&self) -> Vec2 {
        self.sway
    }
}

/// Where and how large a layer is drawn.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Placement {
    /// Top left corner, in screen pixels from the top left of the screen.
    pub top_left: Vec2,
    /// Size over the layer's own (a sprite's pixels, a text's font size).
    pub scale: f32,
    /// Fills the whole screen instead (`StretchToScreen`).
    pub stretch: bool,
}

/// `LoadSplashesData`'s scales for a screen of `view` pixels: the splash's
/// layers measured from the screen's centre, their offsets (and, for
/// `ScaleUpWhenNeeded`, their sizes) grown by the screen's size over 800 × 480.
/// `ButtonSizeMultiplier` (a phone's dpi correction, at least 1) is 1 here.
pub fn placement(splash: &SplashData, layer: &LayerData, index: usize, view: Vec2, sway: Vec2) -> Placement {
    let ui = (view.x / DESIGN.x).min(view.y / DESIGN.y);
    let size_scale = if splash.scale_up_when_needed { ui } else { 1.0 };
    let offset_scale = if splash.scale_up_when_needed || splash.scale_up_only_offset_when_needed { ui } else { 1.0 };
    let offset = (Vec2::new(layer.offset.x, layer.offset.y) - DESIGN / 2.0) * offset_scale;
    let swayed = if layer.is_static {
        Vec2::ZERO
    } else if layer.height != -1.0 {
        sway * layer.height
    } else {
        sway * index as f32
    };
    Placement { top_left: view / 2.0 + swayed + offset, scale: layer.scale * size_scale, stretch: layer.stretch_to_screen }
}

/// The layers from the back to the front (`SplashScreen.DrawNormal`'s
/// depth, drawn back to front): layers on the foreground over those that
/// are not, a greater `Height` nearer, the ones without one by their
/// position in the list.
pub fn draw_order(splash: &SplashData) -> Vec<usize> {
    let layers = &splash.layers.items;
    let depth = |i: usize| {
        let l = &layers[i];
        let height = if l.height != -1.0 { l.height } else { (i + 1) as f32 / layers.len() as f32 };
        -height + if l.on_foreground { DEPTH_FOREGROUND } else { DEPTH_BACKGROUND }
    };
    let mut order: Vec<usize> = (0..layers.len()).collect();
    order.sort_by(|&a, &b| depth(b).total_cmp(&depth(a)));
    order
}

/// A colour as C# drew it, times `alpha` (the splash's fade), as a straight
/// (not premultiplied) colour with an alpha. XNA's colours are premultiplied:
/// `80808080` is white at half strength, and a channel above the alpha
/// (`770094FF`) adds light. Bevy's blend is straight, so the colour is
/// divided by its alpha.
pub fn straight_tint(color: XColor, alpha: f32) -> [f32; 4] {
    let [r, g, b, a] = color.to_f32();
    if a <= 0.0 {
        return [0.0; 4];
    }
    [r / a, g / a, b / a, (a * alpha).clamp(0.0, 1.0)]
}

/// The top left corner shift that turns a box of `size` about its own
/// centre (what a UI node does) into a turn about its top left corner
/// (what a C# sprite with a zero origin does): `angle` clockwise, in radians.
pub fn corner_pivot_shift(size: Vec2, angle: f32) -> Vec2 {
    let (sin, cos) = angle.sin_cos();
    let centre = size / 2.0;
    // Screen y points down, so a positive angle is clockwise.
    Vec2::new(cos * centre.x - sin * centre.y, sin * centre.x + cos * centre.y) - centre
}
