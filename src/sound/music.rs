//! Level music (`SoundManager`'s songs, `PlayLevelMusic`): what song should
//! be heard and how loud, apart from anything that plays it.
//!
//! Each map names up to three songs (`MusicAmbient`, `MusicDynamic1`,
//! `MusicDynamic2`; files under `Sounds/Music`). When the map starts the
//! ambient one plays, looped (`SoundManager.SetLevelMusics`); a map with no
//! ambient song fades the music out. `PlayLevelMusic` (and its templates
//! `PlayMusic(type)` and `PlayMusicLooped(type)`) plays one of the three, or
//! fades the music out for `None` (`SoundManager.PlayLevelMusic`).
//!
//! There is one song at a time (`MediaPlayer`). Asking for the song that is
//! playing changes nothing, but a song that is fading out is called back to
//! full volume. Asking for another one while a song plays fades that one out
//! first (volume eased to 0 at `K = 3`, a bit over a second) and then starts
//! the new one from the beginning, fading it in; with nothing playing it
//! starts at once, from silence. So a song that continues over two maps (the
//! same ambient on `2WeeksAgo` and `Assembly`) keeps playing through the
//! change.
//!
//! [`Jukebox`] is that state, stepped by [`Jukebox::update`] in real time
//! (music is no gameplay: slow motion does not slow it). It hands what the
//! media player should do to [`MusicCue`] messages; `playback` plays them
//! with Bevy's audio. The music is on by default (`MusicOn`); `--no-music`
//! starts with it off, and `M` switches it (the C# menu's button,
//! `MusicSwitchOff/On`).
//!
//! `RUST_LOG=tilerider::sound=debug` lists the songs started and stopped.
//!
//! Not ported:
//! - Pausing and resuming the song with the pause menu
//!   (`PauseCurrentSong`, `UnpauseCurrentSong`, the `Pausing` transition and
//!   resuming a paused song in `PlayOrResumeSong`): the port has no pause
//!   menu.
//! - The menu's song (`MainMenuSong`) and the level end's (`LevelEndSong`,
//!   `Short1`: C# played it only with `Settings.SkipLevelEndSplash` off, and
//!   it is on): no menus, and the song is not imported.
//! - The music director (`MusicDirector`: `Dynamic1` when a battle starts,
//!   `Ambient` when it ends, from `BattleDirector`): it works only once the
//!   `SpawnEnemy` actions have switched it on, and they are not ported.
//! - `MediaPlayer.GameHasControl` (another app's music playing on a phone
//!   switches the game's off) and the saved `MusicOn` setting of the
//!   progress file: there is no phone, no saved setting.
//! - `MediaPlayer.Volume` was set only when it moved by more than a tenth of
//!   `MasterMusicVolume`, to spare the platforms' media APIs; here it is set
//!   every frame.
//! - The Windows task chain (`TaskAdd`) that started the songs off the
//!   game's thread.

use std::collections::VecDeque;

use bevy::prelude::*;

use crate::actions::{AddMapAction, Run};
use crate::content::actions::defs::{LevelMusicType, PlayLevelMusic};
use crate::game::{GameState, LoadedMap};

/// `SoundManager.MasterMusicVolume`.
pub const MASTER_VOLUME: f32 = 0.4;

/// `new Hardastic(0, 3f)`: the speed of the music volume's easing, per second.
const FADE_K: f32 = 3.0;

/// What the media player should do (`MediaPlayer.Play`, `Stop`).
#[derive(Message, Debug, Clone, PartialEq, Eq)]
pub enum MusicCue {
    /// Start the song from the beginning; `looped` is `IsRepeating`.
    Play { track: String, looped: bool },
    /// Stop it.
    Stop,
}

/// `SongTransitionState` (without `Pausing`, see "Not ported").
#[derive(Clone, Debug, PartialEq, Eq)]
enum Transition {
    None,
    /// Fading the song out to start `pending`.
    Changing { track: String, looped: bool },
    /// Fading the song out to stop it.
    Stopping,
}

/// The `MediaPlayer`: the song it was given last, and whether it plays it.
#[derive(Clone, Debug)]
struct Media {
    /// `Queue.ActiveSong`: stays after the song stops.
    active: Option<String>,
    playing: bool,
    repeating: bool,
}

