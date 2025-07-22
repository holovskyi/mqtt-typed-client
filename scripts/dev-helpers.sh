#!/bin/bash
# Development helpers for mqtt-typed-client project
# 
# Usage:
#   source scripts/dev-helpers.sh
#   # or add to your ~/.bashrc:
#   # source /path/to/mqtt-typed-client/scripts/dev-helpers.sh

# Function to run examples with path autocompletion
run_example() {
    if [[ -z "$1" ]]; then
        echo "Usage: run_example <example_name_or_path>" >&2
        echo "Available examples:" >&2
        ls examples/*.rs 2>/dev/null | xargs -n1 basename -s .rs | sort >&2
        return 1
    fi
    
    local example_name
    if [[ $1 == examples/* ]]; then
        # Extract filename without path and extension
        example_name=$(basename "$1" .rs)
    else
        example_name="$1"
    fi
    
    # Check if we're in the right directory
    if [[ ! -f "Cargo.toml" ]] || [[ ! -d "examples" ]]; then
        echo "Error: Must be run from mqtt-typed-client project root" >&2
        echo "Current directory: $(pwd)" >&2
        return 1
    fi
    
    # Check if example exists
    if [[ ! -f "examples/${example_name}.rs" ]]; then
        echo "Error: Example 'examples/${example_name}.rs' not found" >&2
        echo "Available examples:" >&2
        ls examples/*.rs 2>/dev/null | xargs -n1 basename -s .rs | sort >&2
        return 1
    fi
    
    echo "🚀 Running example: $example_name" >&2
    cargo run --example "$example_name"
}

# Autocompletion function
_run_example_completion() {
    local cur="${COMP_WORDS[COMP_CWORD]}"
    local suggestions=""
    
    # If started with "examples/", show files with full path
    if [[ $cur == examples/* ]]; then
        suggestions=$(compgen -f -- "$cur" | grep '\.rs$')
    else
        # Otherwise show just filenames
        if [[ -d "examples" ]]; then
            suggestions=$(ls examples/*.rs 2>/dev/null | xargs -n1 basename -s .rs)
        fi
    fi
    
    COMPREPLY=($(compgen -W "$suggestions" -- "$cur"))
}

# Register autocompletion
complete -F _run_example_completion run_example

# Aliases
alias cre='run_example'
alias example='run_example'

# List examples with descriptions
list_examples() {
    echo "📚 Available examples:" >&2
    if [[ -d "examples" ]]; then
        ls examples/*.rs 2>/dev/null | while read -r file; do
            local name=$(basename "$file" .rs)
            local first_line=$(head -n 20 "$file" | grep -E "^//[^/]" | head -n 1 | sed 's|^//\s*||')
            printf "  %-25s %s\n" "$name" "$first_line"
        done >&2
    else
        echo "No examples directory found" >&2
    fi
}

alias les='list_examples'

# Quick broker management
start_broker() {
    echo "🐳 Starting MQTT broker..." >&2
    cd dev && docker-compose up -d && cd ..
}

stop_broker() {
    echo "🛑 Stopping MQTT broker..." >&2
    cd dev && docker-compose down && cd ..
}

# Show broker status
broker_status() {
    echo "📊 MQTT Broker Status:" >&2
    cd dev && docker-compose ps && cd ..
}

alias broker-start='start_broker'
alias broker-stop='stop_broker'
alias broker-status='broker_status'

echo "✅ mqtt-typed-client development helpers loaded!" >&2
echo "Available commands: cre, les, broker-start, broker-stop, broker-status" >&2
