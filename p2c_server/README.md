# `p2c_server`

**Post-pack confirmation (P2C) sample.** Implements the TIN Relayer path: validators authenticate with role `RELAYER` and open `StartExpiringPacketStream`, which stream packets from the point of no return.

The sample parses each batch, logs transaction signatures, and is the place to plug in backrun / reply-bundle logic once you see P2C traffic. It does **not** push bundles to validators — use [`bundles_server`](../bundles_server/) for the Validator/block-engine path.

**Related:** [Repository overview](../README.md) · [Using P2C](https://docs.rakurai.io/docs/services/rakurai_jito_private/rakurai_docs/transaction_inclusion/post_pack/using_p2c)

---

## 1. What it serves

| Service | Role | RPCs |
|---------|------|------|
| `auth.AuthService` | `RELAYER` | challenge / tokens |
| `block_engine.BlockEngineRelayer` | `RELAYER` | `StartExpiringPacketStream` |

---

## 2. Run

```bash
RUST_LOG=info cargo run --release -p p2c_server -- \
  --bind 0.0.0.0:10001
```

Register the listen URL with Rakurai for P2C. Logs each P2C tx signature; replace that with your backrun / reply-bundle logic.
