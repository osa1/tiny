use futures_util::stream::StreamExt;
use std::time::Duration;

use tiny::irc_state::IrcState;
use tiny::ServerInfo;

fn main() {
    let executor = tokio::runtime::Runtime::new().unwrap();

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

    executor.spawn(async {
        println!("Sleeping for 3 seconds before the second connection");
        tokio::timer::delay(tokio::clock::now() + Duration::from_secs(3)).await;
        match tiny::connect(server_info).await {
            Ok((mut client, mut rcv_ev)) => {
                println!("client created, spawning incoming msg handler task");

                tokio::spawn(async move {
                    while let Some(ev) = rcv_ev.next().await {
                        println!("ev: {:?}", ev);
                    }
                });

                println!("sleeping for 5 seconds before joining #justtesting");
                tokio::timer::delay(tokio::clock::now() + Duration::from_secs(5)).await;
                client.join("#justtesting");
                // FIXME: Just to avoid dropping the client
                tokio::timer::delay(tokio::clock::now() + Duration::from_secs(10000)).await;
            }
            Err(err) => {
                println!("connect failed: {:?}", err);
            }
        }
    });

    executor.shutdown_on_idle();
}
