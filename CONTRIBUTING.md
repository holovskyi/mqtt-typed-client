# Contributing to mqtt_typed_client

Thank you for your interest in contributing to mqtt_typed_client! We welcome contributions from everyone.

## Code of Conduct

This project adheres to the Rust [Code of Conduct](https://www.rust-lang.org/policies/code-of-conduct). 
By participating, you are expected to uphold this code.

## How to Contribute

### Reporting Bugs

Before creating a bug report, please:
1. Check the existing issues to see if the bug has already been reported
2. Update to the latest version to see if the issue persists

When creating a bug report, please include:
- A clear and descriptive title
- Steps to reproduce the behavior
- Expected behavior vs actual behavior
- Your environment (OS, Rust version, dependency versions)
- Minimal code example that demonstrates the issue

### Suggesting Features

Feature requests are welcome! Please:
1. Check existing issues to avoid duplicates
2. Clearly describe the feature and its use case
3. Explain why this feature would be beneficial to other users

### Pull Requests

1. **Fork** the repository
2. **Create** a feature branch: `git checkout -b feature/my-new-feature`
3. **Make** your changes
4. **Add tests** for new functionality
5. **Run tests**: `cargo test --all`
6. **Run clippy**: `cargo clippy --all-targets --all-features`
7. **Format code**: `cargo fmt --all`
8. **Commit** your changes: `git commit -am 'Add some feature'`
9. **Push** to the branch: `git push origin feature/my-new-feature`
10. **Submit** a pull request

### Development Setup

```bash
# Clone the repository
git clone https://github.com/yourusername/mqtt_typed_client.git
cd mqtt_typed_client

# Run tests
cargo test --all

# Run examples
cargo run --example main_example

# Check formatting and linting
cargo fmt --all
cargo clippy --all-targets --all-features
```

### Testing

- Write tests for new functionality
- Ensure all existing tests pass
- Include integration tests for complex features
- Test with different MQTT brokers if possible

### Documentation

- Update documentation for new features
- Add examples for complex functionality
- Ensure all public APIs are documented
- Update CHANGELOG.md for notable changes

### Code Style

- Follow Rust standard formatting (`cargo fmt`)
- Address all clippy warnings
- Use descriptive variable and function names
- Add comments for complex logic

## Project Structure

```
mqtt_typed_client/
├── src/
│   ├── client/          # High-level client API
│   ├── routing/         # Message routing system
│   ├── topic/           # Topic pattern handling
│   ├── structured/      # Structured subscribers
│   └── lib.rs          # Main library entry
├── macros/             # Procedural macros
├── examples/           # Usage examples
└── docs/              # Additional documentation
```

## Questions?

Feel free to open an issue for any questions about contributing!

## License

By contributing, you agree that your contributions will be licensed under the same license as the project (MIT OR Apache-2.0).
