#!/usr/bin/env python3
"""Ends the runs stranded behind a dead self-hosted pool.

Runs on a schedule on a GitHub-hosted runner (.github/workflows/dyna-pool-watch.yml). GitHub lets a job wait 24
hours for a runner, so a pool that dies would hold every run routed to it for a day.

Waiting is not enough to call a job stuck, because a run queued behind a busy pool waits too. So a run counts as
stranded once it has a job queued for the pool and no job of it started or finished for STALL_MINUTES. Then the
watch looks at the pool's jobs across the repository, and cancels the stranded runs only when, twice in a row
QUIET_RECHECK apart, none of them is running or finished in that time. That is a pool that is down, or one that
cannot start a runner. The watch ends red, and a rerun can use GitHub-hosted runners once the pool's lease expires.

One sweep serves the whole repository: a watcher per run held one of GitHub's runners for as long as the run lasted.
"""
from datetime import datetime
import json
import os
import subprocess
import sys
import time

LABEL = "dyna-"
# A pool that lost its last job can be between jobs, so one quiet look proves nothing.
QUIET_RECHECK = 180


def seconds(stamp):
    """A GitHub timestamp as a Unix time, or 0 when it is empty."""
    return datetime.fromisoformat(stamp).timestamp() if stamp else 0


def on_pool(job, label):
    """Whether the job asks for a Dyna runner: any label that starts with the prefix (dyna-small, dyna-medium, ...)."""
    return any(name.startswith(label) for name in job.get("labels", []))


def last_progress(run, jobs):
    """The newest time the run was created or any of its jobs started or finished."""
    stamps = [seconds(job.get(key)) for job in jobs for key in ("started_at", "completed_at")]
    return max(stamps + [seconds(run["created_at"])])


def pool_worked(jobs, label, since):
    """Whether one of these jobs shows a runner serving the pool now or since then.

    A job that never left the queue has a start time and may have a finish time, so
    only a job that held a runner counts.
    """
    for job in jobs:
        if not on_pool(job, label):
            continue
        if job["status"] == "in_progress" or (job.get("runner_name") and seconds(job.get("completed_at")) >= since):
            return True
    return False


def runs(api, repo, status):
    """`api` returns every page of a listing."""
    pages = api(f"repos/{repo}/actions/runs?status={status}&per_page=100")
    return [run for page in pages for run in page["workflow_runs"]]


def jobs_of(api, repo, run):
    return [job for page in api(f"repos/{repo}/actions/runs/{run['id']}/jobs?filter=latest&per_page=100")
            for job in page["jobs"]]


def stranded(api, repo, label, now, stall, me):
    """The unfinished runs with a job queued for the pool and no progress for `stall` seconds, and none when the
    pool served any job since then (a busy pool, not a dead one)."""
    unfinished = [run for status in ("queued", "in_progress") for run in runs(api, repo, status) if run["id"] != me]
    found = []
    # The oldest runs reached the pool first, so a busy pool usually answers at once. A run younger than the stall
    # can't be stranded yet, and only a running one can show the pool working.
    for run in sorted(unfinished, key=lambda run: run["id"]):
        old = now - seconds(run["created_at"]) >= stall
        if not old and run["status"] != "in_progress":
            continue
        jobs = jobs_of(api, repo, run)
        if pool_worked(jobs, label, now - stall):
            return []
        queued = sum(job["status"] == "queued" and on_pool(job, label) for job in jobs)
        if old and queued and now - last_progress(run, jobs) >= stall:
            found.append((run, queued))
    return found


def watch(api, cancel, clock, sleep, repo, me, stall, label=LABEL):
    """Returns the exit code: 1 when it cancelled runs, else 0."""
    if not stranded(api, repo, label, clock(), stall, me):
        return 0
    sleep(QUIET_RECHECK)
    found = stranded(api, repo, label, clock(), stall, me)
    if not found:
        return 0
    for run, queued in found:
        print(f"::error::{run['name']} ({run['html_url']}): {queued} job(s) waited {stall // 60} minutes and no job "
              "on the self-hosted pool is running. Cancelling it; a rerun uses GitHub-hosted runners once the "
              "pool's lease expires.")
        cancel(run["id"])
    return 1


def gh(*args):
    return subprocess.run(["gh", *args], check=True, capture_output=True, text=True).stdout


def main():
    repo = os.environ["GITHUB_REPOSITORY"]
    return watch(lambda path: json.loads(gh("api", "--paginate", "--slurp", path)),
                 lambda run: gh("run", "cancel", "-R", repo, str(run)), time.time, time.sleep, repo,
                 int(os.environ["GITHUB_RUN_ID"]), int(os.environ.get("STALL_MINUTES", "20")) * 60,
                 os.environ.get("POOL_LABEL", LABEL))


if __name__ == "__main__":
    sys.exit(main())
