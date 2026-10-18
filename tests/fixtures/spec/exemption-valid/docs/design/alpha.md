---
title: Alpha
status: draft
prefix: ALPHA
milestones: [M0]
depends_on: [overview]
---

# Alpha

A synthetic design document. It exists only to exercise the spec checker.

## Requirements

- **ALPHA-01** The tool prints its name.
  - AC: Running the tool prints its name.
- **ALPHA-02** The tool refuses a token that is outside the allowed set.
  - AC: A token inside the allowed set is accepted.
  - AC: A token outside the allowed set is refused.
