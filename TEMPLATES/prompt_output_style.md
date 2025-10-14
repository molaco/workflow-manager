# Output Format Specification

You must output your response in valid YAML format that can be parsed by Python's `yaml.safe_load()`.

## Required Structure

```yaml
objective: "The research objective being addressed"
prompts:
  - title: "Short descriptive title for this research prompt"
    query: "The specific question or investigation task for the agent"
    focus:
      - "Key aspect 1 to investigate"
      - "Key aspect 2 to investigate"
      - "Key aspect 3 to investigate"

  - title: "Second research prompt title"
    query: "Another specific investigation task"
    focus:
      - "Focus area 1"
      - "Focus area 2"
```

## Field Definitions

- **objective**: A clear statement of the overall research goal (string)
- **prompts**: A list of research tasks (array)
  - **title**: Brief, descriptive name for this investigation (string, 3-8 words)
  - **query**: The actual prompt/question to send to the research agent (string, detailed and specific)
  - **focus**: List of 2-5 key aspects this query should investigate (array of strings)

## Guidelines

1. **YAML Validity**: Ensure proper indentation (2 spaces), quoting for special characters
2. **Query Specificity**: Each query should be actionable and focused on a specific aspect
3. **Comprehensive Coverage**: Generate 3-7 prompts that thoroughly cover the objective
4. **No Overlap**: Each prompt should investigate distinct aspects
5. **Parseable**: The output must be valid YAML without markdown formatting or extra commentary

## Example Output

```yaml
objective: "Understand how user authentication is implemented in the application"
prompts:
  - title: "Authentication Flow Analysis"
    query: "Investigate the complete authentication flow from login to session management. Examine the code paths, middleware, and database interactions involved."
    focus:
      - "Login endpoint implementation"
      - "Session creation and storage"
      - "Token generation mechanism"

  - title: "Password Security Review"
    query: "Analyze how passwords are hashed, stored, and verified. Check for security best practices and potential vulnerabilities."
    focus:
      - "Hashing algorithm used"
      - "Salt generation and storage"
      - "Validation process"

  - title: "Authorization Patterns"
    query: "Examine how user permissions and roles are checked throughout the application. Document the authorization middleware and guard implementations."
    focus:
      - "Role-based access control"
      - "Permission checking mechanisms"
      - "Protected route implementation"
```

## Important Notes

- Output ONLY the YAML structure, no additional text before or after
- If using markdown code fences, use ```yaml
- Ensure all strings with special characters are properly quoted
- Keep queries concise but comprehensive (1-3 sentences each)
