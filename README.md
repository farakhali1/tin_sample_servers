# TIN sample servers

**Reference gRPC servers for Rakurai TIN partners.** After you register a public URL with Rakurai, opted-in validators discover and connect to your endpoint over the TIN gRPC API. The crates show the minimum setup you need for the two TIN paths: pushing bundles into the validator, and consuming post-pack (P2C) streams for backruns.

These are working samples meant for local/dev and partner integration — not hardened production services. Copy the auth + service wiring, then replace dummy tip bundles and signature logging with your own searcher or backrun logic.

**Audience:** TIN partners building a block engine (bundles) and/or a post-pack (P2C) consumer.

---

## 1. Which binary?

| Crate | Serves | Use when |
|-------|--------|----------|
| [`bundles_server`](./bundles_server/) | Bundles | You authenticate as `VALIDATOR` and stream packets/bundles to validators |
| [`p2c_server`](./p2c_server/) | P2C | You authenticate as `RELAYER` and receive post-pack updates (then optionally backrun) |

| Path | Auth role | Service | Direction |
|------|-----------|---------|-----------|
| Bundles | `VALIDATOR` | `block_engine.BlockEngineValidator` | You → validator |
| Post-pack | `RELAYER` | `block_engine.BlockEngineRelayer` | Validator → you |
| Both | — | `auth.AuthService` | Challenge → bearer token |

Run one binary if you only need that path. Same advertised URL for both paths needs **both** binaries behind one listener (or your own server that exposes Auth + Validator + Relayer together).

---

## 2. Discovery vs regioned endpoints (bundles)

You do **not** manually register each regional URL with Rakurai. You register **one discovery URL**; your server advertises regions in the RPC response.

| Piece | Manual with Rakurai? | Role |
|-------|----------------------|------|
| Discovery URL | **Yes** — share once | Stored on-chain; validators dial this first |
| `global_endpoint` / `regioned_endpoints` | **No** — you return them | Validator probes regions, reconnects to lowest-latency |

Flow:

1. Run `bundles_server` (or your engine) with Auth + `BlockEngineValidator`.
2. Share the discovery URL with Rakurai (e.g. `http://api.example.com:2345`).
3. Validator → discovery → `GetBlockEngineEndpoints` → your list of URLs.
4. Validator ranks `regioned_endpoints` by latency (falls back to `global_endpoint`).
5. Validator reconnects to the chosen `block_engine_url` for `SubscribePackets` / `SubscribeBundles`.

### Where you add multiple regions

Edit **`get_block_engine_endpoints`** in [`bundles_server/src/main.rs`](./bundles_server/src/main.rs) (or the same RPC in your own server). The sample fills `regioned_endpoints` from `--public-url` only. For multi-region, put every regional URL in that `vec`:

```rust
regioned_endpoints: vec![
    BlockEngineEndpoint {
        block_engine_url: "https://fra.example.com".into(),
        shredstream_receiver_address: String::new(),
    },
    BlockEngineEndpoint {
        block_engine_url: "https://nyc.example.com".into(),
        shredstream_receiver_address: String::new(),
    },
],
```

There is no CLI flag, client-config field, or Rakurai form for the regional list — only this response. Redeploy discovery after changing it. Each listed host must run Auth + Validator gRPC.

P2C has no discovery/region list: you share the Relayer listen URL; validators open `StartExpiringPacketStream` on that host.

---

## 3. Quick start

```bash
cargo build --release -p bundles_server

RUST_LOG=info ./target/release/bundles_server \
  --bind 0.0.0.0:10000 \
  --public-url http://<HOST_IP>:10000
```

| Flag | Meaning |
|------|---------|
| `--bind` | Listen address (`0.0.0.0` is fine) |
| `--public-url` | Returned by `GetBlockEngineEndpoints`; must be reachable from validators (not `127.0.0.1` / `0.0.0.0`) |

Share that public URL with Rakurai. Tip bundles **0.001 SOL** to a [Rakurai tip account](https://docs.rakurai.io/docs/services/rakurai_jito_private/rakurai_docs/transaction_inclusion/tips).

P2C:

```bash
RUST_LOG=info cargo run --release -p p2c_server -- \
  --bind 0.0.0.0:10001
```

---

## 4. Authentication

Default allowlist: on `getLeaderSchedule` **and** `getClusterNodes` with Rakurai `client_id`.

---

## 5. Flags

| Flag | Default | Applies to | Purpose |
|------|---------|------------|---------|
| `--bind` | `0.0.0.0:10000` (`10001` for `p2c_server`) | all | gRPC listen address |
| `--public-url` | *(set for remote validators)* | `bundles_server` | URL from `GetBlockEngineEndpoints` |
| `--rpc-url` (`RPC_URL`) | mainnet-beta public RPC | all | Allowlist + latest blockhash |
| `--leader-refresh-secs` | `120` | all | Allowlist refresh interval |

---

## 6. Layout

| Crate | Role |
|-------|------|
| [`bundles_server`](./bundles_server/) | Bundles sample |
| [`p2c_server`](./p2c_server/) | P2C sample |
| [`common`](./common/) | Auth, allowlist, dummy bundle helpers |
| [`protos`](./protos/) | `.proto` + tonic codegen |

---

## Related

- [Setup guide](https://docs.rakurai.io/docs/services/rakurai_jito_private/rakurai_docs/transaction_inclusion/setup_guide)
- [Using P2C](https://docs.rakurai.io/docs/services/rakurai_jito_private/rakurai_docs/transaction_inclusion/post_pack/using_p2c)
- [Tips](https://docs.rakurai.io/docs/services/rakurai_jito_private/rakurai_docs/transaction_inclusion/tips)
