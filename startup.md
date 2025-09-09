ouroboros orchestrate \
    --topology swarm \
    --coordinator-provider claude \
    --model claude-sonnet-4-20250514 \
    --working-dir $(pwd) \
    --disable-providers qwen-code,cursor-agent,ouroboros-code \
    --timeout 480 \
    --dry-run \
    "$(cat task.md)"
