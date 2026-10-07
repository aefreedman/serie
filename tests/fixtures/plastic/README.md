# Plastic backend fixtures

Derived from read-only outputs of cm 11.0.16.10371 in a dedicated test repository. Repository names, organization/server identities, owner email, and test-label name have been replaced consistently with example values. These are sanitized fixtures, not byte-for-byte live captures. Numeric changeset/object/revision IDs and graph topology are preserved.

Commands run in the test workspace:

```text
cm find changeset "order by changesetid desc limit 100" --xml --encoding=utf-8
cm find merge "limit 100" --xml --encoding=utf-8
cm find branch "limit 100" --xml --encoding=utf-8
cm find label "limit 100" --xml --encoding=utf-8
cm status --header --xml --encoding=utf-8
cm log cs:16 --xml --encoding=utf-8 --repositorypaths
```

The history fixtures contain 40 changesets, 9 ordinary integrations, and 48 branches. Label fixtures include both an empty query and a live-derived MARKER record targeting changeset 16. Primary parent 17 -> 13 is distinct from integration 14 -> 17. Root 0 has PARENT=-1. Object and revision IDs are not changeset numbers.

Status and find use different example server aliases (`1234567890123@cloud` and `example-org@unity`), preserving the observed alias-mapping case. Production retains original status identity and discloses mapping to the uniform identity of an explicitly repository-scoped query. Branch CHANGESET annotations are not inferred from branch hierarchy.

Nonordinary integration types, malformed/boundary XML, and label escaping have additional synthetic coverage. Run `cargo test --test plastic_backend --locked` for backend parser and bounded-process tests, or `cargo test --locked` for all application tests. No tests require access to the original repository.
