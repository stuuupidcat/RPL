#!/usr/bin/env python3
"""CI checks for RPL patterns, patterns.rs sync, and UI stderr hygiene.

Checks (any failure => exit 1):
1. Every docs/patterns-pest/{clippy,codeql,cve,ub}/*.rpl is listed in patterns.rs
   (also reports list entries missing on disk).
2. patterns.rs entries match filesystem order (category order + name sort).
3. Each pattern's diag `name` appears in a non-ignore-on-host tests/ui stderr.
4. Extra .stderr files: ignore-on-host targets, or revision mismatches.
"""

from __future__ import annotations

import re
import sys
from collections import defaultdict
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
PATTERNS_DIR = ROOT / "docs" / "patterns-pest"
PATTERNS_RS = ROOT / "crates" / "rpl_patterns" / "src" / "patterns.rs"
UI_DIR = ROOT / "tests" / "ui"

CATEGORIES = ("clippy", "codeql", "cve", "ub")

GLOBAL_IGNORE_ON_HOST_RE = re.compile(r"(?m)^//\s*@\s*ignore-on-host\b")
REV_IGNORE_ON_HOST_RE = re.compile(r"(?m)^//\s*@\s*\[([^\]]+)\]\s*ignore-on-host\b")
REVISIONS_RE = re.compile(r"(?m)^//\s*@\s*revisions:\s*(.+)$")
PATTERN_LITERAL_RE = re.compile(r'"((?:clippy|codeql|cve|ub)/[^"]+\.rpl)"')
LINT_NAME_PATS = (
    re.compile(r"\brpl::([A-Za-z0-9_]+)\b"),
    re.compile(r"#\[(?:deny|warn|allow)\((?:rpl::)?([A-Za-z0-9_]+)\)\]"),
)


def header_directives(text: str) -> str:
    lines: list[str] = []
    for line in text.splitlines():
        s = line.strip()
        if not s or s.startswith("//"):
            lines.append(line)
            continue
        break
    return "\n".join(lines)


def filesystem_patterns() -> list[str]:
    entries: list[str] = []
    for category in CATEGORIES:
        directory = PATTERNS_DIR / category
        if not directory.is_dir():
            continue
        names = sorted(
            p.name for p in directory.iterdir() if p.is_file() and p.suffix == ".rpl"
        )
        entries.extend(f"{category}/{name}" for name in names)
    return entries


def parse_patterns_rs() -> list[str]:
    text = PATTERNS_RS.read_text(encoding="utf-8")
    return PATTERN_LITERAL_RE.findall(text)


def extract_diag_block(text: str) -> str | None:
    m = re.search(r"\bdiag\s*\{", text)
    if not m:
        return None
    depth = 0
    start: int | None = None
    for i, ch in enumerate(text[m.start() :], m.start()):
        if ch == "{":
            if depth == 0:
                start = i + 1
            depth += 1
        elif ch == "}":
            depth -= 1
            if depth == 0 and start is not None:
                return text[start:i]
    return None


def diag_names(text: str) -> list[str]:
    body = extract_diag_block(text)
    if body is None:
        return []
    names: list[str] = []
    seen: set[str] = set()
    for n in re.findall(r'\bname\s*=\s*"([^"]+)"', body):
        key = n.replace("-", "_")
        if key not in seen:
            seen.add(key)
            names.append(key)
    return names


def resolve_rs_for_stderr(stderr: Path) -> tuple[Path | None, str | None]:
    """Map foo.inline.stderr -> (foo.rs, 'inline'); foo.stderr -> (foo.rs, None)."""
    stem = stderr.name[: -len(".stderr")]
    parts = stem.split(".")
    for i in range(len(parts), 0, -1):
        cand = stderr.with_name(".".join(parts[:i]) + ".rs")
        if cand.is_file():
            rev = ".".join(parts[i:]) or None
            return cand, rev
    return None, None


def parse_rs_meta(rs: Path) -> tuple[bool, set[str], set[str]]:
    """Return (file_ignore_on_host, ignored_revisions, declared_revisions)."""
    header = header_directives(rs.read_text(encoding="utf-8", errors="replace"))
    file_ignored = bool(GLOBAL_IGNORE_ON_HOST_RE.search(header))
    ignored_revs = {m.group(1) for m in REV_IGNORE_ON_HOST_RE.finditer(header)}
    declared: set[str] = set()
    m = REVISIONS_RE.search(header)
    if m:
        declared = set(m.group(1).split())
    return file_ignored, ignored_revs, declared


def is_ignore_on_host(rs: Path, revision: str | None) -> bool:
    file_ignored, ignored_revs, _ = parse_rs_meta(rs)
    if file_ignored:
        return True
    return bool(revision and revision in ignored_revs)


def collect_coverage_lints() -> dict[str, list[Path]]:
    """lint name -> stderr files that are not ignore-on-host."""
    lint_to_files: dict[str, list[Path]] = defaultdict(list)
    for stderr in sorted(UI_DIR.rglob("*.stderr")):
        rs, rev = resolve_rs_for_stderr(stderr)
        if rs is not None and is_ignore_on_host(rs, rev):
            continue
        content = stderr.read_text(encoding="utf-8", errors="replace")
        found: set[str] = set()
        for pat in LINT_NAME_PATS:
            found |= set(pat.findall(content))
        for lint in found:
            lint_to_files[lint].append(stderr)
    return lint_to_files


