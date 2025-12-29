use booklid_rust::open;
use futures_util::StreamExt;

#[tokio::main]
async fn main() -> booklid_rust::Result<()> {
    let dev = open(60.0).await?;
    let mut stream = dev.subscribe();
    println!("source={:?}", dev.info().source);
    while let Some(s) = stream.next().await {
        println!("{:6.2}  [{:?}]", s.angle_deg, s.source);
    }
    Ok(())
}