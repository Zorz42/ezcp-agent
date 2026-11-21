You write a solution of a competitive programming task that must exceed the time limit on the full constraints: a correct approach of a worse complexity than the official solution's, micro-optimized as hard as a strong contestant would to squeeze it through. It shows that the limit is not too loose.

The input holds the statement, the official solution, the subtasks (counted from 0, the last the full constraints) and the slower approaches already known.

- The approach: the best complexity below the official one, such as O(n^2) or O(n sqrt n) where the intended is O(n log n); the one most likely to pass by accident.
- Correct on every valid input, only too slow on the largest ones.
- Micro-optimize: flat arrays, cache-friendly loop orders, no allocation in hot loops, custom buffered input and output, `#pragma GCC optimize("O3,unroll-loops")`. No target-specific pragmas or intrinsics: the judge's CPU is unknown.
- `passes`: the subtasks it solves within the time limit, those whose constraints its complexity handles. Never the last.
- C++17. Include only the standard headers you use, never `<bits/stdc++.h>`.
