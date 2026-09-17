//! `auth.AuthService` sample implementation (challenge + leader-schedule gate).

use {
    crate::{leader_schedule::LeaderScheduleCache, tokens::TokenStore},
    log::info,
    protos::auth::{
        GenerateAuthChallengeRequest, GenerateAuthChallengeResponse, GenerateAuthTokensRequest,
        GenerateAuthTokensResponse, RefreshAccessTokenRequest, RefreshAccessTokenResponse, Role,
        auth_service_server::AuthService,
    },
    rand::{Rng, distr::Alphanumeric},
    solana_pubkey::Pubkey,
    solana_signature::Signature,
    std::{str::FromStr, sync::Arc},
    tonic::{Request, Response, Status},
};

pub struct AuthServiceImpl {
    tokens: Arc<TokenStore>,
    leaders: LeaderScheduleCache,
    /// Roles this server accepts (e.g. only VALIDATOR on block_engine).
    allowed_roles: Vec<Role>,
}

impl AuthServiceImpl {
    pub fn new(
        tokens: Arc<TokenStore>,
        leaders: LeaderScheduleCache,
        allowed_roles: Vec<Role>,
    ) -> Self {
        Self {
            tokens,
            leaders,
            allowed_roles,
        }
    }
}

#[tonic::async_trait]
impl AuthService for AuthServiceImpl {
    async fn generate_auth_challenge(
        &self,
        request: Request<GenerateAuthChallengeRequest>,
    ) -> Result<Response<GenerateAuthChallengeResponse>, Status> {
        let req = request.into_inner();
        let role =
            Role::try_from(req.role).map_err(|_| Status::invalid_argument("invalid role"))?;
        if !self.allowed_roles.contains(&role) {
            return Err(Status::invalid_argument(format!(
                "role {role:?} not supported by this sample server"
            )));
        }

        let pubkey = parse_pubkey(&req.pubkey)?;
        if !self.leaders.is_authorized(&pubkey) {
            return Err(Status::permission_denied(
                "pubkey is not a Rakurai leader (must be on leader schedule with client_id=8)",
            ));
        }

        let challenge: String = rand::rng()
            .sample_iter(&Alphanumeric)
            .take(32)
            .map(char::from)
            .collect();
        self.tokens.put_challenge(challenge.clone(), pubkey, role);
        info!("AuthService: issued auth challenge for {pubkey} role={role:?}");
        Ok(Response::new(GenerateAuthChallengeResponse { challenge }))
    }

    async fn generate_auth_tokens(
        &self,
        request: Request<GenerateAuthTokensRequest>,
    ) -> Result<Response<GenerateAuthTokensResponse>, Status> {
        let req = request.into_inner();
        let pubkey = parse_pubkey(&req.client_pubkey)?;

        // Client signs "{pubkey}-{challenge}" and also sends that string as `challenge`.
        let (expected_pubkey, role) = self
            .tokens
            .take_challenge(challenge_key_from_formatted(&req.challenge, &pubkey)?)
            .ok_or_else(|| Status::permission_denied("unknown or expired challenge"))?;

        if expected_pubkey != pubkey {
            return Err(Status::permission_denied("pubkey mismatch"));
        }

        let signature = Signature::try_from(req.signed_challenge.as_slice())
            .map_err(|_| Status::invalid_argument("invalid signature bytes"))?;
        if !signature.verify(pubkey.as_ref(), req.challenge.as_bytes()) {
            return Err(Status::permission_denied("bad challenge signature"));
        }

        let (access_token, refresh_token) = self.tokens.issue_tokens(pubkey, role);
        info!("AuthService: issued auth tokens for {pubkey} role={role:?}");
        Ok(Response::new(GenerateAuthTokensResponse {
            access_token: Some(access_token),
            refresh_token: Some(refresh_token),
        }))
    }

    async fn refresh_access_token(
        &self,
        request: Request<RefreshAccessTokenRequest>,
    ) -> Result<Response<RefreshAccessTokenResponse>, Status> {
        let refresh = request.into_inner().refresh_token;
        let access_token = self
            .tokens
            .refresh_access(&refresh)
            .ok_or_else(|| Status::permission_denied("invalid or expired refresh token"))?;
        Ok(Response::new(RefreshAccessTokenResponse {
            access_token: Some(access_token),
        }))
    }
}

fn parse_pubkey(bytes: &[u8]) -> Result<Pubkey, Status> {
    if bytes.len() == 32 {
        return Pubkey::try_from(bytes)
            .map_err(|_| Status::invalid_argument("invalid pubkey bytes"));
    }
    // Some clients may send base58.
    let s = std::str::from_utf8(bytes).map_err(|_| Status::invalid_argument("pubkey not utf8"))?;
    Pubkey::from_str(s).map_err(|_| Status::invalid_argument("invalid pubkey"))
}

/// Challenge map is keyed by the raw server challenge, while the client submits
/// the formatted `"pubkey-challenge"` string. Recover the original challenge suffix.
fn challenge_key_from_formatted<'a>(
    formatted: &'a str,
    pubkey: &Pubkey,
) -> Result<&'a str, Status> {
    let prefix = format!("{pubkey}-");
    formatted
        .strip_prefix(&prefix)
        .ok_or_else(|| Status::invalid_argument("challenge must be \"{pubkey}-{challenge}\""))
}

/// Extract bearer token auth context from a tonic request.
pub fn require_bearer<T>(
    request: &Request<T>,
    tokens: &TokenStore,
    expected_role: Role,
) -> Result<crate::tokens::AuthContext, Status> {
    let meta = request
        .metadata()
        .get("authorization")
        .ok_or_else(|| Status::unauthenticated("missing Authorization header"))?
        .to_str()
        .map_err(|_| Status::unauthenticated("invalid Authorization header"))?;
    let ctx = tokens
        .authorize(meta)
        .ok_or_else(|| Status::unauthenticated("invalid or expired access token"))?;
    if ctx.role != expected_role {
        return Err(Status::permission_denied(format!(
            "token role {:?} does not match required {:?}",
            ctx.role, expected_role
        )));
    }
    Ok(ctx)
}
