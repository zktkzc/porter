use cidr::IpCidr;
use clap::Parser;
use std::net::IpAddr;
use tokio::net::TcpStream;
use tokio::runtime::Runtime;
use tokio::sync::mpsc;

#[derive(Parser, Debug)]
struct Args {
    #[arg(conflicts_with("cidr"), required_unless_present("cidr"))]
    addr: Option<IpAddr>,

    #[arg(long)]
    cidr: Option<IpCidr>,

    #[arg(long, default_value_t = 1)]
    port_start: u16,

    #[arg(long, default_value_t = 1024)]
    port_end: u16,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    assert!(args.port_start > 0);
    assert!(args.port_end >= args.port_start);

    let rt = Runtime::new()?;
    let (tx, mut rx) = mpsc::channel(10);
    rt.block_on(async {
        let mut tasks = vec![];

        let (mut from_single, mut from_cidr);
        let addresses: &mut dyn Iterator<Item = IpAddr>;
        match (args.addr, args.cidr) {
            (Some(addr), _) => {
                from_single = vec![addr].into_iter();
                addresses = &mut from_single;
            }
            (_, Some(cidr)) => {
                from_cidr = cidr.iter().map(|net| net.address());
                addresses = &mut from_cidr;
            }
            (_, _) => unreachable!(),
        }

        for addr in addresses {
            for port in args.port_start..=args.port_end {
                let tx = tx.clone();
                let task = tokio::spawn(async move {
                    let scan_attempt = scan(addr, port, tx).await;
                    if let Err(e) = scan_attempt {
                        eprintln!("error: {}", e);
                    }
                });

                tasks.push(task);
            }
        }

        for task in tasks {
            task.await.unwrap();
        }
    });

    drop(tx);

    while let Ok((addr, port)) = rx.try_recv() {
        println!("{}:{}", addr, port);
    }

    Ok(())
}

async fn scan(
    addr: IpAddr,
    port: u16,
    results_tx: mpsc::Sender<(IpAddr, u16)>,
) -> Result<(), mpsc::error::SendError<(IpAddr, u16)>> {
    let connection_attempt = TcpStream::connect((addr, port)).await;
    if let Ok(_open) = connection_attempt {
        results_tx.send((addr, port)).await?;
    }

    Ok(())
}
