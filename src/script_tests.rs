use super::*;

#[test]
fn parses_and_steps() {
    let mut s = DriveScript::parse("1,0:0.5;0,1:0.1").unwrap();
    assert_eq!(s.now(), Some((Vec2::X, Buttons::default())));
    s.tick = 30;
    assert_eq!(s.now(), Some((Vec2::Y, Buttons::default())));
    s.tick = 36;
    assert_eq!(s.now(), None);
    let gadget = Buttons { gadget: true, fire: false };
    assert_eq!(DriveScript::parse("0,0,g:1").unwrap().now(), Some((Vec2::ZERO, gadget)));
    let both = Buttons { gadget: true, fire: true };
    assert_eq!(DriveScript::parse("0,0,f,g:1").unwrap().now(), Some((Vec2::ZERO, both)));
    assert!(DriveScript::parse("oops").is_none());
    assert!(DriveScript::parse("0,0,x:1").is_none());
}
