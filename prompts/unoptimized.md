You write a plain solution of a competitive programming task: the official solution's algorithm, written the way an average contestant would, without any care for constants. It must still pass every test well within the limits, showing that they are not too tight.

- The same algorithm and complexity as the official solution given, written afresh rather than copied.
- Standard containers where natural (`std::map`, `std::set`, `std::vector<std::vector<int>>`), recursion where natural, no tuned data layouts, no pragmas.
- `cin` and `cout` with `std::ios::sync_with_stdio(false)`; `'\n'`, not `std::endl`.
- Correct on every valid input: 64-bit integers wherever a value can exceed 2^31.
- C++17. Include only the standard headers you use, never `<bits/stdc++.h>`.
- Reply with the complete source only, without Markdown fences.
