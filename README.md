[package]
name = "RotiRide"
version = "0.1.0"
edition = "2024"

[[bin]]
name="server"
path="src/main.rs"

[[bin]]
name="client"
path="src/client.rs"

[dependencies]
anyhow = "1.0.98"
bytes = "1.10.1"
futures = "0.3.31"
h3 = "0.0.8"
h3-quinn = "0.0.10"
http = "1.3.1"
quinn = "0.11.8"
rcgen = "0.14.2"
rustls = {version="0.23.29",features = ["aws_lc_rs"]}
tokio = {version ="1.46.1" , features = ["full"]}

