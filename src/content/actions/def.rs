//! How an action type is defined: its fields with what a level editor
//! needs to know of them, read from and written to the map's XML.
//!
//! C# kept this as attributes on the fields of `GameLibrary/ActionData.cs`
//! (`[ContentSerializer(Optional = true)]`, `[ReferenceTo(...)]`,
//! `[Description(...)]`, `[FieldReference(...)]`), read by reflection in
//! the level editor and by XNA's serializer in the game. Here the
//! [`actions!`] macro makes, from one declaration per type, the struct,
//! its [`Field`] table, reading and writing.
//!
//! Definitions know nothing of running: the game registers a system for
//! a type (`crate::actions`), a level editor needs only these.

use super::TemplateArgs;
use crate::content::node::Node;
use crate::content::xna::{XColor, XRect, XVec2};

/// What a field holds, for an editor to pick a control.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Int,
    Float,
    Bool,
    Text,
    /// `"x y"`.
    Vec2,
    /// `"AARRGGBB"`.
    Color,
    /// `"x y width height"`, map cells.
    Rect,
    /// `"1 2 3"`.
    Ints,
    /// `<Item>a</Item><Item>b</Item>`.
    Texts,
    /// One of these names.
    Enum(&'static [&'static str]),
}

/// What a field's value names (`[ReferenceTo(typeof(…))]`): an editor
/// offers the map's items of that kind.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ref {
    /// A map action, or a template (`ActionData`).
    Action,
    /// `TriggerData`, by `Id`.
    Trigger,
    /// `PlayerData`, by `Id`.
    Player,
    /// `SimpleCarData`, by `Id`.
    SimpleCar,
    /// `EntityData`, by `Id`.
    Entity,
    /// `HintData`, by `Id`.
    Hint,
}

/// Values found elsewhere that a field may take
/// (`[FieldReference(File, Path)]`): an editor offers them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lookup {
    /// Groups used anywhere in the map (`MapData.GroupPath`).
    Groups,
    /// The map's variable names (`MapData.VariablePath`).
    Variables,
    /// Splash names in `Splashes/Splashes.xml`.
    Splashes,
    /// Songs of the content project (`MapData.MusicPath`).
    Music,
}

/// One field of an action type.
#[derive(Clone, Copy, Debug)]
pub struct Field {
    /// The XML element (the C# name).
    pub name: &'static str,
    pub kind: Kind,
    /// `[Description]`, from the field's doc comment.
    pub doc: &'static str,
    /// `[ContentSerializer(Optional = true)]`: may be left out (and is,
    /// when null).
    pub optional: bool,
    /// `[Key]`: names the item.
    pub key: bool,
    pub refs: &'static [Ref],
    pub lookup: Option<Lookup>,
    /// The default, as the element written for it.
    pub default: fn() -> Node,
}

impl Field {
    pub const fn new(
        name: &'static str,
        kind: Kind,
        doc: &'static str,
        default: fn() -> Node,
    ) -> Self {
        Field {
            name,
            kind,
            doc,
            optional: false,
            key: false,
            refs: &[],
            lookup: None,
            default,
        }
    }

    pub const fn optional(mut self) -> Self {
        self.optional = true;
        self
    }

    pub const fn key(mut self) -> Self {
        self.key = true;
        self
    }

    pub const fn refs(mut self, refs: &'static [Ref]) -> Self {
        self.refs = refs;
        self
    }

    pub const fn lookup(mut self, lookup: Lookup) -> Self {
        self.lookup = Some(lookup);
        self
    }

    /// The description, without the doc comment's line breaks.
    pub fn description(&self) -> String {
        self.doc.split_whitespace().collect::<Vec<_>>().join(" ")
    }
}

/// What every type of action has (`ActionData`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Base {
    pub name: String,
    pub groups: Vec<String>,
    pub run_on_map_start: bool,
    pub next_action: Option<String>,
}

