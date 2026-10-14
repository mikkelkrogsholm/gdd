# Conformance-cases

`cases.yaml` beskriver 58 positive og negative forventninger til standardens profiler. `diagnostics.csv` indeholder stabile diagnosecodes. Case-ID'er er identiteter for testsuiten, ikke noder i eksempelmodellen.

En case vælger baseartifact og profile, angiver eventuel scope og ændringer samt forventet dimension/resultat. `patches` bruger [RFC 6902 JSON Patch](https://www.rfc-editor.org/rfc/rfc6902). [run.py](run.py) opretter konkrete lokale fixtures for condition-cases og udfører validatoren i midlertidige bundles. Originaleksemplerne ændres ikke.

`verification` bevarer kategoriseringen fra standardfasen. Den historiske authoring-kontrol er dokumenteret i [standardreviewet](../evidence/standard-review.md). Den aktuelle validatorudførelse findes separat i [validator-conformance.json](../evidence/validator-conformance.json). `expected` bruges kun til assertionen efter faktisk kørsel, aldrig som resultat i validatoren.

```sh
uv run python conformance/run.py --output evidence/validator-conformance.json
uv run pytest -q
```

Harnesset kontrollerer forventet dimension/resultat og krævet diagnose. Det bevarer alle rapportchecks, også når FAIL og UNKNOWN optræder sammen. Parallel-cases vurderer den særskilte planoperation; den hævder ikke action-readiness. Import-casene kontrollerer påviseligt datatab og eksplicit UNKNOWN ved uunderstøttet profile/version. Derfor er 58 matchende forventninger ikke en certificering af alle mulige standardkrav.

Synthetic receipts i casepatches er testinputs til repræsentationsschemaet. De er ikke registrerede godkendelser, observerede kodefiler eller udført applikationsevidens.

[graphify-cases.yaml](graphify-cases.yaml) tilføjer 18 adaptercases. Kør `uv run python conformance/run_graphify.py`; de oprindelige 58 forventninger er bevaret. Importconditions i den oprindelige runner materialiseres nu som egentlige Graphify-JSON-inputs. Den faktisk producerede AST-/eksportfixture og dens hashes findes under [examples/graphify](../examples/graphify/README.md). Pytest kontrollerer også tamper, CLI, typed IDs og unaccepted proposals.

[visualization-cases.yaml](visualization-cases.yaml) tilføjer 16 artifactcases for `gdd-viewer-html/0.1`. Kør `uv run python conformance/run_visualization.py`. [run_browser.cjs](run_browser.cjs) tester desuden faktiske offline browserhandlinger, grafidentitet, hostile tekst, numerisk præcision, foldning, download og responsivitet. Se [VIEWER.md](../VIEWER.md) for runtime og verifikationsscope.
