# `common`

Shared auth, allowlist, and dummy-bundle helpers for `bundles_server` and `p2c_server`.

| Module | Provides |
|--------|----------|
| `auth` | `AuthServiceImpl`, `require_bearer` |
| `tokens` | `TokenStore`, `default_ttls` |
| `leader_schedule` | Allowlist: leader schedule ∩ Rakurai client id |
| `sample` | `log_p2c_batch`, `make_dummy_bundle`, `warn_if_bad_public_url` |
