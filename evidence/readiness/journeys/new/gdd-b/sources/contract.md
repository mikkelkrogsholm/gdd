# Accepted parcel contract B

`python -m parcel.cli WEIGHT ZONE` accepts a positive integer weight in grams
and zone `domestic` or `remote`. It prints one integer cent amount followed by a
newline on stdout, then exits 0. Weight is a decimal integer string; whitespace,
signs and leading zeros are accepted when Python int parses a positive value.
Invalid/missing arguments exit 2 with an English error on stderr and no quote.
No upper weight limit is imposed. Quotes do not mutate state.

Begun kilograms are (weight_grams + 999) // 1000. Domestic quotes multiply this
by 100 cents; remote quotes multiply by 150 cents. Remote quotes add a flat 50 cents handling fee once per quote; domestic quotes add no fee.
The pricing API rejects bool, non-integer or nonpositive weight with ValueError,
and an unknown zone with ValueError. The tariff API rejects an unknown zone.

CLI owns argument parsing and printing and calls pricing.quote. Pricing owns
validation and weight rounding and calls tariffs.rate. Tariffs owns zone rates.
CLI must not directly import tariffs. These three components need no plugins,
network, persistence, auth, shared mutable state, or background work.
