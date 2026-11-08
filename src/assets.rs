//! Asset names as the C# content uses them, and loading XML as text.
//!
//! Content paths in the map XML (`Entities\CargoQuest`) have backslashes,
//! no extension and arbitrary case: Windows and the XNA content pipeline
//! ignored case, Linux and the web do not. `assets/manifest.txt` (written by
//! `tools/import_content.py`) lists the real files; names are resolved
//! through it.

use std::collections::HashMap;
use std::sync::LazyLock;

use bevy::asset::{AssetLoader, LoadContext, io::Reader};
use bevy::prelude::*;

static MANIFEST: LazyLock<HashMap<String, &'static str>> = LazyLock::new(|| {
    include_str!("../assets/manifest.txt")
        .lines()
        .filter(|l| !l.is_empty())
        .map(|l| (l.to_ascii_lowercase(), l))
        .collect()
});

/// Resolves a content path (relative to `assets/`, any slashes or case,
/// with or without extension) to the real asset path.
pub fn resolve(path: &str, default_ext: &str) -> Option<&'static str> {
    let norm = path.trim().replace('\\', "/").to_ascii_lowercase();
    MANIFEST
        .get(&norm)
        .or_else(|| MANIFEST.get(&format!("{norm}.{default_ext}")))
        .copied()
}

/// A sprite under `Sprites/`, by its content name (`Cars\SandBuggy\Base`).
pub fn sprite_path(content_name: &str) -> Option<&'static str> {
    resolve(&format!("Sprites/{content_name}"), "png")
}

/// The XML of a single-player map by name.
pub fn map_path(name: &str) -> Option<&'static str> {
    resolve(&format!("Maps/Singleplayer/{name}"), "xml")
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
        &["xml"]
    }
}

pub struct ContentAssetsPlugin;

impl Plugin for ContentAssetsPlugin {
    fn build(&self, app: &mut App) {
        app.init_asset::<XmlText>()
            .init_asset_loader::<XmlTextLoader>();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_any_case_and_slashes() {
        assert_eq!(
            sprite_path("cars\\sandbuggy\\base"),
            Some("Sprites/Cars/SandBuggy/Base.png")
        );
        assert_eq!(map_path("editortest"), Some("Maps/Singleplayer/EditorTest.xml"));
        assert_eq!(sprite_path("Nope\\Missing"), None);
    }
}
