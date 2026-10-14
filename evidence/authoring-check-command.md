# Udført authoring-kontrol

Kørt fra projektroden med exit code 0:

```sh
uv run --no-project --with jsonschema==4.24.0 --with PyYAML==6.0.3 python evidence/authoring-check.source.txt > evidence/authoring-check-output.txt 2>&1
```

Den præcise scripttekst er arkiveret i `authoring-check.source.txt`. Til reproduktion fra projektroden kan samme tekst køres med:

```sh
uv run --no-project --with jsonschema==4.24.0 --with PyYAML==6.0.3 python evidence/authoring-check.source.txt
```

Scriptet er en afgrænset kontrol af de vedlagte standardartefakter. Det er ikke den GDD-validator, som standarden specificerer, og det vurderer ikke fuld readiness, autorisation eller applikationsadfærd. Det skriver kun authoring-evidens.
