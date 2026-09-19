#!/usr/bin/env python3
"""Verify that Flatpak cargo sources match Cargo.lock.

Flatpak builds run Cargo in offline mode against the vendored source tree
described by cargo-sources.json. If Cargo.lock changes but the generated
Flatpak sources are not refreshed, Cargo can fail much later with a resolver
error such as "candidate versions found which didn't match". This check keeps
that failure immediate and actionable without touching the network.
"""

from __future__ import annotations

import argparse
import ast
import json
import re
import sys
from pathlib import Path
from typing import Any
from urllib.parse import parse_qs, urlparse


ROOT = Path(__file__).resolve().parents[1]
LOCK_FILE = ROOT / "Cargo.lock"
DEFAULT_SOURCES_FILE = ROOT / "cargo-sources.json"
CRATES_IO_SOURCE = "registry+https://github.com/rust-lang/crates.io-index"
GIT_ROOT_COPY_RE = re.compile(
    r'^(?:mkdir -p "[^"]+" && )?'
    r'(?:rm -rf "[^"]+/\.git" && )?'
    r'cp -r --reflink=auto "(?P<src>[^"]+)/\." "(?P<dest>[^"]+)"'
    r'(?: && rm -rf "[^"]+/\.git")?$'
)
SUBDIR_COPY_RE = re.compile(
    r'^(?:mkdir -p "[^"]+" && )?'
    r'cp -r --reflink=auto "(?P<src>[^"]+)" "(?P<dest>[^"]+)"$'
)
GIT_CACHE_SHORT_RE = re.compile(
    r"^flatpak-cargo/git/.+-([0-9a-f]{7})(?:/.*)?$"
)


def display_path(path: Path) -> str:
    try:
        return str(path.relative_to(ROOT))
    except ValueError:
        return str(path)


def resolve_sources_file(path: str | None) -> Path:
    if path is None:
        return DEFAULT_SOURCES_FILE

    sources_file = Path(path)
    if sources_file.is_absolute():
        return sources_file

    return ROOT / sources_file


def parse_lock_string(value: str) -> str:
    """Parse the simple quoted string values Cargo.lock uses."""
    try:
        parsed = ast.literal_eval(value)
    except (SyntaxError, ValueError) as exc:
        raise ValueError(f"unable to parse Cargo.lock value {value!r}") from exc

    if not isinstance(parsed, str):
        raise ValueError(f"expected a string in Cargo.lock, got {value!r}")

    return parsed


def iter_lock_packages(lock_file: Path) -> list[dict[str, str]]:
    """Return package tables from Cargo.lock with only fields we need."""
    packages: list[dict[str, str]] = []
    current: dict[str, str] | None = None

    for raw_line in lock_file.read_text(encoding="utf-8").splitlines():
        line = raw_line.strip()

        if line == "[[package]]":
            if current is not None:
                packages.append(current)
            current = {}
            continue

        if current is None or "=" not in line:
            continue

        key, raw_value = line.split("=", 1)
        key = key.strip()
        if key not in {"name", "version", "source", "checksum"}:
            continue

        current[key] = parse_lock_string(raw_value.strip())

    if current is not None:
        packages.append(current)

    return packages


def iter_source_entries(value: Any) -> list[dict[str, Any]]:
    """Flatten cargo-sources.json entries defensively."""
    if isinstance(value, dict):
        return [value]

    if isinstance(value, list):
        entries: list[dict[str, Any]] = []
        for item in value:
            entries.extend(iter_source_entries(item))
        return entries

    return []


def registry_packages(lock_file: Path) -> list[dict[str, str]]:
    return sorted(
        (
            package
            for package in iter_lock_packages(lock_file)
            if package.get("source") == CRATES_IO_SOURCE
        ),
        key=lambda package: (package["name"], package["version"]),
    )


