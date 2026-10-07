// Backend-only gate until the application declares `mod plastic`.
// This imports the exact durable backend and its inline fixture/process tests.
#[allow(dead_code)]
#[path = "../src/plastic.rs"]
mod plastic;
