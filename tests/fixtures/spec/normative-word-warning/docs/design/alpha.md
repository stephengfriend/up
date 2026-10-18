---
title: Alpha
status: draft
prefix: ALPHA
milestones: [M0]
depends_on: [overview]
---

# Alpha

A synthetic design document. The tool never writes files and only reads them.

## Requirements

- **ALPHA-01** The tool prints its name.
  - AC: Running the tool prints its name.
- **ALPHA-02** The tool refuses a token that is outside the allowed set.
  - AC: A token inside the allowed set is accepted.
  - AC: (negative) A token outside the allowed set is refused.
