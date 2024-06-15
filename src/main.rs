use std::{collections::HashMap, time::Duration};

use hydra::{
    Application, ChildSpec, GenServer, GenServerOptions, Message, Node, Pid, Process, Receivable,
    Registry, RegistryOptions, SupervisionStrategy, Supervisor, SupervisorOptions,
};
use serde::{Deserialize, Serialize};

// #[derive(Debug, Clone, Serialize, Deserialize)]
// enum MapMessage<K, V>
// where
//     K: Receivable,
//     V: Receivable,
// {
//     Add(K, V),
//     Push(String),
// }

#[derive(Debug, Clone, Serialize, Deserialize)]
enum MapMessage {
    Add(String, String),
    Delete(String),
    Get(String),
    Result(Option<String>),
    Timer,
}

// impl<K: DeserializeOwned, V: DeserializeOwned> DeserializeOwned for MapMessage<K, V> {}

// struct MapServer<K, V>{
//     state: HashMap<K, V>
// }

#[derive(Debug, Default)]
struct MapServer {
    name: String,
    state: HashMap<String, String>,
}

// impl<K, V> GenServer for MapServer<K, V> where
//     K: Receivable,
//     V: Receivable,
impl GenServer for MapServer {
    type Message = MapMessage;
    // type Message = MapMessage<K, V>;

    async fn init(&mut self) -> Result<(), hydra::ExitReason> {
        // Process::send_after(Process::current(), GenServerMessage::Cast(MapMessage::Timer), Duration::from_secs(1));
        // dbg!(Process::whereis("MapServer"));
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
            MapMessage::Result(_) => Ok(None),
            MapMessage::Timer => Ok(None),
        }
    }

    async fn handle_cast(&mut self, message: Self::Message) -> Result<(), hydra::ExitReason> {
        match message {
            MapMessage::Timer => {
                let time = self.state.get("_timer").cloned().unwrap_or(String::new());
                dbg!(time);
                // dbg!(Process::whereis(&self.name));
                let x = self.state.entry("_timer".to_string()).or_insert(String::from("0"));
                *x = (x.parse::<i32>().unwrap() + 1).to_string();

                Self::cast_after(
                    Process::current(),
                    MapMessage::Timer,
                    Duration::from_secs(1),
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
        MapServer { state: HashMap::new(), name }
    }
    
    pub fn child_spec(name: String) -> ChildSpec {
        ChildSpec::new("MapServer").start(move || {
            MapServer::new(name.clone()).start_link(GenServerOptions::default())
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct MyChild {
    child: Pid,
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
