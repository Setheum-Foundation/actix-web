use std::sync::mpsc;
use std::{thread, time::Duration};

#[cfg(feature = "openssl")]
extern crate tls_openssl as openssl;
#[cfg(feature = "rustls")]
extern crate tls_rustls as rustls;

#[cfg(feature = "openssl")]
use openssl::ssl::SslAcceptorBuilder;

use actix_web::{test, web, App, HttpResponse, HttpServer};

#[cfg(unix)]
#[actix_rt::test]
async fn test_start() {
    let addr = test::unused_addr();
    let (tx, rx) = mpsc::channel();

    thread::spawn(move || {
        let sys = actix_rt::System::new();

        sys.block_on(async {
            let srv = HttpServer::new(|| {
                App::new().service(
                    web::resource("/").route(web::to(|| HttpResponse::Ok().body("test"))),
                )
            })
            .workers(1)
            .backlog(1)
            .max_connections(10)
            .max_connection_rate(10)
            .keep_alive(10)
            .client_timeout(5000)
            .client_shutdown(0)
            .server_hostname("localhost")
            .system_exit()
            .disable_signals()
            .bind(format!("{}", addr))
            .unwrap()
            .run();

            let _ = tx.send((srv, actix_rt::System::current()));
        });

        let _ = sys.run();
    });
    let (srv, sys) = rx.recv().unwrap();

    #[cfg(feature = "client")]
    {
        use actix_http::client;

        let client = awc::Client::builder()
            .connector(
                client::Connector::new()
                    .timeout(Duration::from_millis(100))
                    .finish(),
            )
            .finish();

        let host = format!("http://{}", addr);
        let response = client.get(host.clone()).send().await.unwrap();
        assert!(response.status().is_success());
    }

    // stop
    let _ = srv.stop(false);

    thread::sleep(Duration::from_millis(100));
    let _ = sys.stop();
}

#[cfg(feature = "openssl")]
fn ssl_acceptor() -> SslAcceptorBuilder {
    use openssl::{
        pkey::PKey,
        ssl::{SslAcceptor, SslMethod},
        x509::X509,
    };

    let cert = rcgen::generate_simple_self_signed(vec!["localhost".to_owned()]).unwrap();
    let cert_file = cert.serialize_pem().unwrap();
    let key_file = cert.serialize_private_key_pem();
    let cert = X509::from_pem(cert_file.as_bytes()).unwrap();
    let key = PKey::private_key_from_pem(key_file.as_bytes()).unwrap();

    let mut builder = SslAcceptor::mozilla_intermediate(SslMethod::tls()).unwrap();
    builder.set_certificate(&cert).unwrap();
    builder.set_private_key(&key).unwrap();

    Ok(builder)
}

#[actix_rt::test]
#[cfg(feature = "openssl")]
async fn test_start_ssl() {
    use actix_web::HttpRequest;

    let addr = test::unused_addr();
    let (tx, rx) = mpsc::channel();

    thread::spawn(move || {
        let sys = actix_rt::System::new();
        let builder = ssl_acceptor();

        let srv = HttpServer::new(|| {
            App::new().service(web::resource("/").route(web::to(|req: HttpRequest| {
                assert!(req.app_config().secure());
                HttpResponse::Ok().body("test")
            })))
        })
        .workers(1)
        .shutdown_timeout(1)
        .system_exit()
        .disable_signals()
        .bind_openssl(format!("{}", addr), builder)
        .unwrap();

        sys.block_on(async {
            let srv = srv.run();
            let _ = tx.send((srv, actix_rt::System::current()));
        });

        let _ = sys.run();
    });
    let (srv, sys) = rx.recv().unwrap();

    use openssl::ssl::{SslConnector, SslMethod, SslVerifyMode};
    let mut builder = SslConnector::builder(SslMethod::tls()).unwrap();
    builder.set_verify(SslVerifyMode::NONE);
    let _ = builder
        .set_alpn_protos(b"\x02h2\x08http/1.1")
        .map_err(|e| log::error!("Can not set alpn protocol: {:?}", e));

    let client = awc::Client::builder()
        .connector(
            awc::Connector::new()
                .ssl(builder.build())
                .timeout(Duration::from_millis(100)),
        )
        .finish();

    let host = format!("https://{}", addr);
    let response = client.get(host.clone()).send().await.unwrap();
    assert!(response.status().is_success());

    // stop
    let _ = srv.stop(false);

    thread::sleep(Duration::from_millis(100));
    let _ = sys.stop();
}
