use anyhow::Result;
use bytes:: Bytes;
use h3::server;
use quinn::{Endpoint, ServerConfig};
use rustls::{pki_types::PrivateKeyDer};
use std::sync::Arc;

#[tokio::main]
async fn main() {
    rustls::crypto::aws_lc_rs::default_provider()
    .install_default()
    .unwrap();
    // println!("Hello, world!");
    let cert = generate_self_signed_cert()?;
    let mut tls_config =TlsServerConfig::builder()
       .with_no_client_auth()
       .with_single_cert(cert.cert_chain,cert.private_key)?;
    tls_config.alpn_protocols = vec![b"h3".to_vec()];

    let server_config = ServerConfig::with_crypto(Arc::new(
        quinn::crypto::rustls::QuicClientConfig::try_from(tls_config)?,
    ));
    let endpoint = Endpoint::server(server_config, "127.0.0.1:443".parse()?)?;
    println!("HTTP/3 server listening on 127.0.0.1:443");

    while let Some(conn) = endpoint.accept().await {
        let conn = conn.await?;
        tokio::spawn(async move {
            let mut h3_conn = h3::server::Connection::new(h3_quinn::Connection::new(conn))
            .await
            .unwrap();
        loop {
            match h3_conn.accept().await {
                Ok(Some(req_resolver))=> {
                    tokio::spawn(async move {
                        let (req, mut stream ) = req_resolver.resolve_request().await.unwrap() ;
                        println!(
                            "Got request for path: {}, protocol: {:?}",
                            req.uri().path(),
                            req.version()
                        );
                        let response_body = match req.uri().path() {
                            "/" => "hello from http3",
                            "/test" => "hello from http3 test endpoint",
                            "/health" => "hello from http3 health check",
                            _ => "hello from http3 - unknown endpoint",
                            
                        };

                        let response = http::Response::builder()
                        .status(200)
                        .header("content-type", "text/plain")
                        .body(())
                        .unwrap();

                    stream.send_response(response).await.unwrap();
                    stream.send_data(Bytes::from(response_body)).await.unwrap();
                    stream.finish().await.unwrap();


                    });
                }
            }
        }
        })
        // 
    }
}
struct  CertificateChain {
    cer_chain: Vec<rustls::pki_types::CertificateDer<'static>>,
    private_key: PrivateKeyDer<'static>,
}

fn generate_self_signed_cert() -> Result<CertificateChain> {
    let cert = rcgen::generate_simple_self_signed(vec!["localhost".to_string()])?;
    let private_key = PrivateKeyDer::Pkcs8(cert.key_pair.serialize_dec().into());
    let cert_chain = vec![cert.cert.dec().clone()];

    Ok(CertificateChain {
        cer_chain,
        private_key
    })
}