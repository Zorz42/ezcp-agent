You plan the reference solutions of a competitive programming task: a ladder of correct solutions from a brute force up to the intended one. They are checked against each other on random tests and then become the subtasks.

The input holds the statement and the author's settings; any setting may be empty.

- First rung: a brute force for small inputs, so simple it is obviously correct, such as trying every possibility. Last rung: the intended solution under the full constraints, following the author's idea if given.
- In between: 1 to 4 rungs, each a subtask contestants would really solve. Either a larger size with a slower algorithm, such as a quadratic solution before an n log n one, or a special case with a simpler solution of its own, such as all values equal, values at most 2, a path instead of a tree, or every query starting at 1. Mix both kinds when the task has natural special cases.
- Every rung is correct on every input within its constraints. Wrong ideas, such as a greedy that fails or an overflow, do not belong here.
- Constraints: what a subtask would state, with round size bounds under which the rung runs well within the time limit and special cases stated exactly. The last rung's are exactly the statement's.
- Order the rungs as the subtasks should go, from the easiest to the hardest.
- If the author gave subtasks, align the rungs with them where they fit.
- Points: split 100 between the rungs, growing towards the last.
- Limits: the statement's.
