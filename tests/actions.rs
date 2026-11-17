//! Map actions: references resolve to what they do.

use tilerider::content::actions::{Act, resolve};
use tilerider::content::map::MapData;

fn map() -> MapData {
    MapData::parse(
        r#"<XnaContent><Asset Type="GameLibrary.MapData"><Name>T</Name>
        <Triggers><Item><Id>1</Id><Area>0 0 2 2</Area><Actions><Item>Open</Item></Actions></Item></Triggers>
        <Actions>
          <Item Type="GameLibrary.ShowSuccess"><Name>Win</Name></Item>
          <Item Type="GameLibrary.ActionSequence"><Name>Open</Name><Actions><Item>Act2</Item><Item>DestroyEntity(7)</Item></Actions><TimeBetweenInSec>0.5</TimeBetweenInSec></Item>
          <Item Type="GameLibrary.ActivateEntityActionData"><Name>Act2</Name><NextAction>Win</NextAction><ItemId>2</ItemId><Deactivate>true</Deactivate></Item>
          <Item Type="GameLibrary.ApplyImpulseActionData"><Name>Push</Name><ItemId>0</ItemId><Impulse>-7 3</Impulse></Item>
          <Item Type="GameLibrary.ActivateTriggerActionData"><Name>Off</Name><TriggerIds>1 3</TriggerIds><Deactivate>true</Deactivate></Item>
          <Item Type="GameLibrary.CloneObject"><Name>Clone</Name></Item>
        </Actions></Asset></XnaContent>"#,
    )
    .unwrap()
}

#[test]
fn named_actions_resolve_with_what_follows_them() {
    let m = map();
    let act2 = resolve(&m, "Act2");
    assert_eq!(act2[0].act, Act::SetActive { item: 2, active: false });
    assert_eq!(act2[0].next.as_deref(), Some("Win"));
    assert_eq!(resolve(&m, "Win")[0].act, Act::LevelComplete);
    assert_eq!(
        resolve(&m, "Open")[0].act,
        Act::Sequence {
            actions: vec!["Act2".into(), "DestroyEntity(7)".into()],
            step: 0.5
        }
    );
    assert_eq!(
        resolve(&m, "Push")[0].act,
        Act::Impulse {
            item: 0,
            impulse: [-7.0, 3.0],
            random: [0.0, 0.0]
        }
    );
    assert_eq!(resolve(&m, "Off")[0].act, Act::SetTriggers { ids: vec![1, 3], active: false });
    assert_eq!(resolve(&m, "Clone")[0].act, Act::Unsupported("CloneObject".into()));
    assert!(resolve(&m, "Nobody").is_empty());
}

#[test]
fn templates_resolve_from_their_arguments() {
    let m = map();
    let one = |r: &str| resolve(&m, r).remove(0).act;
    assert_eq!(one("ActivateEntity(1001)"), Act::SetActive { item: 1001, active: true });
    assert_eq!(one("DeactivateTrigger(4)"), Act::SetTriggers { ids: vec![4], active: false });
    assert_eq!(one("Damage(4001, 5)"), Act::Damage { item: 4001, amount: 5, direction: 0.0 });
    assert_eq!(
        one("Seq(A B na, 1.5)"),
        Act::Sequence {
            actions: vec!["A".into(), "B".into(), "na".into()],
            step: 1.5
        }
    );
    assert_eq!(one("NextMap()"), Act::LevelComplete);
    assert_eq!(one("Save()"), Act::Nothing);
    assert_eq!(one("OpenDoor(3)"), Act::Unsupported("OpenDoor".into()));
}

#[test]
fn triggers_default_to_armed_and_many_uses() {
    let m = map();
    let t = &m.triggers.items[0];
    assert!(t.active);
    assert_eq!(t.amount, 999);
}
