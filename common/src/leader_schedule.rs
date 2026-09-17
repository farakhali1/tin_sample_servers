//! Caches identities allowed to authenticate: on the leader schedule **and**
//! advertising Rakurai client id (`8`) via `getClusterNodes`.
//!
//! See <https://github.com/solana-foundation/solana-validator-client-ids/blob/main/client-ids.csv>.

use {
    log::{info, warn},
    solana_pubkey::Pubkey,
    solana_rpc_client::rpc_client::RpcClient,
    std::{
        collections::HashSet,
        str::FromStr,
        sync::{Arc, RwLock},
        time::Duration,
    },
};

/// Rakurai entry in solana-validator-client-ids (`client_id,client_name` → `8,Rakurai`).
pub const RAKURAI_CLIENT_ID: &str = "Rakurai";

#[derive(Clone, Default)]
struct Snapshot {
    /// Identity pubkeys that are both scheduled leaders and Rakurai nodes.
    authorized: HashSet<Pubkey>,
}

/// Periodically refreshes the allowlist used by `AuthService`.
#[derive(Clone)]
pub struct LeaderScheduleCache {
    inner: Arc<RwLock<Snapshot>>,
    allow_any: bool,
}

impl LeaderScheduleCache {
    pub fn new(rpc_url: String, allow_any: bool, refresh: Duration) -> Self {
        let cache = Self {
            inner: Arc::new(RwLock::new(Snapshot::default())),
            allow_any,
        };
        if !allow_any {
            let c = cache.clone();
            std::thread::Builder::new()
                .name("auth-allowlist".into())
                .spawn(move || c.refresh_loop(rpc_url, refresh))
                .expect("spawn auth allowlist refresher");
        } else {
            info!("LeaderScheduleCache: validator auth allowlist disabled (--allow-any-validator)");
        }
        cache
    }

    pub fn is_authorized(&self, pubkey: &Pubkey) -> bool {
        if self.allow_any {
            return true;
        }
        self.inner
            .read()
            .expect("auth allowlist lock")
            .authorized
            .contains(pubkey)
    }

    fn refresh_loop(&self, rpc_url: String, refresh: Duration) {
        let client = RpcClient::new_with_timeout(rpc_url, Duration::from_secs(90));
        loop {
            match Self::fetch_authorized(&client) {
                Ok(authorized) => {
                    info!(
                        "LeaderScheduleCache: auth allowlist refreshed: {} Rakurai leaders (client_id={RAKURAI_CLIENT_ID})",
                        authorized.len()
                    );
                    *self.inner.write().expect("auth allowlist lock") = Snapshot { authorized };
                }
                Err(err) => warn!("LeaderScheduleCache: failed to refresh auth allowlist: {err:#}"),
            }
            std::thread::sleep(refresh);
        }
    }

    fn fetch_authorized(client: &RpcClient) -> anyhow::Result<HashSet<Pubkey>> {
        let mut leaders = HashSet::new();
        if let Some(schedule) = client.get_leader_schedule(None)? {
            for pubkey_str in schedule.keys() {
                if let Ok(pk) = Pubkey::from_str(pubkey_str) {
                    leaders.insert(pk);
                }
            }
        }

        let mut rakurai = HashSet::new();
        for node in client.get_cluster_nodes()? {
            let Some(client_id) = node.client_id.as_deref() else {
                continue;
            };
            // RPC returns a decimal string (e.g. "8"); accept trimmed / numeric forms.
            if client_id.trim() != RAKURAI_CLIENT_ID {
                continue;
            }
            if let Ok(pk) = Pubkey::from_str(&node.pubkey) {
                rakurai.insert(pk);
            }
        }

        Ok(leaders.intersection(&rakurai).copied().collect())
    }
}