def check_membership_and_order(
    fs_list: list[str], rs_list: list[str]
) -> tuple[list[str], list[str]]:
    err1: list[str] = []
    err2: list[str] = []

    fs_set = set(fs_list)
    rs_set = set(rs_list)

    missing_in_rs = sorted(fs_set - rs_set)
    missing_on_disk = sorted(rs_set - fs_set)
    for path in missing_in_rs:
        err1.append(f"present in docs/patterns-pest but missing from patterns.rs: {path}")
    for path in missing_on_disk:
        err1.append(f"listed in patterns.rs but missing on disk: {path}")

    if rs_list != fs_list:
        # Focus order when membership is otherwise equal-ish
        err2.append("patterns.rs order does not match filesystem order "
                    f"(categories {CATEGORIES}, sorted within each).")
        err2.append(f"  expected ({len(fs_list)}):")
        for p in fs_list:
            err2.append(f"    {p}")
        err2.append(f"  actual ({len(rs_list)}):")
        for p in rs_list:
            err2.append(f"    {p}")
        # Compact first mismatch hint
        for i, (a, b) in enumerate(zip(fs_list, rs_list)):
            if a != b:
                err2.append(f"  first mismatch at index {i}: expected {a!r}, got {b!r}")
                break
        if len(fs_list) != len(rs_list):
            err2.append(
                f"  length differs: filesystem={len(fs_list)} patterns.rs={len(rs_list)}"
            )

    return err1, err2


def check_ui_coverage(fs_list: list[str], lint_map: dict[str, list[Path]]) -> list[str]:
    errors: list[str] = []
    for rel in fs_list:
        path = PATTERNS_DIR / rel
        names = diag_names(path.read_text(encoding="utf-8", errors="replace"))
        if not names:
            errors.append(f"{rel}: no diag `name = \"...\"` (cannot verify UI coverage)")
            continue
        if not any(lint_map.get(n) for n in names):
            errors.append(
                f"{rel}: none of diag names found in non-ignore-on-host stderr: "
                + ", ".join(names)
            )
    return errors


def related_stderrs(rs: Path) -> list[tuple[Path, str | None]]:
    """All stderr files that resolve back to this .rs."""
    out: list[tuple[Path, str | None]] = []
    stem = rs.stem
    for stderr in sorted(rs.parent.glob(f"{stem}*.stderr")):
        resolved, rev = resolve_rs_for_stderr(stderr)
        if resolved == rs:
            out.append((stderr, rev))
    return out


def check_extra_stderrs() -> list[str]:
    errors: list[str] = []
    seen: set[Path] = set()

    for rs in sorted(UI_DIR.rglob("*.rs")):
        file_ignored, ignored_revs, declared = parse_rs_meta(rs)
        for stderr, rev in related_stderrs(rs):
            seen.add(stderr.resolve())
            rel = stderr.relative_to(ROOT).as_posix()

            if file_ignored:
                errors.append(
                    f"{rel}: extra stderr for file-level ignore-on-host "
                    f"({rs.relative_to(ROOT).as_posix()})"
                )
                continue

            if rev and rev in ignored_revs:
                errors.append(
                    f"{rel}: extra stderr for revision ignore-on-host "
                    f"({rs.relative_to(ROOT).as_posix()} [{rev}])"
                )
                continue

            if declared:
                if rev is None:
                    errors.append(
                        f"{rel}: bare stderr while //@revisions declares "
                        f"{sorted(declared)} ({rs.relative_to(ROOT).as_posix()})"
                    )
                elif rev not in declared:
                    errors.append(
                        f"{rel}: revision {rev!r} not in //@revisions "
                        f"{sorted(declared)} ({rs.relative_to(ROOT).as_posix()})"
                    )
            elif rev is not None:
                # No //@revisions, but stderr has a revision suffix and no matching .rs
                alt_rs = stderr.with_name(f"{rs.stem}.{rev}.rs")
                if not alt_rs.is_file():
                    errors.append(
                        f"{rel}: revision-like stderr without //@revisions "
                        f"({rs.relative_to(ROOT).as_posix()})"
                    )

    # stderr with no resolvable .rs
    for stderr in sorted(UI_DIR.rglob("*.stderr")):
        if stderr.resolve() in seen:
            continue
        rs, rev = resolve_rs_for_stderr(stderr)
        if rs is None:
            errors.append(
                f"{stderr.relative_to(ROOT).as_posix()}: no corresponding .rs file"
            )
    return errors


def print_section(title: str, errors: list[str]) -> None:
    print(f"=== {title} ===")
    if not errors:
        print("OK")
    else:
        for e in errors:
            print(f"ERROR: {e}")
    print()


def main() -> int:
    if not PATTERNS_DIR.is_dir():
        print(f"ERROR: patterns dir missing: {PATTERNS_DIR}", file=sys.stderr)
        return 1
    if not PATTERNS_RS.is_file():
        print(f"ERROR: patterns.rs missing: {PATTERNS_RS}", file=sys.stderr)
        return 1
    if not UI_DIR.is_dir():
        print(f"ERROR: tests/ui missing: {UI_DIR}", file=sys.stderr)
        return 1

    fs_list = filesystem_patterns()
    rs_list = parse_patterns_rs()
    lint_map = collect_coverage_lints()

    err1, err2 = check_membership_and_order(fs_list, rs_list)
    # If membership already differs, order section still reports full mismatch;
    # avoid double-counting identical membership lines in exit logic only.
    err3 = check_ui_coverage(fs_list, lint_map)
    err4 = check_extra_stderrs()

    print_section("1. patterns-pest vs patterns.rs membership", err1)
    print_section("2. patterns.rs order", err2)
    print_section("3. UI coverage (non-ignore-on-host)", err3)
    print_section("4. extra stderr files", err4)

    failed = bool(err1 or err2 or err3 or err4)
    if failed:
        print(
            f"FAILED: membership={len(err1)} order={len(err2)} "
            f"coverage={len(err3)} extra_stderr={len(err4)}"
        )
        return 1

    print("All pattern checks passed.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
