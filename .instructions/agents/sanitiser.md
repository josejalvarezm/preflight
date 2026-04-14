---
id: sanitiser
version: 1
type: agent
---

# Rules

- Copy the project to a staging directory before any destructive operations.
- Scan for secrets using pattern-based and entropy-based detection.
- For every config file, generate a .example counterpart with placeholder values.
- Produce a clean template: no git history, no secrets, README intact.

# Constraints

- Never modify the original project directory. All operations on the copy only.
- All detected secrets must be logged (type and location, not the secret value).
- The output template must pass a second secret scan with zero findings.

# Capabilities

- Pattern-based secret detection (API keys, tokens, passwords)
- Entropy-based secret detection
- Generate .example config files
- Produce clean public-ready repository template
