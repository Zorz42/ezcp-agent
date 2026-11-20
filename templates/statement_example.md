# Sum

**Time limit:** 1 s · **Memory limit:** 256 MB

You are given $n$ integers $a_1, a_2, \dots, a_n$. Print their sum.

## Input

The first line contains an integer $n$.

The second line contains $n$ integers $a_1, a_2, \dots, a_n$, separated by spaces.

## Output

Print a single integer: $a_1 + a_2 + \dots + a_n$.

## Constraints

- $1 \le n \le 200\,000$
- $-10^9 \le a_i \le 10^9$

## Subtasks

| Subtask | Points | Constraints |
| --- | --- | --- |
| 1 | 20 | $n \le 10$ |
| 2 | 30 | $0 \le a_i \le 1\,000$ |
| 3 | 50 | No additional constraints. |

## Samples

### Sample 1

Input:

```
3
1 2 3
```

Output:

```
6
```

### Sample 2

Input:

```
4
1000000000 1000000000 -5 1000000000
```

Output:

```
2999999995
```

## Notes

In the first sample, $1 + 2 + 3 = 6$.

In the second sample, the sum does not fit in a 32-bit integer.
