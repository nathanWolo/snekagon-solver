# Lessons so far

What the log has taught, distilled. `LOG.md` has the record.

## How to read the numbers

Confirmed: fixed length, fresh units. Gate: stopped where the test decided, so it reads high. Tune:
screens and profiles, for ranking only. Node counts are exact; seconds are this machine's.

## About the problem

- Each earlier ply costs the first solver about ten times more (ply 9: 0.5 ms, ply 8: 6 ms, ply 7: 50 ms).

## About measuring

- Timing noise is 1-2% per unit of 10 positions (A/A). Node counts repeat exactly.

## About engineering

- Counting moves by popcount instead of listing them makes perft 3 take 0.13 s.

## Open questions for the owner

- Is 5% the right bar for keeping a speed change?
- The reference solver could not be run here (downloaded code). A same-machine baseline needs the
  owner to build and run it.
