"""Build, verify and safely restore the original concert experiment Git tree.

The byte index is the trust input. Check its identity before using an independently
downloaded bundle; matching a supplied index proves integrity, not authorship.
"""

import argparse
import gzip
import hashlib
import io
import json
from pathlib import Path, PurePosixPath
import shutil
import subprocess
import tarfile
import tempfile


FORMAT = "concert-archive/1"


def sha256(data):
    return hashlib.sha256(data).hexdigest()


def safe_path(value):
    """Accept canonical relative POSIX paths only, including no backslashes."""
    if not isinstance(value, str) or not value or "\\" in value or "\x00" in value:
        raise ValueError("Invalid archive path")
    path = PurePosixPath(value)
    if not path.parts or path.is_absolute() or path.as_posix() != value or any(p in (".", "..") for p in path.parts):
        raise ValueError("Archive path must be canonical and relative: " + value)
    return path


def git(repository, *arguments):
    return subprocess.check_output(["git", "-C", str(repository), *arguments])


def build(repository, revision, archive, index, source_path="examples/concert-app"):
    """Read a committed Git tree; create a deterministic regular-file bundle."""
    archive, index = Path(archive), Path(index)
    if archive.exists() or archive.is_symlink() or index.exists() or index.is_symlink():
        raise ValueError("Archive and index outputs must not already exist")
    commit = git(repository, "rev-parse", revision + "^{commit}").decode().strip()
    tree = git(repository, "rev-parse", commit + "^{tree}").decode().strip()
    safe_path(source_path)
    entries = {}
    for entry in git(repository, "ls-tree", "-rz", commit, "--", source_path).split(b"\x00"):
        if not entry:
            continue
        identity, raw_path = entry.split(b"\t", 1)
        mode, kind, blob = identity.decode().split()
        path = raw_path.decode("utf-8")
        safe_path(path)
        if kind != "blob" or mode not in ("100644", "100755"):
            raise ValueError("Only regular Git files are supported: " + path)
        entries[path] = {"git_mode": mode, "git_blob": blob}
    if not entries:
        raise ValueError("The selected Git subtree is empty")
    files = {}
    with tarfile.open(fileobj=io.BytesIO(git(repository, "archive", "--format=tar", commit, source_path)), mode="r:") as original:
        for member in original:
            if member.isdir():
                continue
            if not member.isfile() or member.name not in entries:
                raise ValueError("Unexpected Git archive member: " + member.name)
            data = original.extractfile(member).read()
            blob = hashlib.sha1(b"blob " + str(len(data)).encode() + b"\x00" + data).hexdigest()
            if blob != entries[member.name]["git_blob"]:
                raise ValueError("Git blob bytes did not match: " + member.name)
            files[member.name] = data
    if set(files) != set(entries):
        raise ValueError("Git archive did not recover the complete tree")
    archive.parent.mkdir(parents=True, exist_ok=True)
    index.parent.mkdir(parents=True, exist_ok=True)
    with archive.open("xb") as output:
        with gzip.GzipFile(fileobj=output, mode="wb", filename="", mtime=0, compresslevel=9) as compressed:
            with tarfile.open(fileobj=compressed, mode="w", format=tarfile.PAX_FORMAT) as bundle:
                for path, data in sorted(files.items()):
                    member = tarfile.TarInfo(path)
                    member.size = len(data)
                    member.mode = int(entries[path]["git_mode"][-3:], 8)
                    member.mtime = member.uid = member.gid = 0
                    member.uname = member.gname = ""
                    bundle.addfile(member, io.BytesIO(data))
    document = {
        "format": FORMAT,
        "source": {"git_commit": commit, "git_tree": tree, "git_path": source_path},
        "archive": {"path": archive.name, "sha256": sha256(archive.read_bytes()), "size": archive.stat().st_size},
        "files": [{"original_path": path, "retrieval_path": path, "sha256": sha256(data),
                   "size": len(data), **entries[path]} for path, data in sorted(files.items())],
    }
    with index.open("x", encoding="utf-8") as output:
        json.dump(document, output, indent=2)
        output.write("\n")
    check(archive, index)
    return document


