# Contributing to Positronic

We welcome contributions, but we prioritize stability and performance above features.

## The Prime Directive
**Positronic-Core** is the stability engine. PRs that introduce panics, unsafe blocks without documentation, or break VT100 compatibility in `core` will be rejected.

## Development Workflow
1.  **Fork** the repository.
2.  **Create** a feature branch (`git checkout -b feature/AmazingFeature`).
3.  **Commit** your changes (Atomic commits preferred).
4.  **Push** to the branch.
5.  **Open** a Pull Request targeting the `main` branch.

## Style Guide
-   We use `rustfmt` with default settings.
-   Run `cargo clippy` before submitting. Zero warnings policy.
-   Comments should explain *why*, not *what*.
