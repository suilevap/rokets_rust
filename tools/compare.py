#!/usr/bin/env python3
"""Run one driving scenario in the C# game and in the port, and compare.

Usage:
  tools/compare.py MAP --drive SCRIPT [--start X,Y] [--ids A,B] [--run ACTION] [--rokets PATH]

  tools/compare.py AiTest --start 3.5,14.5 --ids 4001,1007 \\
      --drive "0,0,g:1;0,0:0.3;0,-1:2.5;0,0:2"

Both games drive the player's car by the same script (`--drive`: the
stick in world directions, y up, `g` to hold the gadget) and record the
listed entities (map ids; the player's car when none) every 0.1 s of game
time: the C# game through its debug server (`/drive`, `/trace`), the port
with `--trace`. The script prints both side by side every 0.5 s and the
largest gaps, in the C# game's units and axes (y down).

Needs the C# game built in Debug (`dotnet build` in the rokets checkout)
and, headless, Xvfb and PulseAudio (rokets `docs/RUNNING_ON_LINUX.md`);
the C# game runs about twenty times slower than real time there, so a
few seconds of driving take a couple of minutes.

The port has no `SimpleCars` (enemies, turrets) yet: pick a map without
them, or they shoot and push the C# car. The port runs the script from the map's start, the C# game from when it
answers `/drive`, some seconds later: pick a map without a scripted intro
(a `SetControl` or a paused `ShowText` at the start, as on Middleware), or
the two intros fall at different script times.
"""
import argparse
import math
import os
import re
import subprocess
import sys
import time
import urllib.error
import urllib.request
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
PORT = 7778
# What the C# game printed, for when it falls over.
CS_LOG = "/tmp/compare_cs.log"


def total_seconds(script):
    return sum(float(part.split(":")[1]) for part in script.split(";") if part.strip())


def player(map_name):
    """The local player's id and heading, from the map."""
    xml = (ROOT / "assets/Maps/Singleplayer" / f"{map_name}.xml").read_text(encoding="utf-8-sig")
    # The `Players` car with the `Local` control, else the `SimpleCars` one.
    for section in ("Players", "SimpleCars"):
        cars = xml[xml.find(f"<{section}>"):xml.find(f"</{section}>")]
        for item in re.findall(r"<Item>(.*?)</Item>", cars, re.S):
            if "<Control>Local</Control>" in item and "<IsPrototype>true</IsPrototype>" not in item:
                pid = int(re.search(r"<Id>(\d+)</Id>", item).group(1))
                d = re.search(r"<DirectionDeg>([^<]+)</DirectionDeg>", item)
                return pid, float(d.group(1)) if d else 0.0
    sys.exit(f"{map_name}: no local player")


def parse(lines):
    """{(t, id): (x, y, angle, speed)} from `t id x y angle speed` lines."""
    out = {}
    for line in lines:
        parts = line.split()
        if len(parts) == 6:
            t, i, x, y, a, v = parts
            out[(round(float(t), 1), int(i))] = (float(x), float(y), float(a), float(v))
    return out


def run_port(args, ids, seconds):
    env = dict(os.environ, BEVY_ASSET_ROOT=str(ROOT), RUST_LOG="warn,tilerider::script=info")
    env.setdefault("VK_ICD_FILENAMES", "/usr/share/vulkan/icd.d/lvp_icd.json")
    # At least a fixed step a frame: enough frames for the whole script.
    frames = int(seconds * 60) + 60
    cmd = ["cargo", "run", "-q", "--", "-m", args.map, "--drive", args.drive,
           "--trace", ",".join(map(str, ids)), "--screenshot", "/tmp/compare_port.png", "--frames", str(frames)]
    if args.start:
        cmd += ["--start", args.start]
    for action in args.run:
        cmd += ["--run", action]
    if not os.environ.get("DISPLAY"):
        cmd = ["xvfb-run", "-a", "-s", "-screen 0 800x480x24"] + cmd
    print("port:", " ".join(cmd[-12:]), file=sys.stderr)
    r = subprocess.run(cmd, cwd=ROOT, env=env, capture_output=True, text=True)
    # Bevy logs to stdout; colour codes would get in the way of parsing.
    out = re.sub(r"\x1b\[[0-9;]*m", "", r.stdout + r.stderr)
    lines = [re.sub(r".*\btrace ", "", l) for l in out.splitlines() if " trace " in l]
    if not lines:
        print(f"port: no trace (exit {r.returncode}); its output ends:\n" + "\n".join(out.splitlines()[-15:]), file=sys.stderr)
    return parse(lines)


def get(path):
    """The answer's body, an error's included."""
    try:
        with urllib.request.urlopen(f"http://localhost:{PORT}{path}", timeout=30) as r:
            return r.read().decode()
    except urllib.error.HTTPError as e:
        return e.read().decode()