impl Base {
    pub const FIELDS: &'static [Field] = &[
        Field::new("Name", Kind::Text, "Action name", || Node::new("Name")).key(),
        Field::new("Groups", Kind::Texts, "Group by", || Node::new("Groups"))
            .optional()
            .lookup(Lookup::Groups),
        Field::new(
            "RunOnMapStart",
            Kind::Bool,
            "Run this action as soon as the map is loaded",
            || Node::with_text("RunOnMapStart", "false"),
        )
        .optional(),
        Field::new(
            "NextAction",
            Kind::Text,
            "Run after this one when it succeeds",
            || Node::new("NextAction"),
        )
        .optional()
        .refs(&[Ref::Action]),
    ];

    /// Reads the common fields of an action of type `def` (whose defaults
    /// fill what is left out).
    pub fn read(node: &Node, def: Option<&DefInfo>) -> Result<Base, String> {
        let mut b = Base {
            name: def.map_or("", |d| d.name).to_string(),
            run_on_map_start: def.is_some_and(|d| d.run_on_map_start),
            ..Base::default()
        };
        read_field(node, "Name", &mut b.name)?;
        read_field(node, "Groups", &mut b.groups)?;
        read_field(node, "RunOnMapStart", &mut b.run_on_map_start)?;
        read_field(node, "NextAction", &mut b.next_action)?;
        Ok(b)
    }

    pub fn write(&self, node: &mut Node) {
        let [name, groups, start, next] = Self::FIELDS else {
            unreachable!()
        };
        write_field(node, name, &self.name);
        write_field(node, groups, &self.groups);
        write_field(node, start, &self.run_on_map_start);
        write_field(node, next, &self.next_action);
    }

    /// `NextAction`, if it names one.
    pub fn next(&self) -> Option<&str> {
        self.next_action.as_deref().filter(|n| !n.trim().is_empty())
    }
}

/// The templates an action type answers to, each with how it reads its
/// arguments (`ActionTemplates.cs`).
pub type Templates<T> = Vec<(&'static str, fn(&TemplateArgs) -> Option<T>)>;

/// An action type's data.
pub trait ActionDef: Clone + std::fmt::Debug + Default + PartialEq + Send + Sync + 'static {
    const INFO: DefInfo;

    /// Its own fields (not [`Base`]'s) from an action's element; what is
    /// left out keeps its default.
    fn read(node: &Node) -> Result<Self, String>;

    /// Appends its own fields, in order, to an action's element.
    fn write(&self, node: &mut Node);

    /// Templates that make it: `("Impulse", |args| …)` for
    /// `Impulse(itemId,impulseX,impulseY)`.
    fn templates() -> Templates<Self> {
        Vec::new()
    }
}

/// An action type, for code that does not know it (an editor's menu).
#[derive(Clone, Copy, Debug)]
pub struct DefInfo {
    /// The C# class, without `GameLibrary.` (`ApplyImpulseActionData`).
    pub kind: &'static str,
    pub doc: &'static str,
    /// The name a new action of the type gets.
    pub name: &'static str,
    /// Whether a new one runs on map start (and one that leaves
    /// `RunOnMapStart` out does).
    pub run_on_map_start: bool,
    /// Its own fields, in the order they are written.
    pub fields: &'static [Field],
    /// Reads an action of the type, writes it back and reads that again:
    /// an error if anything differs or the element has fields the type
    /// does not.
    pub check: fn(&Node) -> Result<(), String>,
}

impl DefInfo {
    /// [`Base::FIELDS`], then the type's own.
    pub fn all_fields(&self) -> impl Iterator<Item = &'static Field> {
        Base::FIELDS.iter().chain(self.fields)
    }
}

/// The element of an action of type `T`: `<Item Type="GameLibrary.…">`.
pub fn write_action<T: ActionDef>(base: &Base, data: &T) -> Node {
    let mut node = Node::new("Item");
    node.set_attr("Type", format!("GameLibrary.{}", T::INFO.kind));
    base.write(&mut node);
    data.write(&mut node);
    node
}

