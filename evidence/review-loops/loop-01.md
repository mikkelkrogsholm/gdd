# Loop 1 — almindeligt og adversarial review

Status: verificeret efter rettelser. En frisk baseline gav 172 beståede pytest-tests. Fire parallelle read-only reviews dækkede model/readiness/planlægning, autorisation/records, Graphify og viewer; lead gennemgik CLI, pakning og krydskontrakter. Alle implementeringsændringer i checkout foretages af lead. Workers har kun forberedt reviewbare diffs i isolerede tempkopier.

Der er registreret 19 fundgrupper i [loops.json](loops.json). De dækker forkert grønt resultat, forkert negativt resultat, ukontrollerede inputfejl, informationsfidelitet og konkret navigation. Hvert fund kræver en reproduktion/negativ regression samt relevante positive kontroller før lukning. Spekulativ stor-graf-performance, nye produktfeatures og fuld eksternt-formatcertificering er ikke føjet til opgaven.

Lead har selv reproduceret duplicate `--scope` via kontekstschemaet, initialt manifest-symlinkloop via offentlig API og no-replace-operationer på lokale filer, tomme mapper og symlinks. Workerregressioner køres først mod den gamle kode, når det kan isoleres, og derefter mod den integrerede kode. Browserchecks bruger `file://`, offline Chromium og friske fixtures; output gemmes separat fra foregående fasers historik.

Foreløbig evidens: [baseline](baseline-tests.txt), [CLI før rettelse](loop-01-cli-before.txt), [manifest-symlink før rettelse](loop-01-manifest-symlink-before.json), [viewer før rettelse](loop-01-viewer-before.txt). Disse fejloutputs er bevidst bevaret som reproduktioner og er ikke slutstatus.

Eksklusiv, atomisk bundlepublikation bruger platformens no-replace primitive. Lokal macOS SDK `sys/stdio.h` angiver `RENAME_EXCL=0x00000004`; [Apple beskriver exclusive rename](https://developer.apple.com/documentation/foundation/urlresourcevalues/volumesupportsexclusiverenaming). Linuxvarianten følger [renameat2/RENAME_NOREPLACE](https://man7.org/linux/man-pages/man2/rename.2.html). Der er ingen fallback til en operation, som erstatter eksisterende output.

Integreret resultat: 310 pytest-tests består; 58/58 standardcases, 18/18 Graphify-cases og 16/16 renderer-cases matcher. 20/20 offline Chromium-checks består uden script-/konsolfejl eller eksterne requests. De 19 fundgrupper er rettet med regressioner i `tests/test_review_*.py` og de nye browsercases. En frisk loop 2 er startet; det er endnu ikke dokumenteret, at der ikke findes flere fund.
