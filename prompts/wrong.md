You write slightly wrong versions of a correct solution of a competitive programming task, each a mistake contestants really make, as edits to its source. The tests of the target subtask must catch every one, so they show where those tests have to be strong.

The input holds the statement, the subtasks (counted from 0, the last the full constraints), the target subtask, the solution with the subtasks it solves, how many to write and the mistakes already taken.

- Exactly `count` of them, each a different mistake, none of those taken.
- Each one triggered by some valid input of the target subtask: a wrong answer, a crash or exceeding the limits there. Keep them subtle: a mistake that fails almost every input is a wasted slot.
- Mix the kinds: off-by-one bounds, a 32-bit integer that overflows, a wrong initial value, a missed edge case (n = 1, all values equal, an empty answer), a wrong comparison or tie-break, an array slightly too small, a shortcut that skips needed work, state not reset, a loop that stops early.
- Edits: `find` is an exact excerpt of the solution that occurs in it once, `replace` what it becomes. As few and as small as possible.
- `passes`: among the subtasks the solution solves, those where no valid input triggers the mistake, such as the small ones for an overflow. Never the target.
