"""Actual application checks through the public workflow API."""

from pathlib import Path
import runpy


def test_archived_native_results_replay_red_green_and_current_acceptance(tmp_path):
    script = Path(__file__).resolve().parents[1] / "examples/agent-native/replay.py"
    demo = runpy.run_path(str(script))
    result = demo["replay"](tmp_path / "demo")
    assert result["baseline_exit"] == 1
    assert result["final_exit"] == 0
    assert result["acceptance_current"] is True
    assert result["events"] == 10