/// [`DefInfo::check`] for `T`.
pub fn check<T: ActionDef>(node: &Node) -> Result<(), String> {
    let info = T::INFO;
    if let Some(c) = node
        .children
        .iter()
        .find(|c| !info.all_fields().any(|f| f.name == c.name))
    {
        return Err(format!("{} has no field {}", info.kind, c.name));
    }
    let (base, data) = (Base::read(node, Some(&info))?, T::read(node)?);
    let written = write_action(&base, &data);
    let again = (Base::read(&written, Some(&info))?, T::read(&written)?);
    if again != (base, data) {
        return Err(format!(
            "{} reads back differently:\n{}",
            info.kind,
            written.to_xml()
        ));
    }
    Ok(())
}

/// A field's value as XNA writes it.
pub trait Value: Sized {
    const KIND: Kind;

    /// From the field's element (not `Null`).
    fn read(node: &Node) -> Result<Self, String>;

    /// Into the field's element.
    fn write(&self, node: &mut Node);

    /// The value of a `Null="true"` element, if the type has one.
    fn null() -> Option<Self> {
        None
    }

    fn is_null(&self) -> bool {
        false
    }
}

/// Reads the field `name` into `value` if the element has it.
pub fn read_field<T: Value>(node: &Node, name: &str, value: &mut T) -> Result<(), String> {
    let Some(n) = node.child(name) else {
        return Ok(());
    };
    if n.is_null() {
        if let Some(v) = T::null() {
            *value = v;
        }
        return Ok(());
    }
    *value = T::read(n).map_err(|e| format!("{name}: {e}"))?;
    Ok(())
}

/// Appends `field`'s element (none for a null optional one).
pub fn write_field<T: Value>(node: &mut Node, field: &Field, value: &T) {
    if field.optional && value.is_null() {
        return;
    }
    let mut n = Node::new(field.name);
    value.write(&mut n);
    node.children.push(n);
}

fn parse<T: std::str::FromStr>(s: &str, what: &str) -> Result<T, String> {
    s.trim().parse().map_err(|_| format!("bad {what} {s:?}"))
}

impl Value for i64 {
    const KIND: Kind = Kind::Int;

    fn read(node: &Node) -> Result<Self, String> {
        // `<ItemId></ItemId>` is 0, as C# read it.
        if node.text.trim().is_empty() {
            Ok(0)
        } else {
            parse(&node.text, "integer")
        }
    }

    fn write(&self, node: &mut Node) {
        node.text = self.to_string();
    }
}

impl Value for f32 {
    const KIND: Kind = Kind::Float;

    fn read(node: &Node) -> Result<Self, String> {
        parse(&node.text, "number")
    }

    fn write(&self, node: &mut Node) {
        node.text = self.to_string();
    }
}

impl Value for bool {
    const KIND: Kind = Kind::Bool;

    fn read(node: &Node) -> Result<Self, String> {
        match node.text.trim().to_ascii_lowercase().as_str() {
            "true" | "1" => Ok(true),
            "false" | "0" | "" => Ok(false),
            other => Err(format!("bad bool {other:?}")),
        }
    }

    fn write(&self, node: &mut Node) {
        node.text = self.to_string();
    }
}

impl Value for String {
    const KIND: Kind = Kind::Text;

    fn read(node: &Node) -> Result<Self, String> {
        Ok(node.text.clone())
    }

    fn write(&self, node: &mut Node) {
        node.text.clone_from(self);
    }
}

/// A reference that may be null (`Null="true"`).
impl<T: Value> Value for Option<T> {
    const KIND: Kind = T::KIND;

    fn read(node: &Node) -> Result<Self, String> {
        T::read(node).map(Some)
    }

    fn write(&self, node: &mut Node) {
        match self {
            Some(v) => v.write(node),
            None => node.set_attr("Null", "true"),
        }
    }

    fn null() -> Option<Self> {
        Some(None)
    }

    fn is_null(&self) -> bool {
        self.is_none()
    }
}

