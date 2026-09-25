# Lessons

Patterns to avoid repeating. Added after a correction from the user.

## 2026-08-21 — Don't invent an invariant the code doesn't have

**What happened.** Diagnosing a broken layout, I wrote validation that warned whenever
a layout's `width_percent` values didn't sum to 100. The user pushed back: widths don't
need to sum to 100, they need to *stay on screen* — and overlapping windows would be
fine. Checking `calculate_slots` showed they were right and I was doubly wrong: summing
under 100 is harmless (the last column absorbs the slack), and summing over 100 doesn't
overlap, it runs off the screen edge.

**The rule I should have followed.** Before encoding a constraint, read the code that
consumes the value and ask what actually breaks. "Sums to 100" was an aesthetic
assumption imported from CSS-style layout; the engine's real contract was narrower and
asymmetric. A validation rule that warns on harmless input trains the user to ignore
warnings.

**How to apply.** When adding validation: for each condition, name the concrete failure
it predicts, and write a test that demonstrates that failure in the engine itself. If
you can't produce the failure, the rule is wrong. (Both warning conditions here now
have a matching test in `windows::tests` proving the on-screen consequence.)

## 2026-08-21 — Ask what the user is optimizing for before rebalancing their config

**What happened.** I "fixed" the user's layout by rebalancing widths to sum to 100,
which left their browser at 15% (~380px). They pointed out that made it useless —
they'd rather have overlap than five unusable slivers.

**How to apply.** Column count is a constraint, not a given. When a layout has more
windows than the screen can usefully hold, ask which windows need to be readable
*simultaneously* before dividing up the pixels. That answer drives the design — here it
was the difference between five tiled columns and three tiled plus one overlaid.

## 2026-08-21 — Verify against the real environment, not assumed numbers

**What happened.** I reasoned about widths assuming a 1920px monitor. It's 2560.
Separately, the Claude app turned out to enforce a ~616px minimum width, so a column
narrower than that silently does nothing.

**How to apply.** For anything positional, measure. `GetWindowRect` on the placed
windows after applying a layout costs one command and catches both wrong screen
assumptions and apps that refuse the size you asked for.

## 2026-08-21 — Two self-inflicted file-handling mistakes

- A Python splice using `t.index(...)` for the end marker silently deleted four methods
  between the insertion point and the marker. **Rule:** after a splice-style edit, run
  the build before doing anything else, and diff the symbol list against `HEAD`.
- I overwrote `tasks/user.md` with `cat >`, destroying a cumulative handoff log.
  **Rule:** read a file before overwriting it, even a "scratch" one. Append-style docs
  look like scratch files right up until you've deleted six months of them.
- Round-tripping a UTF-8 file through PowerShell `Get-Content`/`Set-Content` mangled
  box-drawing characters (cp1252 misread, then re-encoded). **Rule:** edit UTF-8 files
  with Python using explicit `encoding='utf-8'`, never PowerShell's default pipeline.
