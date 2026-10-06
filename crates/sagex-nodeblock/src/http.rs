use crate::{
    config::Config,
    consensus::{CommitRequest, Consensus},
    model::{CommittedRecord, LedgerRecord},
    store::Store,
};
use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post},
};
use axum_server::tls_rustls::RustlsConfig;
use rustls::{RootCertStore, ServerConfig, pki_types::{CertificateDer, PrivateKeyDer}};
use serde::Serialize;
use std::{fs, io::BufReader, net::SocketAddr, sync::Arc, time::Duration};

#[derive(Clone)]
struct AppState {
    config: Arc<Config>,
    store: Arc<Store>,
    consensus: Consensus,
}

#[derive(Serialize)]
struct Health {
    node_id: String,
    height: u64,
    records: u64,
    validator_count: usize,
    consensus: &'static str,
}

#[derive(Serialize)]
struct ErrorBody {
    error: String,
}

#[derive(serde::Deserialize)]
struct BlocksQuery { from: u64 }

struct ApiError(anyhow::Error);

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let message = self.0.to_string();
        let status = if message.contains("already committed") || message.contains("locked") {
            StatusCode::CONFLICT
        } else if message.contains("quorum") {
            StatusCode::SERVICE_UNAVAILABLE
        } else {
            StatusCode::BAD_REQUEST
        };
        (status, Json(ErrorBody { error: message })).into_response()
    }
}

impl From<anyhow::Error> for ApiError {
    fn from(error: anyhow::Error) -> Self { Self(error) }
}

pub async fn serve(config: Config, store: Arc<Store>) -> anyhow::Result<()> {
    install_pqc_provider()?;
    let config = Arc::new(config);
    let client = build_peer_client(&config)?;
    let consensus = Consensus::new(config.clone(), store.clone(), client)?;
    let state = AppState { config: config.clone(), store, consensus };
    let api_router = Router::new()
        .route("/v1/records", post(submit_record))
        .route("/v1/records/{watermark}", get(lookup_record))
        .route("/v1/health", get(health))
        .with_state(state.clone());
    let peer_router = Router::new()
        .route("/internal/v1/vote", post(vote))
        .route("/internal/v1/commit", post(commit))
        .route("/internal/v1/blocks", get(blocks))
        .with_state(state);

    let api_tls = RustlsConfig::from_config(Arc::new(build_server_tls(
        &config.tls,
        &config.tls.gateway_ca,
    )?));
    let peer_tls = RustlsConfig::from_config(Arc::new(build_server_tls(
        &config.tls,
        &config.tls.validator_ca,
    )?));
    let api_addr: SocketAddr = config.api_listen.parse()?;
    let peer_addr: SocketAddr = config.peer_listen.parse()?;

    let api_server = axum_server::bind_rustls(api_addr, api_tls)
        .serve(api_router.into_make_service());
    let peer_server = axum_server::bind_rustls(peer_addr, peer_tls)
        .serve(peer_router.into_make_service());
    tokio::try_join!(api_server, peer_server)?;
    Ok(())
}

fn install_pqc_provider() -> anyhow::Result<()> {
    let mut provider = rustls::crypto::aws_lc_rs::default_provider();
    provider.kx_groups = vec![rustls::crypto::aws_lc_rs::kx_group::MLKEM768];
    provider
        .install_default()
        .map_err(|_| anyhow::anyhow!("a rustls crypto provider was already installed"))
}

async fn submit_record(
    State(state): State<AppState>,
    Json(record): Json<LedgerRecord>,
) -> Result<Json<CommittedRecord>, ApiError> {
    state.consensus.propose(record.clone()).await?;
    let committed = state.store.lookup(&record.watermark)?
        .ok_or_else(|| ApiError(anyhow::anyhow!("record was not committed locally")))?;
    Ok(Json(committed))
}

async fn lookup_record(
    State(state): State<AppState>,
    Path(watermark): Path<String>,
) -> Result<Response, ApiError> {
    state.consensus.sync_from_peers().await?;
    match state.store.lookup(&watermark)? {
        Some(record) => Ok(Json(record).into_response()),
        None => Ok(StatusCode::NOT_FOUND.into_response()),
    }
}

async fn health(State(state): State<AppState>) -> Result<Json<Health>, ApiError> {
    let (height, _) = state.store.tip()?;
    Ok(Json(Health {
        node_id: state.config.node_id.clone(),
        height,
        records: state.store.record_count()?,
        validator_count: state.config.peers.len() + 1,
        consensus: "ml-dsa-65-quorum-certified-ledger",
    }))
}

async fn vote(
    State(state): State<AppState>,
    Json(block): Json<crate::model::Block>,
) -> Result<Json<crate::model::Vote>, ApiError> {
    Ok(Json(state.consensus.vote_async(&block).await?))
}

async fn commit(
    State(state): State<AppState>,
    Json(request): Json<CommitRequest>,
) -> Result<StatusCode, ApiError> {
    state.consensus.commit_async(&request.block, &request.certificate).await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn blocks(
    State(state): State<AppState>,
    Query(query): Query<BlocksQuery>,
) -> Result<Json<Vec<CommittedRecord>>, ApiError> {
    Ok(Json(state.store.blocks_from(query.from)?))
}

fn build_peer_client(config: &Config) -> anyhow::Result<reqwest::Client> {
    let cert = fs::read(&config.tls.peer_client_certificate)?;
    let key = fs::read(&config.tls.peer_client_private_key)?;
    let mut identity = cert;
    identity.extend_from_slice(&key);
    let identity = reqwest::Identity::from_pem(&identity)?;
    let ca = reqwest::Certificate::from_pem(&fs::read(&config.tls.validator_ca)?)?;
    Ok(reqwest::Client::builder()
        .identity(identity)
        .add_root_certificate(ca)
        .timeout(Duration::from_secs(8))
        .build()?)
}

fn build_server_tls(config: &crate::config::TlsConfig, client_ca_path: &str) -> anyhow::Result<ServerConfig> {
    let mut cert_reader = BufReader::new(fs::File::open(&config.server_certificate)?);
    let certs: Vec<CertificateDer<'static>> = rustls_pemfile::certs(&mut cert_reader)
        .collect::<Result<_, _>>()?;
    let mut key_reader = BufReader::new(fs::File::open(&config.server_private_key)?);
    let key: PrivateKeyDer<'static> = rustls_pemfile::private_key(&mut key_reader)?
        .ok_or_else(|| anyhow::anyhow!("server private key PEM contains no supported key"))?;
    let mut ca_reader = BufReader::new(fs::File::open(client_ca_path)?);
    let ca_certs = rustls_pemfile::certs(&mut ca_reader).collect::<Result<Vec<_>, _>>()?;
    let mut roots = RootCertStore::empty();
    for cert in ca_certs { roots.add(cert)?; }
    let verifier = rustls::server::WebPkiClientVerifier::builder(Arc::new(roots)).build()?;
    let mut tls = ServerConfig::builder_with_protocol_versions(&[&rustls::version::TLS13])
        .with_client_cert_verifier(verifier)
        .with_single_cert(certs, key)?;
    tls.alpn_protocols = vec![b"h2".to_vec(), b"http/1.1".to_vec()];
    Ok(tls)
}
