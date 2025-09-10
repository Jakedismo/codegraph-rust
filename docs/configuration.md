Configuration Management
========================

Overview
- Shared `Settings` live in `codegraph-core::config` and power environment-specific loading, validation, and secure secrets.
- Sources merge order:
  1. `config/default.(toml|yaml|json)`
  2. `config/{env}.(toml|yaml|json)` (env from `APP_ENV` or `RUST_ENV`)
  3. `config/local.toml` (optional, dev overrides)
  4. Environment variables with prefix `CODEGRAPH__` (e.g., `CODEGRAPH__SERVER__PORT=8080`)
  5. Optional encrypted secrets file `config/secrets.enc`

Secrets
- Encrypt/decrypt with `cargo run -p measure_baseline_performance --bin config_tool`:
  - Generate key: `config_tool generate-key` (outputs base64 32 bytes)
  - Encrypt file: `CONFIG_ENC_KEY=... config_tool encrypt --input secrets.toml --output config/secrets.enc`
  - Decrypt file: `CONFIG_ENC_KEY=... config_tool decrypt --input config/secrets.enc`
- At runtime, set `CONFIG_ENC_KEY` for decryption. Decrypted TOML merges into config.

Validation and Schema
- `Settings::validate()` enforces basic invariants (port, vector dimension, etc.).
- Generate JSON Schema: `config_tool generate-schema --out config/schema.json`.

Hot Reloading
- A file watcher monitors the `config/` directory and applies changes at runtime.
- The live settings are accessible via `AppState.settings: Arc<RwLock<Settings>>` in the API.

Environment Examples
- Dev: `APP_ENV=development cargo run --bin codegraph-api`
- Prod: `APP_ENV=production CONFIG_ENC_KEY=... ./codegraph-api`

Environment Variable Overrides
- Use double-underscore to navigate settings: `CODEGRAPH__SERVER__PORT=9090`.

Security Notes
- Never commit plaintext secrets. Use encrypted file or environment variables.
- Key must be base64-encoded 32 bytes (`CONFIG_ENC_KEY`).

