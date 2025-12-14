---
title: Installation
description: How to install undox
---

# Installation

## From Source

Currently, undox must be built from source. Make sure you have [Rust](https://rustup.rs/) installed, then:

```bash
git clone https://github.com/undox/undox
cd undox
cargo install --path .
```

## Verify Installation

```bash
undox --version
```

## Requirements

- **Rust 1.75+** (uses Rust 2024 edition features)
- **Git** (for cloning, and future remote source support)

## Next Steps

Once installed, follow the [Quickstart](/getting-started/quickstart) guide to create your first documentation site.
