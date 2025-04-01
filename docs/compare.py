#!/bin/python3

import os
import subprocess
import time
import yaml
import re

# inline-mir-threshold
# inline-mir-forwarder-threshold
# inline-mir-hint-threshold
# cross-crate-inline-threshold

compare: dict[int, dict]

if os.path.exists("compare.yaml"):
    with open("compare.yaml", "r") as f:
        compare = yaml.safe_load(f)
        # print(config)
else:
    compare = dict()

print("Warming up...")

child = subprocess.run(
    ["cargo", "uitest", "--release"],
    env={**os.environ},
    stdout=subprocess.DEVNULL,
    stderr=subprocess.DEVNULL,
    encoding="utf-8",
)

# Tests that we want to investigate which specific functions are inlined.
skip = [
    "tests/ui/cve_2020_35862/cve_2020_35862_manually_inlined.rs",
    "tests/ui/cve_2020_35862/cve_2020_35862.rs",
    "tests/ui/cve_2021_38190/cve_2021_38190.rs",
]

re_result = re.compile(
    "test result: (FAIL|ok). ((?P<failed>\d+) failed; )?(?P<passed>\d+) passed; (?P<ignored>\d+) ignored;"
)

for cfg in [
    0,
    10,
    20,
    30,
    40,
    50,
    60,
    80,
    100,
    150,
    200,
    300,
    400,
    500,
    600,
    700,
    800,
    900,
    1000,
]:
    print(f"Running with -Z inline-mir-threshold={cfg}")
    t1 = time.time()
    child = subprocess.run(
        ["cargo", "uitest", "--release", "--", *(["--skip", skip] for skip in skip)],
        env={
            **os.environ,
            "RPL_TEST_INLINE_MIR_THRESHOLD": str(cfg),
            # "RPL_TEST_THREADS": "8",
        },
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        encoding="utf-8",
        timeout=120,
    )
    t2 = time.time()
    print(f"Done with {cfg} in {t2-t1:.2f}s")
    # if child.returncode != 0:
    #     raise ValueError(f"Error running with {cfg}: {child.stderr}\n{child.stdout}")
    # if child.stdout:
    #     print(f"Output: {child.stdout}")
    # if child.stderr:
    #     print(f"Error: {child.stderr}")
    matched = re.search(re_result, child.stdout)
    print(matched)
    assert "test result:" in child.stdout
    assert matched is not None, f"Error parsing output: {child.stdout}"
    groupdict = matched.groupdict()
    failed = (
        int(groupdict["failed"])
        if "failed" in groupdict and groupdict["failed"] is not None
        else 0
    )
    assert (child.returncode == 0) == (
        failed == 0
    ), f"Error running with {cfg}: {child.stderr}\n{child.stdout}"
    passed = int(groupdict["passed"])
    ignored = int(groupdict["ignored"])

    compare[cfg] = {
        "time": t2 - t1,
        "passed": passed,
        "failed": failed,
        "ignored": ignored,
    }

print("Done with all tests")

print("Writing results to compare.yaml")

with open("docs/compare.yaml", "w") as f:
    yaml.safe_dump(compare, f)
print("Done writing results to compare.yaml")
