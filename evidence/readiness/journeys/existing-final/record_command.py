"""Execute real trial commands and preserve their unedited output."""
import datetime
import json
import pathlib
import subprocess
import sys

root = pathlib.Path(__file__).resolve().parent
evidence = root / "evidence"
evidence.mkdir(exist_ok=True)
label, *command = sys.argv[1:]
result = subprocess.run(command, cwd=root, capture_output=True, text=True)
(evidence / f"{label}.stdout.txt").write_text(result.stdout)
(evidence / f"{label}.stderr.txt").write_text(result.stderr)
receipt = {
    "command": command,
    "cwd": str(root),
    "timestamp": datetime.datetime.now(datetime.timezone.utc).isoformat(),
    "exit_code": result.returncode,
    "stdout": f"evidence/{label}.stdout.txt",
    "stderr": f"evidence/{label}.stderr.txt",
}
(evidence / f"{label}.command.json").write_text(json.dumps(receipt, indent=2) + "\n")
print(json.dumps(receipt))
print(result.stdout, end="")
print(result.stderr, end="", file=sys.stderr)
sys.exit(result.returncode)
