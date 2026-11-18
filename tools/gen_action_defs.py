#!/usr/bin/env python3
"""Add the C# action types missing from src/content/actions/defs.rs.

Usage: tools/gen_action_defs.py [path/to/rokets] [--write]   (default: ../rokets)

Reads GameLibrary/*.cs and, for each non-abstract ActionData class that
defs.rs does not declare yet, writes its declaration for the `actions!`
macro: fields in C#'s order (base classes first, as XNA writes them),
Rust types, defaults (initializers, then the constructor's assignments),
`optional`, `key`, where values come from (`[ReferenceTo]`,
`[FieldReference]`) and `[Description]` as doc comments. Enums and
records (C# classes used as fields) they need come with them.

Without --write it prints what it would add. Either way it checks the
types defs.rs already has against C# (same fields, same order).

Definitions are edited by hand afterwards (docs, templates); the script
never touches one that exists.
"""
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
ARGS = [a for a in sys.argv[1:] if not a.startswith("--")]
WRITE = "--write" in sys.argv
SRC = Path(ARGS[0] if ARGS else ROOT.parent / "rokets")
LIB = SRC / "PavXnaGame1" / "GameLibrary"
DEFS = ROOT / "src" / "content" / "actions" / "defs.rs"
ENUM_MARK = "// New enums and records go above this line (tools/gen_action_defs.py).\n"
TYPE_MARK = "    // New types go above this line (tools/gen_action_defs.py).\n"

RUST_KEYWORDS = {
    "type": "kind", "ref": "reference", "move": "moves", "loop": "looped", "match": "matched",
    "static": "is_static", "use": "uses", "box": "boxed", "in": "inside",
}

# `[ReferenceTo(typeof(X))]`: what the map keys items of type X by.
REF_SOURCES = {
    "ActionData": ["ACTIONS", "Source::Templates"],
    "TriggerData": ["TRIGGERS"],
    "PlayerData": ["PLAYERS"],
    "SimpleCarData": ["SIMPLE_CARS"],
    "EntityData": ["ENTITIES"],
    "HintData": ["HINTS"],
}
# `[FieldReference(Path = MapData.X)]`.
PATH_SOURCES = {
    "GroupPath": "GROUPS",
    "VariablePath": "VARIABLES",
    "SplashPath": "SPLASHES",
    "SoundPath": "SOUNDS",
    "MusicPath": "MUSIC",
    "MapPartsPath": "MAP_PARTS",
}
NAMED_COLORS = {
    "White": (255, 255, 255, 255), "Black": (0, 0, 0, 255), "Transparent": (0, 0, 0, 0),
    "Red": (255, 0, 0, 255), "Green": (0, 128, 0, 255), "Lime": (0, 255, 0, 255),
    "Blue": (0, 0, 255, 255), "Yellow": (255, 255, 0, 255), "Orange": (255, 165, 0, 255),
    "Gray": (128, 128, 128, 255), "Purple": (128, 0, 128, 255), "Cyan": (0, 255, 255, 255),
}


def strip_comments(s):
    """Removes // and /* */ comments, leaving strings alone; keeps
    `/// <summary>` text as `@summary(...)` markers."""
    out, i, n = [], 0, len(s)
    while i < n:
        c = s[i]
        if c == '"':
            j = i + 1
            while j < n and s[j] != '"':
                j += 2 if s[j] == "\\" else 1
            out.append(s[i:j + 1])
            i = j + 1
        elif c == "'" and i + 2 < n:
            j = s.index("'", i + 2 if s[i + 1] == "\\" else i + 1)
            out.append(s[i:j + 1])
            i = j + 1
        elif s.startswith("///", i):
            j = s.find("\n", i)
            j = n if j < 0 else j
            text = re.sub(r"</?summary>", "", s[i + 3:j]).strip()
            if text:
                out.append(f"@summary({text})")
            i = j
        elif s.startswith("//", i):
            j = s.find("\n", i)
            i = n if j < 0 else j
        elif s.startswith("/*", i):
            i = s.index("*/", i) + 2
        else:
            out.append(c)
            i += 1
    return "".join(out)


def block_end(s, open_at):
    """Index after the `}` closing the `{` at open_at (strings skipped)."""
    depth, i = 0, open_at
    while i < len(s):
        c = s[i]
        if c == '"':
            i = s.index('"', i + 1) + 1
            continue
        if c == "{":
            depth += 1
        elif c == "}":
            depth -= 1
            if depth == 0:
                return i + 1
        i += 1
    raise ValueError("unbalanced braces")


