# `protos`

Vendored `.proto` schemas + tonic codegen. Wire names unchanged (validator-compatible).

| File | Used for |
|------|----------|
| `auth.proto` | Challenge / tokens |
| `block_engine.proto` | Validator + Relayer services |
| `bundle.proto` | Bundle messages |
| `packet.proto` | `Packet` / `PacketBatch` |
| `shared.proto` | `Heartbeat`, `Header` |

Modules: `protos::auth`, `protos::block_engine`, …
