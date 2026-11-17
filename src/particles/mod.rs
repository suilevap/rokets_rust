//! Particles: smoke, sparks, booms, as a fixed pool of sprites that each
//! move and change from a begin state to an end state over a lifetime.
//!
//! C# origin: `Particles/` (`Particle`, `ParticleStyle`, `ParticlesPool`)
//! and the styles in `ParticleSystemManager` (2011 onwards). The design
//! is kept: an effect is a [`Style`] (plain data: two states and a
//! lifetime) plus a spawn call; the pool is small and fixed, so a burst
//! of effects costs nothing extra; most effects are *optional*
//! ([`Pool::try_add`]) and the oldest give way to new ones when it is
//! full, while the few that matter ([`Pool::add`]) are never replaced.
//!
//! Everything here is in our space (y up, angles counter-clockwise, see
//! `units`); C# styles are converted where they are written down
//! ([`styles`]). Drawing is `render/particles`.

pub mod styles;

use std::collections::VecDeque;

use bevy::math::Vec2;

/// How a particle looks and moves at one moment of its life.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct State {
    /// World units per second along its direction.
    pub speed: f32,
    /// Times the texture's size in the C# game's pixels.
    pub scale: Vec2,
    /// How fast the sprite turns, radians per second.
    pub view_rotation: f32,
    /// How fast its direction of travel turns, radians per second.
    pub rotation: f32,
    /// Premultiplied RGBA (XNA's colours), 0..1.
    pub color: [f32; 4],
    /// Its `DepthConst` height in C#: 0 on the ground, negative above it.
    /// Only says whether it is drawn under the cars or over them.
    pub height: f32,
}

/// An effect: what a particle starts and ends as.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Style {
    pub begin: State,
    pub end: State,
    /// Seconds.
    pub lifetime: f32,
    /// Content name of the sprite under `Sprites/`.
    pub texture: &'static str,
}

impl Style {
    /// The state `t` seconds into its life (`ParticleStyle.GetState`): all
    /// values move linearly; a particle fading to transparent keeps its
    /// begin colour and fades it out.
    pub fn state_at(&self, t: f32) -> State {
        let a = (t / self.lifetime).clamp(0.0, 1.0);
        let lerp = |x: f32, y: f32| x + (y - x) * a;
        let (b, e) = (&self.begin, &self.end);
        let color = if e.color == [0.0; 4] {
            b.color.map(|c| c * (1.0 - a))
        } else {
            std::array::from_fn(|i| lerp(b.color[i], e.color[i]))
        };
        State {
            speed: lerp(b.speed, e.speed),
            scale: b.scale + (e.scale - b.scale) * a,
            view_rotation: lerp(b.view_rotation, e.view_rotation),
            rotation: lerp(b.rotation, e.rotation),
            color,
            height: lerp(b.height, e.height),
        }
    }
}

/// One live particle.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Particle {
    pub position: Vec2,
    /// Direction of travel, radians.
    pub direction: f32,
    /// The sprite's angle, radians.
    pub view_direction: f32,
    pub age: f32,
    pub style: &'static Style,
    pub state: State,
    /// May be replaced by a newer particle while alive.
    optional: bool,
}

/// A fixed number of particles (`ParticlesPool`).
#[derive(Clone, Debug)]
pub struct Pool {
    slots: Vec<Option<Particle>>,
    /// Slots to fill next, oldest first; optional particles stay in it.
    free: VecDeque<usize>,
}

impl Pool {
    pub fn new(capacity: usize) -> Pool {
        Pool {
            slots: vec![None; capacity],
            free: (0..capacity).collect(),
        }
    }

    /// Adds a particle that may give way to newer ones when the pool is
    /// full (most effects).
    pub fn try_add(&mut self, style: &'static Style, position: Vec2, direction: f32, view_direction: f32) {
        if let Some(i) = self.free.pop_front() {
            self.slots[i] = Some(new_particle(style, position, direction, view_direction, true));
            self.free.push_back(i);
        }
    }

    /// Adds a particle that lives out its life, if there is a slot.
    pub fn add(&mut self, style: &'static Style, position: Vec2, direction: f32, view_direction: f32) {
        if let Some(i) = self.free.pop_front() {
            self.slots[i] = Some(new_particle(style, position, direction, view_direction, false));
        }
    }

    /// Ages and moves every particle by `dt` seconds of game time.
    pub fn step(&mut self, dt: f32) {
        for (i, slot) in self.slots.iter_mut().enumerate() {
            let Some(p) = slot else {
                continue;
            };
            p.age += dt;
            if p.age >= p.style.lifetime {
                if !p.optional {
                    self.free.push_front(i);
                }
                *slot = None;
                continue;
            }
            p.state = p.style.state_at(p.age);
            p.view_direction += p.state.view_rotation * dt;
            p.direction += p.state.rotation * dt;
            p.position += Vec2::from_angle(p.direction) * p.state.speed * dt;
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = &Particle> {
        self.slots.iter().flatten()
    }

    pub fn clear(&mut self) {
        *self = Pool::new(self.slots.len());
    }
}

fn new_particle(style: &'static Style, position: Vec2, direction: f32, view_direction: f32, optional: bool) -> Particle {
    Particle {
        position,
        direction,
        view_direction,
        age: 0.0,
        style,
        state: style.begin,
        optional,
    }
}
