
When i press edit and go to modify a file path

Output Style:
[TEXT] Path to output style format file
<empty> 

i press Enter and then Tab and it shows tab completion

┌ Tab Completion ────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────┐ 
│📁 RESULTS
│📁 TRASH
│📁 examples
│📁 src
│📁 templates
│📄 Cargo.toml
│📄 IMPL.md
│📄 TUI_VIEWS.md
└────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────

if i press enter in a file like "Cargo.toml" the Tab Completion closes and

the file path gets updated to "/home/molaco/Documents/japanese/workflow-manager/Cargo.toml"

and it looks like this

Output Style:
[TEXT] Path to output style format file
/home/molaco/Documents/japanese/workflow-manager/Cargo.toml

In the other hand, if i choose a directory like "examples" the path gets updated to:

/home/molaco/Documents/japanese/workflow-manager/examples/

and the Tab Completion does not close which is good but it shows this

┌ Tab Completion ──────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────
|📁 examples
└───────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────

but this is wrong. what i should see is:

┌ Tab Completion ──────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────
📁../
📄new_research_agent.rs
📄research_agent.rs
📄tasks_agent.rs
📄test2.rs
📄test_discovery.rs
📄test_workflow.rs
└───────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────

which are the actual contents of the directory example.