def statements(body):
    """Splits a class body into member declarations at depth 0."""
    out, cur, depth, i = [], [], 0, 0
    while i < len(body):
        c = body[i]
        if c == '"':
            j = body.index('"', i + 1)
            cur.append(body[i:j + 1])
            i = j + 1
            continue
        cur.append(c)
        if c == "{":
            depth += 1
        elif c == "}":
            depth -= 1
            if depth == 0:
                rest = body[i + 1:].lstrip()
                if not rest.startswith("=") and not rest.startswith(";"):
                    out.append("".join(cur).strip())
                    cur = []
        elif c == ";" and depth == 0:
            out.append("".join(cur).strip())
            cur = []
        i += 1
    if "".join(cur).strip():
        out.append("".join(cur).strip())
    return [s for s in out if s and s != ";"]


class Cs:
    def __init__(self):
        self.classes = {}  # name -> dict
        self.enums = {}  # (namespace, name) -> [variants]
        self.consts = {}

    def read(self, text):
        text = strip_comments(text)
        self._scan(text, [])

    def _scan(self, text, namespaces):
        pos = 0
        header = re.compile(
            r"(?:@summary\(([^)]*)\)\s*)*"
            r"(?:\[[^\]]*\]\s*)*"
            r"(?:(?:public|internal|static|abstract|sealed|partial)\s+)*"
            r"(namespace|class|enum)\s+([\w.]+)\s*(?::\s*([\w.,\s<>]+?))?\s*\{"
        )
        while True:
            m = header.search(text, pos)
            if not m:
                return
            open_at = m.end() - 1
            end = block_end(text, open_at)
            body = text[open_at + 1:end - 1]
            kind, name = m.group(2), m.group(3)
            if kind == "namespace":
                self._scan(body, namespaces + [name.split(".")[-1]])
            elif kind == "enum":
                variants = []
                for v in body.split(","):
                    v = v.strip()
                    if not v:
                        continue
                    vname, _, value = v.partition("=")
                    variants.append((vname.strip(), value.strip() or None))
                self.enums[(namespaces[-1] if namespaces else "", name)] = variants
            else:
                summary = re.findall(r"@summary\(([^)]*)\)", text[m.start():m.end()])
                bases = [b.strip() for b in (m.group(4) or "").split(",") if b.strip()]
                self.classes[name] = {
                    "name": name,
                    "abstract": "abstract" in text[m.start():m.end()],
                    "base": bases[0] if bases else None,
                    "body": body,
                    "doc": " ".join(summary),
                }
                for c in re.finditer(r"const\s+int\s+(\w+)\s*=\s*(-?\d+)", body):
                    self.consts[f"{name}.{c.group(1)}"] = c.group(2)
                self._scan(body, namespaces)
            pos = end

    def chain(self, name):
        """The class and its bases, base first."""
        out = []
        while name in self.classes:
            out.insert(0, self.classes[name])
            name = self.classes[name]["base"]
        return out

    def is_action(self, name):
        return any(c["name"] == "ActionData" for c in self.chain(name))

    def enum(self, cs_type):
        parts = cs_type.split(".")
        name = parts[-1]
        found = [k for k in self.enums if k[1] == name]
        if len(parts) > 1:
            found = [k for k in found if k[0] == parts[-2]] or found
        return found[0] if found else None


def members(cls):
    """Serialized fields and properties of one class, and its constructor's
    assignments."""
    fields, assigns = [], {}
    for st in statements(cls["body"]):
        attrs = []
        st = re.sub(r"@summary\([^)]*\)\s*", "", st)
        while st.startswith("["):
            depth, quoted = 0, False
            for i, c in enumerate(st):
                if c == '"':
                    quoted = not quoted
                if quoted:
                    continue
                depth += c == "["
                depth -= c == "]"
                if depth == 0:
                    attrs.append(st[1:i])
                    st = st[i + 1:].lstrip()
                    break
        ctor = re.match(rf"public\s+{cls['name']}\s*\(\s*\)\s*(?::\s*base\(\)\s*)?\{{(.*)\}}\s*$", st, re.S)
        if ctor:
            for a in re.finditer(r"(\w+)\s*=\s*([^;=][^;]*);", ctor.group(1)):
                assigns[a.group(1)] = a.group(2).strip()
            continue
        m = re.match(
            r"public\s+(?:new\s+)?(?!override|static|const|virtual|abstract|class|enum)([\w.<>\[\]?]+)\s+(\w+)\s*(.*)$",
            st, re.S)
        if not m or "(" in m.group(3).split("=")[0]:
            continue
        cs_type, name, rest = m.group(1), m.group(2), m.group(3).strip()
        if any(a.startswith(("ContentSerializerIgnore", "XmlIgnore")) for a in attrs):
            continue
        init = None
        if rest.startswith("{"):
            end = block_end(rest, 0)
            if not re.search(r"\bset\b", rest[:end]):
                continue  # get only: not serialized
            after = rest[end:].strip()
            if after.startswith("="):
                init = after[1:].rstrip(";").strip()
        elif rest.startswith("="):
            init = rest[1:].rstrip(";").strip()
        fields.append({"cs_type": cs_type, "name": name, "attrs": attrs, "init": init})
    return fields, assigns