impl Value for Vec<i64> {
    const KIND: Kind = Kind::Ints;

    fn read(node: &Node) -> Result<Self, String> {
        node.text
            .split_whitespace()
            .map(|v| parse(v, "integer"))
            .collect()
    }

    fn write(&self, node: &mut Node) {
        node.text = self
            .iter()
            .map(i64::to_string)
            .collect::<Vec<_>>()
            .join(" ");
    }

    fn null() -> Option<Self> {
        Some(Vec::new())
    }

    /// Null and empty are one: an optional empty list is left out.
    fn is_null(&self) -> bool {
        self.is_empty()
    }
}

impl Value for Vec<String> {
    const KIND: Kind = Kind::Texts;

    fn read(node: &Node) -> Result<Self, String> {
        Ok(node
            .children_named("Item")
            .map(|i| i.text.clone())
            .collect())
    }

    fn write(&self, node: &mut Node) {
        node.children = self
            .iter()
            .map(|s| Node::with_text("Item", s.as_str()))
            .collect();
    }

    fn null() -> Option<Self> {
        Some(Vec::new())
    }

    /// Null and empty are one: an optional empty list is left out.
    fn is_null(&self) -> bool {
        self.is_empty()
    }
}

impl Value for XVec2 {
    const KIND: Kind = Kind::Vec2;

    fn read(node: &Node) -> Result<Self, String> {
        XVec2::parse(&node.text).ok_or_else(|| format!("bad Vector2 {:?}", node.text))
    }

    fn write(&self, node: &mut Node) {
        node.text = format!("{} {}", self.x, self.y);
    }
}

impl Value for XColor {
    const KIND: Kind = Kind::Color;

    fn read(node: &Node) -> Result<Self, String> {
        XColor::parse_argb(&node.text).ok_or_else(|| format!("bad Color {:?}", node.text))
    }

    fn write(&self, node: &mut Node) {
        node.text = format!("{:02X}{:02X}{:02X}{:02X}", self.a, self.r, self.g, self.b);
    }
}

impl Value for XRect {
    const KIND: Kind = Kind::Rect;

    fn read(node: &Node) -> Result<Self, String> {
        XRect::parse(&node.text).ok_or_else(|| format!("bad Rectangle {:?}", node.text))
    }

    fn write(&self, node: &mut Node) {
        node.text = format!("{} {} {} {}", self.x, self.y, self.w, self.h);
    }
}

