

EZCP reference, from its GUIDE.md:

## Rules

- **All randomness comes from the generator's `rng`.** A test is stored and rebuilt as its generator
  plus a seed. Never use `rand`, the clock, `HashMap`/`HashSet` iteration order, or a random value
  drawn outside the generator and captured. Captured constants are fine. `--seeds` and
  `with_reproducibility_checks(n)` catch violations (`GeneratorNotReproducible`).
- Inputs and official outputs have their whitespace normalised (runs collapse, one trailing newline)
  unless `trim_whitespace(false)`.

## Writing inputs

| Value | Written as |
| --- | --- |
| number, `char`, `bool`, `&str`, `String` | itself (`bool` as `1`/`0`) |
| `Vec`, slice, array, tuple | one line, space-separated |
| `#[derive(ToOutput)]` struct | each field on its own line, in order; empty fields skipped |
| `Counted(vec)` | length line, then the `Vec` |
| `Lines(vec)` | one value per line |
| `CountedLines(vec)` | length line, then one value per line |
| `Graph` | per its `GraphFormat` |

A value that already ends a line (tuple, struct, `Lines`, `Graph`) is not followed by a space, so a
`Vec` of them is one per line. Several test cases in one input:

```rust,ignore
#[derive(ToOutput)]
struct Case { n: usize, values: Vec<i64> }

// t cases whose n add up to 200000.
let sizes = rng.partition(200_000_usize, t, 1);
Counted(sizes.into_iter().map(|n| Case { n, values: rng.random_vec(n, 1..=1_000_000_000) }).collect())
```

A hand-written impl writes into the buffer: `impl ToOutput for X { fn write_output(self, out: &mut String) { ... } }`.

## Rng

`random_range(a..=b)` (integers, floats, `char`), `random_range_skewed(range, k)` (max of `k + 1`
draws; negative `k`: min of `1 - k`), `random_vec(n, range)`, `random_interval(range)` (`l <= r`),
`distinct(k, range)`, `permutation(n)` (of `1..=n`), `shuffled(iter)`, `shuffle(&mut slice)`,
`partition(total, parts, min_part)`, `random_string(n, alphabet)`, `choose(&slice)`,
`weighted_index(&[w0, w1, ...])`, `random_bool(p)`, `random_f64()`, `next_u64()`.

## Graph

Nodes are `0..n` in code and written from 1 (`zero_based()` to change that). Edges are written in
random order with random end order (directed edges keep their direction).

- Trees (format `Tree`: `n`, then `n - 1` edges): `new_random_tree` (about `ln n` deep),
  `new_random_uniform_tree` (`sqrt n`), `new_random_deep_tree` (`n / 2`), `new_random_path`,
  `new_star`, `new_caterpillar(rng, n, spine)`, `new_binary_tree`.
- Graphs (format `EdgeList`: `n m`, then the edges): `new_empty`, `new_full`, `new_random(rng, n, m)`,
  `new_random_connected(rng, n, m)`, `new_random_bipartite(rng, n, m)`, `new_random_forest(rng, n,
  trees)`, `new_cycle`, `new_grid(rng, rows, columns)`, and directed `new_empty_directed`,
  `new_random_directed`, `new_random_dag`.
- `with_format(GraphFormat::ParentList)`: `n`, then the parents of nodes 2..n rooted at 1, and the
  weights on a third line if any. `GraphFormat::EdgesOnly`: the edges alone, for a custom header.
- `with_random_weights(rng, 1..=x)` / `with_weights(vec)` add a weight after each edge; add every
  edge first. `with_shuffled_labels(rng)` hides structure left in the numbering (grids, paths).
- `add_edge(u, v)` (ignores loops and duplicates), `has_edge`, `edges()`, `neighbours`, `degree`,
  `num_nodes`, `num_edges`, `is_tree`, `is_connected`, `is_bipartite`, `connected_components`.

Mix tree shapes in one generator: a solution is often slow or wrong on only one of them.

```rust,ignore
match rng.weighted_index(&[3, 2, 2, 1, 1]) {
    0 => Graph::new_random_uniform_tree(rng, n),
    1 => Graph::new_random_deep_tree(rng, n),
    2 => Graph::new_random_path(rng, n),
    3 => Graph::new_star(rng, n),
    _ => Graph::new_caterpillar(rng, n, n.div_ceil(2)),
}
```

