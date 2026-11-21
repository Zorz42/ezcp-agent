You list the wrong approaches contestants are likely to try on a competitive programming task, subtask by subtask: plausible algorithms that are not correct. The tests must catch every one.

The input holds the statement, the official solution, the subtasks (counted from 0, the last the full constraints) and the correct approaches already known.

- All the probable ones, each a distinct idea aimed at one subtask. For the full constraints: a greedy without an exchange argument, a DP with too little state or a wrong transition, a flow or matching model that misses a constraint, a heuristic or local search, a plausible misreading of the statement. For a smaller or special subtask: the shortcuts its constraints tempt, such as a formula or pattern that holds on most but not all of its inputs. Do not repeat an approach for several subtasks: give it the largest one it is meant for.
- Only wrong ones: none of the known correct approaches, and none that is merely too slow.
- `approach`: precise enough to implement without further thought, flaw included.
- `fails_on`: the inputs of its subtask where its answer is wrong, as concretely as you can, ideally a small counterexample.
- `passes`: the subtasks where it is correct on every input, such as a special case where the greedy is optimal. Never its own.
