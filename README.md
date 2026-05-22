# x

A monorepo containing Rust crates and JavaScript/TypeScript packages.

## Structure

```
x/
├── Cargo.toml          # Rust workspace manifest
├── crates/
│   ├── core/           # Core Rust library
│   └── utils/          # Utility Rust library
├── packages/
│   ├── ui/             # React UI components
│   ├── graph/           # Graph utilities (d3, relay)
│   └── i18n/            # Internationalization (fbtee, grats)
├── rustfmt.toml
├── .cargo/
│   └── config.toml
└── README.md
```

## Rust Crates

```bash
cargo build        # Build all crates
cargo check        # Check all crates
cargo test         # Run all tests
```

## JS/TS Packages

```bash
pnpm install
pnpm build         # Build all packages
pnpm lint          # Lint all packages
```

## Tooling

- **oxfmt** — Rust formatting (https://crates.io/crates/oxfmt)
- **oxlint-config** — Rust linting configuration support
- **rustfmt** — Standard Rust formatter

## Packages

### Crates

| Crate | Description |
|-------|-------------|
| `crates/core` | Core functionality |
| `crates/utils` | Common utilities |

### npm

| Package | Version | Description |
|---------|---------|-------------|
| `react` | 19.2.6 | UI library |
| `relay-runtime` | 21.0.0 | GraphQL runtime |
| `d3` | 7.9.0 | Data visualization |
| `stylex` | 0.3.0 | CSS-in-JS |
| `grats` | 0.0.36 | GraphQL TypeScript |
| `fbtee` | 1.8.0 | React i18n framework |