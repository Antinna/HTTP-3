use anyhow::{Result}; // Removed 'Ok' as it's a variant, not a type to import directly
use bytes::Bytes;
use quinn::{Endpoint, ServerConfig};
use rustls::{pki_types::PrivateKeyDer, ServerConfig as TlsServerConfig}; // Alias ServerConfig to TlsServerConfig to avoid name collision with quinn::ServerConfig
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<()> { // Changed main to return Result<()> to handle errors
    // Install the default crypto provider for rustls.
    // This is necessary for rustls to function correctly, especially with AWS-LC-RS.
    rustls::crypto::aws_lc_rs::default_provider()
        .install_default()
        .unwrap(); // Panics if installation fails, which is acceptable for a startup step.

    // Generate a self-signed certificate and private key for the server.
    let cert_chain_and_key = generate_self_signed_cert()?;

    // Build the TLS server configuration using the generated certificate and key.
    // TlsServerConfig::builder() is used to construct the rustls server configuration.
    let mut tls_config = TlsServerConfig::builder()
        .with_no_client_auth() // No client authentication required for this server
        .with_single_cert(
            cert_chain_and_key.cert_chain, // Corrected field name from `cert.cert_chain` to `cert_chain_and_key.cert_chain`
            cert_chain_and_key.private_key,
        )?;

    // Set the ALPN (Application-Layer Protocol Negotiation) protocols.
    // "h3" is the ALPN for HTTP/3.
    tls_config.alpn_protocols = vec![b"h3".to_vec()];

    // Create the Quinn server configuration from the rustls TLS configuration.
    // Quinn requires a `quinn::crypto::rustls::QuicServerConfig` for its crypto setup.
    let server_config = ServerConfig::with_crypto(Arc::new(
        quinn::crypto::rustls::QuicServerConfig::try_from(tls_config)?,
    ));

    // Bind the Quinn endpoint to the specified address.
    let endpoint = Endpoint::server(server_config, "127.0.0.1:443".parse()?)?;
    println!("HTTP/3 server listening on 127.0.0.1:443");

    // Main server loop: accept incoming connections.
    while let Some(conn) = endpoint.accept().await {
        // Await the connection to be established.
        let conn = conn.await?;

        // Spawn a new task to handle each incoming QUIC connection.
        tokio::spawn(async move {
            // Create an h3 server connection from the Quinn connection.
            let mut h3_conn = h3::server::Connection::new(h3_quinn::Connection::new(conn))
                .await
                .unwrap(); // Panics if h3 connection setup fails

            // Loop to accept and handle HTTP/3 requests on this connection.
            loop {
                match h3_conn.accept().await {
                    // If a request resolver is received, spawn a task to handle the request.
                    Ok(Some(req_resolver)) => {
                        tokio::spawn(async move {
                            // Resolve the request to get the HTTP request and the stream.
                            let (req, mut stream) = req_resolver.resolve_request().await.unwrap(); // Panics on error

                            println!(
                                "Got request for path: {}, protocol: {:?}",
                                req.uri().path(),
                                req.version()
                            );

                            // Determine the response body based on the request path.
                            let response_body = match req.uri().path() {
                                "/" => "hello from http3",
                                "/test" => "hello from http3 test endpoint",
                                "/health" => "hello from http3 health check",
                                _ => "hello from http3 - unknown endpoint",
                            };

                            // Build the HTTP response.
                            let response = http::Response::builder()
                                .status(200)
                                .header("content-type", "text/plain")
                                .body(()) // Body is empty for the header part
                                .unwrap(); // Panics if response building fails

                            // Send the response headers.
                            stream.send_response(response).await.unwrap(); // Panics on error
                            // Send the response data (body).
                            stream.send_data(Bytes::from(response_body)).await.unwrap(); // Panics on error
                            // Finish the stream, indicating no more data will be sent.
                            stream.finish().await.unwrap(); // Panics on error
                        });
                    }
                    // If no more requests are available on this connection, break the loop.
                    Ok(None) => break,
                    // If an error occurs while accepting a request, break the loop.
                    Err(_) => break,
                }
            }
        });
    }
    Ok(()) // Indicate successful execution of the main function
}

// Struct to hold the certificate chain and private key.
struct CertificateChain {
    cert_chain: Vec<rustls::pki_types::CertificateDer<'static>>, // Corrected field name to `cert_chain`
    private_key: PrivateKeyDer<'static>,
}

// Function to generate a simple self-signed certificate for localhost.
fn generate_self_signed_cert() -> Result<CertificateChain> {
    // Generate a simple self-signed certificate for "localhost".
    let cert = rcgen::generate_simple_self_signed(vec!["localhost".to_string()])?;

    // Extract the private key in PKCS8 DER format.
    // `cert.signing_key.serialize_der()` is used to get the DER-encoded private key.
    let private_key = PrivateKeyDer::Pkcs8(cert.signing_key.serialize_der().into());

    // Extract the certificate chain in DER format.
    // `cert.cert.der().clone()` is used to get the DER-encoded certificate.
    let cert_chain = vec![cert.cert.der().clone()];

    // Return the CertificateChain struct.
    Ok(CertificateChain {
        cert_chain, // Uses the corrected field name
        private_key,
    })
}
