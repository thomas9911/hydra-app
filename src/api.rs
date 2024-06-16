use crate::{HttpServer, MapMessage, MapServer};
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{Html, IntoResponse, Response};
use axum::Json;
use hydra::{GenServer, Process};
use std::future::Future;

pub async fn root() -> impl IntoResponse {
    Html(
        r#"
        <head>
            <script src="https://unpkg.com/htmx.org@1.9.12"></script>
            <script src="https://unpkg.com/htmx.org@1.9.12/dist/ext/json-enc.js"></script>
            <link rel="stylesheet" href="https://cdnjs.cloudflare.com/ajax/libs/jsoneditor/10.0.3/jsoneditor.css"
                integrity="sha512-iOFdnlwX6UGb55bU5DL0tjWkS/+9jxRxw2KiRzyHMZARASUSwm0nEXBcdqsYni+t3UKJSK7vrwvlL8792/UMjQ=="
                crossorigin="anonymous" referrerpolicy="no-referrer" />
            <script src="https://cdnjs.cloudflare.com/ajax/libs/jsoneditor/10.0.3/jsoneditor.min.js"
                integrity="sha512-KrNxmbQP5Edirsv+ahh/HcaHBXINizYSODFpcDI2cFoFCH35Z+bvtbOjwuzaWVOSkfWqGq4P/yzPljcPEnSU7A=="
                crossorigin="anonymous" referrerpolicy="no-referrer"></script>
        </head>
        
        <body>
            <p>State:</p>
            <pre hx-get="/api" hx-trigger="load">
            Loading....
            </pre>
            <p>Update state</p>
            <form>
                <input type="text" id="key" name="key" /><br />
                <div id="jsoneditor"></div>
                <input hx-post="/htmx/custom/put_value" hx-ext="json-enc" type="submit" value="Submit" />
            </form>
        
            <script>
                const container = document.getElementById("jsoneditor");
                const options = {};
                const editor = new JSONEditor(container, options);
        
                document.body.addEventListener("htmx:configRequest", function (evt) {
                    if (evt.detail.path == "/htmx/custom/put_value") {
                        evt.detail.headers["Content-Type"] = "application/json";
                        evt.detail.verb = "PUT";
                        let key = evt.detail.parameters.key;
                        evt.detail.parameters = editor.get();
                        evt.detail.path = "/api/" + key;
        
                        console.log(evt);
                    }
                });
            </script>
        </body>     
        "#,
    )
}

pub async fn get_state(State(state): State<HttpServer>) -> Response {
    let map_name = state.map_name.clone();

    run_in_process(async move {
        if let Some(pid) = Process::whereis(&map_name) {
            match MapServer::call(pid, MapMessage::All, None).await {
                Ok(MapMessage::AllResult(value)) => {
                    serde_json::to_string(&value).unwrap().into_response()
                }
                _ => internal_server_error("Error"),
            }
        } else {
            internal_server_error("Error")
        }
    })
    .await
}

pub async fn get_value(State(state): State<HttpServer>, Path(key): Path<String>) -> Response {
    let map_name = state.map_name.clone();

    run_in_process(async move {
        if let Some(pid) = Process::whereis(&map_name) {
            match MapServer::call(pid, MapMessage::Get(key), None).await {
                Ok(MapMessage::Result(value)) => {
                    serde_json::to_string(&value).unwrap().into_response()
                }
                _ => internal_server_error("Error"),
            }
        } else {
            internal_server_error("Error")
        }
    })
    .await
}

pub async fn put_value(
    State(state): State<HttpServer>,
    Path(key): Path<String>,
    Json(value): Json<serde_value::Value>,
) -> Response {
    let map_name = state.map_name.clone();

    run_in_process(async move {
        if let Some(pid) = Process::whereis(&map_name) {
            match MapServer::call(pid, MapMessage::Add(key, value), None).await {
                Ok(MapMessage::Result(value)) => {
                    serde_json::to_string(&value).unwrap().into_response()
                }
                _ => internal_server_error("Error"),
            }
        } else {
            internal_server_error("Error")
        }
    })
    .await
}

pub async fn delete_value(State(state): State<HttpServer>, Path(key): Path<String>) -> Response {
    let map_name = state.map_name.clone();

    run_in_process(async move {
        if let Some(pid) = Process::whereis(&map_name) {
            match MapServer::call(pid, MapMessage::Delete(key), None).await {
                Ok(MapMessage::Result(value)) => {
                    serde_json::to_string(&value).unwrap().into_response()
                }
                _ => internal_server_error("Error"),
            }
        } else {
            internal_server_error("Error")
        }
    })
    .await
}

fn internal_server_error(error: &str) -> Response {
    (StatusCode::INTERNAL_SERVER_ERROR, error.to_string()).into_response()
}

async fn run_in_process<T: std::fmt::Debug + Send + 'static>(
    f: impl Future<Output = T> + Send + 'static,
) -> T {
    let (tx, rx) = tokio::sync::oneshot::channel();
    Process::spawn(async {
        tx.send(f.await).unwrap();
    });

    rx.await.unwrap()
}
