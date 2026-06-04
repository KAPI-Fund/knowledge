use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct ApiEnvelope<T> {
  pub data: T,
}
