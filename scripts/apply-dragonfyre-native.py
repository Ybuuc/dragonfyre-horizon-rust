"""Apply the tracked Dragonfyre native mixin to a prepared Horizon workspace.

Creates a new, isolated output under outputs/native-reproduction. Never changes
the given Horizon workspace, existing Stage 07/09 trees, launcher or game saves.
Requires an already compiled horizon-mixins executable and prepared Horizon tree.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import shutil
import subprocess
import sys
import tempfile
import tomllib
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / "native" / "dragonfyre-quests"


def full_path(value: str) -> Path:
    return Path(value).expanduser().resolve()


def digest(path: Path) -> str:
    h = hashlib.sha256()
    with path.open("rb") as stream:
        while chunk := stream.read(1024 * 1024):
            h.update(chunk)
    return h.hexdigest()


def ensure_distinct(source: Path, output: Path, label: str) -> None:
    if source == output or output.is_relative_to(source) or source.is_relative_to(output):
        raise ValueError(f"Output must not overlap {label}: {source} / {output}")


def wire_necklace_health(applied: Path) -> str:
    # The pre-health point is inside a let-else binding in enter_guest.
    # The pinned mixin engine cannot capture that local.
    path = applied / "crates/horizon-client/src/game/lan_actions.rs"
    source = path.read_bytes()
    anchor = b"        let _ = motion.set_health(g.health, false);"
    if source.count(anchor) != 1:
        raise ValueError("Expected exactly one guest health restoration point")
    newline = b"\r\n" if anchor + b"\r\n" in source else b"\n"
    hook = (
        b"        super::native_hooks::dragonfyre_necklace::sync_bonus("
        b"&mut self.player_attachments, &mut motion);"
    )
    path.write_bytes(source.replace(anchor, hook + newline + anchor, 1))
    return digest(path)


def wire_swim_amulet(applied: Path) -> dict[str, str]:
    hashes = {}

    def edit(relative: str, *replacements: tuple[str, str, int]) -> None:
        path = applied / relative
        source = path.read_bytes()
        newline = "\r\n" if b"\r\n" in source else "\n"
        for old, new, count in replacements:
            anchor = old.replace("\n", newline).encode()
            if source.count(anchor) != count:
                raise ValueError(f"{relative}: unexpected swimming hook location: {old}")
            source = source.replace(anchor, new.replace("\n", newline).encode())
        path.write_bytes(source)
        hashes[relative] = digest(path)

    edit(
        "crates/horizon-world/src/living_fluid.rs",
        (
            "    fn water_movement_efficiency(&mut self) -> Result<f64, Self::Error>;",
            "    fn water_movement_efficiency(&mut self) -> Result<f64, Self::Error>;\n"
            "    fn water_propulsion_multiplier(&mut self) -> f32 { 1.0 }",
            1,
        ),
        (
            "    move_relative(world, input, speed)?;",
            "    let speed = speed * world.water_propulsion_multiplier();\n"
            "    move_relative(world, input, speed)?;",
            1,
        ),
    )
    edit(
        "crates/horizon-client/src/motion/environment.rs",
        (
            "    fn dolphins_grace(&mut self) -> Result<bool> {",
            "    fn water_propulsion_multiplier(&mut self) -> f32 {\n"
            "        self.player.dragonfyre_swim_factor() as f32\n"
            "    }\n"
            "    fn dolphins_grace(&mut self) -> Result<bool> {",
            1,
        ),
        (
            "            self.player.velocity.y += f64::from(0.04f32);",
            "            let swim_factor = self.player.dragonfyre_swim_factor();\n"
            "            self.player.velocity.y += f64::from(0.04f32) * swim_factor;",
            2,
        ),
    )
    edit(
        "crates/horizon-client/src/motion.rs",
        (
            "            self.player.velocity.y += f64::from(-0.04f32);",
            "            let swim_factor = self.player.dragonfyre_swim_factor();\n"
            "            self.player.velocity.y += f64::from(-0.04f32) * swim_factor;",
            1,
        ),
    )
    return hashes


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--workspace",
        type=full_path,
        default=(ROOT / "work" / "native-package-pristine").resolve(),
        help="Prepared, pinned Horizon workspace (must contain Cargo.toml)",
    )
    parser.add_argument(
        "--mixins",
        type=full_path,
        default=(ROOT / "work" / "mixins-target" / "debug" / "horizon-mixins.exe").resolve(),
        help="Compiled horizon-mixins executable",
    )
    parser.add_argument(
        "--output-parent",
        type=full_path,
        default=(ROOT / "outputs" / "native-reproduction").resolve(),
        help="Parent of a uniquely named isolated run directory",
    )
    args = parser.parse_args()
    workspace = args.workspace
    mixins = args.mixins
    output_parent = args.output_parent

    manifest_path = SOURCE / "horizon-mixin.toml"
    if not manifest_path.is_file():
        parser.error(f"Dragonfyre mixin manifest missing: {manifest_path}")
    if not (workspace / "Cargo.toml").is_file():
        parser.error(f"Horizon Cargo.toml missing: {workspace / 'Cargo.toml'}")
    if not mixins.is_file():
        parser.error(f"Mixin executable missing: {mixins}")
    ensure_distinct(workspace, output_parent, "Horizon workspace")
    ensure_distinct(SOURCE, output_parent, "Dragonfyre source")

    with manifest_path.open("rb") as stream:
        manifest = tomllib.load(stream)
    expected_injections = len(manifest.get("injection", []))
    expected_modules = len(manifest.get("module", []))
    if expected_injections < 1:
        parser.error("Mixin manifest contains no injections")

    output_parent.mkdir(parents=True, exist_ok=True)
    run = Path(tempfile.mkdtemp(prefix="dragonfyre-native-", dir=output_parent))
    mods = run / "mods"
    applied = run / "applied"
    shutil.copytree(
        SOURCE,
        mods / "dragonfyre_quests",
        ignore=shutil.ignore_patterns(".git", "target", "Cargo.lock", "*.exe", "*.dll"),
    )
    command = [
        str(mixins), "apply", "--workspace", str(workspace),
        "--mods", str(mods), "--out", str(applied),
    ]
    print(f"Reproduction run: {run}", flush=True)
    print(f"Horizon source:    {workspace}", flush=True)
    print(f"Mixin executable:  {mixins}", flush=True)
    print(f"Manifest expects: {expected_injections} injections, {expected_modules} modules", flush=True)
    result = subprocess.run(
        command, cwd=ROOT, capture_output=True, text=True, errors="replace",
    )
    (run / "apply.log").write_text(
        result.stdout + "\n--- STDERR ---\n" + result.stderr, encoding="utf-8",
    )
    report: dict = {
        "status": "failed",
        "command": command,
        "exit_code": result.returncode,
        "horizon_source": str(workspace),
        "mixin_executable": str(mixins),
        "mixin_sha256": digest(mixins),
        "manifest_sha256": digest(manifest_path),
        "expected_injections": expected_injections,
        "expected_modules": expected_modules,
        "applied": str(applied),
    }

    if result.returncode != 0:
        (run / "apply-report.json").write_text(json.dumps(report, indent=2), encoding="utf-8")
        print(f"FAIL: mixins exited {result.returncode}. Log: {run / 'apply.log'}", file=sys.stderr)
        return result.returncode or 1
    try:
        receipt = json.loads(result.stdout)
    except json.JSONDecodeError as exc:
        report["error"] = f"Cannot decode mixins receipt: {exc}"
        (run / "apply-report.json").write_text(json.dumps(report, indent=2), encoding="utf-8")
        print(f"FAIL: invalid receipt. Log: {run / 'apply.log'}", file=sys.stderr)
        return 1

    report["receipt"] = receipt
    if (
        receipt.get("status") != "ok"
        or receipt.get("injections") != expected_injections
        or receipt.get("modules") != expected_modules
    ):
        report["error"] = "Receipt counts/status differ from tracked manifest"
        (run / "apply-report.json").write_text(json.dumps(report, indent=2), encoding="utf-8")
        print(f"FAIL: receipt mismatch. Report: {run / 'apply-report.json'}", file=sys.stderr)
        return 1

    generated_engine = applied / "native-crates" / "dragonfyre_quests" / "engine"
    original_engine = SOURCE / "engine"
    mismatches: list[str] = []
    verified = 0
    for origin in original_engine.rglob("*"):
        if not origin.is_file() or origin.suffix not in (".rs", ".toml"):
            continue
        relative = origin.relative_to(original_engine)
        target = generated_engine / relative
        if not target.is_file() or digest(origin) != digest(target):
            mismatches.append(str(relative))
        else:
            verified += 1
    report["verified_engine_files"] = verified
    report["engine_mismatches"] = mismatches
    report["status"] = "ok" if not mismatches else "engine_mismatch"
    if not mismatches:
        try:
            report["necklace_health_source_sha256"] = wire_necklace_health(applied)
            report["swim_amulet_source_sha256"] = wire_swim_amulet(applied)
        except (OSError, ValueError) as error:
            report["status"] = "necklace_integration_failed"
            report["error"] = str(error)
    (run / "apply-report.json").write_text(json.dumps(report, indent=2), encoding="utf-8")

    if mismatches:
        print(f"FAIL: {len(mismatches)} generated engine files differ: {mismatches}", file=sys.stderr)
        return 1
    if report["status"] != "ok":
        print(f"FAIL: {report['error']}", file=sys.stderr)
        return 1

    print(f"PASS: mixins applied; {verified} engine source files match GitHub.", flush=True)
    print(f"Generated source: {applied}", flush=True)
    print(f"Receipt/log: {run / 'apply-report.json'} / {run / 'apply.log'}", flush=True)
    print("No Horizon binary built or game launched. Keep this output for next build step.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