def run_cs(args, ids, seconds, heading):
    exe = Path(args.rokets) / "PavXnaGame1/PavXnaGame1/bin/Debug/net8.0"
    env = dict(os.environ, ALSOFT_DRIVERS="pulse", XDG_RUNTIME_DIR=os.environ.get("XDG_RUNTIME_DIR", "/run/user/0"))
    xvfb = None
    if not env.get("DISPLAY"):
        xvfb = subprocess.Popen(["Xvfb", ":98", "-screen", "0", "800x480x24"], stderr=subprocess.DEVNULL)
        env["DISPLAY"] = ":98"
    subprocess.run(["pulseaudio", "--start", "--exit-idle-time=-1"], env=env, capture_output=True)
    log = open(CS_LOG, "w")
    game = subprocess.Popen(["dotnet", "TileRider.dll", "-m", args.map, "-window", "-server", str(PORT), "-nosplash"],
                            cwd=exe, env=env, stdout=log, stderr=subprocess.STDOUT)
    try:
        for _ in range(180):
            try:
                get("/state")
                break
            except OSError:
                time.sleep(1)
        get("/pause?paused=false")
        # The server answers before the map's entities are there; move
        # the car until it is in the asked cell.
        for _ in range(60):
            if args.start:
                x, y = args.start.split(",")
                get(f"/run/move_entity?item_id={ids[0]}&position={x}%2C{y}&direction_deg={heading}")
                cell = f'"tile_x":{int(float(x))},"tile_y":{int(float(y))},'
            else:
                cell = f'"id":{ids[0]},'
            if cell in get(f"/entities?id={ids[0]}"):
                break
            time.sleep(2)
        else:
            sys.exit("c#: the player's car did not appear (or would not move to --start)")
        # The car may not be there yet when the server first answers.
        for action in args.run:
            print("c#: run", action, get("/run?cmd=" + urllib.request.quote(action, safe="")).strip(), file=sys.stderr)
        for _ in range(60):
            answer = get("/drive?script=" + urllib.request.quote(args.drive, safe=",:") + "&ids=" + ",".join(map(str, ids)))
            if "no local car" not in answer:
                break
            time.sleep(2)
        if answer.strip() != "ok":
            sys.exit(f"c#: /drive said {answer}")
        print("c#: driving (slow headless)…", file=sys.stderr)
        record = {}
        for _ in range(1200):
            record = parse(get("/trace").splitlines())
            if record and max(t for t, _ in record) >= seconds - 0.05:
                break
            time.sleep(2)
        return record
    except OSError as e:
        tail = "\n".join(Path(CS_LOG).read_text(errors="replace").splitlines()[-20:])
        sys.exit(f"c#: the game stopped answering ({e}); {CS_LOG} ends:\n{tail}")
    finally:
        game.kill()
        if xvfb:
            xvfb.kill()


def main():
    p = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    p.add_argument("map")
    p.add_argument("--drive", required=True)
    p.add_argument("--start", help="map cell x,y for the player's car")
    p.add_argument("--ids", help="map ids to compare (default: the player's car)")
    p.add_argument("--run", action="append", default=[], help="a map action to run when the script starts (repeatable)")
    p.add_argument("--rokets", default=str(ROOT.parent / "rokets"))
    args = p.parse_args()

    pid, heading = player(args.map)
    ids = [int(i) for i in args.ids.split(",")] if args.ids else [pid]
    if pid not in ids:
        ids.insert(0, pid)
    seconds = total_seconds(args.drive)
    port = run_port(args, ids, seconds)
    cs = run_cs(args, ids, seconds, heading)
    print(f"samples: C# {len(cs)}, port {len(port)}", file=sys.stderr)

    print(f"{'t':>5} {'id':>5} | {'C# x':>8} {'y':>8} {'angle':>7} {'speed':>6} | "
          f"{'port x':>8} {'y':>8} {'angle':>7} {'speed':>6} | {'gap':>6}")
    worst = {}
    for (t, i) in sorted(set(cs) & set(port)):
        c, r = cs[(t, i)], port[(t, i)]
        gap = math.hypot(c[0] - r[0], c[1] - r[1])
        if gap > worst.get(i, (0, 0))[1]:
            worst[i] = (t, gap)
        if round(t * 10) % 5 == 0:
            print(f"{t:5.1f} {i:5d} | {c[0]:8.2f} {c[1]:8.2f} {c[2]:7.1f} {c[3]:6.2f} | "
                  f"{r[0]:8.2f} {r[1]:8.2f} {r[2]:7.1f} {r[3]:6.2f} | {gap:6.2f}")
    for i, (t, gap) in sorted(worst.items()):
        print(f"id {i}: largest gap {gap:.2f} units at t={t:.1f}")
    if not set(cs) & set(port):
        sys.exit("no common samples: did both games run the script?")


if __name__ == "__main__":
    main()
