# Development Scripts

This directory contains helper scripts to improve the development experience.

## dev-helpers.sh

Bash functions and aliases for easier development workflow.

### Installation

**Option 1: Temporary (current session only)**
```bash
source scripts/dev-helpers.sh
```

**Option 2: Permanent (add to your ~/.bashrc)**
```bash
echo "source $(pwd)/scripts/dev-helpers.sh" >> ~/.bashrc
source ~/.bashrc
```

**Option 3: Project-specific (recommended)**
```bash
# Add to your project's .bashrc or create a direnv .envrc file
echo "source scripts/dev-helpers.sh" > .envrc
direnv allow  # if using direnv
```

### Available Commands

| Command | Alias | Description |
|---------|-------|-------------|
| `run_example <name>` | `cre` | Run examples with tab completion |
| `list_examples` | `les` | List all examples with descriptions |
| `start_broker` | `broker-start` | Start local MQTT broker |
| `stop_broker` | `broker-stop` | Stop local MQTT broker |
| `broker_status` | `broker-status` | Show broker status |

### Usage Examples

```bash
# Run examples (with tab completion!)
cre 000<TAB>                    # completes to 000_hello_world
cre examples/001<TAB>           # completes to examples/001_ping_pong.rs

# List available examples
les

# Broker management
broker-start                    # Start local MQTT broker
broker-status                   # Check if broker is running
broker-stop                     # Stop broker
```

### Features

- **Tab completion**: Works with both short names and full paths
- **Error handling**: Checks if you're in the right directory
- **Proper stderr**: Preserves cargo error output
- **Example validation**: Verifies example exists before running
- **Broker helpers**: Quick broker management commands

### Troubleshooting

**"Command not found" error:**
```bash
# Make sure you sourced the script
source scripts/dev-helpers.sh
```

**Tab completion not working:**
```bash
# Completion requires bash, not sh/zsh
echo $SHELL  # should show bash
```

**"Must be run from project root" error:**
```bash
# Navigate to project root first
cd /path/to/mqtt-typed-client
cre 000_hello_world
```
