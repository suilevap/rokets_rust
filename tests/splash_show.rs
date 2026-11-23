//! `ShowSplash` and `HideSplash()` hand the splash to the screen that draws
//! it.

use bevy::prelude::*;
use tilerider::actions::{ActionsPlugin, CurrentActions, SplashCommand, run};
use tilerider::content::map::MapData;

fn app() -> App {
    let map = MapData::parse(
        r#"<XnaContent><Asset Type="GameLibrary.MapData"><Name>T</Name><Actions>
             <Item Type="GameLibrary.ShowSplash"><Name>Show</Name>
               <SplashName>Blackout</SplashName><AutoHideTimeSec>2</AutoHideTimeSec>
               <Clickable>false</Clickable><ActionOnHide>Done</ActionOnHide></Item>
             <Item Type="GameLibrary.ShowSplash"><Name>Open</Name>
               <SplashName>ShowCount</SplashName><AutoHideTimeSec>-1</AutoHideTimeSec>
               <Clickable>false</Clickable><ActionOnHide Null="true" /></Item>
             <Item Type="GameLibrary.ShowSplash"><Name>Nothing</Name>
               <SplashName Null="true" /><AutoHideTimeSec>2</AutoHideTimeSec>
               <Clickable>false</Clickable><ActionOnHide Null="true" /></Item>
             <Item Type="GameLibrary.SetVariable"><Name>Done</Name></Item>
           </Actions></Asset></XnaContent>"#,
    )
    .unwrap();
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, ActionsPlugin)).insert_resource(CurrentActions(map.actions));
    app
}

fn commands(app: &mut App) -> Vec<SplashCommand> {
    app.world_mut().resource_mut::<Messages<SplashCommand>>().drain().collect()
}

#[test]
fn a_splash_is_shown_with_its_time() {
    let mut app = app();
    assert!(run(app.world_mut(), "Show", None, 0));
    assert!(run(app.world_mut(), "Open", None, 0));
    assert_eq!(
        commands(&mut app),
        vec![
            SplashCommand::Show { name: "Blackout".into(), auto_hide: 2.0 },
            SplashCommand::Show { name: "ShowCount".into(), auto_hide: -1.0 },
        ]
    );
}

#[test]
fn no_name_hides_the_splash() {
    let mut app = app();
    assert!(run(app.world_mut(), "Nothing", None, 0));
    assert!(run(app.world_mut(), "HideSplash()", None, 0));
    assert_eq!(commands(&mut app), vec![SplashCommand::Hide, SplashCommand::Hide]);
}
