use crate::MessageType;
use lazy_static::lazy_static;
use std::fmt::Display;
use tokio::sync::Mutex;
use tower_lsp::Client;

#[derive(Clone)]
pub enum Logger {
    Lsp(Client),
    Print,
}

pub struct MyMessageType(pub MessageType);

impl Display for MyMessageType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self.0 {
            MessageType::ERROR => "ERROR",
            MessageType::WARNING => "WARNING",
            MessageType::INFO => "INFO",
            MessageType::LOG => "LOG",
            _ => "",
        })
    }
}

impl Logger {
    pub async fn log<M: Display>(&self, typ: MessageType, message: M) {
        let typ = MyMessageType(typ);
        match self {
            Self::Lsp(c) => c.log_message(typ.0, message).await,
            Self::Print => println!("{typ}: {message}"),
        }
    }

    pub async fn set(new_logger: &Logger) {
        let mut guard = LOGGER.lock().await;
        *guard = new_logger.clone();
    }
}

lazy_static! {
    static ref LOGGER: Mutex<Logger> = Mutex::new(Logger::Print);
}

pub async fn log<M: Display>(typ: MessageType, message: M) {
    let guard = LOGGER.lock().await;
    guard.log(typ, message).await;
}

#[macro_export]
macro_rules! error {
    ($($args:tt)*) => {
        log(MessageType::ERROR, &format!($($args)*)).await;
    };
}

#[macro_export]
macro_rules! warn {
    ($($args:tt)*) => {
        log(MessageType::WARNING, &format!($($args)*)).await;
    };
}

#[macro_export]
macro_rules! info {
    ($($args:tt)*) => {
        log(MessageType::INFO, &format!($($args)*)).await;
    };
}
