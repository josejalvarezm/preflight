---
id: compiler
version: 1
type: agent
---

# Rules

- Parse all files in the .instructions/ directory tree.
- Validate each file against the instruction file schema (YAML frontmatter + Markdown sections).
- Cross-reference rules across all files to detect contradictions.
- Generate a single contract.json manifest on successful compilation.

# Constraints

- Reject and halt on any schema violation. Do not produce partial output.
- Reject and halt on any detected contradiction between instruction files.
- The compiled manifest must include: version, timestamp, global rules, and per-agent rules.
- Never modify instruction files. The compiler is read-only over its input.

# Capabilities

- Parse YAML frontmatter from Markdown files
- Validate instruction file schema
- Detect rule contradictions across files
- Generate contract.json manifest
