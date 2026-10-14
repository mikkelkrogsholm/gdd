# GDD — indgang for agenter

Læsevejledning til 0.2-udkastet. Normative krav står i [SPEC.md](SPEC.md) og de henviste kapitler.

1. Læs formatversion, modelmanifest og det betroede mandat. Antag ikke en godkendelsesmode.
2. Registrér input som kilder; vedligehold wiki/OKF. Skeln mellem udsagn, observationer, fortolkninger og accepterede krav.
3. Brug eksisterende Graphify-data som observationer med proveniens. Foreslå intention eksplicit.
4. Iterér grafen med stabile ID'er, faste kontrakter og fri pseudokode. Registrér væsentlige spørgsmål.
5. Saml opgavens fulde kontrakter, ancestors, relevante data, maskiner, globale/lokale regler, scenarier og designreferencer. Afgræns skriveområdet separat.
6. Vurder gyldighed, strukturel parathed og autorisation hver for sig. Et schema-PASS er ikke et adfærds-PASS.
7. Implementér kun inden for opgaven og mandatet. Koordiner fælles kontrakter og modelændringer ved parallelitet.
8. Forbind kodebindings, checkplaner og udført evidens med stabile ID'er og præcise revisioner. En undtagelse ændrer ikke FAIL/UNKNOWN til PASS.
9. Vis afledte resultater for mennesket, og omsæt feedback gennem agenten.
10. Lever scope, ændrede ID'er, beslutninger, revisioner, resultater og åbne spørgsmål. Svæk ikke krav skjult for at få grønne checks.

Ved eksisterende kodegrafer: brug [Graphify-adapteren](GRAPHIFY.md) til et tabsfrit observationsbundle. Vælg derefter fortolkning og stabile model-ID'er i en eksplicit mapping; propose laver en uaccepteret skitse, som skal gennem projektets normale modelændringsproces. Producentclaims er ikke autorisation eller implementeringsevidens.

Pakken indeholder nu [GDD-validatoren](VALIDATOR.md). Kør `uv run gdd validate <manifest> --profile readiness --scope <node-id>` for strukturel parathed. Model, readiness, authorization og implementation rapporteres særskilt efter [valideringskontrakten](standard/VALIDATION.md). Opfind ikke værktøjsresultater, og behandl ikke planlagte tests som udførte.

## Agent-workflow

Brug [WORKFLOW.md](WORKFLOW.md) til konkrete udviklingsplaner og run-events samt [lead](agents/gdd-lead.md)/[worker](agents/gdd-worker.md)-instruktionerne i eksisterende agenter. Grafens kontekstclosure er læsegrundlag; planens scope og eksplicitte filområder bestemmer arbejdet. Workflow 0.1 er et værktøjslag oven på GDD 0.2 og giver ingen selvstændig autorisation eller evidens.
