"""Include the portable skill in regular and editable wheels, excluding local Cargo output."""

from pathlib import Path

from hatchling.builders.hooks.plugin.interface import BuildHookInterface


class SkillWheelHook(BuildHookInterface):
    def initialize(self, version, build_data):
        skill = Path(self.root) / "skills/gdd"
        scripts = skill / "scripts"
        # Hatch bypasses exclusion rules for a whole force-included directory.
        # Keep recursive inclusion for each sibling of the local target folder.
        paths = [path for path in skill.iterdir() if path != scripts]
        paths.extend(path for path in scripts.iterdir() if path.name != "target")
        for path in paths:
            if path.name != "__pycache__" and path.suffix not in {".pyc", ".pyo"}:
                destination = Path("gdd_validator/skill") / path.relative_to(skill)
                build_data["force_include"][str(path)] = destination.as_posix()
