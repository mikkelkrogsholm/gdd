from pathlib import Path
import runpy


def test_all_standard_cases():
    harness = runpy.run_path(
        str(Path(__file__).resolve().parents[1] / "conformance/run.py")
    )
    result = harness["run"]()
    failures = [r for r in result["cases"] if not r["matched"]]
    assert not failures, failures


def test_graphify_adapter_cases():
    harness = runpy.run_path(
        str(Path(__file__).resolve().parents[1] / "conformance/run_graphify.py")
    )
    result = harness["run"]()
    assert result["total"] == result["matched"] == 18, [
        r for r in result["cases"] if not r["matched"]
    ]