def load_sources(
    sources_file: Path,
) -> tuple[dict[str, dict[str, Any]], dict[str, str]]:
    entries = iter_source_entries(
        json.loads(sources_file.read_text(encoding="utf-8"))
    )

    archives: dict[str, dict[str, Any]] = {}
    inline_checksums: dict[str, str] = {}

    for entry in entries:
        dest = entry.get("dest")
        if not isinstance(dest, str):
            continue

        if entry.get("type") == "archive":
            archives[dest] = entry
            continue

        if (
            entry.get("type") == "inline"
            and entry.get("dest-filename") == ".cargo-checksum.json"
        ):
            contents = entry.get("contents")
            if not isinstance(contents, str):
                continue
            try:
                checksum_data = json.loads(contents)
            except json.JSONDecodeError:
                continue
            package_checksum = checksum_data.get("package")
            if isinstance(package_checksum, str):
                inline_checksums[dest] = package_checksum

    return archives, inline_checksums


def git_root_copy_strip_command(dest: str) -> str:
    return f'rm -rf "{dest}/.git"'


def mkdir_parent_command(dest: str) -> str:
    return f'mkdir -p "{str(Path(dest).parent)}"'


def ensure_mkdir_prefix(command: str, dest: str) -> str:
    mkdir = mkdir_parent_command(dest)
    if command.startswith(f"{mkdir} && "):
        return command
    return f"{mkdir} && {command}"


def rewrite_git_root_copy_command(command: str) -> str:
    match = GIT_ROOT_COPY_RE.match(command)
    if match is None:
        return command

    dest = match.group("dest")
    src = match.group("src")
    strip = git_root_copy_strip_command(dest)
    rewritten = (
        f"{strip} && "
        f'cp -r --reflink=auto "{src}/." "{dest}" && '
        f"{strip}"
    )
    if dest.startswith("cargo/vendor-git/"):
        rewritten = ensure_mkdir_prefix(rewritten, dest)
    return rewritten


def rewrite_git_root_copies(sources_file: Path) -> int:
    text = sources_file.read_text(encoding="utf-8")
    data = json.loads(text)
    replacements: list[tuple[str, str]] = []

    for entry in iter_source_entries(data):
        commands = entry.get("commands")
        if not isinstance(commands, list):
            continue
        for command in commands:
            if not isinstance(command, str):
                continue
            rewritten = rewrite_git_root_copy_command(command)
            if rewritten != command:
                replacements.append((command, rewritten))

    if not replacements:
        return 0

    new_text = text
    for old, new in replacements:
        old_json = json.dumps(old)
        new_json = json.dumps(new)
        if old_json not in new_text:
            raise ValueError(
                f"unable to rewrite git-root copy command {old!r} in "
                f"{display_path(sources_file)}"
            )
        new_text = new_text.replace(old_json, new_json, 1)

    sources_file.write_text(new_text, encoding="utf-8")
    return len(replacements)


def check_git_root_copies(sources_file: Path) -> list[str]:
    problems: list[str] = []
    entries = iter_source_entries(
        json.loads(sources_file.read_text(encoding="utf-8"))
    )

    for entry in entries:
        if entry.get("type") != "shell":
            continue
        commands = entry.get("commands")
        if not isinstance(commands, list):
            continue
        for command in commands:
            if not isinstance(command, str):
                continue
            match = GIT_ROOT_COPY_RE.match(command)
            if match is None:
                continue
            dest = match.group("dest")
            if git_root_copy_strip_command(dest) not in command:
                problems.append(
                    f"git-root copy leaves .git in {dest}: {command}"
                )

    return problems


def parse_git_lock_source(
    source: str,
) -> tuple[str, str | None, str | None, str | None]:
    raw = source[4:] if source.startswith("git+") else source
    parsed = urlparse(raw)
    path = parsed.path.rstrip("/")
    if path.endswith(".git"):
        path = path[:-4]
    repo = f"{parsed.scheme}://{parsed.netloc}{path}"
    rev = parse_qs(parsed.query).get("rev", [None])[0]
    commit = parsed.fragment or None
    short = (commit or rev or "")[:7] or None
    return repo, rev, commit, short


