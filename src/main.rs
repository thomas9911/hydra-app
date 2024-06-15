use std::{collections::HashMap, future::Future, time::Duration};

use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::get,
    Router,
};
use hydra::{
    Application, ChildSpec, GenServer, GenServerOptions, Message, Node, Pid, Process, Receivable,
    Registry, RegistryOptions, SupervisionStrategy, Supervisor, SupervisorOptions,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
enum MapMessage {
    All,
    Add(String, String),
    Delete(String),
    Get(String),
    AllResult(HashMap<String, String>),
    Result(Option<String>),
    Timer,
}

#[derive(Debug, Default)]
struct MapServer {
    name: String,
    state: HashMap<String, String>,
}

impl GenServer for MapServer {
    type Message = MapMessage;

    async fn init(&mut self) -> Result<(), hydra::ExitReason> {
        Process::register(Process::current(), self.name.clone()).ok();
        Self::cast_after(
            Process::current(),
            MapMessage::Timer,
            Duration::from_secs(1),
        );

        Ok(())
    }

    async fn handle_call(
        &mut self,
        message: Self::Message,
        _from: hydra::From,
    ) -> Result<Option<Self::Message>, hydra::ExitReason> {
        match message {
            MapMessage::Delete(key) => Ok(Some(MapMessage::Result(self.state.remove(&key)))),
            MapMessage::Add(key, value) => {
                Ok(Some(MapMessage::Result(self.state.insert(key, value))))
            }
            MapMessage::Get(key) => Ok(Some(MapMessage::Result(self.state.get(&key).cloned()))),
            MapMessage::All => Ok(Some(MapMessage::AllResult(self.state.clone()))),
            MapMessage::AllResult(_) => Ok(None),
            MapMessage::Result(_) => Ok(None),
            MapMessage::Timer => Ok(None),
        }
    }

    async fn handle_cast(&mut self, message: Self::Message) -> Result<(), hydra::ExitReason> {
        match message {
            MapMessage::Timer => {
                let time = self.state.get("_timer").cloned().unwrap_or(String::new());
                // dbg!(time);
                // dbg!(Process::whereis(&self.name));
                let x = self
                    .state
                    .entry("_timer".to_string())
                    .or_insert(String::from("0"));
                *x = (x.parse::<i32>().unwrap() + 1).to_string();

                Self::cast_after(
                    Process::current(),
                    MapMessage::Timer,
                    Duration::from_millis(10),
                );
                Ok(())
            }
            _ => Ok(()),
        }
    }

    async fn handle_info(
        &mut self,
        message: Message<Self::Message>,
    ) -> Result<(), hydra::ExitReason> {
        dbg!(message);
        Ok(())
    }
}

impl MapServer {
    pub fn new(name: String) -> Self {
        MapServer {
            state: HashMap::new(),
            name,
        }
    }

    pub fn child_spec(name: String) -> ChildSpec {
        ChildSpec::new("MapServer")
            .start(move || MapServer::new(name.clone()).start_link(GenServerOptions::default()))
    }
}

#[derive(Clone)]
pub struct HttpServer {
    pub map_name: String,
}

impl HttpServer {
    pub async fn init(map_name: String) {
        let app = Router::new()
            .route("/", get(root))
            .with_state(HttpServer { map_name });

        let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
        axum::serve(listener, app).await.unwrap();
    }

    pub async fn spawn_link(map_name: String) -> Result<Pid, hydra::ExitReason> {
        Ok(Process::spawn_link(Self::init(map_name)))
    }

    pub fn child_spec(map_name: String) -> ChildSpec {
        ChildSpec::new("HttpServer").start(move || Self::spawn_link(map_name.clone()))
    }
}

async fn root(State(state): State<HttpServer>) -> Response {
    let map_name = state.map_name.clone();
    // let (tx, rx) = tokio::sync::oneshot::channel();
    // Process::spawn(async move {
    //     let out = if let Some(pid) = Process::whereis(&map_name) {
    //         match MapServer::call(pid, MapMessage::All, None).await {
    //             Ok(MapMessage::AllResult(value)) => serde_json::to_string(&value).unwrap().into_response(),
    //             _ => (StatusCode::INTERNAL_SERVER_ERROR, "Error").into_response(),
    //         }
    //     } else {
    //         (StatusCode::INTERNAL_SERVER_ERROR, "Error").into_response()
    //     };

    //     tx.send(out).unwrap();
    // });

    // rx.await.unwrap()

    run_in_process(async move {
        if let Some(pid) = Process::whereis(&map_name) {
            match MapServer::call(pid, MapMessage::All, None).await {
                Ok(MapMessage::AllResult(value)) => {
                    serde_json::to_string(&value).unwrap().into_response()
                }
                _ => (StatusCode::INTERNAL_SERVER_ERROR, "Error").into_response(),
            }
        } else {
            (StatusCode::INTERNAL_SERVER_ERROR, "Error").into_response()
        }
    })
    .await
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

struct MyApp;

impl Application for MyApp {
    fn config() -> hydra::ApplicationConfig {
        hydra::ApplicationConfig::default()
            .with_tracing_subscribe(true)
            .with_tracing_panics(true)
            .with_graceful_shutdown(true)
    }

    async fn start(&self) -> Result<hydra::Pid, hydra::ExitReason> {
        let supervisor = Supervisor::with_children([
            Registry::new(String::from("MapRegistry")).child_spec(RegistryOptions::new()),
            MapServer::child_spec("CoolMap".to_string()),
            HttpServer::child_spec("CoolMap".to_string()),
        ])
        .strategy(SupervisionStrategy::OneForOne)
        .start_link(SupervisorOptions::new())
        .await?;

        Ok(supervisor)
    }
}

fn main() {
    MyApp.run()
}
