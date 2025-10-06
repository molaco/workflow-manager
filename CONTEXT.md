# Shared Context Between Agents

Multiple agents can share context and manage concurrent read/write access using various approaches:

## 1. Shared Context File with File-Based Locking

```python
import fcntl  # Unix file locking
import json

# Agent writes to shared context
with open("shared_context.json", "a+") as f:
    fcntl.flock(f.fileno(), fcntl.LOCK_EX)  # Exclusive lock
    context = json.load(f)
    context["agent_1_result"] = "..."
    json.dump(context, f)
    fcntl.flock(f.fileno(), fcntl.LOCK_UN)  # Unlock
```

## 2. Python asyncio Locks (if agents share same process)

```python
import asyncio

context_lock = asyncio.Lock()
shared_context = {}

async def agent_1():
    async with context_lock:
        shared_context["data"] = "agent 1 result"

async def agent_2():
    async with context_lock:
        data = shared_context.get("data")
```

## 3. Message Queue System (most robust)

- Use Redis, RabbitMQ, or simple SQLite database
- Agents publish/subscribe to shared context
- Natural turn management through queue ordering

## 4. For ClaudeSDKClient Use Cases

The challenge: Each `ClaudeSDKClient` instance manages its own conversation state internally. To share context between agents:

### Option A: Sequential with shared file

```python
# Agent 1 runs, writes results to context.json
# Agent 2 reads context.json as input file, continues work
# Agent 3 reads updated context.json, etc.
```

### Option B: Parent coordinator agent

```python
# Main agent spawns Task agents
# Each Task agent returns results
# Main agent aggregates and provides to next Task agent
```

### Option C: Custom context manager

```python
class SharedContext:
    def __init__(self, file_path):
        self.file_path = file_path
        self.lock = asyncio.Lock()

    async def read(self):
        async with self.lock:
            with open(self.file_path) as f:
                return json.load(f)

    async def write(self, data):
        async with self.lock:
            with open(self.file_path, 'w') as f:
                json.dump(data, f)
```

## Key Considerations

- **File-based locking**: Works across processes, simple but slower
- **asyncio locks**: Fast, but only works within same Python process
- **Message queues**: Most robust for distributed systems, adds complexity
- **ClaudeSDKClient**: Each instance has isolated conversation state, requires explicit context sharing through files or coordinator patterns
