# scripts

Local integration, migration, fixture, and deployment helper scripts.

- `local-dev.sh`: prepares a local smoke-test layout with placeholder configs and mock transport disk directories on Linux/macOS.
- `local-dev.ps1`: prepares the same local smoke-test layout from PowerShell.
- `generate-fixtures.ps1`: generates reusable `INITIALIZED` and importable `SEALED` transport disk fixtures plus HMAC request samples under `target/fixtures`.
- `check-deploy.ps1`: static checks for deploy files, udev safety, config placeholders, and script secret hygiene.

Scripts must not contain real credentials, site secrets, database passwords, or RustFS secrets.