def iter_lock_git_packages(lock_file: Path) -> list[dict[str, str]]:
    packages: list[dict[str, str]] = []
    for package in iter_lock_packages(lock_file):
        source = package.get("source", "")
        if not source.startswith("git+"):
            continue
        repo, rev, commit, short = parse_git_lock_source(source)
        packages.append(
            {
                **package,
                "repo": repo,
                "rev": rev or "",
                "commit": commit or "",
                "short": short or "",
            }
        )
    return packages


def lock_git_by_short() -> dict[str, dict[str, str]]:
    result: dict[str, dict[str, str]] = {}
    for package in iter_lock_git_packages(LOCK_FILE):
        short = package.get("short")
        if not short:
            continue
        result[short] = {
            "repo": package["repo"],
            "rev": package["rev"] or short,
        }
    return result


def cargo_config_entry(entries: list[dict[str, Any]]) -> dict[str, Any] | None:
    for entry in entries:
        if entry.get("type") == "inline" and entry.get("dest-filename") == "config":
            return entry
    return None


def cargo_config_contents(entries: list[dict[str, Any]]) -> str:
    entry = cargo_config_entry(entries)
    if entry is None:
        return ""
    contents = entry.get("contents")
    return contents if isinstance(contents, str) else ""


def parse_cargo_config_git_revs(contents: str) -> dict[str, set[str]]:
    mapped: dict[str, set[str]] = {}
    current_git: str | None = None
    current_rev: str | None = None

    def flush() -> None:
        nonlocal current_git, current_rev
        if current_git and current_rev:
            mapped.setdefault(current_git, set()).add(current_rev)
        current_git = None
        current_rev = None

    for raw_line in contents.splitlines():
        line = raw_line.strip()
        if line.startswith("[source."):
            flush()
            continue
        if line.startswith("git = "):
            current_git = parse_lock_string(line.split("=", 1)[1].strip())
            continue
        if line.startswith("rev = "):
            current_rev = parse_lock_string(line.split("=", 1)[1].strip())
    flush()
    return mapped


def git_cache_short(src: str) -> str | None:
    path = src[:-2] if src.endswith("/.") else src
    match = GIT_CACHE_SHORT_RE.match(path)
    return match.group(1) if match else None


def command_src_dest(command: str) -> tuple[str, str] | None:
    match = GIT_ROOT_COPY_RE.match(command)
    if match is not None:
        return match.group("src"), match.group("dest")
    match = SUBDIR_COPY_RE.match(command)
    if match is not None:
        return match.group("src"), match.group("dest")
    return None


def vendor_crate_name(dest: str) -> str | None:
    prefix = "cargo/vendor/"
    name = dest[len(prefix) :] if dest.startswith(prefix) else ""
    if name and "/" not in name:
        return name
    return None


def vendor_git_crate_name(dest: str) -> str | None:
    parts = dest.split("/")
    if len(parts) == 4 and parts[0] == "cargo" and parts[1] == "vendor-git":
        return parts[3]
    return None


def extra_vendor_dest(short: str, crate_name: str) -> str:
    return f"cargo/vendor-git/{short}/{crate_name}"


def replace_command_dest(command: str, old_dest: str, new_dest: str) -> str:
    command = command.replace(f'"{old_dest}/.git"', f'"{new_dest}/.git"')
    return command.replace(f'"{old_dest}"', f'"{new_dest}"')


def extra_git_shorts(entries: list[dict[str, Any]]) -> set[str]:
    copies: list[tuple[str, str]] = []
    for entry in entries:
        if entry.get("type") != "shell":
            continue
        commands = entry.get("commands")
        if not isinstance(commands, list):
            continue
        for command in commands:
            if not isinstance(command, str):
                continue
            parsed = command_src_dest(command)
            if parsed is None:
                continue
            src, dest = parsed
            short = git_cache_short(src)
            crate = vendor_crate_name(dest) or vendor_git_crate_name(dest)
            if short is None or crate is None:
                continue
            copies.append((short, crate))

    last_short_by_crate: dict[str, str] = {}
    for short, crate in copies:
        last_short_by_crate[crate] = short

    extra: set[str] = set()
    for short, crate in copies:
        if last_short_by_crate.get(crate) != short:
            extra.add(short)
    return extra


