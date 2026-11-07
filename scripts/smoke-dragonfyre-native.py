"""Smoke-test a compiled Dragonfyre/Horizon client without touching the live launcher profile.

This starts the native client DIRECTLY (not via Horizon Offline Launcher) in a
new isolated test directory. It does not install/replace a launcher version,
modify an existing world, or prove Stage 10 gameplay.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import uuid

ROOT = Path(__file__).resolve().parents[1]


def file_hash(path: Path) -> str:
    h = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            h.update(chunk)
    return h.hexdigest()


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--run",
        type=Path,
        default=ROOT / "outputs/native-reproduction/dragonfyre-native-bi4vueka",
        help="Previous isolated apply+build run containing build-debug.json",
    )
    parser.add_argument(
        "--resources",
        type=Path,
        default=Path.home() / "AppData/Roaming/horizon-launcher/versions/"
        "nightly-2026-11-05-66f3981/.local/horizon-client/26.3",
        help="Installed Horizon 26.3 resource folder",
    )
    parser.add_argument(
        "--mod",
        action="append",
        type=Path,
        help="Horizon .hmod packages to load; defaults to the two Stage 07 item packs",
    )
    parser.add_argument(
        "--no-mods",
        action="store_true",
        help="Test bare client boot without .hmod items",
    )
    parser.add_argument(
        "--check-only",
        action="store_true",
        help="Verify binary/resources/pack paths and SHA without launching the game",
    )
    args = parser.parse_args()

    run = args.run.resolve()
    build_report = run / "build-debug.json"
    if not build_report.is_file():
        parser.error(f"Missing successful build receipt: {build_report}")
    try:
        report = json.loads(build_report.read_text(encoding="utf-8"))
    except (ValueError, OSError) as exc:
        parser.error(f"Cannot read build receipt: {exc}")
    binary = (run / "horizon-client-debug.exe").resolve()
    if report.get("status") != "ok" or report.get("exit_code") != 0:
        parser.error("Build receipt does not report success")
    if Path(report.get("binary", "")).resolve() != binary:
        parser.error("Build report points to a different executable")
    if not binary.is_file():
        parser.error(f"Compiled executable does not exist: {binary}")
    actual_sha = file_hash(binary)
    if actual_sha.lower() != str(report.get("sha256", "")).lower():
        parser.error("Executable SHA-256 does not match build receipt")

    resources = args.resources.resolve()
    if not resources.is_dir() or not (resources / "items.json").is_file():
        parser.error(f"Horizon resources or items.json not found in {resources}")
    if args.no_mods and args.mod:
        parser.error("Cannot combine --no-mods with --mod")
    if args.no_mods:
        mods: list[Path] = []
    elif args.mod:
        mods = [path.resolve() for path in args.mod]
    else:
        mods = [
            ROOT / "outputs/native-stage-07/Dragonfyre-cisco_mod.hmod",
            ROOT / "outputs/native-stage-07/Dragonfyre-farmersdelight.hmod",
        ]
    for mod in mods:
        if not mod.is_file():
            parser.error(f"Horizon .hmod is missing: {mod}")

    print(f"Verified native executable: {binary}", flush=True)
    print(f"SHA-256: {actual_sha}", flush=True)
    print(f"Resources: {resources}", flush=True)
    print(f"Items packs: {len(mods)}", flush=True)
    if args.check_only:
        print("PRECHECK PASS. No game was launched.", flush=True)
        return 0

    # Separate game, home, config and launcher-home roots from real installs.
    scratch = run / f"smoke-{uuid.uuid4().hex[:12]}"
    game = scratch / "game"
    game.mkdir(parents=True)
    (game / "options.txt").write_text(
        "lang:ru_ru\nrenderDistance:2\nsimulationDistance:5\nmaxFps:60\n"
        "enableVsync:false\n",
        encoding="utf-8",
    )

    env = os.environ.copy()
    for var, folder in {
        "HORIZON_HOME": "home",
        "HORIZON_LAUNCHER_HOME": "launcher",
        "XDG_DATA_HOME": "data",
        "XDG_CONFIG_HOME": "config",
        "HORIZON_LAUNCHER_TRASH": "trash",
        "HORIZON_LAUNCHER_CLIPBOARD": "clipboard",
        "HORIZON_LAUNCHER_DESKTOP": "desktop",
    }.items():
        env[var] = str(scratch / folder)
    env["HORIZON_NO_AUDIO"] = "1"  # Existing audio 0xc0000008 problem.
    env["HORIZON_LAUNCHER_NO_SELF_UPDATE"] = "1"
    env["HORIZON_TELEMETRY_URL"] = "off"
    # Do not inherit diagnostic/fixture/crash injection settings.
    for key in list(env):
        if key.startswith(("HORIZON_COMMAND_", "HORIZON_HARNESS_", "HORIZON_DFQ_CRASH")):
            env.pop(key, None)
    env.pop("HORIZON_DFQ_ECONOMY_FIXTURE", None)

    command = [
        str(binary), "--resources", str(resources), "--game-dir", str(game),
        "--world", "SmokeNative", "--creative", "--seed", "7",
        "--render-distance", "2", "--test-terrain", "--no-pause",
        "--width", "960", "--height", "540", "--name", "DFQSmoke",
    ]
    for mod in mods:
        command.extend(["--dev-mod", str(mod)])
    log = scratch / "client.log"
    receipt = scratch / "smoke-report.json"
    print(f"TEST ONLY: {scratch}", flush=True)
    print("A game window may open. Check whether it loads, then close it normally.", flush=True)
    print("Audio is disabled. The live Dragonfyre launcher/profile/worlds are untouched.", flush=True)
    print(f"Runtime log: {log}", flush=True)
    try:
        with log.open("w", encoding="utf-8", errors="replace") as stream:
            result = subprocess.run(
                command,
                cwd=game,
                env=env,
                stdout=stream,
                stderr=subprocess.STDOUT,
            )
    except OSError as exc:
        receipt.write_text(
            json.dumps({"status": "launch_error", "error": str(exc), "command": command}, indent=2),
            encoding="utf-8",
        )
        print(f"Could not launch: {exc}", file=sys.stderr)
        return 1

    data = {
        "status": "process_exited",
        "exit_code": result.returncode,
        "binary_sha256": actual_sha,
        "game_dir": str(game),
        "resources": str(resources),
        "mods": [str(m) for m in mods],
        "log": str(log),
        "command": command,
        "manual_gameplay_verified": False,
    }
    receipt.write_text(json.dumps(data, indent=2) + "\n", encoding="utf-8")
    print(f"Process exit code: {result.returncode}", flush=True)
    print(f"Report: {receipt}", flush=True)
    print("Window boot, input, GUI and persistence require visual/user verification.")
    return result.returncode


if __name__ == "__main__":
    raise SystemExit(main())
