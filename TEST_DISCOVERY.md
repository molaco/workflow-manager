# Testing Workflow Discovery

## Current Status
- ✅ Discovery system implemented (2 paths only)
- ✅ `workflow-manager/workflows/` directory created
- ✅ Install script created
- ⏳ Need to copy binaries to test

## Test Steps

### Step 1: Build test_workflow
```bash
cd /home/molaco/Documents/japanese
cargo build --example test_workflow
```

### Step 2: Copy binary to workflows directory
```bash
cp target/debug/examples/test_workflow workflow-manager/workflows/
chmod +x workflow-manager/workflows/test_workflow
```

### Step 3: Verify binary is there
```bash
ls -la workflow-manager/workflows/
# Should see: test_workflow (executable)
```

### Step 4: Test metadata extraction manually
```bash
workflow-manager/workflows/test_workflow --workflow-metadata
# Should output JSON metadata
```

### Step 5: Test discovery
```bash
cargo run --example test_discovery
```

**Expected output:**
```
Discovered 1 workflows:

✓ Test Workflow (test_workflow)
  Binary: /home/molaco/Documents/japanese/workflow-manager/workflows/test_workflow
  Fields: 2
    - input (Input File): [FILE PATH] Input file (e.g., 'data.txt')
    - batch_size (Batch Size): [NUMBER] Tasks to run in parallel (1-5)
```

## Alternative: Use install script
```bash
cd /home/molaco/Documents/japanese/workflow-manager
chmod +x install-workflow.sh
./install-workflow.sh test_workflow
```

## Troubleshooting

### If discovery finds 0 workflows:
1. Check `workflow-manager/workflows/` has the binary (not .rs file)
2. Check binary is executable: `ls -l workflow-manager/workflows/test_workflow`
3. Check metadata works: `workflow-manager/workflows/test_workflow --workflow-metadata`
4. Check `CARGO_MANIFEST_DIR` path: Add debug print in `get_search_paths()`

### If "Permission denied":
```bash
chmod +x workflow-manager/workflows/test_workflow
```

## Next: Test TUI
Once discovery works:
```bash
cargo run  # Launch TUI
# Should show test_workflow in the list
```