/// An enum written by name (C# enums in action data).
///
/// ```ignore
/// xna_enum! {
///     /// How "Level complete" looks (`SuccessIndicator`).
///     SuccessIndicator { Standard, DoorOpen, AntiSuccess }
/// }
/// ```
///
/// The first variant is the default.
#[macro_export]
macro_rules! xna_enum {
    ($(#[doc = $doc:literal])* $name:ident { $first:ident $(, $variant:ident)* $(,)? }) => {
        $(#[doc = $doc])*
        #[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
        pub enum $name {
            #[default]
            $first,
            $($variant),*
        }

        impl $name {
            pub const NAMES: &'static [&'static str] = &[stringify!($first) $(, stringify!($variant))*];
            const ALL: &'static [$name] = &[$name::$first $(, $name::$variant)*];
        }

        impl $crate::content::actions::def::Value for $name {
            const KIND: $crate::content::actions::def::Kind = $crate::content::actions::def::Kind::Enum(Self::NAMES);

            fn read(node: &$crate::content::node::Node) -> Result<Self, String> {
                let text = node.text.trim();
                // By name, or by number as C# also accepted.
                Self::NAMES
                    .iter()
                    .position(|n| *n == text)
                    .or_else(|| text.parse().ok())
                    .and_then(|i: usize| Self::ALL.get(i).copied())
                    .ok_or_else(|| format!("bad {} {text:?}", stringify!($name)))
            }

            fn write(&self, node: &mut $crate::content::node::Node) {
                node.text = Self::NAMES[*self as usize].to_string();
            }
        }
    };
}

/// Action types, one declaration each:
///
/// ```ignore
/// actions! {
///     /// Pushes an entity.
///     Impulse("ApplyImpulseActionData") {
///         /// The entity pushed; 0: the one that set the action off.
///         item_id "ItemId": i64 = 0, refs(&[Player, SimpleCar, Entity]);
///         impulse "Impulse": XVec2 = XVec2::ZERO;
///         /// Times a random 0..1, added to `Impulse`.
///         random_impulse "RandomImpulse": XVec2 = XVec2::ZERO, optional;
///     } => templates {
///         "Impulse(itemId,impulseX,impulseY)" => |a| Some(Impulse { item_id: a.int(0)?, ..Impulse::default() }),
///     }
/// }
/// ```
///
/// - after the C# class: `name = "…"` (a new one's name, from its C#
///   constructor) and `on_map_start = true`;
/// - a field is its Rust name, its XML name, type, default, then
///   [`Field`]'s flags: `optional`, `key`, `refs(…)`, `lookup(…)`;
/// - its doc comment is its description;
/// - `=> templates` (optional) are the type's `ActionTemplates`, each
///   with its C# signature (an editor shows it; the game picks by name
///   and number of arguments).
///
/// It also makes `ALL`, the [`DefInfo`] of every type declared.
#[macro_export]
macro_rules! actions {
    ($(
        $(#[doc = $doc:literal])*
        $name:ident ($kind:literal $(, name = $dname:literal)? $(, on_map_start = $start:literal)?) {
            $(
                $(#[doc = $fdoc:literal])*
                $field:ident $xml:literal : $ty:ty = $default:expr $(, $flag:ident $(($($arg:tt)*))?)* ;
            )*
        }
        $(=> templates { $($tname:literal => $tf:expr),* $(,)? })?
    )*) => {
        $(
            $(#[doc = $doc])*
            #[derive(Clone, Debug, PartialEq)]
            pub struct $name {
                $($(#[doc = $fdoc])* pub $field: $ty,)*
            }

            impl Default for $name {
                fn default() -> Self {
                    $name { $($field: $default,)* }
                }
            }

            impl $crate::content::actions::def::ActionDef for $name {
                const INFO: $crate::content::actions::def::DefInfo = $crate::content::actions::def::DefInfo {
                    kind: $kind,
                    doc: concat!($($doc, "\n",)* ""),
                    name: [$($dname,)? ""][0],
                    run_on_map_start: [$($start,)? false][0],
                    fields: &[$(
                        $crate::content::actions::def::Field::new(
                            $xml,
                            <$ty as $crate::content::actions::def::Value>::KIND,
                            concat!($($fdoc, "\n",)* ""),
                            || {
                                let mut n = $crate::content::node::Node::new($xml);
                                let v: $ty = $default;
                                $crate::content::actions::def::Value::write(&v, &mut n);
                                n
                            },
                        )
                        $(.$flag($($($arg)*)?))*
                    ),*],
                    check: $crate::content::actions::def::check::<$name>,
                };

                #[allow(unused_variables)]
                fn read(node: &$crate::content::node::Node) -> Result<Self, String> {
                    #[allow(unused_mut)]
                    let mut v = $name::default();
                    $($crate::content::actions::def::read_field(node, $xml, &mut v.$field)?;)*
                    Ok(v)
                }

                #[allow(unused_variables, unused_mut)]
                fn write(&self, node: &mut $crate::content::node::Node) {
                    let mut fields = Self::INFO.fields.iter();
                    $($crate::content::actions::def::write_field(node, fields.next().unwrap(), &self.$field);)*
                }

                $(
                    fn templates() -> $crate::content::actions::def::Templates<Self> {
                        vec![$(($tname, $tf)),*]
                    }
                )?
            }
        )*

        /// Every action type declared here.
        pub const ALL: &[$crate::content::actions::def::DefInfo] =
            &[$(<$name as $crate::content::actions::def::ActionDef>::INFO),*];
    };
}
