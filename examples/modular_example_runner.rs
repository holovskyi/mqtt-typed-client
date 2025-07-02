mod modular_example;


#[tokio::main]
async fn main() {
    modular_example::run_example().await.unwrap();
}