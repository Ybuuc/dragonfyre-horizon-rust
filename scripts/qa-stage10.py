"""Run Stage 10 checks against an existing, isolated Dragonfyre QA directory."""
import argparse
from collections import Counter
import gzip
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
import sys
import time

ROOT = Path(__file__).resolve().parents[1]
WORLD = "Stage10DragonPack"
ITEMS = (
    "cisco_mod:true_dragons_breath",
    "irons_spellbooks:dragonskin",
    "irons_spellbooks:arcane_essence",
)
PACKS = {
    "cisco_mod": "1187f30ec22ec5769d8f558a7729672ea932801cbcb34c427968b491f11ebac1",
    "irons_spellbooks": "1eb5550303b64d7d1b9f2299939e453314db422dfe2789956f95857443d5a97f",
}


def sha256(path):
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def inventory(path, work):
    sys.path.insert(0, str(work))
    from dragonfyre_nbt import decode

    _, root = decode(gzip.decompress(path.read_bytes()))
    totals = Counter()
    for item in root.value[b"Inventory"].value[1]:
        name = item[b"id"].value.decode("ascii")
        count = item.get(b"count") or item[b"Count"]
        totals[name] += count.value
    return {name: totals[name] for name in ITEMS}


def run(args):
    start = time.monotonic()
    profile = Path(os.environ["APPDATA"]) / "horizon/instances/dragonfyre"
    game = args.game_dir.resolve()
    if game == profile.resolve() or game.is_relative_to(profile.resolve()):
        raise ValueError("Use an isolated QA copy, outside the live profile")
    world = game / "saves" / WORLD
    if not (world / "level.dat").is_file():
        raise ValueError(f"Missing QA world: {world}")
    mods = game / "horizon-mods"
    enabled = set((game / "horizon-mods.txt").read_text().splitlines())
    for name in PACKS:
        if f"mod.{name}:true" not in enabled:
            raise ValueError(f"Disabled QA mod: {name}")

    work = args.repo.resolve() / "work"
    if args.player:
        player = world / "players/data" / f"{args.player}.dat"
        print(json.dumps(inventory(player, work), indent=2))
        return

    version = Path(os.environ["APPDATA"]) / "horizon-launcher/versions/dragonfyre"
    binary = args.client or version / "horizon-client.exe"
    binary = binary.resolve()
    receipt = json.loads(args.receipt.read_text(encoding="utf-8-sig"))
    client_hash = sha256(binary)
    if receipt.get("status") != "ok" or receipt.get("exit_code") != 0 or client_hash != receipt.get("sha256", "").lower():
        raise ValueError("Client hash does not match a successful build receipt")
    hashes = {name: sha256(mods / f"Dragonfyre-{name}.hmod") for name in PACKS}
    # Allow newly rebuilt packs by explicitly supplying their expected hashes.
    expected = dict(PACKS)
    for name, digest in args.pack_sha:
        if name not in PACKS or not re.fullmatch("[0-9a-fA-F]{64}", digest):
            raise ValueError(f"Invalid expected package hash: {name}")
        expected[name] = digest.lower()
    if hashes != expected:
        raise ValueError(f"QA package SHA-256 mismatch: {hashes}")

    env = dict(os.environ, PYTHONDONTWRITEBYTECODE="1", HORIZON_NO_AUDIO="1")
    subprocess.run(
        [sys.executable, str(work / "check_dragonfyre_dragon_pack.py"),
         str(mods / "Dragonfyre-cisco_mod.hmod")],
        env=env, check=True,
    )
    data = {"client_sha256": client_hash, "pack_sha256": hashes,
            "preflight_seconds": round(time.monotonic() - start, 2)}
    if args.mode == "check":
        print(json.dumps(data, indent=2))
        return
    resources = (args.resources or version / ".local/horizon-client/26.3").resolve()
    if not (resources / "items.json").is_file():
        raise ValueError(f"Missing prepared Horizon resources: {resources}")
    output = game / "qa-runs"
    output.mkdir(exist_ok=True)
    output = output / str(time.time_ns())
    output.mkdir()
    smoke = output / "smoke.json"
    command = [str(binary), "--resources", str(resources), "--game-dir", str(game),
               "--home", str(game / "home"), "--quickPlaySingleplayer", WORLD,
               "--render-distance", "4", "--width", "960", "--height", "540"]
    if args.mode == "smoke":
        command.extend(["--world-frames", "120", "--smoke-report", str(smoke)])
    print(f"QA log: {output / 'client.log'}", flush=True)
    start = time.monotonic()
    with (output / "client.log").open("w", encoding="utf-8") as log:
        result = subprocess.run(command, cwd=game, env=env, stdout=log, stderr=subprocess.STDOUT)
    data.update(exit_code=result.returncode, client_seconds=round(time.monotonic() - start, 2))
    if smoke.is_file():
        data["world_smoke"] = json.loads(smoke.read_text(encoding="utf-8"))
    (output / "report.json").write_text(json.dumps(data, indent=2) + "\n", encoding="utf-8")
    if result.returncode != 0:
        if "is already open in another instance" in (output / "client.log").read_text(encoding="utf-8", errors="replace"):
            raise ValueError(f"Close the Horizon window using this QA world, then retry. See {output}")
        raise ValueError(f"Client exited with {result.returncode}. See {output}")
    if args.mode == "smoke":
        result = data.get("world_smoke", {})
        if not result.get("player") or result.get("error") is not None or not result.get("chunks"):
            raise ValueError(f"World entry not confirmed. See {output}")
    print(json.dumps(data, indent=2))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--game-dir", type=Path, required=True)
    parser.add_argument("--repo", type=Path, default=ROOT)
    parser.add_argument("--receipt", type=Path, default=ROOT / "outputs/native-reproduction/dragonfyre-native-0_6jcdcu/build-release.json")
    parser.add_argument("--client", type=Path)
    parser.add_argument("--resources", type=Path)
    parser.add_argument("--mode", choices=("check", "smoke", "play"), default="check")
    parser.add_argument("--pack-sha", nargs=2, action="append", default=[], metavar=("MOD", "SHA256"))
    parser.add_argument("--player", type=str, help="Read item counts from this saved player UUID")
    args = parser.parse_args()
    if args.player and not re.fullmatch(r"[0-9a-fA-F]{8}(?:-[0-9a-fA-F]{4}){3}-[0-9a-fA-F]{12}", args.player):
        parser.error("Expected a player UUID")
    try:
        run(args)
    except (OSError, ValueError, KeyError, subprocess.CalledProcessError) as exc:
        parser.exit(1, f"QA failed: {exc}\n")


if __name__ == "__main__":
    main()
