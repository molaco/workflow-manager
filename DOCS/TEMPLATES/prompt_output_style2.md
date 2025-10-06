# CRITICAL: Output Format - YAML ONLY

You MUST output ONLY valid YAML that can be parsed by Python's `yaml.safe_load()`.

**DO NOT include:**
- Markdown headers (##, ###)
- Explanatory text before or after the YAML
- Markdown formatting
- Commentary outside the YAML structure

**OUTPUT ONLY THIS STRUCTURE:**

```yaml
objective: "Brief restatement of the research goal"
prompts:
  - title: "Concise investigation name (3-8 words)"
    query: "Detailed, specific question for the research agent to investigate. Include what to search for, what code/files to examine, and what insights to gather. Be specific and actionable (1-3 sentences)."
    focus:
      - "Key aspect 1 to investigate"
      - "Key aspect 2 to investigate"
      - "Key aspect 3 to investigate"

  - title: "Second investigation name"
    query: "Another specific research task with clear direction on what to find and analyze."
    focus:
      - "Focus area 1"
      - "Focus area 2"
```

## Rules

1. **YAML ONLY** - Start directly with `objective:`, no preamble
2. **Generate 3-7 prompts** - Comprehensive coverage without overlap
3. **Specific queries** - Each query should be actionable and focused
4. **Proper quoting** - Use quotes for strings with special characters (`:`, `?`, etc.)
5. **Valid syntax** - 2-space indentation, proper list formatting
6. **No extra text** - ONLY the YAML structure

## Field Requirements

- `objective`: Single-line string restating the research goal
- `title`: 3-8 word descriptive name
- `query`: 1-3 sentence investigation prompt with specific direction
- `focus`: 2-5 key aspects this query should investigate

## Example (this is what your ENTIRE output should look like):

```yaml
objective: "Understand authentication implementation in the application"
prompts:
  - title: "Authentication Flow Analysis"
    query: "Investigate the complete authentication flow from login to session management. Examine the code paths, middleware, database interactions, and token generation. Search for login endpoints, session storage, and authentication middleware files."
    focus:
      - "Login endpoint implementation"
      - "Session creation and storage"
      - "Token generation mechanism"

  - title: "Password Security Review"
    query: "Analyze how passwords are hashed, stored, and verified. Check for security best practices like bcrypt/argon2 usage, salt generation, and timing attack prevention. Look for password validation code and database schemas."
    focus:
      - "Hashing algorithm used"
      - "Salt generation and storage"
      - "Validation process"
```

**REMEMBER: Output ONLY the YAML structure, nothing else!**
