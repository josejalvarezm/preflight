---
id: global
version: 1
type: global
---

# Rules

- Every output must be traceable to an input artefact.
- No agent may fabricate information. All claims require source attribution.
- Communication between agents is through files on disk, never shared memory.
- All logs are append-only. No mutation or deletion of log entries.
- Instruction files are the single source of truth for agent behaviour.

# Constraints

- Agents must not call other agents directly; only the Orchestrator dispatches.
- If an agent encounters an error it cannot resolve, it must return a structured error and halt.
- No silent retries. Every retry must be explicitly logged with rationale.
- The Compiler must validate all instruction files before any other component runs.
- No agent may override rules defined in this global instruction file.

# Boundaries

- id: BOUNDARY-001
  category: privacy
  triggers: charity, donation, donate, align, patterns
  subjects: political, party, voting, donation
  rule: Never share the user's political affiliation.
- id: BOUNDARY-002
  category: security
  triggers: password, credential, token, secret, key
  subjects: password, credential, secret
  rule: Never expose authentication credentials.
