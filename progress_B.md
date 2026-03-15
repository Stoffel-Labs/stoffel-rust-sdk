# Agent B Progress: RFC-008 Configuration System

## 2026-03-15: Initial implementation

### Completed
- Created `src/config/mod.rs` with all RFC-008 configuration types:
  - `MpcConfig` with defaults (parties=5, threshold=1, random instance_id, HoneyBadger backend)
  - `MpcBackendConfig` tagged enum (HoneyBadger, Avss with curve)
  - `NetworkConfig` with bind_address, peers, timeouts
  - `PreprocessingConfig` with defaults (triples=1000, random_shares=500, min_triples=100, generate_on_startup=true)
  - `Curve` enum (Bls12_381, Bn254, Curve25519, Ed25519) with field_bits() and security_bits()
  - `StoffelConfig` top-level TOML config aggregating all sub-configs
- Created `src/config/validation.rs` with validation functions:
  - `validate_mpc()` checks parties >= 4 and parties >= 3*threshold + 1
  - `validate_network()` checks expected_parties >= 2
- Implemented `StoffelConfig::load(path)` for TOML file loading
- Implemented `StoffelConfig::load_with_env(path)` for TOML + env overrides
- Environment variable overrides for all config fields (STOFFEL_PARTIES, STOFFEL_THRESHOLD, etc.)
- Added `pub mod config;` to lib.rs
- Comprehensive test suite covering:
  - Default values for all config types
  - Validation (valid and invalid configurations)
  - TOML serialization/deserialization roundtrips
  - Environment variable overrides
  - Serde attribute behavior (defaults applied, tagged enums, rename_all)

### Design decisions
- Split validation into separate `validation.rs` for clarity and testability
- Used `#[serde(tag = "protocol", rename_all = "lowercase")]` for MpcBackendConfig to get clean TOML
- Used `#[serde(rename_all = "kebab-case")]` for Curve to match CLI conventions (bls12-381, bn254, etc.)
- Environment variable parsing uses a generic helper for consistent error messages
- STOFFEL_CURVE env var promotes backend to AVSS if not already set (curve only applies to AVSS)
