# The official KDL test suite

`input/` and `expected_kdl/` are the KDL 2.0 test cases from the KDL
specification, copied unmodified from
[kdl-org/kdl](https://github.com/kdl-org/kdl) at commit
`89c1087d5e7f530de328f18b6a0fad54ca8ea227` (2026-08-31),
`tests/test_cases/`.

They are © the KDL contributors and licensed under the
[Creative Commons Attribution-ShareAlike 4.0 International License](https://creativecommons.org/licenses/by-sa/4.0/),
as `LICENSE.md` beside this file says. That license covers these files
only; they are test data, read by `tests/kdl_conformance.rs`, and no part
of the isolex compiler, which is Apache-2.0.

Every input must parse exactly when it has a file of the same name in
`expected_kdl/` (an input meant to fail ends in `_fail` and has none), and
then to the same document as that file, which is the input printed back
out in a canonical form: comments and empty child blocks gone, duplicate
properties down to the rightmost, properties sorted by name.

The files are stored byte for byte (`.gitattributes` marks the directory
`-text`): some cases are about CR LF and CR newlines.
