//! `AttachDestroyOnContact`: what a contact has to be to destroy the entity.

use tilerider::actions::destroy_on_contact::destroys;
use tilerider::content::actions::TemplateArgs;
use tilerider::content::actions::def::ActionDef;
use tilerider::content::actions::defs::AttachDestroyOnContact;
use tilerider::content::node::Node;

#[test]
fn without_subscribers_any_visible_entity_destroys_it() {
    assert!(destroys(None, Some(1005), true));
    assert!(destroys(None, None, true), "even one with no map id");
}

#[test]
fn with_subscribers_only_they_do() {
    assert!(destroys(Some(&[3001]), Some(3001), true));
    assert!(!destroys(Some(&[3001]), Some(3002), true));
    assert!(!destroys(Some(&[3001]), None, true));
}

#[test]
fn a_hidden_entity_does_not() {
    assert!(!destroys(None, Some(1005), false));
    assert!(!destroys(Some(&[3001]), Some(3001), false), "a car just through a teleport");
}

fn read(xml: &str) -> AttachDestroyOnContact {
    AttachDestroyOnContact::read(&Node::parse(xml).unwrap()).unwrap()
}

#[test]
fn the_campaign_maps_write_it_three_ways() {
    // DoorsWorld: null; Teleport: one id; TheRoad: one id, on ActionOnCreate.
    let any = read(r#"<Item><ItemId>1039</ItemId><Subscribers Null="true" /></Item>"#);
    assert_eq!((any.item_id, any.subscribers), (1039, vec![]));
    let one = read("<Item><ItemId>1004</ItemId><Subscribers>4001</Subscribers></Item>");
    assert_eq!((one.item_id, one.subscribers), (1004, vec![4001]));
}

#[test]
fn the_template_attaches_to_the_actor_for_one_subscriber() {
    let t = AttachDestroyOnContact::templates();
    let (name, make) = t.iter().find(|(n, _)| n.starts_with("DestroyOnContact")).expect("template");
    assert_eq!(*name, "DestroyOnContact(sub)");
    let made = make(&TemplateArgs(vec!["4001".into()])).expect("made");
    assert_eq!((made.item_id, made.subscribers), (0, vec![4001]));
}
