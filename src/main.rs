use axum::routing::get;
use axum::Router;
use hydra::{
    Application, ChildSpec, GenServer, GenServerOptions, Message, Pid, Process, Registry,
    RegistryOptions, SupervisionStrategy, Supervisor, SupervisorOptions,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Duration;
mod api;

#[derive(Debug, Clone, Serialize, Deserialize)]
enum MapMessage {
    All,
    Add(String, serde_value::Value),
    Delete(String),
    Get(String),
    AllResult(HashMap<String, serde_value::Value>),
    Result(Option<serde_value::Value>),
    Timer,
}

#[derive(Debug, Default)]
struct MapServer {
    name: String,
    state: HashMap<String, serde_value::Value>,
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
                // let time = self.state.get("_timer").cloned().unwrap_or(serde_value::Value::Unit);
                // dbg!(time);
                // dbg!(Process::whereis(&self.name));
                let x = self
                    .state
                    .entry("_timer".to_string())
                    .or_insert(serde_value::Value::I32(0));
                let serde_value::Value::I32(y) = x else {
                    panic!()
                };

                *y += 1;

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
            .route("/", get(api::root))
            .route("/api", get(api::get_state))
            .route(
                "/api/:key",
                get(api::get_value)
                    .put(api::put_value)
                    .delete(api::delete_value),
            )
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
