use crate::MessageType;
use std::fmt::Display;
use std::sync::Mutex;
use tokio::runtime::Handle;
use tower_lsp::Client;

#[cfg(test)]
use std::cell::RefCell;
#[cfg(test)]
use std::sync::mpsc;

pub enum Logger {
    Lsp(Handle, Client),
    Print,
}

#[cfg(test)]
pub enum LogProcessor {
    None,

    // Panics on any diagnostic message
    Strict,

    // Filters diagnostic and sends to channel
    Diagnostics(mpsc::Sender<(MessageType, String)>),
}

#[derive(Eq, PartialEq)]
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

#[cfg(test)]
impl LogProcessor {
    fn handle_message(&self, typ: MessageType, message: String) {
        let typ = MyMessageType(typ);
        match self {
            Self::None => (),
            Self::Strict => {
                if let MessageType::ERROR | MessageType::WARNING = typ.0 {
                    panic!("unexpected diagnostic: '{typ}, {message}'");
                }
            }
            Self::Diagnostics(x) => {
                if let MessageType::ERROR | MessageType::WARNING = typ.0 {
                    x.send((typ.0, message)).unwrap();
                }
            }
        }
    }

    pub fn local_set(new_processor: LogProcessor) {
        let old = LOCAL_PROCESSOR.replace(new_processor);
        drop(old);
    }
}

static LOGGER: Mutex<Logger> = Mutex::new(Logger::Print);

#[cfg(test)]
thread_local! {
    static LOCAL_PROCESSOR: RefCell<LogProcessor> = const { RefCell::new(LogProcessor::None) };
}

pub fn log_message<M: Display>(typ: MessageType, message: M) {
    LOGGER.lock().unwrap().log(typ, message.to_string());
    #[cfg(test)]
    LOCAL_PROCESSOR.with(|x| {
        x.borrow().handle_message(typ, message.to_string());
    });
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