/// The level music, as `SoundManager` kept it (`MusicOn`,
/// `_levelMusic`, `_currentMusicVolume`, `_songTransitionState`).
#[derive(Resource, Debug)]
pub struct Jukebox {
    on: bool,
    /// The names of the three songs, by [`LevelMusicType`] (`Ambient`,
    /// `Dynamic1`, `Dynamic2`). A map that leaves one out keeps the last
    /// map's (C# only ever set them).
    levels: [Option<String>; 3],
    /// `_currentMusicVolume`.
    volume: f32,
    destination: f32,
    transition: Transition,
    media: Media,
    cues: VecDeque<MusicCue>,
}

impl Default for Jukebox {
    fn default() -> Self {
        Jukebox {
            on: true,
            levels: [None, None, None],
            volume: 0.0,
            destination: 0.0,
            transition: Transition::None,
            media: Media {
                active: None,
                playing: false,
                repeating: false,
            },
            cues: VecDeque::new(),
        }
    }
}

fn slot(kind: LevelMusicType) -> Option<usize> {
    match kind {
        LevelMusicType::None => None,
        LevelMusicType::Ambient => Some(0),
        LevelMusicType::Dynamic1 => Some(1),
        LevelMusicType::Dynamic2 => Some(2),
    }
}

impl Jukebox {
    /// A jukebox with the music on or off (`SoundManager.MusicOn`).
    pub fn new(on: bool) -> Jukebox {
        Jukebox { on, ..default() }
    }

    pub fn is_on(&self) -> bool {
        self.on
    }

    /// `MusicSwitchOff`: the music stops at once and stays off.
    pub fn switch_off(&mut self) {
        if self.on {
            self.on = false;
            self.stop_media();
        }
    }

    /// `MusicSwitchOn`: the next song asked for plays.
    pub fn switch_on(&mut self) {
        self.on = true;
    }

    /// The song of a kind, as the map (or an earlier one) named it.
    pub fn level_track(&self, kind: LevelMusicType) -> Option<&str> {
        self.levels[slot(kind)?].as_deref()
    }

    /// The three songs the maps have named, to load.
    pub fn level_tracks(&self) -> impl Iterator<Item = &str> {
        self.levels.iter().flatten().map(String::as_str)
    }

    /// The song that is playing, with whether it repeats, even while it
    /// fades out.
    pub fn playing(&self) -> Option<(&str, bool)> {
        let track = self.media.active.as_deref().filter(|_| self.media.playing)?;
        Some((track, self.media.repeating))
    }

    /// How loud the song is now (`_currentMusicVolume`), 0..`MASTER_VOLUME`.
    pub fn volume(&self) -> f32 {
        self.volume
    }

    /// `SoundManager.SetLevelMusics`: a map's songs. A name given is
    /// remembered, and the ambient song plays, looped; no ambient song
    /// fades the music out.
    pub fn set_level_music(&mut self, ambient: &str, dynamic1: &str, dynamic2: &str) {
        for (i, name) in [ambient, dynamic1, dynamic2].into_iter().enumerate() {
            let name = name.trim();
            if !name.is_empty() {
                self.levels[i] = Some(name.to_string());
                if i == 0 {
                    self.play_or_resume(name, true);
                }
            } else if i == 0 {
                self.stop_current_song();
            }
        }
    }

    /// `SoundManager.PlayLevelMusic`: a song of the map, or a fade out for
    /// `None` and for a kind the map has no song for.
    pub fn play_level_music(&mut self, kind: LevelMusicType, looped: bool) {
        match slot(kind).and_then(|i| self.levels[i].clone()) {
            Some(track) => self.play_or_resume(&track, looped),
            None => self.stop_current_song(),
        }
    }

