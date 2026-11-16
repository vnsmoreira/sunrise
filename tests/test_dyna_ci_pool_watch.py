"""The scheduled watch cancels runs stranded behind a dead pool, and only those."""
from datetime import datetime, timezone
from pathlib import Path
import sys
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "tools/ci"))
import dyna_ci_pool_watch as watch  # noqa: E402

NOW = 1_800_000_000
STALL = 20 * 60


def stamp(minutes_ago):
    return datetime.fromtimestamp(NOW - minutes_ago * 60, timezone.utc).isoformat()


def run(ident, status, minutes_ago):
    return {"id": ident, "status": status, "created_at": stamp(minutes_ago), "name": f"run {ident}",
            "html_url": f"https://example.invalid/{ident}"}


def job(status, label="dyna-large", started=None, completed=None, runner=None):
    return {"status": status, "labels": [label], "started_at": started and stamp(started),
            "completed_at": completed and stamp(completed), "runner_name": runner}


class Watch(unittest.TestCase):
    def sweep(self, runs, jobs, me=999):
        requests, cancelled, sleeps = [], [], []

        def api(path):
            requests.append(path)
            if "/jobs?" in path:
                return [{"jobs": jobs[int(path.split("/runs/")[1].split("/")[0])]}]
            status = path.split("status=")[1].split("&")[0]
            return [{"workflow_runs": [entry for entry in runs if entry["status"] == status]}]

        code = watch.watch(api, cancelled.append, lambda: NOW, sleeps.append, "o/r", me, STALL)
        return code, cancelled, sleeps, requests

    def test_a_dead_pool_has_its_stranded_runs_cancelled_after_a_second_quiet_look(self):
        runs = [run(1, "queued", 45), run(2, "in_progress", 30), run(3, "queued", 5)]
        jobs = {1: [job("queued")],
                # Its routing job finished on GitHub's runner; its gate job has waited for the pool since.
                2: [job("completed", "ubuntu-latest", 29, 28, "GitHub Actions 1"), job("queued")],
                3: [job("queued")]}
        code, cancelled, sleeps, _ = self.sweep(runs, jobs)
        self.assertEqual((code, cancelled, sleeps), (1, [1, 2], [watch.QUIET_RECHECK]))

    def test_a_busy_pool_cancels_nothing(self):
        runs = [run(1, "queued", 45), run(2, "in_progress", 30)]
        jobs = {1: [job("queued")], 2: [job("in_progress", started=10, runner="dyna-ci-1")]}
        self.assertEqual(self.sweep(runs, jobs)[:3], (0, [], []))

    def test_a_pool_that_finished_a_job_within_the_stall_is_alive(self):
        runs = [run(1, "queued", 45), run(2, "in_progress", 40)]
        jobs = {1: [job("queued")], 2: [job("completed", started=30, completed=10, runner="dyna-ci-1"), job("queued")]}
        self.assertEqual(self.sweep(runs, jobs)[:3], (0, [], []))

    def test_a_job_that_never_held_a_runner_is_no_sign_of_life(self):
        # A cancelled queued job has start and finish times but no runner.
        runs = [run(1, "in_progress", 45)]
        jobs = {1: [job("completed", started=25, completed=25), job("queued")]}
        self.assertEqual(self.sweep(runs, jobs)[:2], (1, [1]))

    def test_a_run_with_recent_progress_or_no_pool_job_is_left_alone(self):
        runs = [run(1, "in_progress", 45), run(2, "queued", 45)]
        jobs = {1: [job("completed", "ubuntu-latest", 12, 5, "GitHub Actions 2"), job("queued")],
                2: [job("queued", "ubuntu-latest")]}
        self.assertEqual(self.sweep(runs, jobs)[:3], (0, [], []))

    def test_the_watch_skips_its_own_run_and_reads_no_young_queued_run(self):
        runs = [run(999, "in_progress", 45), run(3, "queued", 5)]
        jobs = {999: [job("queued")]}
        code, cancelled, _, requests = self.sweep(runs, jobs)
        self.assertEqual((code, cancelled), (0, []))
        self.assertFalse([path for path in requests if "/jobs?" in path])


if __name__ == "__main__":
    unittest.main()
