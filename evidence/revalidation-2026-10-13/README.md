# Genkontrol af GDD-validator 0.2.0

Brugerens gentagne validatoropgave er afstemt med den allerede afsluttede implementation. Alle 60 filer registreret i [leverancemanifestet](../validator-assets.json) har uændrede bytehashes. Eksisterende kode og leverancer er bevaret; denne omgang tilføjer frisk kontroloutput.

| Kontrol | Faktisk resultat | Output |
| --- | --- | --- |
| Hele pytest-suiten | 74 beståede tests | [tests.txt](tests.txt) |
| Standardens conformance-cases | 58/58 forventninger matcher | [conformance.json](conformance.json) |
| CLI, action.reserve | readiness PASS, exit 0 | [backend.json](backend.json) |
| CLI, implementation uden udført evidens | UNKNOWN, exit 2 | [implementation.json](implementation.json) |
| CLI, implementation uden betroet mandat | authorization UNRESOLVED, exit 2 | [authorization.json](authorization.json) |

Kommandoer fra projektroden:

```sh
uv run pytest -q
uv run python conformance/run.py --output evidence/revalidation-2026-10-13/conformance.json
uv run gdd validate examples/concert.gdd.yaml --profile readiness --scope action.reserve
uv run gdd validate examples/concert.gdd.yaml --profile records
uv run gdd validate examples/concert.gdd.yaml --profile records --activity implementation --scope action.reserve
```

To read-only underagenter har parallelt genkontrolleret de tidligere konkrete reviewfund. Modelreviewet bekræftede edge-selectors, metaschema før traversal, inert const/$ref og adskillelsen mellem records og modelrevision. Protokolreviewet bekræftede context/readiness, parentbørn, beskyttelse mod urelateret supersession, parallelplanens deklarationer og community-scope. Ingen reproducerbar restfejl blev fundet inden for den afgrænsede genkontrol. Lead har verificeret den integrerede testkørsel og CLI-resultaterne.

Den tidligere build-/installationskontrol er bevaret i [validatorreviewet](../validator-review.md). Pakke- og kodehashes er uændrede. Der er ikke udført en ny global installation eller publicering.

[VALIDATOR.md](../../VALIDATOR.md) beskriver profiler og begrænsninger. Matchende conformance-forventninger omfatter også forventede FAIL/UNKNOWN-resultater; de er ikke applikationsevidens eller fuld certificering af alle mulige standardkrav. Produktions-Graphify-import og rendering er fortsat UNKNOWN.
