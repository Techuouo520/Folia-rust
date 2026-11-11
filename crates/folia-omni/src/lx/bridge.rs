// Native bridge state belongs exclusively to its script thread; network tasks return through a bounded channel.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::mpsc::{self, Receiver};

use rquickjs::{Ctx, Function, Object};
use serde_json::{Value, json};

use super::{SourceError, SourceMetadata, crypto, http};

type Requests = Rc<RefCell<HashMap<u64, tokio::task::JoinHandle<()>>>>;

pub(super) struct Network {
    pub runtime: tokio::runtime::Runtime,
    pub responses: Receiver<String>,
    pending: Requests,
}

impl Network {
    pub fn new() -> Result<Self, SourceError> {
        let runtime = tokio::runtime::Builder::new_current_thread().enable_all().build()?;
        let (_, responses) = mpsc::sync_channel(32);
        Ok(Self { runtime, responses, pending: Rc::default() })
    }

    pub fn install<'js>(
        &mut self,
        ctx: &Ctx<'js>,
        client: reqwest::Client,
        info: &SourceMetadata,
        script: &str,
    ) -> rquickjs::Result<Object<'js>> {
        let (sender, receiver) = mpsc::sync_channel(32);
        self.responses = receiver;
        let pending = self.pending.clone();
        let handle = self.runtime.handle().clone();
        let native = Function::new(ctx.clone(), move |op: String, input: String| -> String {
            let result = (|| {
                if input.len() > 32 * 1024 * 1024 {
                    return Err(SourceError::Limit);
                }
                let args: Value = serde_json::from_str(&input).map_err(|_| SourceError::Protocol)?;
                match op.as_str() {
                    "request" => {
                        let id = args[0].as_u64().ok_or(SourceError::Protocol)?;
                        let url = args[1].as_str().ok_or(SourceError::Protocol)?.to_owned();
                        http::validate_url(&url)?;
                        let options = args[2].clone();
                        let mut pending = pending.borrow_mut();
                        pending.retain(|_, task| !task.is_finished());
                        if pending.len() >= 16 {
                            return Err(SourceError::Limit);
                        }
                        let (sender, client) = (sender.clone(), client.clone());
                        let task = handle.spawn(async move {
                            let reply = match http::request(client, &url, &options).await {
                                Ok(value) => json!({"id":id,"value":value}),
                                Err(err) => json!({"id":id,"error":err.to_string()}),
                            };
                            if sender.try_send(reply.to_string()).is_err() {
                                tracing::debug!("source response discarded after cancellation");
                            }
                        });
                        pending.insert(id, task);
                        Ok(Value::Null)
                    }
                    "cancel" => {
                        if let Some(task) = args[0].as_u64().and_then(|id| pending.borrow_mut().remove(&id)) {
                            task.abort();
                        }
                        Ok(Value::Null)
                    }
                    _ => crypto::dispatch(&op, &args),
                }
            })();
            match result {
                Ok(value) => json!({"value":value}).to_string(),
                Err(err) => json!({"error":err.to_string()}).to_string(),
            }
        })?;
        ctx.globals().set("__lxNative", native)?;
        let mut info = json!(info);
        info["rawScript"] = json!(script);
        ctx.globals().set("__lxInfo", ctx.json_parse(info.to_string())?)?;
        ctx.eval(include_str!("bridge.js"))
    }

    pub fn cancel(&self) {
        for (_, task) in self.pending.borrow_mut().drain() {
            task.abort();
        }
    }
}

impl Drop for Network {
    fn drop(&mut self) {
        self.cancel();
    }
}
