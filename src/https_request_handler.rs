use rustls_acme::caches::DirCache;
use rustls_acme::futures_rustls::rustls::ServerConfig;
use rustls_acme::{AcmeAcceptor, AcmeConfig};
use tokio_util::compat::FuturesAsyncReadCompatExt;
use tokio_util::compat::TokioAsyncReadCompatExt;
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

    let rustls_config = ServerConfig::builder()
      .with_safe_defaults()
      .with_no_client_auth()
      .with_cert_resolver(state.resolver());
    let acceptor = state.acceptor();

    tokio::spawn(async move {
      loop {
        match state.next().await.unwrap() {
          Ok(ok) => log::info!("event: {:?}", ok),
          Err(err) => log::error!("error: {:?}", err),
        }
      }
    });

    self.serve(acceptor, Arc::new(rustls_config)).await;
  }

  async fn serve(self, acceptor: AcmeAcceptor, rustls_config: Arc<ServerConfig>) {
    let listener = self.listener;
    loop {
      let tcp = listener.accept().await.unwrap().0.compat();
      let rustls_config = rustls_config.clone();
      let accept_future = acceptor.accept(tcp);

      let request_handler = self.request_handler.clone();

      tokio::spawn(async move {
        match accept_future.await.unwrap() {
          None => log::info!("received TLS-ALPN-01 validation request"),
          Some(start_handshake) => {
            let tls = start_handshake
              .into_stream(rustls_config)
              .await
              .unwrap()
              .compat();
            Http::new()
              .serve_connection(tls, request_handler)
              .await
              .unwrap()
          }
        }
      });
    }
  }

  pub(crate) fn https_port(&self) -> u16 {
    self.https_port
  }
}
