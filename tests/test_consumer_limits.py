"""Consumer resource limits are explicit and independent of Python recursion."""

import json

import pytest

from gdd_validator.loading import (
    MAX_CONTAINER_DEPTH,
    InputError,
    parse,
    parse_json,
    resolve_local_path,
)


@pytest.mark.parametrize("loader", [parse_json, parse])
@pytest.mark.parametrize("container", ["array", "mapping"])
def test_json_container_depth_boundary(loader, container):
    opening, closing = ("[", "]") if container == "array" else ('{"value":', "}")
    raw = opening * MAX_CONTAINER_DEPTH + "0" + closing * MAX_CONTAINER_DEPTH
    assert loader(raw) is not None
    with pytest.raises(InputError, match="nesting limit") as error:
        loader(opening + raw + closing)
    assert error.value.code == "FORMAT_UNSUPPORTED"


@pytest.mark.parametrize("container", ["sequence", "mapping", "indentless"])
def test_yaml_container_depth_boundary(container):
    def document(depth):
        if container == "sequence":
            return "\n".join("  " * i + "-" for i in range(depth)) + " 0\n"
        if container == "mapping":
            return "\n".join("  " * i + "value:" for i in range(depth)) + " 0\n"
        return "values:\n" + "\n".join("  " * i + "-" for i in range(depth - 1)) + " 0\n"

    assert parse(document(MAX_CONTAINER_DEPTH)) is not None
    with pytest.raises(InputError, match="nesting limit") as error:
        parse(document(MAX_CONTAINER_DEPTH + 1))
    assert error.value.code == "FORMAT_UNSUPPORTED"


def test_brackets_and_escaped_quotes_inside_strings_do_not_count():
    value = '[{"' * (MAX_CONTAINER_DEPTH + 1) + '\\"' + "]}" * MAX_CONTAINER_DEPTH
    for loader in (parse_json, parse):
        assert loader(json.dumps({value: value})) == {value: value}
    assert parse("value: '" + "[" * 1000 + "'") == {"value": "[" * 1000}
    assert parse("value: |\n  " + "[" * 1000 + "\n") == {"value": "[" * 1000 + "\n"}


def test_resolver_preserves_missing_paths_and_follows_valid_links(tmp_path):
    assert resolve_local_path(tmp_path / "missing" / "file") == tmp_path / "missing" / "file"
    alias = tmp_path / "alias"
    alias.symlink_to(tmp_path, target_is_directory=True)
    assert resolve_local_path(alias / "missing") == tmp_path / "missing"


@pytest.mark.parametrize("mutual", [False, True])
def test_resolver_rejects_loops_in_existing_prefixes(tmp_path, mutual):
    first, second = tmp_path / "first", tmp_path / "second"
    first.symlink_to(second.name if mutual else first.name)
    if mutual:
        second.symlink_to(first.name)
    for path in (first, first / "missing"):
        with pytest.raises(InputError) as error:
            resolve_local_path(path)
        assert error.value.code == "ASSET_PATH_INVALID"


@pytest.mark.parametrize("entry", ["verify", "import", "manifest"])
def test_graphify_loop_is_known_invalid_but_missing_input_is_unknown(tmp_path, entry):
    from gdd_validator.graphify_io import import_graphify, load_observations, verify_graphify
    from gdd_validator.report import exit_code

    def report(path):
        if entry == "verify":
            return verify_graphify(path)
        if entry == "import":
            return import_graphify(path, tmp_path / "output", profile="node-link")
        return load_observations(path)[2]

    assert exit_code(report(tmp_path / "missing")) == 2
    loop = tmp_path / "loop"
    loop.symlink_to(loop.name)
    result = report(loop)
    assert exit_code(result) == 1
    assert result["checks"][0]["diagnostics"][0]["code"] == "ASSET_PATH_INVALID"
    assert not (tmp_path / "output").exists()