def verified_files(archive, index):
    """Verify the compressed bytes and every indexed file before returning data."""
    document = json.loads(Path(index).read_text(encoding="utf-8"))
    if document.get("format") != FORMAT:
        raise ValueError("Unsupported archive index format")
    raw = Path(archive).read_bytes()
    if sha256(raw) != document["archive"]["sha256"] or len(raw) != document["archive"]["size"]:
        raise ValueError("Compressed archive digest or size mismatch")
    expected = {}
    for entry in document["files"]:
        path = str(safe_path(entry["original_path"]))
        retrieval = str(safe_path(entry["retrieval_path"]))
        if path != retrieval or path in expected or entry["git_mode"] not in ("100644", "100755"):
            raise ValueError("Duplicate or unsupported index entry: " + path)
        expected[path] = entry
    for path in expected:
        if any(str(parent) in expected for parent in PurePosixPath(path).parents if str(parent) != "."):
            raise ValueError("Index contains a file/directory conflict: " + path)
    files = {}
    try:
        with tarfile.open(fileobj=io.BytesIO(raw), mode="r:gz") as bundle:
            for member in bundle:
                path = str(safe_path(member.name))
                if not member.isfile() or member.linkname or path not in expected or path in files:
                    raise ValueError("Unexpected, linked or duplicate archive member: " + path)
                entry = expected[path]
                if member.size != entry["size"] or member.mode != int(entry["git_mode"][-3:], 8):
                    raise ValueError("Archive member metadata mismatch: " + path)
                data = bundle.extractfile(member).read()
                blob = hashlib.sha1(b"blob " + str(len(data)).encode() + b"\x00" + data).hexdigest()
                if sha256(data) != entry["sha256"] or blob != entry["git_blob"]:
                    raise ValueError("Recovered file digest mismatch: " + path)
                files[path] = data
    except (tarfile.TarError, EOFError, OSError) as error:
        raise ValueError("Unreadable archive: " + str(error)) from error
    if set(files) != set(expected):
        raise ValueError("Archive members do not match the complete index")
    return document, files


def check(archive, index):
    document, files = verified_files(archive, index)
    return {"format": FORMAT, "verified": True, "git_commit": document["source"]["git_commit"],
            "archive_sha256": document["archive"]["sha256"], "files": len(files),
            "recovered_bytes": sum(map(len, files.values()))}


def restore(archive, index, output):
    """Restore validated regular files into a new directory, never extract links."""
    document, files = verified_files(archive, index)
    output = Path(output).absolute()
    if output.exists() or output.is_symlink():
        raise ValueError("Restore output must not already exist")
    if not output.parent.is_dir():
        raise ValueError("Restore output parent must already exist")
    if any(parent.is_symlink() for parent in (output.parent, *output.parent.parents)):
        raise ValueError("Restore output ancestors must not be symlinks")
    entries = {entry["original_path"]: entry for entry in document["files"]}
    # Build privately before reserving the final name. mkdir refuses a path that
    # appeared during verification; no existing destination is ever replaced.
    with tempfile.TemporaryDirectory(prefix=".concert-restore-", dir=output.parent) as temporary:
        staging = Path(temporary)
        for name, data in files.items():
            target = staging.joinpath(*PurePosixPath(name).parts)
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_bytes(data)
            target.chmod(int(entries[name]["git_mode"][-3:], 8))
            if sha256(target.read_bytes()) != entries[name]["sha256"]:
                raise ValueError("Restored bytes did not match: " + name)
        output.mkdir()
        try:
            for child in staging.iterdir():
                shutil.move(str(child), output / child.name)
        except BaseException:
            shutil.rmtree(output)
            raise
    return {**check(archive, index), "output": str(output)}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    actions = parser.add_subparsers(dest="action", required=True)
    for action in ("build", "check", "restore"):
        command = actions.add_parser(action)
        command.add_argument("--archive", type=Path, required=True)
        command.add_argument("--index", type=Path, required=True)
        if action == "build":
            command.add_argument("--repository", type=Path, required=True)
            command.add_argument("--revision", required=True)
            command.add_argument("--source-path", default="examples/concert-app")
        if action == "restore":
            command.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    try:
        if args.action == "build":
            build(args.repository, args.revision, args.archive, args.index, args.source_path)
            result = check(args.archive, args.index)
        elif args.action == "restore":
            result = restore(args.archive, args.index, args.output)
        else:
            result = check(args.archive, args.index)
    except (ValueError, KeyError, TypeError, json.JSONDecodeError, FileExistsError) as error:
        parser.exit(1, "Archive refused: " + str(error) + "\n")
    print(json.dumps(result, indent=2))


if __name__ == "__main__":
    main()
