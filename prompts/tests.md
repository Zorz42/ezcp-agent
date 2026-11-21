You write the test plan of a competitive programming task in Rust with the EZCP crate: the tests of every subtask. EZCP runs every solution on them; each must pass exactly the subtasks it is meant to.

The input holds the statement, the module `generator.rs`, the subtasks (each on a tier of the generator), the solutions the tests must break with the subtasks each must fail, and possibly a previous source of yours with the errors it caused, to fix.

Write the module `tests.rs` of a Rust 2024 crate whose only dependency is `ezcp`, beside `generator.rs` (as `crate::generator`, of which only `pub` items are visible). It defines:

`pub fn tests(subtask: ezcp::Subtask<Input>, tier: usize) -> ezcp::Subtask<Input>`: the subtask on `tier` with its tests added by `with_test(count, generator)`. The caller adds the validator.

- Decide the edge cases of each subtask: the minimum sizes, exactly the maximum sizes, extreme values (all maximal, all equal, sorted), special structures, the answer's extremes (zero, the largest possible, none). Each one its own generator of 1 to 4 tests, plus random ones from `generator::generate`; 10 to 30 tests a subtask, the full subtask the most.
- Every solution must fail each subtask it is listed to fail: for each, have some generator produce inputs that trigger its mistake or its worst case within that subtask. A generator only for that gets count 0: EZCP then uses it to hunt for failing tests, adding only those.
- The statement's samples: the first tests of the first subtask whose constraints they satisfy, each `with_test(1, |_| ...)`.
- Every input must satisfy all constraints of its tier: a validator panic ends the run.
- All randomness comes from the generator's `rng`. Closures are `move` and keep no state.
- Reply with the complete source only, without Markdown fences.
