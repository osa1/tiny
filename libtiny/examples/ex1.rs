
use tiny::ServerInfo;

use futures_util::stream::StreamExt;

fn main() {
    let server_info = ServerInfo {
        addr: "chat.freenode.net".to_string(),
        port: 6667,
        pass: None,
        hostname: "omer".to_string(),
        realname: "omer".to_string(),
        nicks: vec!["osa1".to_string()],
        auto_join: vec![],
        nickserv_ident: None,
    };


    let executor = tokio::runtime::Runtime::new().unwrap();

    executor.spawn(async {
        match tiny::connect(server_info).await {
            Ok((client, mut rcv_ev)) => {
                println!("client created");
                while let Some(ev) = rcv_ev.next().await {
                    println!("ev: {:?}", ev);
                }
            }
            Err(err) => {
                println!("connect failed: {:?}", err);
            }
        }
    });

    executor.shutdown_on_idle();
}