    /// `SoundManager.PlayOrResumeSong`: plays the song; one that is
    /// already playing goes on (a fading one is called back); another one
    /// playing fades out first.
    pub fn play_or_resume(&mut self, track: &str, looped: bool) {
        if !self.on {
            return;
        }
        if !self.media.playing {
            self.volume = 0.0;
        }
        let same = self.media.active.as_deref() == Some(track);
        if self.media.playing && same {
            // `IsRepeating` stays as it was.
            self.transition = Transition::None;
            self.destination = MASTER_VOLUME;
        } else if !self.media.playing {
            self.destination = MASTER_VOLUME;
            self.transition = Transition::None;
            self.media = Media {
                active: Some(track.to_string()),
                playing: true,
                repeating: looped,
            };
            self.cues.push_back(MusicCue::Play {
                track: track.to_string(),
                looped,
            });
        } else {
            self.transition = Transition::Changing {
                track: track.to_string(),
                looped,
            };
            self.destination = 0.0;
        }
    }

    /// `SoundManager.StopCurrentSong`: fades the song out and stops it.
    pub fn stop_current_song(&mut self) {
        self.transition = Transition::Stopping;
        self.destination = 0.0;
    }

    /// The song ended by itself (the media player's state turns `Stopped`).
    pub fn song_ended(&mut self) {
        self.media.playing = false;
    }

    fn stop_media(&mut self) {
        if self.media.playing {
            self.cues.push_back(MusicCue::Stop);
        }
        self.media.playing = false;
    }

    /// `SoundManager.Update`: eases the volume and carries out the
    /// transitions that have faded out.
    pub fn update(&mut self, dt: f32) {
        if !self.on {
            return;
        }
        // `Hardastic.Update`.
        self.volume += (self.destination - self.volume) * FADE_K * dt;
        if dt * FADE_K > 1.0 {
            self.volume = self.destination;
        }
        match self.transition.clone() {
            Transition::Changing { track, looped } if self.volume < 0.01 => {
                self.stop_media();
                self.transition = Transition::None;
                self.play_or_resume(&track, looped);
            }
            Transition::Stopping if self.volume < 0.005 => {
                self.stop_media();
                self.transition = Transition::None;
            }
            _ => {}
        }
    }

    /// What the media player is to do, oldest first.
    pub fn take_cues(&mut self) -> Vec<MusicCue> {
        self.cues.drain(..).collect()
    }
}

/// `PlayLevelMusic`, as the action. It works with nothing but a
/// [`Jukebox`], for tests.
pub struct MusicActionsPlugin;

impl Plugin for MusicActionsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Jukebox>()
            .add_message::<MusicCue>()
            .add_map_action::<PlayLevelMusic, _>(play_level_music);
    }
}

pub struct MusicPlugin;

impl Plugin for MusicPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(MusicActionsPlugin)
            .add_systems(OnEnter(GameState::Playing), start_level_music)
            .add_systems(Update, (switch_music, step_music, send_cues).chain());
        #[cfg(feature = "sound")]
        app.add_plugins(super::playback::MusicPlaybackPlugin);
    }
}

fn play_level_music(In(run): In<Run<PlayLevelMusic>>, mut jukebox: ResMut<Jukebox>) -> bool {
    jukebox.play_level_music(run.data.level_music_type, run.data.looped);
    true
}

/// The map's songs, when it starts (C# queued it for the map's first update).
fn start_level_music(map: Res<LoadedMap>, mut jukebox: ResMut<Jukebox>) {
    let name = |s: &Option<String>| s.clone().unwrap_or_default();
    jukebox.set_level_music(
        &name(&map.data.music_ambient),
        &name(&map.data.music_dynamic1),
        &name(&map.data.music_dynamic2),
    );
}

/// `M`: the music's button of the C# menu.
fn switch_music(keys: Res<ButtonInput<KeyCode>>, mut jukebox: ResMut<Jukebox>) {
    if keys.just_pressed(KeyCode::KeyM) {
        if jukebox.is_on() {
            jukebox.switch_off();
        } else {
            jukebox.switch_on();
        }
        info!("music {}", if jukebox.is_on() { "on" } else { "off" });
    }
}

fn step_music(time: Res<Time<Real>>, mut jukebox: ResMut<Jukebox>) {
    jukebox.update(time.delta_secs());
}

pub(super) fn send_cues(mut jukebox: ResMut<Jukebox>, mut cues: MessageWriter<MusicCue>) {
    for cue in jukebox.take_cues() {
        debug!("music {cue:?}");
        cues.write(cue);
    }
}
