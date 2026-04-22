# Contributing to quicssh-rs

Thank you for your interest in contributing to quicssh-rs!

## About This Fork

This is a fork of [oowl/quicssh-rs](https://github.com/oowl/quicssh-rs) focused on **stabilization and robustness** for production use in diverse network environments.

## Where to Report Issues

### Issues Specific to This Fork

If you encounter issues related to:
- MTU configuration and VPN compatibility (Tailscale, WireGuard, etc.)
- Network stability improvements introduced in this fork
- Documentation or code quality improvements specific to this fork

**Please open an issue in this repository**: https://github.com/aruz/quicssh-rs/issues

### General quicssh-rs Issues

If you encounter issues unrelated to this fork's modifications:
- Basic QUIC/SSH proxy functionality
- Core protocol implementation
- Feature requests for the original project

**Please consider reporting to the upstream repository**: https://github.com/oowl/quicssh-rs/issues

This helps the original maintainer improve the base project for everyone.

## Contributing Code

### Coding Guidelines

Before submitting a pull request, please review our coding guidelines in [CLAUDE.md](CLAUDE.md):

- Write all comments in English
- Reference relevant RFCs and standards (e.g., RFC 9000, RFC 8899, RFC 8200)
- Follow existing code style and formatting

### Pull Request Process

1. Fork this repository
2. Create a feature branch (`git checkout -b feature/your-feature`)
3. Make your changes following the coding guidelines
4. Test your changes thoroughly
5. Commit with clear, descriptive messages
6. Push to your fork and submit a pull request

### Testing

```bash
# Build and check
cargo build
cargo check
cargo clippy

# Format code
cargo fmt

# Run tests (when available)
cargo test
```

## Upstream Contributions

If your contribution would benefit the original quicssh-rs project and is not specific to this fork's stabilization focus, please consider:

1. Submitting a PR to [oowl/quicssh-rs](https://github.com/oowl/quicssh-rs) first
2. We can then incorporate the upstream changes into this fork

This approach helps the entire quicssh-rs ecosystem.

## License

By contributing, you agree that your contributions will be licensed under the MIT License, maintaining copyright attribution to both the original author (Jun Ouyang) and this fork's maintainer.

## Questions?

If you have questions about contributing, feel free to:
- Open an issue for discussion
- Review existing issues and pull requests
- Check the [README.md](README.md) for project overview

Thank you for helping make quicssh-rs more stable and reliable!
