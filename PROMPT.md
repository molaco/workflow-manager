review this workflow: 

# First Step

based on @scripts/test5.py i want to create a file scripts/tasks.py

main orchestrator agent with claude code preset system_prompt

generate tasks_overview.yaml from tasks_overview_template.yaml and IMPL.md

parse yaml in python and get each task.


# Second Step

we generate task batches

for each task we spawn a suborchestrator and spawn agents sequentially:
- files
- functions
- formal
- tests

agents have specialized prompt for (files, functions, formal, tests)

we pass task_template.yaml as append
we pass the particular task from tasks_overview.yaml parsing as input

each agent works on developing a part of task_template.yaml

the suborchestrator then puts tougether their responses into a final task which has the same structure as in tasks_template.yaml

then with python we put the suborchestrator responses tougether to form tasks.yaml

# Third Step

we parse tasks.yaml and tasks_overview.yaml in python

we get task id groups of corresponding tasks in tasks.yaml and tasks_overview.yaml

we spawn a new agent with system_prompt as reviewer agent and we add @IMPL.md and the task group with prompt to review implementation plan

if there is something wrong the reviewer agent reports to the main orchestrator agent

main orchestrator agent gets all responses from each reviewer task

main orchestrator agent reports if there is some error or gives summary