def append_extra_git_sources(contents: str, extra: dict[str, dict[str, str]]) -> str:
    sections: list[str] = []
    for short, info in sorted(extra.items()):
        source_name = f"vendored-git-{short}"
        if f"[source.{source_name}]" in contents:
            continue
        repo = info["repo"]
        rev = info["rev"]
        sections.append(
            f"[source.{source_name}]\n"
            f'directory = "cargo/vendor-git/{short}"\n'
            f"\n"
            f'[source."{repo}?rev={rev}"]\n'
            f'git = "{repo}"\n'
            f'rev = "{rev}"\n'
            f'replace-with = "{source_name}"\n'
        )
    if not sections:
        return contents
    return contents.rstrip() + "\n\n" + "\n".join(sections)


def rewrite_duplicate_git_revs(sources_file: Path) -> int:
    data = json.loads(sources_file.read_text(encoding="utf-8"))
    entries = iter_source_entries(data)
    extra_shorts = extra_git_shorts(entries)
    if not extra_shorts:
        return 0

    lock_info = lock_git_by_short()
    extra_info = {
        short: lock_info[short] for short in extra_shorts if short in lock_info
    }
    changed = 0

    for index, entry in enumerate(entries):
        if entry.get("type") != "shell":
            continue
        commands = entry.get("commands")
        if not isinstance(commands, list):
            continue
        for command_index, command in enumerate(commands):
            if not isinstance(command, str):
                continue
            parsed = command_src_dest(command)
            if parsed is None:
                continue
            src, dest = parsed
            short = git_cache_short(src)
            crate = vendor_crate_name(dest) or vendor_git_crate_name(dest)
            if short not in extra_shorts or crate is None:
                continue
            new_dest = extra_vendor_dest(short, crate)
            new_command = command
            if dest != new_dest:
                new_command = replace_command_dest(new_command, dest, new_dest)
                for later in entries[index + 1 :]:
                    if later.get("type") == "shell":
                        break
                    if later.get("dest") == dest:
                        later["dest"] = new_dest
                        changed += 1
            new_command = ensure_mkdir_prefix(new_command, new_dest)
            if new_command != command:
                commands[command_index] = new_command
                changed += 1

    config_entry = cargo_config_entry(entries)
    if config_entry is not None and extra_info:
        old_contents = config_entry.get("contents", "")
        if isinstance(old_contents, str):
            new_contents = append_extra_git_sources(old_contents, extra_info)
            if new_contents != old_contents:
                config_entry["contents"] = new_contents
                changed += 1

    if changed:
        sources_file.write_text(
            json.dumps(data, indent=4, sort_keys=False) + "\n",
            encoding="utf-8",
        )
    return changed


def check_duplicate_git_revs(sources_file: Path) -> list[str]:
    problems: list[str] = []
    entries = iter_source_entries(
        json.loads(sources_file.read_text(encoding="utf-8"))
    )
    extra_shorts = extra_git_shorts(entries)
    lock_info = lock_git_by_short()
    mapped = parse_cargo_config_git_revs(cargo_config_contents(entries))

    for entry in entries:
        if entry.get("type") != "shell":
            continue
        commands = entry.get("commands")
        if not isinstance(commands, list):
            continue
        for command in commands:
            if not isinstance(command, str):
                continue
            parsed = command_src_dest(command)
            if parsed is None:
                continue
            src, dest = parsed
            short = git_cache_short(src)
            if short not in extra_shorts:
                continue
            expected_prefix = f"cargo/vendor-git/{short}/"
            if not dest.startswith(expected_prefix):
                problems.append(
                    f"extra git rev {short} still copies to {dest}"
                )
            if mkdir_parent_command(dest) not in command:
                problems.append(
                    f"extra git rev {short} copy is missing mkdir -p for {dest}"
                )

    for short in sorted(extra_shorts):
        info = lock_info.get(short)
        if info is None:
            problems.append(f"extra git rev {short} is not in Cargo.lock")
            continue
        if info["rev"] not in mapped.get(info["repo"], set()):
            problems.append(
                f"cargo/config missing offline mapping for {info['repo']} "
                f"rev {info['rev']}"
            )

    return problems


