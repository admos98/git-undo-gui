#!/usr/bin/env python3
"""Turn a compiler log into GitHub check annotations.

Job logs are not readable from the API (they live on a storage host some
environments cannot reach), so anything that fails here is re-emitted as an
annotation on the file and line that caused it. That makes CI output as useful
as a local terminal, and it means an agent can iterate on a Rust build it cannot
run itself.

    python3 scripts/ci/annotate.py /tmp/build.log
"""
import re
import sys

MAX_BLOCK = 6000
MAX_ANNOTATIONS = 30


def esc(text: str) -> str:
    return text.replace("%", "%25").replace("\r", "%0D").replace("\n", "%0A")


COMPILER = ("error", "warning", "Diff in")

# "---- guard::tests::foo stdout ----" ... up to the next header or the summary.
TEST_FAILURE = re.compile(
    r"(?ms)^---- (\S+) stdout ----\n(.*?)(?=^---- |\n^failures:$|\n^test result:)"
)


def blocks(log: str):
    """Split a cargo log into diagnostic blocks worth annotating."""
    # Compiler diagnostics first: they carry file/line information.
    parts = re.split(r"(?m)^(?=error(?:\[|:)|warning(?:\[|:)|Diff in )", log)
    for part in parts:
        if part.lstrip().startswith(COMPILER):
            yield part.rstrip()

    # Then failing tests, which have no file/line of their own.
    for match in TEST_FAILURE.finditer(log):
        name, body = match.group(1), match.group(2).rstrip()
        yield f"test {name} FAILED\n{body}"


def locate(block: str):
    """Find file/line for the annotation, if the block has one."""
    for pattern in (r"--> ([^\s:]+):(\d+):(\d+)", r"^Diff in ([^:]+):(\d+)"):
        match = re.search(pattern, block, re.M)
        if match:
            return match.group(1), match.group(2)
    return None, None


def main() -> int:
    path = sys.argv[1] if len(sys.argv) > 1 else "/tmp/build.log"
    try:
        with open(path, encoding="utf-8", errors="replace") as handle:
            log = handle.read()
    except FileNotFoundError:
        print(f"::error::no build log at {path}")
        return 0

    emitted = 0
    for block in blocks(log):
        if emitted >= MAX_ANNOTATIONS:
            print("::notice::further diagnostics truncated; see the job log")
            break
        file, line = locate(block)
        prefix = f" file={file},line={line}" if file else ""
        print(f"::error{prefix}::{esc(block[:MAX_BLOCK])}")
        emitted += 1

    if emitted == 0:
        print(f"::error::{esc(log[-MAX_BLOCK:])}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
