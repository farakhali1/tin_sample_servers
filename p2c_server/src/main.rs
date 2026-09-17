//! Sample `p2c_server` — post-pack confirmation (P2C) updates receiver.
//!
//! Exposes:
//! - `auth.AuthService` with role `RELAYER`
//! - `block_engine.BlockEngineRelayer` (`StartExpiringPacketStream`)

use {
    anyhow::Context,
    clap::Parser,
    common::{
        AuthServiceImpl, LeaderScheduleCache, TokenStore, auth::require_bearer, log_p2c_batch,
        tokens::default_ttls,
    },
    log::{info, warn},
    protos::{
        auth::{Role, auth_service_server::AuthServiceServer},
        block_engine::{
            AccountsOfInterestRequest, AccountsOfInterestUpdate, PacketBatchUpdate,
            ProgramsOfInterestRequest, ProgramsOfInterestUpdate, StartExpiringPacketStreamResponse,
            block_engine_relayer_server::{BlockEngineRelayer, BlockEngineRelayerServer},
        },
        shared::Heartbeat,
    },
    std::{net::SocketAddr, sync::Arc, time::Duration},
    tokio::sync::mpsc,
    tokio_stream::{StreamExt, wrappers::ReceiverStream},
    tonic::{Request, Response, Status, Streaming, transport::Server},
};

#[derive(Debug, Parser)]
#[command(
    name = "p2c_server",
    about = "TIN sample — receive post-pack confirmations"
)]
struct Args {
    /// Listen address, e.g. 0.0.0.0:10001
    #[arg(long, default_value = "0.0.0.0:10001")]
    bind: SocketAddr,

    /// Solana JSON-RPC URL used to refresh the leader schedule.
    #[arg(
        long,
        env = "RPC_URL",
        default_value = "https://api.mainnet-beta.solana.com"
    )]
    rpc_url: String,

    /// Skip Rakurai leader allowlist (local testing only).
    #[arg(long, default_value_t = false)]
    allow_any_validator: bool,

    /// How often to refresh leader schedule + getClusterNodes.
    #[arg(long, default_value_t = 120)]
    leader_refresh_secs: u64,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    let args = Args::parse();

    let (challenge_ttl, access_ttl, refresh_ttl) = default_ttls();
    let tokens = Arc::new(TokenStore::new(challenge_ttl, access_ttl, refresh_ttl));
    let leaders = LeaderScheduleCache::new(
        args.rpc_url.clone(),
        args.allow_any_validator,
        Duration::from_secs(args.leader_refresh_secs),
    );

    let auth = AuthServiceImpl::new(tokens.clone(), leaders, vec![Role::Relayer]);
    let relayer = BlockEngineRelayerImpl { tokens };

    info!("P2C Server: listening on {}", args.bind);
    Server::builder()
        .add_service(AuthServiceServer::new(auth))
        .add_service(BlockEngineRelayerServer::new(relayer))
        .serve(args.bind)
        .await
        .context("gRPC serve")?;
    Ok(())
}

struct BlockEngineRelayerImpl {
    tokens: Arc<TokenStore>,
}

#[tonic::async_trait]
impl BlockEngineRelayer for BlockEngineRelayerImpl {
    type SubscribeAccountsOfInterestStream =
        ReceiverStream<Result<AccountsOfInterestUpdate, Status>>;
    type SubscribeProgramsOfInterestStream =
        ReceiverStream<Result<ProgramsOfInterestUpdate, Status>>;
    type StartExpiringPacketStreamStream =
        ReceiverStream<Result<StartExpiringPacketStreamResponse, Status>>;

    async fn subscribe_accounts_of_interest(
        &self,
        request: Request<AccountsOfInterestRequest>,
    ) -> Result<Response<Self::SubscribeAccountsOfInterestStream>, Status> {
        let _ctx = require_bearer(&request, &self.tokens, Role::Relayer)?;
        let (tx, rx) = mpsc::channel(4);
        let _ = tx
            .send(Ok(AccountsOfInterestUpdate {
                accounts: vec!["*".to_string()],
            }))
            .await;
        Ok(Response::new(ReceiverStream::new(rx)))
    }

    async fn subscribe_programs_of_interest(
        &self,
        request: Request<ProgramsOfInterestRequest>,
    ) -> Result<Response<Self::SubscribeProgramsOfInterestStream>, Status> {
        let _ctx = require_bearer(&request, &self.tokens, Role::Relayer)?;
        let (tx, rx) = mpsc::channel(4);
        let _ = tx
            .send(Ok(ProgramsOfInterestUpdate {
                programs: vec!["*".to_string()],
            }))
            .await;
        Ok(Response::new(ReceiverStream::new(rx)))
    }

    async fn start_expiring_packet_stream(
        &self,
        request: Request<Streaming<PacketBatchUpdate>>,
    ) -> Result<Response<Self::StartExpiringPacketStreamStream>, Status> {
        let ctx = require_bearer(&request, &self.tokens, Role::Relayer)?;
        info!(
            "P2C Server: StartExpiringPacketStream (P2C) from {}",
            ctx.pubkey
        );

        let mut inbound = request.into_inner();
        let (tx, rx) = mpsc::channel(32);

        let heartbeat_tx = tx.clone();
        tokio::spawn(async move {
            let mut tick = tokio::time::interval(Duration::from_secs(5));
            let mut count = 0u64;
            loop {
                tick.tick().await;
                count = count.wrapping_add(1);
                let msg = StartExpiringPacketStreamResponse {
                    heartbeat: Some(Heartbeat { count }),
                };
                if heartbeat_tx.send(Ok(msg)).await.is_err() {
                    break;
                }
            }
        });

        tokio::spawn(async move {
            while let Some(msg) = inbound.next().await {
                match msg {
                    Ok(PacketBatchUpdate { msg: Some(m) }) => match m {
                        protos::block_engine::packet_batch_update::Msg::Batches(batch) => {
                            let _packets = log_p2c_batch(&batch);
                            // Replace with your backrun / reply-bundle logic.
                        }
                        protos::block_engine::packet_batch_update::Msg::Heartbeat(hb) => {
                            info!("P2C Server: client heartbeat count={}", hb.count);
                        }
                    },
                    Ok(_) => {}
                    Err(err) => {
                        warn!("P2C Server: packet stream error: {err}");
                        break;
                    }
                }
            }
            info!(
                "P2C Server: StartExpiringPacketStream closed for {}",
                ctx.pubkey
            );
        });

        Ok(Response::new(ReceiverStream::new(rx)))
    }
}
