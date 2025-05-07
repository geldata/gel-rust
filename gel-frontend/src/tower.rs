use hyper::body::{Body, Incoming};
use hyper::{Request, Response};
use hyper_util::service::TowerToHyperService;
use tower::{Service, ServiceBuilder};
use tower_http::ServiceBuilderExt;
use tower_http::compression::Compression;
use tower_http::decompression::Decompression;

pub fn build_tower<B: Body + Send, S: Service<Request<Incoming>, Response = Response<B>>>(
    service: S,
) -> TowerToHyperService<Compression<Decompression<S>>> {
    TowerToHyperService::new(
        ServiceBuilder::new()
            .compression()
            .decompression()
            .service(service),
    )
}

#[cfg(test)]
mod tests {
    use hyper::server::conn::http2;
    use hyper_util::rt::TokioIo;

    use std::{
        mem::MaybeUninit,
        pin::Pin,
        task::{Context, Poll},
    };

    use super::*;

    #[derive(Clone)]
    struct TestService {}

    impl Service<Request<Incoming>> for TestService {
        type Response = Response<String>;
        type Error = std::io::Error;
        type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

        fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
            Poll::Ready(Ok(()))
        }

        fn call(&mut self, req: Request<Incoming>) -> Self::Future {
            todo!()
        }
    }

    // Compile test
    #[test]
    #[ignore]
    fn http2() {
        #![allow(unused)]
        let mut http2 = http2::Builder::new(hyper_util::rt::TokioExecutor::new());
        let service = build_tower(TestService {});
        let socket: tokio::net::TcpStream = Option::None.unwrap();
        let conn = http2.serve_connection(TokioIo::new(socket), service);
    }
}
