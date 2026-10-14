# Completed authoring check

Run from the project root with exit code 0:

```sh
uv run --no-project --with jsonschema==4.24.0 --with PyYAML==6.0.3 python evidence/authoring-check.source.txt > evidence/authoring-check-output.txt 2>&1
```

The exact script text is archived in `authoring-check.source.txt`. To reproduce from the project root, the same text can be run with:

```sh
uv run --no-project --with jsonschema==4.24.0 --with PyYAML==6.0.3 python evidence/authoring-check.source.txt
```

The script is a bounded check of the included standard artifacts. It is not the GDD validator specified by the standard, and it does not assess full readiness, authorization, or application behavior. It writes only authoring evidence.