def check_sources(sources_file: Path) -> list[str]:
    problems: list[str] = []

    if not LOCK_FILE.exists():
        return [f"{display_path(LOCK_FILE)} is missing"]

    if not sources_file.exists():
        return [
            f"{display_path(sources_file)} is missing; "
            "run ./generate-cargo-sources.sh"
        ]

    problems.extend(check_git_root_copies(sources_file))
    problems.extend(check_duplicate_git_revs(sources_file))

    archives, inline_checksums = load_sources(sources_file)

    for package in registry_packages(LOCK_FILE):
        name = package["name"]
        version = package["version"]
        checksum = package.get("checksum")
        dest = f"cargo/vendor/{name}-{version}"
        archive = archives.get(dest)

        if archive is None:
            candidates = sorted(
                candidate
                for candidate in archives
                if candidate.startswith(f"cargo/vendor/{name}-")
            )
            hint = f"; found {', '.join(candidates)}" if candidates else ""
            problems.append(f"missing {dest}{hint}")
            continue

        expected_url_suffix = f"/{name}/{name}-{version}.crate"
        url = archive.get("url")
        if not isinstance(url, str) or not url.endswith(expected_url_suffix):
            problems.append(f"{dest} has unexpected archive URL {url!r}")

        archive_checksum = archive.get("sha256")
        if checksum and archive_checksum != checksum:
            problems.append(
                f"{dest} archive checksum is {archive_checksum!r}, "
                f"expected {checksum!r}"
            )

        inline_checksum = inline_checksums.get(dest)
        if checksum and inline_checksum != checksum:
            problems.append(
                f"{dest} inline checksum is {inline_checksum!r}, "
                f"expected {checksum!r}"
            )

    return problems


def main(argv: list[str]) -> int:
    parser = argparse.ArgumentParser(
        description="Verify or rewrite Flatpak cargo-sources.json."
    )
    parser.add_argument(
        "sources_file",
        nargs="?",
        help="Path to cargo-sources.json (defaults to the repo file)",
    )
    parser.add_argument(
        "--rewrite",
        action="store_true",
        help="Rewrite git-root copies and extra git-rev mappings, then check",
    )
    args = parser.parse_args(argv[1:])

    sources_file = resolve_sources_file(args.sources_file)
    if args.rewrite:
        if not sources_file.exists():
            print(
                f"{display_path(sources_file)} is missing; "
                "run ./generate-cargo-sources.sh",
                file=sys.stderr,
            )
            return 1
        rewritten = rewrite_git_root_copies(sources_file)
        print(
            f"Rewrote {rewritten} git-root vendor copy command(s) in "
            f"{display_path(sources_file)}"
        )
        remapped = rewrite_duplicate_git_revs(sources_file)
        print(
            f"Rewrote {remapped} extra git-rev mapping(s) in "
            f"{display_path(sources_file)}"
        )

    problems = check_sources(sources_file)
    if problems:
        print(
            f"{display_path(sources_file)} is out of sync with Cargo.lock:",
            file=sys.stderr,
        )
        for problem in problems:
            print(f"  - {problem}", file=sys.stderr)
        print(
            "Run ./generate-cargo-sources.sh and commit the updated cargo-sources.json.",
            file=sys.stderr,
        )
        return 1

    print(
        f"{display_path(sources_file)} matches Cargo.lock registry package sources."
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