def snake(name):
    s = re.sub(r"([A-Z]+)([A-Z][a-z])", r"\1_\2", name)
    s = re.sub(r"([a-z\d])([A-Z])", r"\1_\2", s).lower()
    return RUST_KEYWORDS.get(s, s)


def rust_name(cls_name):
    return cls_name[: -len("ActionData")] if cls_name.endswith("ActionData") and cls_name != "ActionData" else cls_name


class Gen:
    def __init__(self, cs, existing_enums):
        self.cs = cs
        self.enums = {}  # (ns, name) -> rust name
        self.records = {}  # class -> rust text
        self.enum_names = {}
        self.existing_enums = existing_enums
        self.warnings = []
        names = {}
        for ns, name in cs.enums:
            names.setdefault(name, []).append(ns)
        for (ns, name), _ in cs.enums.items():
            prefix = ns.replace("Internal", "") if len(names[name]) > 1 else ""
            self.enum_names[(ns, name)] = prefix + name

    def rust_type(self, t):
        if t.endswith("?"):
            return f"Option<{self.rust_type(t[:-1])}>"
        base = t.split(".")[-1]
        simple = {
            "int": "i64", "byte": "i64", "short": "i64", "long": "i64", "uint": "i64",
            "float": "f32", "double": "f32", "bool": "bool", "string": "Option<String>",
            "string[]": "Vec<String>", "int[]": "Vec<i64>", "byte[]": "Vec<i64>",
            "Vector2": "XVec2", "Vector2[]": "Vec<XVec2>", "Rectangle": "XRect", "Color": "XColor",
        }
        if base in simple:
            return simple[base]
        key = self.cs.enum(t)
        if key:
            self.enums[key] = self.enum_names[key]
            return self.enum_names[key]
        if base in self.cs.classes:
            self.record(base)
            return f"Option<{base}>"
        raise ValueError(f"unknown C# type {t}")

    def default(self, rust, cs_type, expr):
        e = None if expr is None else expr.strip()
        if e == "null":
            e = None
        if rust.startswith("Option<"):
            inner = rust[7:-1]
            if e is None:
                return "None"
            if inner == "String":
                return f"Some({e}.into())"
            return f"Some({self.default(inner, cs_type.rstrip('?'), e)})"
        key = next((k for k, v in self.enum_names.items() if v == rust), None)
        if e is None and key:
            return f"{rust}::{self.cs.enums[key][0][0]}"
        if e is None:
            return {
                "i64": "0", "f32": "0.0", "bool": "false", "XVec2": "XVec2::ZERO",
                "XRect": "XRect::default()", "XColor": "XColor::rgba(0, 0, 0, 0)",
            }.get(rust, "Vec::new()" if rust.startswith("Vec<") else f"{rust}::default()")
        if rust == "i64":
            return self.cs.consts.get(e.replace("GameLibrary.", ""), e)
        if rust == "f32":
            v = e.rstrip("fFdD")
            try:
                return repr(float(v))
            except ValueError:
                self.warnings.append(f"float default {e}")
                return "0.0"
        if rust == "bool":
            return e
        if rust == "XVec2":
            if e.endswith("Vector2.Zero"):
                return "XVec2::ZERO"
            if e.endswith("Vector2.One"):
                return "XVec2 { x: 1.0, y: 1.0 }"
            m = re.match(r"new\s+Vector2\(([^,]+)(?:,([^)]+))?\)", e)
            if m:
                x = self.default("f32", "float", m.group(1))
                y = self.default("f32", "float", m.group(2) or m.group(1))
                return f"XVec2 {{ x: {x}, y: {y} }}"
        if rust == "XRect":
            m = re.match(r"new\s+Rectangle\(([^)]*)\)", e)
            if m:
                x, y, w, h = [v.strip() for v in m.group(1).split(",")]
                return f"XRect {{ x: {x}, y: {y}, w: {w}, h: {h} }}"
        if rust == "XColor":
            m = re.match(r"(?:Microsoft\.Xna\.Framework\.)?Color\.(\w+)$", e)
            if m and m.group(1) in NAMED_COLORS:
                return "XColor::rgba({}, {}, {}, {})".format(*NAMED_COLORS[m.group(1)])
            m = re.match(r"new\s+Color\(\s*(\d+)\s*,\s*(\d+)\s*,\s*(\d+)\s*(?:,\s*(\d+))?\s*\)$", e)
            if m:
                r, g, b, a = (min(int(v), 255) for v in (m.group(1), m.group(2), m.group(3), m.group(4) or 255))
                return f"XColor::rgba({r}, {g}, {b}, {a})"
        if rust.startswith("Vec<") and re.match(r"new\s", e):
            if re.search(r"\{\s*\}|\[\s*0\s*\]", e):
                return "Vec::new()"
        if rust in self.enum_names.values():
            return f"{rust}::{e.split('.')[-1]}"
        self.warnings.append(f"default {e} for {rust}: written as the type's default")
        return f"{rust}::default()" if not rust.startswith("Vec<") else "Vec::new()"

    def sources(self, attrs, field):
        out = []
        for a in attrs:
            m = re.match(r"ReferenceTo\(\s*ReferenceKeyType\s*=\s*typeof\(([\w.]+)\)\s*\)", a)
            if m:
                t = m.group(1).split(".")[-1]
                if t in REF_SOURCES:
                    out += REF_SOURCES[t]
                elif self.cs.is_action(t):
                    key = next(
                        (f["name"] for c in reversed(self.cs.chain(t)) for f in members(c)[0]
                         if "Key" in f["attrs"]), "Name")
                    path = f"XnaContent/Asset/Actions/Item[@Type='GameLibrary.{t}']/{key}"
                    out.append(f'Source::Xml {{ file: None, path: "{path}" }}')
                else:
                    self.warnings.append(f"{field}: reference to {t}")
                continue
            m = re.match(r"FieldReference(?:Attribute)?\((.*)\)", a, re.S)
            if m:
                args = dict(re.findall(r"(\w+)\s*=\s*([^,]+)", m.group(1)))
                path = args.get("Path", "").strip()
                name = path.split(".")[-1]
                if name in PATH_SOURCES:
                    out.append(PATH_SOURCES[name])
                elif path.startswith('"'):
                    file = args.get("File", "").strip()
                    file = file if file.startswith('"') else "None"
                    file = f"Some({file})" if file != "None" else file
                    out.append(f'Source::Xml {{ file: {file}, path: "{path.strip(chr(34)).lstrip("/")}" }}')
                else:
                    self.warnings.append(f"{field}: lookup {path}")
        out = list(dict.fromkeys(out))
        if out == ["ACTIONS", "Source::Templates"]:
            return "from(ACTION)"
        if out == ["PLAYERS", "SIMPLE_CARS", "ENTITIES"]:
            return "from(ITEM)"
        return f"from(&[{', '.join(out)}])" if out else None

    def fields(self, chain):
        """(rust lines, XML names) of the fields of a class chain, base first,
        without ActionData's own."""
        decls, assigns = [], {}
        for c in chain:
            fs, a = members(c)
            assigns.update(a)
            if c["name"] != "ActionData":
                decls += fs
        lines, names = [], []
        for f in decls:
            rust = self.rust_type(f["cs_type"])
            init = assigns.get(f["name"], f["init"])
            default = self.default(rust, f["cs_type"], init)
            flags = []
            if any(a.startswith("ContentSerializer") and "Optional" in a for a in f["attrs"]):
                flags.append("optional")
            if "Key" in f["attrs"]:
                flags.append("key")
            src = self.sources(f["attrs"], f["name"])
            if src:
                flags.append(src)
            for a in f["attrs"]:
                d = re.match(r'Description\(\s*Text\s*=\s*"(.*)"\s*\)$', a, re.S)
                if d:
                    lines += [f"/// {l.strip()}" for l in d.group(1).split("\\n")]
            lines.append(f'{snake(f["name"])} "{f["name"]}": {rust} = {default}' + "".join(", " + x for x in flags) + ";")
            names.append(f["name"])
        return lines, names, assigns

    def record(self, name):
        if name in self.records:
            return
        self.records[name] = None
        lines, _, _ = self.fields(self.cs.chain(name))
        doc = self.cs.classes[name]["doc"]
        body = "\n".join("        " + l for l in lines)
        head = f"    /// {doc}\n" if doc else ""
        self.records[name] = f"record! {{\n{head}    {name} {{\n{body}\n    }}\n}}\n"

    def action(self, cls):
        chain = self.cs.chain(cls["name"])
        lines, _, assigns = self.fields(chain)
        params = [f'"{cls["name"]}"']
        name = assigns.get("Name")
        if name and name.startswith('"'):
            params.append(f"name = {name}")
        if assigns.get("RunOnMapStart") == "true":
            params.append("on_map_start = true")
        out = []
        if cls["doc"]:
            out.append(f"    /// {cls['doc']}")
        body = "".join(f"        {l}\n" for l in lines)
        out.append(f"    {rust_name(cls['name'])}({', '.join(params)}) {{\n{body}    }}" if lines
                   else f"    {rust_name(cls['name'])}({', '.join(params)}) {{}}")
        return "\n".join(out) + "\n"

    def enum_decl(self, key):
        variants = self.cs.enums[key]
        values = [v for _, v in variants]
        if any(v is not None for v in values):
            expected = [str(i) for i in range(len(values))]
            if [v if v is not None else expected[i] for i, v in enumerate(values)] != expected:
                self.warnings.append(f"enum {key[1]} has explicit values {values}: read by name only")
        names = ", ".join(v for v, _ in variants)
        return f"xna_enum! {{\n    /// `{key[0] + '.' if key[0] else ''}{key[1]}`.\n    {self.enum_names[key]} {{ {names} }}\n}}\n"


