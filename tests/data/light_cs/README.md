# C# light targets

Reference light maps from the C# game (`suilevap/rokets`, DesktopGL build),
saved right after a map loaded:

- `final.png`: `LightManager.FinalLightTarget`, the light layer laid over
  the map and its 4-cell border (lamps, shadows, penumbra, blur, full
  light over the void);
- `walls.png`: `LightManager._lightMapTargetTmp`, the walls' light;
- on maps with additive lamps, `add.png` (`_finalLightAddTarget`) and
  `add_info.png` (`_lightAddMapTargetTmp`, where they reach).

They were written by a temporary `Texture2D.SaveAsPng` call at the end of
`LightManager.PrepareLightMapImage` (not committed to the C# repository).
`tests/light_cs.rs` bakes the same maps and checks they still match;
`cargo run --example light_dump -- <map> <dir>` writes ours for a look.
