"""Exercise recovered bytes and reject corrupt or unsafe evidence bundles."""

import hashlib
import importlib.util
import io
import json
from pathlib import Path
import subprocess
import tarfile

import pytest


SCRIPT = Path(__file__).resolve().parents[1] / "scripts/concert_archive.py"
spec = importlib.util.spec_from_file_location("concert_archive", SCRIPT)
archive = importlib.util.module_from_spec(spec)
spec.loader.exec_module(archive)


@pytest.fixture
def bundle(tmp_path):
    repository = tmp_path / "source"
    repository.mkdir()
    subprocess.run(["git", "init", "-q", str(repository)], check=True)
    source = repository / "examples/concert-app"
    source.mkdir(parents=True)
    (source / "data.bin").write_bytes(b"original\x00\xff\n")
    (source / "run.sh").write_bytes(b"#!/bin/sh\nexit 0\n")
    (source / "run.sh").chmod(0o755)
    subprocess.run(["git", "-C", str(repository), "add", "."], check=True)
    subprocess.run(["git", "-C", str(repository), "-c", "user.name=Archive Test",
                    "-c", "user.email=archive@example.invalid", "commit", "-qm", "fixture"], check=True)
    compressed, index = tmp_path / "bundle.tar.gz", tmp_path / "index.json"
    archive.build(repository, "HEAD", compressed, index)
    return repository, compressed, index


def test_deterministic_build_and_byte_identical_roundtrip(bundle, tmp_path):
    repository, compressed, index = bundle
    other = tmp_path / "same.tar.gz"
    archive.build(repository, "HEAD", other, tmp_path / "same.json")
    assert other.read_bytes() == compressed.read_bytes()
    output = tmp_path / "recovered"
    receipt = archive.restore(compressed, index, output)
    assert receipt["verified"] and receipt["files"] == 2
    for entry in json.loads(index.read_text())["files"]:
        recovered = output / entry["original_path"]
        assert recovered.read_bytes() == (repository / entry["original_path"]).read_bytes()
        assert recovered.stat().st_mode & 0o777 == int(entry["git_mode"][-3:], 8)


def test_compressed_corruption_never_creates_output(bundle, tmp_path):
    _, compressed, index = bundle
    raw = bytearray(compressed.read_bytes())
    raw[len(raw) // 2] ^= 1
    compressed.write_bytes(raw)
    output = tmp_path / "recovered"
    with pytest.raises(ValueError, match="digest or size"):
        archive.restore(compressed, index, output)
    assert not output.exists()


def test_indexed_file_corruption_is_rejected(bundle, tmp_path):
    _, compressed, index = bundle
    document = json.loads(index.read_text())
    document["files"][0]["sha256"] = "0" * 64
    index.write_text(json.dumps(document))
    with pytest.raises(ValueError, match="Recovered file digest"):
        archive.restore(compressed, index, tmp_path / "recovered")
    assert not (tmp_path / "recovered").exists()


def test_existing_output_and_symlinks_are_refused_without_changes(bundle, tmp_path):
    _, compressed, index = bundle
    existing = tmp_path / "existing"
    existing.mkdir()
    sentinel = existing / "sentinel"
    sentinel.write_bytes(b"keep")
    symlink = tmp_path / "link"
    symlink.symlink_to(existing, target_is_directory=True)
    broken = tmp_path / "broken"
    broken.symlink_to(tmp_path / "missing")
    for output in (existing, sentinel, symlink, broken):
        with pytest.raises(ValueError, match="must not already exist"):
            archive.restore(compressed, index, output)
    with pytest.raises(ValueError, match="ancestors must not be symlinks"):
        archive.restore(compressed, index, symlink / "recovered")
    assert sentinel.read_bytes() == b"keep" and not (existing / "recovered").exists()


@pytest.mark.parametrize("unsafe", ["../outside", "/outside", "a/../outside", "a\\outside", "a//b", "./file", "."])
def test_unsafe_index_paths_are_refused(bundle, tmp_path, unsafe):
    _, compressed, index = bundle
    document = json.loads(index.read_text())
    document["files"][0]["original_path"] = unsafe
    document["files"][0]["retrieval_path"] = unsafe
    index.write_text(json.dumps(document))
    with pytest.raises(ValueError, match="path"):
        archive.restore(compressed, index, tmp_path / "recovered")
    assert not (tmp_path / "recovered").exists()


@pytest.mark.parametrize("kind", ["symlink", "hardlink", "traversal", "duplicate", "extra", "corrupt"])
def test_malicious_members_rejected_even_with_matching_compressed_digest(bundle, tmp_path, kind):
    _, compressed, index = bundle
    with tarfile.open(compressed, "r:gz") as original:
        members = [(entry, original.extractfile(entry).read()) for entry in original]
    with tarfile.open(compressed, "w:gz") as altered:
        for position, (entry, data) in enumerate(members):
            if position == 0 and kind in ("symlink", "hardlink"):
                entry.type = tarfile.SYMTYPE if kind == "symlink" else tarfile.LNKTYPE
                entry.linkname = "../../outside"
                entry.size = 0
            if position == 0 and kind == "traversal":
                entry.name = "../outside"
            if position == 0 and kind == "corrupt":
                data = b"X" + data[1:]
            altered.addfile(entry, io.BytesIO(data))
            if position == 0 and kind == "duplicate":
                altered.addfile(entry, io.BytesIO(data))
        if kind == "extra":
            altered.addfile(tarfile.TarInfo("extra"), io.BytesIO(b""))
    document = json.loads(index.read_text())
    document["archive"]["sha256"] = hashlib.sha256(compressed.read_bytes()).hexdigest()
    document["archive"]["size"] = compressed.stat().st_size
    index.write_text(json.dumps(document))
    with pytest.raises(ValueError):
        archive.restore(compressed, index, tmp_path / "recovered")
    assert not (tmp_path / "recovered").exists()


def test_repository_bundle_matches_index():
    root = SCRIPT.parents[1]
    receipt = archive.check(root / "archive/experiments/concert-7e27757.tar.gz",
                            root / "archive/experiments/concert-7e27757.index.json")
    assert receipt["git_commit"] == "7e27757c34b6fa57ecc69660e263c99e1420649b"
    assert receipt["files"] == 197
