use crate::MessageType;
use lazy_static::lazy_static;
use std::fmt::Display;
use std::sync::Mutex;
use tokio::runtime::Handle;
use tower_lsp::Client;

pub enum Logger {
    Lsp(Handle, Client),
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
    pub fn log(&self, typ: MessageType, message: String) {
        let typ = MyMessageType(typ);
        match self {
            Self::Lsp(handle, c) => {
                let c = c.clone();
                handle.spawn(async move { c.log_message(typ.0, message).await });
            }
            Self::Print => println!("{typ}: {message}"),
        }
    }

    pub fn set(new_logger: Logger) {
        let mut guard = LOGGER.lock().unwrap();
        *guard = new_logger;
    }
}

lazy_static! {
    static ref LOGGER: Mutex<Logger> = Mutex::new(Logger::Print);
}

pub fn log_message<M: Display>(typ: MessageType, message: M) {
    LOGGER.lock().unwrap().log(typ, message.to_string());
}

#[macro_export]
macro_rules! error {
    ($($args:tt)*) => {
        log_message(MessageType::ERROR, &format!($($args)*))
    };
}

#[macro_export]
macro_rules! warn {
    ($($args:tt)*) => {
        log_message(MessageType::WARNING, &format!($($args)*))
    };
}

#[macro_export]
macro_rules! info {
    ($($args:tt)*) => {
        log_message(MessageType::INFO, &format!($($args)*))
    };
}

#[macro_export]
macro_rules! log {
    ($($args:tt)*) => {
        log_message(MessageType::LOG, &format!($($args)*))
    };
}
