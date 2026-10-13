use opentelemetry_proto::tonic::collector::{
    logs::v1::ExportLogsServiceRequest, metrics::v1::ExportMetricsServiceRequest,
    trace::v1::ExportTraceServiceRequest,
};
use prost::Message;
use tiny_http::{Response, Server};

fn to_json<T: Message + Default + serde::Serialize>(body: &[u8]) -> Result<String, String> {
    let msg = T::decode(body).map_err(|e| e.to_string())?;
    serde_json::to_string(&msg).map_err(|e| e.to_string())
}

fn main() {
    let port = std::env::args().nth(1).unwrap_or("4318".into());
    let server = Server::http(format!("0.0.0.0:{port}")).expect("failed to bind");

    for mut req in server.incoming_requests() {
        let mut body = Vec::new();
        if let Err(e) = req.as_reader().read_to_end(&mut body) {
            eprintln!("read error: {e}");
            continue;
        }

        let result = match req.url() {
            "/v1/traces" => to_json::<ExportTraceServiceRequest>(&body),
            "/v1/metrics" => to_json::<ExportMetricsServiceRequest>(&body),
            "/v1/logs" => to_json::<ExportLogsServiceRequest>(&body),
            _ => {
                let _ = req.respond(Response::empty(404));
                continue;
            }
        };

        let status = match result {
            Ok(json) => {
                println!("{json}");
                200
            }
            Err(e) => {
                eprintln!("decode error: {e}");
                400
            }
        };
        let _ = req.respond(Response::empty(status));
    }
}
