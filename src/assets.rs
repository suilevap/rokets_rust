//! Asset names, and loading XML as text.
//!
//! The C# content names assets the Windows way (backslashes, any case) and
//! relies on Windows ignoring case. `tools/import_content.py` normalises the
//! names once, when it imports: every name in `assets/` content is written
//! with forward slashes and the exact case of its file, and the Rust
//! constants are written the same way. Names are therefore used as they are;
//! the helpers below only add the folder and the extension Bevy needs.
//! `tests/asset_names.rs` checks that every name the content uses is a file.

use bevy::asset::{AssetLoader, LoadContext, io::Reader};
use bevy::prelude::*;

/// A sprite under `Sprites/`, by its content name (`Cars/SandBuggy/Base`).
pub fn sprite_path(content_name: &str) -> String {
    format!("Sprites/{content_name}.png")
}

/// A sound effect under `Sounds/` by name (`CoinCollected`).
pub fn sound_path(name: &str) -> String {
    format!("Sounds/{name}.wav")
}

/// A song under `Sounds/Music/` by name (`2_1`).
pub fn music_path(name: &str) -> String {
    format!("Sounds/Music/{name}.ogg")
}

/// The XML of a single-player map by name. The name is case-sensitive, as
/// the file is (`-m escape` does not find `Escape.xml`).
pub fn map_path(name: &str) -> String {
    format!("Maps/Singleplayer/{name}.xml")
}

/// Raw text of an XML file.
#[derive(Asset, TypePath, Debug, Clone)]
pub struct XmlText(pub String);

#[derive(Default, TypePath)]
pub struct XmlTextLoader;

impl AssetLoader for XmlTextLoader {
    type Asset = XmlText;
    type Settings = ();
    type Error = std::io::Error;

    async fn load(
        &self,
        reader: &mut dyn Reader,
        _settings: &(),
        _load_context: &mut LoadContext<'_>,
    ) -> Result<XmlText, Self::Error> {
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).await?;
        let text = String::from_utf8(bytes)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        Ok(XmlText(text))
    }

    fn extensions(&self) -> &[&str] {
        &["xml", "resx"]
    }
}

pub struct ContentAssetsPlugin;

impl Plugin for ContentAssetsPlugin {
    fn build(&self, app: &mut App) {
        app.init_asset::<XmlText>()
            .init_asset_loader::<XmlTextLoader>();
    }
}
