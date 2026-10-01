pub mod issue;
pub mod lookup;

pub async fn healthz() -> &'static str {
    "ok"
}