def main():
    cs = Cs()
    for f in sorted(LIB.glob("*.cs")):
        cs.read(f.read_text(encoding="utf-8-sig"))
    defs = DEFS.read_text()
    existing = dict(re.findall(r'^    (\w+)\("(\w+)"', defs, re.M))
    existing_kinds = set(existing.values())
    existing_enums = set(re.findall(r"^xna_enum! \{\n(?:\s*///.*\n)*\s*(\w+) \{", defs, re.M))
    existing_enums |= set(re.findall(r"^record! \{\n(?:\s*///.*\n)*\s*(\w+) \{", defs, re.M))
    gen = Gen(cs, existing_enums)

    # The types defs.rs has: same fields, in the same order, as C#.
    for rust, kind in existing.items():
        if kind not in cs.classes:
            print(f"defs.rs: {kind} is not a C# class")
            continue
        _, names, _ = gen.fields(cs.chain(kind))
        block = defs[defs.index(f'    {rust}("{kind}"'):]
        block = block[: block.index("\n    }")]
        have = re.findall(r'^\s+\w+ "(\w+)":', block, re.M)
        if have != names:
            print(f"defs.rs: {kind} has {have}, C# {names}")

    todo = [c for n, c in cs.classes.items()
            if cs.is_action(n) and not c["abstract"] and n not in existing_kinds and n != "ActionData"]
    actions = "\n".join(gen.action(c) for c in todo)
    enums = "\n".join(gen.enum_decl(k) for k in gen.enums if gen.enum_names[k] not in existing_enums)
    records = "\n".join(r for n, r in gen.records.items() if n not in existing_enums)
    for w in gen.warnings:
        print("warning:", w, file=sys.stderr)
    new_records = [n for n in gen.records if n not in existing_enums]
    print(f"{len(todo)} types, {len(enums.split('xna_enum!')) - 1} enums, {len(new_records)} records to add",
          file=sys.stderr)
    if not WRITE:
        print(enums + records + actions)
        return
    if ENUM_MARK not in defs or TYPE_MARK not in defs:
        sys.exit(f"{DEFS} lacks the marker lines")
    defs = defs.replace(ENUM_MARK, (enums + "\n" if enums else "") + (records + "\n" if records else "") + ENUM_MARK)
    defs = defs.replace(TYPE_MARK, "\n" + actions + "\n" + TYPE_MARK if actions else TYPE_MARK)
    DEFS.write_text(defs)


if __name__ == "__main__":
    main()
