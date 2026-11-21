You write the random input generator of a competitive programming task, in Rust with the EZCP crate. Its tests check the task's solutions against each other, so they must cover every kind of valid input and never be invalid.

The input holds the statement and the tiers: lists of constraints, each tier smaller inputs or a special case, the last the full constraints. Every input of a tier must satisfy all of its constraints, special cases included. If it also holds a previous source with its compiler errors, fix that source.

Write the module `generator.rs` of a Rust 2024 crate whose only dependency is `ezcp`. It defines, all `pub`:

- `Input`: one test input, implementing `ezcp::ToOutput` so that it is written exactly in the statement's input format; usually a struct with `#[derive(ToOutput)]`.
- `fn generate(rng: &mut ezcp::Rng, tier: usize) -> Input`: a random input within every constraint of the tier. Each call picks one kind with `rng.weighted_index`: random sizes skewed towards the tier's maximum; exactly the maximum sizes; the minimum ones; extreme values (all maximal, all equal, sorted); structures that break naive solutions (paths, stars, deep trees, many duplicates).
- `fn validate(input: &Input, tier: usize)`: asserts every constraint of the tier.

- All randomness comes from `rng`.
- Respect every bound, including guarantees the statement gives in words, such as distinct values, a connected graph or an existing answer.
- The largest tier's input must take well under a second to generate in a release build.
- Reply with the complete source only, without Markdown fences.
