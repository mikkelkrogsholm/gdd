BEGIN TRANSACTION;
CREATE TABLE concerts (
                id TEXT PRIMARY KEY,
                title TEXT NOT NULL,
                date TEXT NOT NULL,
                venue TEXT NOT NULL,
                capacity INTEGER NOT NULL CHECK (capacity >= 0)
            );
INSERT INTO "concerts" VALUES('midnight','Midnight Sessions','2026-11-14','Harbor Hall',8);
INSERT INTO "concerts" VALUES('northern','Northern Lights','2026-11-21','North Stage',3);
INSERT INTO "concerts" VALUES('matinee','Sunday Matinee','2026-11-29','Studio 03',2);
CREATE TABLE reservations (
                id TEXT PRIMARY KEY,
                concert_id TEXT NOT NULL REFERENCES concerts(id),
                principal_id TEXT NOT NULL,
                request_key TEXT NOT NULL,
                state TEXT NOT NULL CHECK (state IN ('reserved', 'cancelled')),
                UNIQUE (principal_id, request_key)
            );
INSERT INTO "reservations" VALUES('22295aa924aa4f62bcf528cfc046cad7','midnight','alice','73ce535d-f1dd-4556-a2ea-2813952e80f6','cancelled');
INSERT INTO "reservations" VALUES('2d3f2bc9fed44168b9a47eb2e163061e','northern','alice','9707c2a7-aa9f-4bcd-a05c-6d80f579172e','cancelled');
INSERT INTO "reservations" VALUES('6122fbd2484a4c55a5c84026a1df2f24','matinee','alice','186a8c7e-4f59-4d5a-a608-36a8ef142def','reserved');
INSERT INTO "reservations" VALUES('800c993865e34b1babb1eda9ccbf0610','northern','alice','1a40e8a2-210f-4d69-ad03-14520be4559d','cancelled');
INSERT INTO "reservations" VALUES('8aa5326178e74174b78e6cc562c28a27','midnight','bob','dc01a515-759f-488d-8cd5-59a10d2ef92e','reserved');
INSERT INTO "reservations" VALUES('fc06858a95244a2196aa8315e2ce8381','northern','bob','c61e56ea-6bf3-4413-a088-4c027c142c39','reserved');
INSERT INTO "reservations" VALUES('d94391d22d3d426ab2efced583773fb0','midnight','alice','c6116c07-c0f5-489a-a257-231942a272fd','reserved');
COMMIT;
