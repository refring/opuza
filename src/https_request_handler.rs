use rustls_acme::caches::DirCache;
use rustls_acme::futures_rustls::rustls::ServerConfig;
use rustls_acme::{is_tls_alpn_challenge, AcmeConfig};
use tokio::io::AsyncWriteExt;
use tokio_rustls::LazyConfigAcceptor;
use {crate::common::*, hyper::server::conn::Http};

pub(crate) struct HttpsRequestHandler {
  request_handler: RequestHandler,
  https_port: u16,
  listener: tokio::net::TcpListener,
  cache_dir: PathBuf,
  acme_domains: Vec<String>,
}

impl HttpsRequestHandler {
  pub(crate) async fn new(
    environment: &mut Environment,
    arguments: &Arguments,
    acme_cache_directory: &Path,
    https_port: u16,
    rpc_client: Option<opuza_monero_client::MoneroRpcClient>,
  ) -> Result<HttpsRequestHandler> {
    let request_handler = RequestHandler::new(environment, &arguments.directory, rpc_client);
    let socket_addr = (arguments.address.as_str(), https_port)
      .to_socket_addrs()
      .context(error::AddressResolutionIo {
        input: &arguments.address,
      })?
      .next()
      .ok_or_else(|| {
        error::AddressResolutionNoAddresses {
          input: arguments.address.clone(),
        }
        .build()
      })?;
    let listener = tokio::net::TcpListener::bind(socket_addr)
      .await
      .context(error::SocketIo { socket_addr })?;
    let local_addr = listener
      .local_addr()
      .context(error::SocketIo { socket_addr })?;
    writeln!(
      environment.stderr,
      "Listening for HTTPS connections on `{}`",
      local_addr,
    )
    .context(error::StderrWrite)?;
    let https_port = local_addr.port();
    let cache_dir = environment.working_directory.join(acme_cache_directory);
    assert!(!arguments.acme_domain.is_empty());
    Ok(HttpsRequestHandler {
      acme_domains: arguments.acme_domain.clone(),
      request_handler,
      https_port,
      listener,
      cache_dir,
    })
  }

  pub(crate) async fn run(self) {
    let acme_domains = self.acme_domains.clone();
    let cache_dir = self
      .cache_dir
      .clone()
      .into_os_string()
      .into_string()
      .unwrap();

    let mut state = AcmeConfig::new(acme_domains)
      .cache_option(Some(DirCache::new(cache_dir)))
      .directory_lets_encrypt(cfg!(test) == false)
      .state();

    let challenge_rustls_config = state.challenge_rustls_config();
    let default_rustls_config = state.default_rustls_config();

    tokio::spawn(async move {
      loop {
        match state.next().await.unwrap() {
          Ok(ok) => log::info!("event: {:?}", ok),
          Err(err) => log::error!("error: {:?}", err),
        }
      }
    });

    self.serve(default_rustls_config, challenge_rustls_config).await;
  }

  async fn serve(self, default_rustls_config: Arc<ServerConfig>, challenge_rustls_config: Arc<ServerConfig>) {
    let listener = self.listener;
    loop {
      let (tcp, _) = listener.accept().await.unwrap();
      let challenge_rustls_config = challenge_rustls_config.clone();
      let default_rustls_config = default_rustls_config.clone();

      let request_handler = self.request_handler.clone();

      tokio::spawn(async move {
        let start_handshake = LazyConfigAcceptor::new(Default::default(), tcp).await.unwrap();

        if is_tls_alpn_challenge(&start_handshake.client_hello()) {
          log::info!("received TLS-ALPN-01 validation request");
          let mut tls = start_handshake.into_stream(challenge_rustls_config).await.unwrap();
          tls.shutdown().await.unwrap();
        } else {
          let tls = start_handshake.into_stream(default_rustls_config).await.unwrap();
          Http::new()
              .serve_connection(tls, request_handler)
              .await
              .unwrap()
        }
      });
    }
  }

  pub(crate) fn https_port(&self) -> u16 {
    self.https_port
  }
}
